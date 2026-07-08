//! The `atproto status` layer: compare what *should* be published (records
//! built from the [`DocIndex`], through the exact same path `italic atproto
//! publish` uses) against what the PDS actually holds, read back via
//! `com.atproto.repo.listRecords`. The PDS is the source of truth — there is no
//! local state file.
//!
//! Unlike [`crate::atproto::publish`], which is networked *and* mutating,
//! `status` is networked but read-only. Each expected record (the publication
//! plus one document per doc in the configured collection) is classified:
//!
//! - **ok** — present on the PDS and semantically identical to the locally
//!   built record ([`compare`](crate::atproto::compare)).
//! - **CHANGED** — present, but its value differs from what the current local
//!   content produces: unpublished local edits, or the record was rewritten by
//!   another client. Either way `italic atproto publish` reconciles.
//! - **MISSING** — absent from the PDS (`italic atproto publish` fixes this).
//! - **ORPHANED** — a document record on the PDS that references this site's
//!   publication but has no matching local doc (deleted or renamed since it was
//!   published).
//!
//! Building the expected records requires the same inputs publishing does —
//! `site.url`, `site.title` (the publication record's name), and readable
//! cover images. MISSING or CHANGED records make the command exit nonzero so CI
//! can gate on it; orphans only warn, since the fix (deleteRecord) is manual.

use crate::atproto::client::{Client, Credentials};
use crate::atproto::{compare, document};
use crate::config::Config;
use crate::doc_index::DocIndex;
use crate::site_data::SiteData;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use std::collections::BTreeMap;

/// A locally-built record and the rkey it should live at.
struct Expected {
    rkey: String,
    record: document::Document,
}

/// A document record as listed from the PDS, reduced to what classification
/// needs.
struct RemoteDoc {
    rkey: String,
    uri: String,
    /// The full record value; `Value::Null` if it failed to convert, which
    /// compares unequal — the safe direction (reads as CHANGED, publish re-puts).
    value: Value,
}

impl RemoteDoc {
    /// The record's `site` field (publication AT-URI), used to attribute the
    /// record to a site when several share one PDS.
    fn site(&self) -> Option<&str> {
        self.value.get("site")?.as_str()
    }
}

/// The classified comparison of expected records against the PDS.
#[derive(Default)]
struct Report {
    /// id_paths whose document record is present and identical, in id_path order.
    published: Vec<String>,
    /// (id_path, rkey) pairs present but differing from local content.
    changed: Vec<(String, String)>,
    /// (id_path, rkey) pairs absent from the PDS, in id_path order.
    missing: Vec<(String, String)>,
    /// AT-URIs of this site's document records with no matching local doc.
    orphaned: Vec<String>,
}

/// Compare expected records against the listed remote records. Pure — the
/// network lives in [`check`]. Orphans are limited to records whose `site`
/// field matches `site_uri`; records belonging to other sites on a shared PDS
/// are ignored.
fn classify(expected: &BTreeMap<String, Expected>, remote: &[RemoteDoc], site_uri: &str) -> Report {
    let mut report = Report::default();
    for (id_path, exp) in expected {
        match remote.iter().find(|r| r.rkey == exp.rkey) {
            None => report.missing.push((id_path.clone(), exp.rkey.clone())),
            Some(r) if compare::equal(&exp.record, &r.value) => {
                report.published.push(id_path.clone())
            }
            Some(_) => report.changed.push((id_path.clone(), exp.rkey.clone())),
        }
    }
    for r in remote {
        if !expected.values().any(|exp| exp.rkey == r.rkey) && r.site() == Some(site_uri) {
            report.orphaned.push(r.uri.clone());
        }
    }
    report
}

