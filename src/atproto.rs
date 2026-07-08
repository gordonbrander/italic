//! The `atproto` layer: sync a built [`DocIndex`](crate::doc_index::DocIndex) to
//! the user's ATProto PDS as `site.standard.*` records.
//!
//! Unlike the build pipeline — pure, offline, deterministic — publishing is
//! networked and authenticated. It reuses the frozen index from
//! [`crate::build::build_index`] (no HTML is rendered), then talks to the PDS
//! over `com.atproto.*` XRPC. There is no local state: record keys are
//! deterministic hashes of the canonical URL / site origin, so re-running
//! *updates* records in place via `putRecord`, and the PDS itself is the source
//! of truth for what is published (see [`status`]). Before writing, each record
//! is compared against the value the PDS already holds ([`compare`]); unchanged
//! records are skipped entirely — no blob upload, no repo commit.

pub mod client;
pub mod compare;
pub mod config;
pub mod cover;
pub mod document;
pub mod status;

use crate::atproto::client::{Client, Credentials};
use crate::atproto::config::Atproto;
use crate::atproto::cover::Cover;
use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use crate::site_data::SiteData;
use anyhow::{Context, Result, anyhow};
use atrium_api::types::{BlobRef, Unknown};
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
    let atproto = &config.atproto;

    let (site_image, static_roots) = cover_inputs(config, site_data);

    // The docs to publish: the deduplicated union of the configured
    // collections, id_path-sorted for deterministic, diffable syncs.
    let docs = index.union_collections(&atproto.collections);

    // Document rkeys are derived from each doc's absolute canonical URL, so the
    // origin is required to disambiguate records — without it, two sites sharing
    // one PDS would collide.
    if config.site_url.is_none() {
        return Err(anyhow!(
            "site.url is required to publish documents — it disambiguates record \
             keys so multiple sites can share one PDS"
        ));
    }

    // Build the expected publication record up front so a missing `site.title`
    // fails fast — before any network work, dry runs included.
    let derived_icon = match &atproto.publication.icon {
        Some(path) => Some(derive_image(path)?),
        None => None,
    };
    let expected_pub = publication_record(config, site_data, derived_icon)?;

    if options.dry_run {
        return dry_run(config, atproto, &docs, site_image, &static_roots);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    runtime.block_on(sync(
        config,
        atproto,
        expected_pub,
        &docs,
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
    site_image: Option<&str>,
    static_roots: &[PathBuf],
) -> Result<()> {
    // `publish` guarantees `site_url` is set before calling us.
    let site_url = config
        .site_url
        .as_deref()
        .expect("site.url required when publishing documents");
    println!("atproto publish --dry-run (no network calls)");
    // Every write is a put at a deterministic rkey — create-or-update; which one
    // it would be is unknowable offline, and doesn't matter.
    match client::env_did()? {
        Some(did) => println!(
            "  put publication {}",
            document::publication_uri(&did, site_url)
        ),
        None => println!(
            "  put publication rkey={} (set ITALIC_ATPROTO_DID to preview the AT-URI)",
            document::publication_rkey(site_url)
        ),
    }
    println!("  documents ({}):", atproto.collections.join(", "));
    for doc in docs {
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
            "    put {} (rkey={}){cover}",
            doc.id_path.display(),
            document::document_rkey(&url)
        );
    }
    Ok(())
}

/// The authenticated sync. Bootstraps the publication, then for each doc builds
/// the expected record (cover blob ref derived locally — the page's `image:`,
/// else `site.image`, resolved through the static roots) and compares it against
/// the value already on the PDS; identical records are skipped, changed ones get
/// their cover uploaded (cached per file so a shared default uploads once) and
/// are put at their stable rkey. Every write is idempotent — rkeys are
/// deterministic — so an interrupted run is simply re-run.
async fn sync(
    config: &Config,
    atproto: &Atproto,
    expected_pub: document::Publication,
    docs: &[&Doc],
    site_image: Option<&str>,
    static_roots: &[PathBuf],
) -> Result<()> {
    let creds = Credentials::load(atproto)?;
    let client = Client::login(&creds).await?;
    println!("authenticated as {} ({})", client.handle(), client.did());

    // Publication bootstrap — documents reference it via `site`.
    // `publish` guarantees `site_url` is set.
    let site_url = config
        .site_url
        .as_deref()
        .expect("site.url required when publishing documents");
    let publication_uri = bootstrap_publication(
        &client,
        expected_pub,
        atproto.publication.icon.as_deref(),
        site_url,
    )
    .await?;

    // What the PDS currently holds, keyed by rkey, for the skip-unchanged check.
    let remote: HashMap<String, Unknown> = client
        .list_records(document::DOCUMENT_NSID)
        .await?
        .into_iter()
        .map(|r| (rkey_from_uri(&r.uri), r.data.value))
        .collect();

    let mut put_docs = 0usize;
    let mut unchanged = 0usize;
    let mut covers = Covers::new(site_image, static_roots);

    for doc in docs {
        let url = document::canonical_url(doc, config.site_url.as_deref(), &config.base_path);
        let rkey = document::document_rkey(&url);

        // Expected record with a locally-derived cover ref; if the PDS already
        // holds an identical record, skip the upload and the put entirely.
        let mut record = document::document(
            doc,
            &publication_uri,
            &config.base_path,
            covers.derive(doc)?,
        );
        if remote
            .get(&rkey)
            .is_some_and(|r| compare::equal(&record, r))
        {
            unchanged += 1;
            continue;
        }

        // Changed (or new): upload the cover for real and put with the uploaded
        // ref, which is authoritative.
        record.cover_image = covers.upload(&client, doc).await?;
        client
            .put_record(document::DOCUMENT_NSID, &rkey, &record)
            .await?;
        put_docs += 1;
        throttle().await;
    }

    println!("done: {put_docs} put, {unchanged} unchanged");
    Ok(())
}

/// Create or update the single `site.standard.publication` record and return its
/// AT-URI. The rkey is derived from the site origin so each site gets its own
/// publication record on a shared PDS. Skips the write when the PDS already
/// holds a record identical to `expected` (whose icon is the locally-derived
/// blob ref); on a put, the icon at `icon_path` is uploaded for real and swapped
/// in, mirroring how `sync` handles cover images.
async fn bootstrap_publication(
    client: &Client,
    expected: document::Publication,
    icon_path: Option<&Path>,
    site_url: &str,
) -> Result<String> {
    let rkey = document::publication_rkey(site_url);
    let uri = document::publication_uri(client.did(), site_url);

    let existing = client
        .list_records(document::PUBLICATION_NSID)
        .await?
        .into_iter()
        .find(|r| rkey_from_uri(&r.uri) == rkey);
    if existing.is_some_and(|r| compare::equal(&expected, &r.data.value)) {
        println!("publication: unchanged {uri}");
        return Ok(uri);
    }

    let mut record = expected;
    if let Some(path) = icon_path {
        record.icon = Some(upload_image(client, path).await?);
    }
    let written = client
        .put_record(document::PUBLICATION_NSID, &rkey, &record)
        .await
        .context("publishing site.standard.publication")?;
    println!("publication: {}", written.uri);
    Ok(written.uri)
}

/// Build the expected `site.standard.publication` record from site config —
/// `site.title` → name (required), `site.url` + `site.base_path` → url,
/// `site.description` → description, `atproto.publication.theme` → basicTheme.
/// Shared by publish and status so the two can't drift. `icon` is the caller's
/// blob ref (locally derived for comparison, or freshly uploaded).
fn publication_record(
    config: &Config,
    site_data: &SiteData,
    icon: Option<BlobRef>,
) -> Result<document::Publication> {
    let name = site_data
        .site
        .get("title")
        .and_then(serde_yaml_ng::Value::as_str)
        .ok_or_else(|| {
            anyhow!(
                "site.title is required to publish — it becomes the publication \
                 record's name (set it under `site:` in config.yaml)"
            )
        })?;
    let site_url = config.site_url.as_deref().ok_or_else(|| {
        anyhow!("site.url is required to publish — it becomes the publication record's url")
    })?;
    let url = format!("{site_url}{}", config.base_path);
    let description = site_data
        .site
        .get("description")
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_string);
    let theme = config
        .atproto
        .publication
        .theme
        .as_ref()
        .map(document::basic_theme);
    Ok(document::publication(
        name.to_string(),
        url,
        description,
        icon,
        theme,
    ))
}

