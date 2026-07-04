//! The `publish` layer: sync a built [`DocIndex`](crate::doc_index::DocIndex) to
//! the user's ATProto PDS as `site.standard.*` records.
//!
//! Unlike the build pipeline — pure, offline, deterministic — publishing is
//! networked, stateful, and authenticated. It reuses the frozen index from
//! [`crate::build::build_index`] (no HTML is rendered), then talks to the PDS
//! over `com.atproto.*` XRPC. A sidecar state file
//! ([`state`]) remembers which records map to which docs so re-running *updates*
//! records instead of duplicating them.

pub mod atproto;
pub mod config;
pub mod document;
pub mod pubstatus;
pub mod state;

use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use crate::publish::atproto::{Client, Credentials};
use crate::publish::config::Publish;
use crate::publish::state::{RecordRef, State};
use anyhow::{Context, Result, anyhow};
use atrium_api::types::BlobRef;
use std::path::Path;
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
pub fn run(config: &Config, index: &DocIndex, options: Options) -> Result<()> {
    let publish = config
        .publish
        .as_ref()
        .ok_or_else(|| anyhow!("no `publish:` block in config.yaml — nothing to publish"))?;

    // Collect the docs to publish, sorted by id_path for deterministic, diffable
    // syncs (the collection iterator is HashMap-backed, so order is otherwise
    // unspecified).
    let mut docs: Vec<&Doc> = index.get_collection(&publish.collection).collect();
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
        return dry_run(config, publish, &docs, &state);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    runtime.block_on(sync(config, publish, &docs, &mut state, state_path))
}

/// Print what a real run would do, without any network calls.
fn dry_run(config: &Config, publish: &Publish, docs: &[&Doc], state: &State) -> Result<()> {
    println!("publish --dry-run (no network calls)");
    println!(
        "  publication: {}",
        state
            .publication_uri
            .as_deref()
            .unwrap_or("(not yet bootstrapped — would be created)")
    );
    println!("  documents ({}):", publish.collection);
    for doc in docs {
        let existing = state.doc(&doc.id_path).and_then(|r| r.document.as_ref());
        let verb = if existing.is_some() {
            "update"
        } else {
            "create"
        };
        let url = document::canonical_url(doc, config.site_url.as_deref(), &config.base_path);
        println!(
            "    {verb} {} (rkey={})",
            doc.id_path.display(),
            document::document_rkey(&url)
        );
    }
    Ok(())
}

/// The authenticated sync. Bootstraps the publication, then for each doc uploads
/// the cover blob (once) and puts the document record at its stable rkey. State
/// is saved after every write so a crash never loses progress.
async fn sync(
    config: &Config,
    publish: &Publish,
    docs: &[&Doc],
    state: &mut State,
    state_path: &Path,
) -> Result<()> {
    let creds = Credentials::load(publish)?;
    let client = Client::login(&creds).await?;
    println!("authenticated as {} ({})", creds.handle, client.did());

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
    // `run` guarantees `site_url` is set.
    let site_url = config
        .site_url
        .as_deref()
        .expect("site.url required when publishing documents");
    let publication_uri =
        bootstrap_publication(&client, publish, site_url, state, state_path).await?;

    let mut put_docs = 0usize;

    for doc in docs {
        // Upload the cover blob once per doc, for the document's coverImage.
        let cover = upload_cover(&client, doc).await?;

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
    publish: &Publish,
    site_url: &str,
    state: &mut State,
    state_path: &Path,
) -> Result<String> {
    let icon = match &publish.publication.icon {
        Some(path) => Some(upload_image(client, path).await?),
        None => None,
    };
    let record = document::publication(&publish.publication, icon)?;
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

/// Upload a doc's `cover:` frontmatter image as a blob, if it has one. The path
/// is resolved relative to the working directory.
async fn upload_cover(client: &Client, doc: &Doc) -> Result<Option<BlobRef>> {
    let Some(path) = doc.data.get("cover").and_then(serde_yaml_ng::Value::as_str) else {
        return Ok(None);
    };
    Ok(Some(upload_image(client, Path::new(path)).await?))
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