/// Verify the site's records against the PDS. Tokio is confined to this
/// function (mirroring [`crate::atproto::publish`]): it builds a current-thread
/// runtime and drives the async check to completion.
pub fn run(config: &Config, site_data: &SiteData, index: &DocIndex) -> Result<()> {
    let atproto = &config.atproto;

    // Same requirement as `publish`: rkeys are derived from absolute canonical
    // URLs, so the origin is needed to reconstruct them.
    let site_url = config.site_url.as_deref().ok_or_else(|| {
        anyhow!(
            "site.url is required to verify published documents — it disambiguates \
             record keys so multiple sites can share one PDS"
        )
    })?;

    // The DID is needed before login to build expected records (documents embed
    // the publication AT-URI); login uses the same DID, so they can't disagree.
    let creds = Credentials::load(atproto)?;
    let site_uri = document::publication_uri(&creds.did, site_url);

    // Build the expected records exactly the way `publish` does — same record
    // builders, same locally-derived cover blob refs — so the two can't drift.
    let docs = index.union_collections(&atproto.collections);
    let (site_image, static_roots) = super::cover_inputs(config, site_data);
    let mut covers = super::Covers::new(site_image, &static_roots);
    let expected: BTreeMap<String, Expected> = docs
        .iter()
        .map(|doc| {
            let url = document::canonical_url(doc, Some(site_url), &config.base_path);
            let record = document::document(doc, &site_uri, &config.base_path, covers.derive(doc)?);
            Ok((
                doc.id_path.to_string_lossy().replace('\\', "/"),
                Expected {
                    rkey: document::document_rkey(&url),
                    record,
                },
            ))
        })
        .collect::<Result<_>>()?;

    let icon = match &atproto.publication.icon {
        Some(path) => Some(super::derive_image(path)?),
        None => None,
    };
    let expected_pub = super::publication_record(config, site_data, icon)
        .context("building the expected publication record")?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    runtime.block_on(check(&creds, site_url, &site_uri, &expected, &expected_pub))
}