/// Cover fallback inputs shared by publish and status: the site-wide default
/// image and the static roots in lookup order (site first, then theme — the
/// reverse of the copy order, where the site overlays the theme).
fn cover_inputs<'a>(config: &Config, site_data: &'a SiteData) -> (Option<&'a str>, Vec<PathBuf>) {
    let site_image = site_data
        .site
        .get("image")
        .and_then(serde_yaml_ng::Value::as_str);
    let mut static_roots = config.static_roots();
    static_roots.reverse();
    (site_image, static_roots)
}

/// Resolves each doc's cover ([`cover::resolve`]) and produces its blob ref two
/// ways: [`Covers::derive`] hashes the file locally (offline — for building the
/// expected record to compare or verify), and [`Covers::upload`] does the real
/// `uploadBlob` when a put is needed. Both are cached by resolved path so a
/// shared image (typically the site-wide default) is read/uploaded at most once
/// per run; skips warn once per distinct value.
struct Covers<'a> {
    site_image: Option<&'a str>,
    static_roots: &'a [PathBuf],
    derived: HashMap<PathBuf, BlobRef>,
    uploaded: HashMap<PathBuf, BlobRef>,
    warned: HashSet<String>,
}

impl<'a> Covers<'a> {
    fn new(site_image: Option<&'a str>, static_roots: &'a [PathBuf]) -> Self {
        Self {
            site_image,
            static_roots,
            derived: HashMap::new(),
            uploaded: HashMap::new(),
            warned: HashSet::new(),
        }
    }

