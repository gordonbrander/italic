//! The `atproto` layer: sync a built [`DocIndex`](crate::doc_index::DocIndex) to
//! the user's ATProto PDS as `site.standard.*` records.
//!
//! Unlike the build pipeline — pure, offline, deterministic — publishing is
//! networked, stateful, and authenticated. It reuses the frozen index from
//! [`crate::build::build_index`] (no HTML is rendered), then talks to the PDS
//! over `com.atproto.*` XRPC. A sidecar state file
//! ([`state`]) remembers which records map to which docs so re-running *updates*
//! records instead of duplicating them.

pub mod client;
pub mod config;
pub mod cover;
pub mod document;
pub mod state;
pub mod status;

use crate::atproto::client::{Client, Credentials};
use crate::atproto::config::Atproto;
use crate::atproto::cover::Cover;
use crate::atproto::state::{RecordRef, State};
use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use crate::site_data::SiteData;
use anyhow::{Context, Result, anyhow};
use atrium_api::types::BlobRef;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How a publish run behaves.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Build records and diff against state, but make no network calls.
    pub dry_run: bool,
}

/// Delay between record writes, a courtesy throttle against the PDS's write rate
/// limits on a large first publish.
const WRITE_THROTTLE: Duration = Duration::from_millis(200);

/// Publish a built [`DocIndex`] to the PDS. Tokio is confined to this function
/// (the build pipeline that produced `index` is sync/rayon); it builds a
/// current-thread runtime and drives the async sync to completion.
pub fn publish(
    config: &Config,
    site_data: &SiteData,
    index: &DocIndex,
    options: Options,
) -> Result<()> {
    let atproto = config
        .atproto
        .as_ref()
        .ok_or_else(|| anyhow!("no `atproto:` block in config.yaml — nothing to publish"))?;

    // Cover fallback inputs: the site-wide default image and the static roots
    // in lookup order (site first, then theme — the reverse of the copy order,
    // where the site overlays the theme).
    let site_image = site_data
        .site
        .get("image")
        .and_then(serde_yaml_ng::Value::as_str);
    let mut static_roots = config.static_roots();
    static_roots.reverse();

    // Collect the docs to publish, sorted by id_path for deterministic, diffable
    // syncs (the collection iterator is HashMap-backed, so order is otherwise
    // unspecified).
    let mut docs: Vec<&Doc> = index.get_collection(&atproto.collection).collect();
    docs.sort_by(|a, b| a.id_path.cmp(&b.id_path));

    // Document rkeys are derived from each doc's absolute canonical URL, so the
    // origin is required to disambiguate records — without it, two sites sharing
    // one PDS would collide.
    if config.site_url.is_none() {
        return Err(anyhow!(
            "site.url is required to publish documents — it disambiguates record \
             keys so multiple sites can share one PDS"
        ));
    }

    let state_path = Path::new(state::STATE_PATH);
    let mut state = State::load(state_path)?;

    if options.dry_run {
        return dry_run(config, atproto, &docs, &state, site_image, &static_roots);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    runtime.block_on(sync(
        config,
        atproto,
        &docs,
        &mut state,
        state_path,
        site_image,
        &static_roots,
    ))
}

/// Print what a real run would do, without any network calls (cover resolution
/// is read-only filesystem checks).
fn dry_run(
    config: &Config,
    atproto: &Atproto,
    docs: &[&Doc],
    state: &State,
    site_image: Option<&str>,
    static_roots: &[PathBuf],
) -> Result<()> {
    println!("atproto publish --dry-run (no network calls)");
    println!(
        "  publication: {}",
        state
            .publication_uri
            .as_deref()
            .unwrap_or("(not yet bootstrapped — would be created)")
    );
    println!("  documents ({}):", atproto.collection);
    for doc in docs {
        let existing = state.doc(&doc.id_path).and_then(|r| r.document.as_ref());
        let verb = if existing.is_some() {
            "update"
        } else {
            "create"
        };
        let url = document::canonical_url(doc, config.site_url.as_deref(), &config.base_path);
        let cover = match cover::resolve(doc, site_image, static_roots) {
            Cover::Resolved(p, source) => format!(", cover: {} (from {source})", p.display()),
            Cover::External(u) => format!(", cover: skipped — external URL {u}"),
            Cover::Missing(raw, source) => {
                format!(", cover: skipped — {source} {raw} not found in static roots")
            }
            Cover::None => String::new(),
        };
        println!(
            "    {verb} {} (rkey={}){cover}",
            doc.id_path.display(),
            document::document_rkey(&url)
        );
    }
    Ok(())
}

/// The authenticated sync. Bootstraps the publication, then for each doc uploads
/// the cover blob (the page's `image:`, else `site.image`, resolved through the
/// static roots and cached per file so a shared default uploads once) and puts
/// the document record at its stable rkey. State is saved after every write so
/// a crash never loses progress.
async fn sync(
    config: &Config,
    atproto: &Atproto,
    docs: &[&Doc],
    state: &mut State,
    state_path: &Path,
    site_image: Option<&str>,
    static_roots: &[PathBuf],
) -> Result<()> {
    let creds = Credentials::load(atproto)?;
    let client = Client::login(&creds).await?;
    println!("authenticated as {} ({})", client.handle(), client.did());

    // Warn (don't fail) if the state was written against a different account.
    if let Some(prev) = &state.did
        && prev != client.did()
    {
        eprintln!(
            "warning: state file belongs to {prev} but you are authenticated as {} — \
             records may not match",
            client.did()
        );
    }
    state.did = Some(client.did().to_string());

    // Publication bootstrap — documents reference it via `site`.
    // `publish` guarantees `site_url` is set.
    let site_url = config
        .site_url
        .as_deref()
        .expect("site.url required when publishing documents");
    let publication_uri =
        bootstrap_publication(&client, atproto, site_url, state, state_path).await?;

    let mut put_docs = 0usize;
    let mut covers = CoverUploader::new(site_image, static_roots);

    for doc in docs {
        // Resolve and upload the doc's coverImage blob (cached by file path).
        let cover = covers.upload(&client, doc).await?;

        // Document record (create-or-update at a stable rkey).
        let url = document::canonical_url(doc, config.site_url.as_deref(), &config.base_path);
        let rkey = document::document_rkey(&url);
        let record = document::document(doc, &publication_uri, &config.base_path, cover);
        let written = client
            .put_record(document::DOCUMENT_NSID, &rkey, &record)
            .await?;
        state.doc_mut(&doc.id_path).document = Some(RecordRef {
            rkey,
            cid: written.cid,
            uri: written.uri,
        });
        state.save(state_path)?;
        put_docs += 1;
        throttle().await;
    }

    println!("done: {put_docs} document(s)");
    Ok(())
}

/// Create or update the single `site.standard.publication` record and return its
/// AT-URI (recorded in state). The rkey is derived from the site origin so each
/// site gets its own publication record on a shared PDS.
async fn bootstrap_publication(
    client: &Client,
    atproto: &Atproto,
    site_url: &str,
    state: &mut State,
    state_path: &Path,
) -> Result<String> {
    let icon = match &atproto.publication.icon {
        Some(path) => Some(upload_image(client, path).await?),
        None => None,
    };
    let record = document::publication(&atproto.publication, icon)?;
    let rkey = document::publication_rkey(site_url);
    let written = client
        .put_record(document::PUBLICATION_NSID, &rkey, &record)
        .await
        .context("publishing site.standard.publication")?;
    state.publication_uri = Some(written.uri.clone());
    state.publication_cid = Some(written.cid.clone());
    state.save(state_path)?;
    println!("publication: {}", written.uri);
    Ok(written.uri)
}

/// Uploads each doc's resolved cover ([`cover::resolve`]), caching blobs by
/// resolved path so a shared image (typically the site-wide default) uploads at
/// most once per run, and warning once per distinct skipped value.
struct CoverUploader<'a> {
    site_image: Option<&'a str>,
    static_roots: &'a [PathBuf],
    cache: HashMap<PathBuf, BlobRef>,
    warned: HashSet<String>,
}