/// The authenticated read pass: log in, list this repo's records, and classify
/// them against the expected set. Returns `Err` if anything is MISSING or
/// CHANGED.
async fn check(
    creds: &Credentials,
    site_url: &str,
    site_uri: &str,
    expected: &BTreeMap<String, Expected>,
    expected_pub: &document::Publication,
) -> Result<()> {
    let client = Client::login(creds).await?;
    println!("authenticated as {} ({})", client.handle(), client.did());

    let pub_rkey = document::publication_rkey(site_url);

    // Publication record. Other publication records on the repo belong to other
    // sites — ignore them.
    let publications = client.list_records(document::PUBLICATION_NSID).await?;
    let pub_status = match publications
        .iter()
        .find(|r| super::rkey_from_uri(&r.uri) == pub_rkey)
    {
        None => {
            println!("  MISSING  publication {site_uri}");
            1
        }
        Some(r) if compare::equal(expected_pub, &r.value) => {
            println!("  ok       publication {site_uri}");
            0
        }
        Some(_) => {
            println!("  CHANGED  publication {site_uri}");
            1
        }
    };

    // Document records.
    let remote: Vec<RemoteDoc> = client
        .list_records(document::DOCUMENT_NSID)
        .await?
        .into_iter()
        .map(|r| RemoteDoc {
            rkey: super::rkey_from_uri(&r.uri),
            uri: r.uri.clone(),
            value: serde_json::to_value(&r.data.value).unwrap_or(Value::Null),
        })
        .collect();

    let report = classify(expected, &remote, site_uri);

    for id_path in &report.published {
        println!("  ok       {id_path}");
    }
    for (id_path, rkey) in &report.changed {
        println!("  CHANGED  {id_path} (rkey={rkey}) — local content differs from the PDS");
    }
    for (id_path, rkey) in &report.missing {
        println!("  MISSING  {id_path} (rkey={rkey})");
    }
    for uri in &report.orphaned {
        println!("  ORPHANED {uri} (no matching local doc — deleted or renamed?)");
    }

    println!(
        "{} published, {} changed, {} missing, {} orphaned",
        report.published.len(),
        report.changed.len(),
        report.missing.len(),
        report.orphaned.len()
    );

    if !report.orphaned.is_empty() {
        eprintln!(
            "note: orphaned records can be removed with com.atproto.repo.deleteRecord \
             (see docs/guides/verifying-atproto.md)"
        );
    }

    let out_of_sync = report.missing.len() + report.changed.len() + pub_status;
    if out_of_sync > 0 {
        bail!("{out_of_sync} record(s) missing or changed — run `italic atproto publish`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn expected(rkey: &str, title: &str) -> Expected {
        Expected {
            rkey: rkey.into(),
            record: document::Document {
                type_: document::DOCUMENT_NSID,
                site: "at://did:plc:abc/site.standard.publication/self".into(),
                title: title.into(),
                published_at: "2024-01-01T00:00:00.000Z".into(),
                updated_at: None,
                path: None,
                description: None,
                cover_image: None,
                content: None,
                text_content: None,
                tags: vec![],
            },
        }
    }

    /// The remote value matching [`expected`]'s serialization.
    fn matching_value(title: &str) -> Value {
        json!({
            "$type": document::DOCUMENT_NSID,
            "site": "at://did:plc:abc/site.standard.publication/self",
            "title": title,
            "publishedAt": "2024-01-01T00:00:00.000Z",
        })
    }

    fn remote(rkey: &str, site: Option<&str>, value: Value) -> RemoteDoc {
        let mut value = value;
        if let (Some(site), Some(map)) = (site, value.as_object_mut()) {
            map.insert("site".into(), Value::String(site.into()));
        }
        RemoteDoc {
            rkey: rkey.into(),
            uri: format!("at://did:plc:abc/site.standard.document/{rkey}"),
            value,
        }
    }

    #[test]
    fn classify_partitions_published_changed_missing_orphaned() {
        let ours = "at://did:plc:abc/site.standard.publication/self";
        let expected: BTreeMap<String, Expected> = [
            ("a.md".to_string(), expected("r1", "A")),
            ("b.md".to_string(), expected("r2", "B")),
            ("c.md".to_string(), expected("r3", "C")),
        ]
        .into();
        let listed = [
            remote("r1", None, matching_value("A")), // identical → published
            remote("r2", None, matching_value("B edited")), // differs → changed
            // r3 absent → missing
            remote("r4", Some(ours), json!({"title": "old"})), // ours, no local doc → orphaned
            remote(
                "r5",
                Some("at://did:plc:other/site.standard.publication/x"),
                json!({}),
            ), // another site — ignored
            remote("r6", None, json!({})),                     // unattributable — ignored
        ];
        let report = classify(&expected, &listed, ours);
        assert_eq!(report.published, vec!["a.md"]);
        assert_eq!(report.changed, vec![("b.md".to_string(), "r2".to_string())]);
        assert_eq!(report.missing, vec![("c.md".to_string(), "r3".to_string())]);
        assert_eq!(
            report.orphaned,
            vec!["at://did:plc:abc/site.standard.document/r4"]
        );
    }

    #[test]
    fn classify_ignores_blob_mimetype_drift() {
        // A derived cover ref (placeholder mimeType) must compare equal to the
        // uploaded one the PDS returns (real mimeType) — the guarantee that
        // publish converges and status doesn't cry wolf.
        let mut exp = expected("r1", "A");
        exp.record.cover_image = Some(document::derived_blob_ref(b"png bytes").unwrap());
        let mut value = matching_value("A");
        value.as_object_mut().unwrap().insert(
            "coverImage".into(),
            json!({
                "$type": "blob",
                "ref": {"$link": document::blob_cid(b"png bytes")},
                "mimeType": "image/png",
                "size": 9
            }),
        );
        let expected: BTreeMap<String, Expected> = [("a.md".to_string(), exp)].into();
        let listed = [remote("r1", None, value)];
        let report = classify(&expected, &listed, "at://irrelevant");
        assert_eq!(report.published, vec!["a.md"]);
        assert!(report.changed.is_empty());
    }

    #[test]
    fn remote_doc_site_accessor() {
        let with = remote("r1", Some("at://did:plc:abc/pub/x"), json!({}));
        assert_eq!(with.site(), Some("at://did:plc:abc/pub/x"));
        let without = remote("r1", None, json!({}));
        assert_eq!(without.site(), None);
        let non_string = remote("r1", None, json!({"site": 42}));
        assert_eq!(non_string.site(), None);
    }
}