    /// Resolve `doc`'s cover to a local file. External URLs and missing files
    /// warn (once per distinct value) and resolve to `None`.
    fn resolve(&mut self, doc: &Doc) -> Option<PathBuf> {
        match cover::resolve(doc, self.site_image, self.static_roots) {
            Cover::Resolved(p, _) => Some(p),
            Cover::External(url) => {
                self.warn_once(
                    &url,
                    format!(
                        "warning: cannot upload external image URL as coverImage for {}: {url} — skipping",
                        doc.id_path.display()
                    ),
                );
                None
            }
            Cover::Missing(raw, source) => {
                self.warn_once(
                    &raw,
                    format!(
                        "warning: {source} image {raw} not found under any static root — skipping coverImage"
                    ),
                );
                None
            }
            Cover::None => None,
        }
    }

    /// The locally-derived blob ref for `doc`'s cover (no network — read + hash,
    /// cached by resolved path). An unreadable file is a hard error — the path
    /// existed at resolve time, so a read failure is genuinely exceptional.
    fn derive(&mut self, doc: &Doc) -> Result<Option<BlobRef>> {
        let Some(path) = self.resolve(doc) else {
            return Ok(None);
        };
        if let Some(blob) = self.derived.get(&path) {
            return Ok(Some(blob.clone()));
        }
        let blob = derive_image(&path)?;
        self.derived.insert(path, blob.clone());
        Ok(Some(blob))
    }

    /// Upload `doc`'s resolved cover (or return the cached blob). Warns if the
    /// PDS mints a different CID than [`Covers::derive`] computed — that would
    /// make the skip-unchanged comparison never match, so it should be loud.
    async fn upload(&mut self, client: &Client, doc: &Doc) -> Result<Option<BlobRef>> {
        let Some(path) = self.resolve(doc) else {
            return Ok(None);
        };
        if let Some(blob) = self.uploaded.get(&path) {
            return Ok(Some(blob.clone()));
        }
        let blob = upload_image(client, &path).await?;
        if let Some(derived) = self.derived.get(&path)
            && !compare::equal(derived, &blob)
        {
            eprintln!(
                "warning: PDS blob CID for {} differs from the locally derived one — \
                 unchanged-record detection will not work for this image",
                path.display()
            );
        }
        self.uploaded.insert(path, blob.clone());
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

/// Read an image from disk and derive its blob ref offline (no upload) — for
/// building expected records ([`document::derived_blob_ref`]).
fn derive_image(path: &Path) -> Result<BlobRef> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading image {} to derive its blob ref", path.display()))?;
    document::derived_blob_ref(&bytes)
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
    use serde_yaml_ng::Mapping;

    #[test]
    fn rkey_from_uri_takes_last_segment() {
        assert_eq!(
            rkey_from_uri("at://did:plc:abc/site.standard.document/3lwa"),
            "3lwa"
        );
    }

    /// A `SiteData` whose `site:` map holds the given string pairs.
    fn site_data(pairs: &[(&str, &str)]) -> SiteData {
        let mut site = Mapping::new();
        for (k, v) in pairs {
            site.insert((*k).into(), (*v).into());
        }
        SiteData {
            site,
            data: Mapping::new(),
        }
    }

    fn config(site_url: &str, base_path: &str) -> Config {
        Config {
            site_url: Some(site_url.to_string()),
            base_path: base_path.to_string(),
            ..Config::default()
        }
    }

    #[test]
    fn publication_record_derives_fields_from_site() {
        let config = config("https://example.com", "/blog");
        let site_data = site_data(&[("title", "My Garden"), ("description", "A blog")]);
        let record = publication_record(&config, &site_data, None).unwrap();
        assert_eq!(record.name, "My Garden");
        // url composes site.url + base_path.
        assert_eq!(record.url, "https://example.com/blog");
        assert_eq!(record.description.as_deref(), Some("A blog"));
        assert!(record.basic_theme.is_none());
    }

    #[test]
    fn publication_record_description_is_optional() {
        let config = config("https://example.com", "");
        let site_data = site_data(&[("title", "My Garden")]);
        let record = publication_record(&config, &site_data, None).unwrap();
        assert_eq!(record.url, "https://example.com");
        assert!(record.description.is_none());
    }

    #[test]
    fn publication_record_requires_site_title() {
        let config = config("https://example.com", "");
        let err = format!(
            "{:#}",
            publication_record(&config, &site_data(&[]), None).unwrap_err()
        );
        assert!(err.contains("site.title"), "{err}");
    }

    #[test]
    fn publication_record_maps_configured_theme() {
        use crate::atproto::config::{BasicTheme, Rgb};
        let mut config = config("https://example.com", "");
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        config.atproto.publication.theme = Some(BasicTheme {
            background: Rgb {
                r: 0x1a,
                g: 0x1a,
                b: 0x2e,
            },
            foreground: white,
            accent: white,
            accent_foreground: white,
        });
        let site_data = site_data(&[("title", "My Garden")]);
        let record = publication_record(&config, &site_data, None).unwrap();
        let theme = record.basic_theme.expect("theme mapped onto the record");
        assert_eq!(theme.type_, document::THEME_BASIC_NSID);
        assert_eq!(theme.background.r, 0x1a);
        assert_eq!(theme.background.b, 0x2e);
    }
}
