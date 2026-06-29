//! The `publish` layer: sync a built [`DocIndex`](crate::doc_index::DocIndex) to
//! the user's ATProto PDS as `site.standard.*` and `app.bsky.feed.post` records.
//!
//! Unlike the build pipeline — pure, offline, deterministic — publishing is
//! networked, stateful, and authenticated. It reuses the frozen index from
//! [`crate::build::build_index`] (no HTML is rendered), then talks to the PDS
//! over `com.atproto.*` XRPC. A sidecar state file
//! ([`state`]) remembers which records map to which docs so re-running *updates*
//! records instead of duplicating them — load-bearing for correctness, since
//! Bluesky posts are create-once.

pub mod atproto;
pub mod bsky;
pub mod config;
pub mod document;
pub mod pubstatus;
pub mod state;

use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use crate::publish::atproto::{Client, Credentials};
use crate::publish::config::{Publish, Thumb};
use crate::publish::document::StrongRef;
use crate::publish::state::{RecordRef, State};
use anyhow::{Context, Result, anyhow};
use atrium_api::types::BlobRef;
use std::path::Path;
use std::time::Duration;

/// Which parts of a publish run to perform. Both features are on by default;
/// `--documents-only`/`--bsky-only` narrow it. The Bluesky feature additionally
/// requires `publish.bluesky.enabled` in config.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Build records and diff against state, but make no network calls.
    pub dry_run: bool,
    /// Sync `site.standard.document` (+ publication) records.
    pub documents: bool,
    /// Create `app.bsky.feed.post` summaries.
    pub bluesky: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            dry_run: false,
            documents: true,
            bluesky: true,
        }
    }
}

/// Delay between record writes, a courtesy throttle against Bluesky's write rate
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

    let bsky_docs = bsky_doc_set(publish, index);

    let state_path = Path::new(state::STATE_PATH);
    let mut state = State::load(state_path)?;

    if options.dry_run {
        return dry_run(publish, &options, &docs, &bsky_docs, &state);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    runtime.block_on(sync(
        config, publish, &options, &docs, &bsky_docs, &mut state, state_path,
    ))
}

/// The id_paths eligible for a Bluesky announcement: the configured bsky
/// collection (falling back to the document collection), as a set for O(1)
/// membership checks while iterating the document list.
fn bsky_doc_set(
    publish: &Publish,
    index: &DocIndex,
) -> std::collections::HashSet<std::path::PathBuf> {
    let name = publish
        .bluesky
        .collection
        .as_deref()
        .unwrap_or(&publish.collection);
    index
        .get_collection(name)
        .map(|d| d.id_path.clone())
        .collect()
}

/// Whether a doc should get a Bluesky announcement this run, ignoring state (the
/// create-once check happens at write time). Honors the feature toggles, the
/// `enabled` flag, the eligible collection, the `announce_after` date guard, and
/// the per-post `bsky: false` opt-out.
fn announce_eligible(
    publish: &Publish,
    options: &Options,
    doc: &Doc,
    bsky_docs: &std::collections::HashSet<std::path::PathBuf>,
) -> bool {
    options.bluesky
        && publish.bluesky.enabled
        && bsky_docs.contains(&doc.id_path)
        && !bsky::opted_out(doc)
        && publish
            .bluesky
            .announce_after
            .is_none_or(|after| doc.date >= after)
}

/// Print what a real run would do, without any network calls.
fn dry_run(
    publish: &Publish,
    options: &Options,
    docs: &[&Doc],
    bsky_docs: &std::collections::HashSet<std::path::PathBuf>,
    state: &State,
) -> Result<()> {
    println!("publish --dry-run (no network calls)");
    println!(
        "  publication: {}",
        state
            .publication_uri
            .as_deref()
            .unwrap_or("(not yet bootstrapped — would be created)")
    );
    if options.documents {
        println!("  documents ({}):", publish.collection);
        for doc in docs {
            let existing = state.doc(&doc.id_path).and_then(|r| r.document.as_ref());
            let verb = if existing.is_some() {
                "update"
            } else {
                "create"
            };
            println!(
                "    {verb} {} (rkey={})",
                doc.id_path.display(),
                document::document_rkey(&doc.id_path)
            );
        }
    }
    if options.bluesky {
        if publish.bluesky.enabled {
            println!("  bluesky posts:");
            for doc in docs {
                if !announce_eligible(publish, options, doc, bsky_docs) {
                    continue;
                }
                let already = state.doc(&doc.id_path).and_then(|r| r.bsky.as_ref());
                if already.is_some() {
                    println!("    skip {} (already posted)", doc.id_path.display());
                } else {
                    println!("    create {}", doc.id_path.display());
                }
            }
        } else {
            println!("  bluesky: disabled (publish.bluesky.enabled = false)");
        }
    }
    Ok(())
}

