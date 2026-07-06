//! The `atproto status` layer: compare what *should* be published (derived from
//! the built [`DocIndex`], exactly as `italic atproto publish` derives it)
//! against what the PDS actually holds, read back via
//! `com.atproto.repo.listRecords`. The PDS is the source of truth — there is no
//! local state file.
//!
//! Unlike [`crate::atproto::publish`], which is networked *and* mutating,
//! `status` is networked but read-only. Each expected record (the publication
//! plus one document per doc in the configured collection) is classified:
//!
//! - **ok** — present on the PDS.
//! - **MISSING** — absent from the PDS (`italic atproto publish` fixes this).
//! - **ORPHANED** — a document record on the PDS that references this site's
//!   publication but has no matching local doc (deleted or renamed since it was
//!   published).
//!
//! Checks are existence-only — rkeys are deterministic hashes of canonical
//! URLs, so presence at the expected rkey is the signal. Content drift (a
//! record edited by another client) is not detected. MISSING records make the
//! command exit nonzero so CI can gate on it; orphans only warn, since the fix
//! (deleteRecord) is manual.

use crate::atproto::client::{Client, Credentials};
use crate::atproto::config::Atproto;
use crate::atproto::document;
use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use anyhow::{Context, Result, anyhow, bail};
use atrium_api::types::Unknown;
use std::collections::BTreeMap;

/// A document record as listed from the PDS, reduced to what classification
/// needs.
struct RemoteDoc {
    rkey: String,
    uri: String,
    /// The record's `site` field (publication AT-URI), used to attribute the
    /// record to a site when several share one PDS.
    site: Option<String>,
}

/// The classified comparison of expected records against the PDS.
#[derive(Default)]
struct Report {
    /// id_paths whose document record is present, in id_path order.
    published: Vec<String>,
    /// (id_path, rkey) pairs absent from the PDS, in id_path order.
    missing: Vec<(String, String)>,
    /// AT-URIs of this site's document records with no matching local doc.
    orphaned: Vec<String>,
}

/// Compare expected records (id_path → rkey) against the listed remote records.
/// Pure — the network lives in [`check`]. Orphans are limited to records whose
/// `site` field matches `site_uri`; records belonging to other sites on a
/// shared PDS are ignored.
fn classify(expected: &BTreeMap<String, String>, remote: &[RemoteDoc], site_uri: &str) -> Report {
    let mut report = Report::default();
    for (id_path, rkey) in expected {
        if remote.iter().any(|r| &r.rkey == rkey) {
            report.published.push(id_path.clone());
        } else {
            report.missing.push((id_path.clone(), rkey.clone()));
        }
    }
    for r in remote {
        if !expected.values().any(|rkey| rkey == &r.rkey) && r.site.as_deref() == Some(site_uri) {
            report.orphaned.push(r.uri.clone());
        }
    }
    report
}

/// Extract the `site` string field from a listed record's value.
fn record_site(value: &Unknown) -> Option<String> {
    let v = serde_json::to_value(value).ok()?;
    Some(v.get("site")?.as_str()?.to_string())
}

/// Verify the site's records against the PDS. Tokio is confined to this
/// function (mirroring [`crate::atproto::publish`]): it builds a current-thread
/// runtime and drives the async check to completion.
pub fn run(config: &Config, index: &DocIndex) -> Result<()> {
    let atproto = config
        .atproto
        .as_ref()
        .ok_or_else(|| anyhow!("no `atproto:` block in config.yaml — nothing to verify"))?;

    // Same requirement as `publish`: rkeys are derived from absolute canonical
    // URLs, so the origin is needed to reconstruct them.
    let site_url = config.site_url.as_deref().ok_or_else(|| {
        anyhow!(
            "site.url is required to verify published documents — it disambiguates \
             record keys so multiple sites can share one PDS"
        )
    })?;

    // Derive the expected record set the same way `publish` does, so the two
    // can never drift.
    let mut docs: Vec<&Doc> = index.get_collection(&atproto.collection).collect();
    docs.sort_by(|a, b| a.id_path.cmp(&b.id_path));
    let expected: BTreeMap<String, String> = docs
        .iter()
        .map(|doc| {
            let url = document::canonical_url(doc, Some(site_url), &config.base_path);
            (
                doc.id_path.to_string_lossy().replace('\\', "/"),
                document::document_rkey(&url),
            )
        })
        .collect();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    runtime.block_on(check(atproto, site_url, &expected))
}