impl<'a> CoverUploader<'a> {
    fn new(site_image: Option<&'a str>, static_roots: &'a [PathBuf]) -> Self {
        Self {
            site_image,
            static_roots,
            cache: HashMap::new(),
            warned: HashSet::new(),
        }
    }

    /// Resolve `doc`'s cover and upload it (or return the cached blob). An
    /// unreadable file is a hard error — the path existed at resolve time, so a
    /// read failure is genuinely exceptional. External URLs and missing files
    /// warn (once per distinct value) and skip.
    async fn upload(&mut self, client: &Client, doc: &Doc) -> Result<Option<BlobRef>> {
        let path = match cover::resolve(doc, self.site_image, self.static_roots) {
            Cover::Resolved(p, _) => p,
            Cover::External(url) => {
                self.warn_once(
                    &url,
                    format!(
                        "warning: cannot upload external image URL as coverImage for {}: {url} — skipping",
                        doc.id_path.display()
                    ),
                );
                return Ok(None);
            }
            Cover::Missing(raw, source) => {
                self.warn_once(
                    &raw,
                    format!(
                        "warning: {source} image {raw} not found under any static root — skipping coverImage"
                    ),
                );
                return Ok(None);
            }
            Cover::None => return Ok(None),
        };
        if let Some(blob) = self.cache.get(&path) {
            return Ok(Some(blob.clone()));
        }
        let blob = upload_image(client, &path).await?;
        self.cache.insert(path, blob.clone());
        Ok(Some(blob))
    }

    /// Print `message` unless `key` (the offending raw value) has already been
    /// warned about this run.
    fn warn_once(&mut self, key: &str, message: String) {
        if self.warned.insert(key.to_string()) {
            eprintln!("{message}");
        }
    }
}

/// Read an image from disk and upload it as a blob.
async fn upload_image(client: &Client, path: &Path) -> Result<BlobRef> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading image {} for blob upload", path.display()))?;
    client
        .upload_blob(bytes)
        .await
        .with_context(|| format!("uploading {}", path.display()))
}

async fn throttle() {
    tokio::time::sleep(WRITE_THROTTLE).await;
}

/// Extract the rkey (last path segment) from an `at://did/collection/rkey` URI.
fn rkey_from_uri(uri: &str) -> String {
    uri.rsplit('/').next().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rkey_from_uri_takes_last_segment() {
        assert_eq!(
            rkey_from_uri("at://did:plc:abc/site.standard.document/3lwa"),
            "3lwa"
        );
    }
}