/// The authenticated sync. Bootstraps the publication, then for each doc:
/// uploads the cover blob (once), creates the bsky post if eligible and not
/// already posted, and puts the document record with a `bskyPostRef` cross-link.
/// State is saved after every write so a crash never loses a created post.
async fn sync(
    config: &Config,
    publish: &Publish,
    options: &Options,
    docs: &[&Doc],
    bsky_docs: &std::collections::HashSet<std::path::PathBuf>,
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

    // Publication bootstrap (only needed when syncing documents, which reference
    // it via `site`).
    let publication_uri = if options.documents {
        Some(bootstrap_publication(&client, publish, state, state_path).await?)
    } else {
        state.publication_uri.clone()
    };

    let mut created_posts = 0usize;
    let mut put_docs = 0usize;
    let mut skipped_posts = 0usize;

    for doc in docs {
        // Upload the cover blob once; reused by the document's coverImage and the
        // bsky link card thumb.
        let cover = upload_cover(&client, doc).await?;

        // Bluesky announcement (create-once).
        let mut bsky_ref: Option<StrongRef> = state
            .doc(&doc.id_path)
            .and_then(|r| r.bsky.as_ref())
            .map(|r| StrongRef {
                uri: r.uri.clone(),
                cid: r.cid.clone(),
            });
        if announce_eligible(publish, options, doc, bsky_docs) {
            if bsky_ref.is_some() {
                skipped_posts += 1;
            } else {
                let thumb = match publish.bluesky.thumb {
                    Thumb::Cover if publish.bluesky.include_link_card => cover.clone(),
                    _ => None,
                };
                let text = bsky::render_text(doc, publish.bluesky.post_template.as_deref())?;
                let embed = if publish.bluesky.include_link_card {
                    let url =
                        document::canonical_url(doc, config.site_url.as_deref(), &config.base_path);
                    Some(bsky::external_embed(url, doc, thumb))
                } else {
                    None
                };
                let post = bsky::feed_post(doc, text, embed);
                let written = client.create_record(bsky::FEED_POST_NSID, &post).await?;
                state.doc_mut(&doc.id_path).bsky = Some(RecordRef {
                    rkey: rkey_from_uri(&written.uri),
                    cid: written.cid.clone(),
                    uri: written.uri.clone(),
                });
                state.save(state_path)?;
                bsky_ref = Some(StrongRef {
                    uri: written.uri,
                    cid: written.cid,
                });
                created_posts += 1;
                throttle().await;
            }
        }

        // Document record (create-or-update at a stable rkey).
        if options.documents {
            let site_uri = publication_uri
                .as_deref()
                .expect("publication bootstrapped when documents enabled");
            let rkey = document::document_rkey(&doc.id_path);
            let record =
                document::document(doc, site_uri, &config.base_path, cover, bsky_ref.clone());
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
    }

    println!(
        "done: {put_docs} document(s), {created_posts} new bsky post(s), \
         {skipped_posts} already posted"
    );
    Ok(())
}

/// Create or update the single `site.standard.publication` record and return its
/// AT-URI (recorded in state). Uses a stable `self` rkey.
async fn bootstrap_publication(
    client: &Client,
    publish: &Publish,
    state: &mut State,
    state_path: &Path,
) -> Result<String> {
    let icon = match &publish.publication.icon {
        Some(path) => Some(upload_image(client, path).await?),
        None => None,
    };
    let record = document::publication(&publish.publication, icon)?;
    let written = client
        .put_record(document::PUBLICATION_NSID, "self", &record)
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
    use crate::publish::config::{Bluesky, Publication};
    use chrono::{DateTime, NaiveDate, Utc};
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn at(date: &str) -> DateTime<Utc> {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    }

    fn publish_cfg() -> Publish {
        Publish {
            pds_host: "https://bsky.social".into(),
            handle: Some("a".into()),
            collection: "all".into(),
            verification: true,
            publication: Publication::default(),
            bluesky: Bluesky {
                enabled: true,
                ..Bluesky::default()
            },
        }
    }

    fn doc(id: &str, date: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id),
            date: at(date),
            ..Doc::default()
        }
    }

    #[test]
    fn rkey_from_uri_takes_last_segment() {
        assert_eq!(
            rkey_from_uri("at://did:plc:abc/app.bsky.feed.post/3lwa"),
            "3lwa"
        );
    }

    #[test]
    fn announce_eligible_respects_toggles_and_guards() {
        let mut publish = publish_cfg();
        let options = Options::default();
        let d = doc("posts/a.md", "2024-05-01");
        let mut set = HashSet::new();
        set.insert(d.id_path.clone());

        assert!(announce_eligible(&publish, &options, &d, &set));

        // Not in the eligible collection.
        assert!(!announce_eligible(&publish, &options, &d, &HashSet::new()));

        // Feature toggled off.
        let docs_only = Options {
            bluesky: false,
            ..options
        };
        assert!(!announce_eligible(&publish, &docs_only, &d, &set));

        // Disabled in config.
        publish.bluesky.enabled = false;
        assert!(!announce_eligible(&publish, &options, &d, &set));
        publish.bluesky.enabled = true;

        // announce_after guard drops older docs.
        publish.bluesky.announce_after = Some(at("2024-06-01"));
        assert!(!announce_eligible(&publish, &options, &d, &set));
        let newer = doc("posts/b.md", "2024-07-01");
        let mut set2 = HashSet::new();
        set2.insert(newer.id_path.clone());
        assert!(announce_eligible(&publish, &options, &newer, &set2));
    }

    #[test]
    fn announce_eligible_honors_opt_out() {
        let publish = publish_cfg();
        let options = Options::default();
        let mut d = doc("posts/a.md", "2024-05-01");
        d.data.insert(
            serde_yaml_ng::Value::String("bsky".into()),
            serde_yaml_ng::Value::Bool(false),
        );
        let mut set = HashSet::new();
        set.insert(d.id_path.clone());
        assert!(!announce_eligible(&publish, &options, &d, &set));
    }
}