/// The authenticated read pass: log in, list this repo's records, and classify
/// them against the expected set. Returns `Err` if anything is MISSING.
async fn check(atproto: &Atproto, site_url: &str, expected: &BTreeMap<String, String>) -> Result<()> {
    let creds = Credentials::load(atproto)?;
    let client = Client::login(&creds).await?;
    println!("authenticated as {} ({})", client.handle(), client.did());

    let site_uri = document::publication_uri(client.did(), site_url);
    let pub_rkey = document::publication_rkey(site_url);

    // Publication record. Other publication records on the repo belong to other
    // sites — ignore them.
    let publications = client.list_records(document::PUBLICATION_NSID).await?;
    let publication_ok = publications
        .iter()
        .any(|r| super::rkey_from_uri(&r.uri) == pub_rkey);
    if publication_ok {
        println!("  ok       publication {site_uri}");
    } else {
        println!("  MISSING  publication {site_uri}");
    }

    // Document records.
    let remote: Vec<RemoteDoc> = client
        .list_records(document::DOCUMENT_NSID)
        .await?
        .iter()
        .map(|r| RemoteDoc {
            rkey: super::rkey_from_uri(&r.uri),
            uri: r.uri.clone(),
            site: record_site(&r.value),
        })
        .collect();

    let report = classify(expected, &remote, &site_uri);

    for id_path in &report.published {
        println!("  ok       {id_path}");
    }
    for (id_path, rkey) in &report.missing {
        println!("  MISSING  {id_path} (rkey={rkey})");
    }
    for uri in &report.orphaned {
        println!("  ORPHANED {uri} (no matching local doc — deleted or renamed?)");
    }

    println!(
        "{} published, {} missing, {} orphaned",
        report.published.len(),
        report.missing.len(),
        report.orphaned.len()
    );

    if !report.orphaned.is_empty() {
        eprintln!(
            "note: orphaned records can be removed with com.atproto.repo.deleteRecord \
             (see docs/guides/verifying-atproto.md)"
        );
    }

    let missing = report.missing.len() + usize::from(!publication_ok);
    if missing > 0 {
        bail!("{missing} record(s) missing — run `italic atproto publish`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(rkey: &str, site: Option<&str>) -> RemoteDoc {
        RemoteDoc {
            rkey: rkey.into(),
            uri: format!("at://did:plc:abc/site.standard.document/{rkey}"),
            site: site.map(String::from),
        }
    }

    #[test]
    fn classify_partitions_published_missing_orphaned() {
        let ours = "at://did:plc:abc/site.standard.publication/self";
        let expected: BTreeMap<String, String> = [
            ("a.md".to_string(), "r1".to_string()),
            ("b.md".to_string(), "r2".to_string()),
        ]
        .into();
        let listed = [
            remote("r1", Some(ours)),   // expected and present
            remote("r3", Some(ours)),   // ours, but no local doc → orphaned
            remote("r4", Some("at://did:plc:other/site.standard.publication/x")), // another site
            remote("r5", None),         // unattributable — ignored
        ];
        let report = classify(&expected, &listed, ours);
        assert_eq!(report.published, vec!["a.md"]);
        assert_eq!(report.missing, vec![("b.md".to_string(), "r2".to_string())]);
        assert_eq!(
            report.orphaned,
            vec!["at://did:plc:abc/site.standard.document/r3"]
        );
    }

    #[test]
    fn record_site_extracts_the_site_field() {
        let value: Unknown = serde_json::from_str(
            r#"{"$type":"site.standard.document","site":"at://did:plc:abc/site.standard.publication/x"}"#,
        )
        .unwrap();
        assert_eq!(
            record_site(&value).as_deref(),
            Some("at://did:plc:abc/site.standard.publication/x")
        );

        let absent: Unknown = serde_json::from_str(r#"{"$type":"site.standard.document"}"#).unwrap();
        assert_eq!(record_site(&absent), None);

        let non_string: Unknown =
            serde_json::from_str(r#"{"$type":"site.standard.document","site":42}"#).unwrap();
        assert_eq!(record_site(&non_string), None);
    }
}
