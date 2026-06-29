//! The `pubstatus` layer: read back the ATProto records `italic publish` wrote
//! and confirm they still exist on the PDS and match local state.
//!
//! Unlike [`crate::publish`], which is networked *and* mutating, `pubstatus` is
//! networked but read-only — it never writes a record or touches the state file.
//! It loads the sidecar state ([`state`](crate::publish::state)), authenticates
//! the same way publish does, then for each recorded record fetches its CID from
//! the PDS and classifies it:
//!
//! - **ok** — present, and its CID matches the one in state.
//! - **CHANGED** — present, but the live CID differs (edited or re-written since
//!   `italic publish` last ran).
//! - **MISSING** — absent from the PDS.
//!
//! State files written before the publication CID was recorded fall back to an
//! existence-only check for the publication record (no expected hash to compare).
//! Any MISSING or CHANGED record makes the command exit nonzero so CI can gate
//! on it.

use crate::config::Config;
use crate::publish::atproto::{Client, Credentials};
use crate::publish::config::Publish;
use crate::publish::state::{RecordRef, STATE_PATH, State};
use crate::publish::{bsky, document};
use anyhow::{Context, Result, anyhow, bail};
use std::path::Path;

/// Which record kinds to verify. Mirrors the `publish` toggles so
/// `--documents-only` / `--bsky-only` scope the check the same way.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Check `site.standard.document` (+ publication) records.
    pub documents: bool,
    /// Check `app.bsky.feed.post` summaries.
    pub bluesky: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            documents: true,
            bluesky: true,
        }
    }
}

/// The outcome of checking one record against the PDS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// Present, and its CID matches local state.
    Ok,
    /// Present, but its CID differs from state.
    Changed,
    /// Absent from the PDS.
    Missing,
}

/// Running tally of record outcomes, for the summary line and exit code.
#[derive(Default)]
struct Tally {
    ok: usize,
    changed: usize,
    missing: usize,
}

impl Tally {
    fn add(&mut self, status: Status) {
        match status {
            Status::Ok => self.ok += 1,
            Status::Changed => self.changed += 1,
            Status::Missing => self.missing += 1,
        }
    }
}

/// Classify a record from its locally-recorded CID and the CID the PDS returned
/// (`None` if the record was not found). Pure — the network lives in [`check`].
fn classify(expected_cid: &str, fetched: Option<&str>) -> Status {
    match fetched {
        None => Status::Missing,
        Some(cid) if cid == expected_cid => Status::Ok,
        Some(_) => Status::Changed,
    }
}

/// Verify the records recorded in state against the PDS. Tokio is confined to
/// this function (mirroring [`crate::publish::run`]): it builds a current-thread
/// runtime and drives the async check to completion.
pub fn run(config: &Config, options: Options) -> Result<()> {
    let publish = config
        .publish
        .as_ref()
        .ok_or_else(|| anyhow!("no `publish:` block in config.yaml — nothing to verify"))?;

    let state = State::load(Path::new(STATE_PATH))?;
    if state.records.is_empty() && state.publication_uri.is_none() {
        bail!("no publish state ({STATE_PATH}) — run `italic publish` first");
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    runtime.block_on(check(publish, &state, options))
}

/// The authenticated read pass: log in, then fetch and classify every recorded
/// record. Returns `Err` if anything is MISSING or CHANGED.
async fn check(publish: &Publish, state: &State, options: Options) -> Result<()> {
    let creds = Credentials::load(publish)?;
    let client = Client::login(&creds).await?;
    println!("authenticated as {} ({})", creds.handle, client.did());

    // Warn (don't fail) if the state was written against a different account —
    // same guard `publish` uses, since the records would belong to another repo.
    if let Some(prev) = &state.did
        && prev != client.did()
    {
        eprintln!(
            "warning: state file belongs to {prev} but you are authenticated as {} — \
             records may not match",
            client.did()
        );
    }

    let mut tally = Tally::default();

    // Publication record. With a recorded CID we detect drift like any other
    // record; older state files (pre-`publication_cid`) only get an existence
    // check, since there's no expected hash to compare against.
    if options.documents
        && let Some(uri) = state.publication_uri.as_deref()
    {
        // The publication rkey is origin-derived; recover it from the recorded URI.
        let rkey = super::rkey_from_uri(uri);
        let fetched = client
            .get_record_cid(document::PUBLICATION_NSID, &rkey)
            .await?;
        let status = match &state.publication_cid {
            Some(cid) => classify(cid, fetched.as_deref()),
            None if fetched.is_some() => Status::Ok,
            None => Status::Missing,
        };
        match status {
            Status::Ok => println!("  ok      publication"),
            Status::Changed => println!("  CHANGED publication"),
            Status::Missing => println!("  MISSING publication"),
        }
        tally.add(status);
    }

    // Document + Bluesky records, per doc, in deterministic id_path order
    // (state.records is a BTreeMap).
    for (id_path, records) in &state.records {
        if options.documents
            && let Some(rec) = &records.document
        {
            let fetched = client
                .get_record_cid(document::DOCUMENT_NSID, &rec.rkey)
                .await?;
            tally.add(report(id_path, "document", rec, fetched.as_deref()));
        }
        if options.bluesky
            && let Some(rec) = &records.bsky
        {
            let fetched = client
                .get_record_cid(bsky::FEED_POST_NSID, &rec.rkey)
                .await?;
            tally.add(report(id_path, "bsky", rec, fetched.as_deref()));
        }
    }

    println!(
        "{} ok, {} missing, {} changed",
        tally.ok, tally.missing, tally.changed
    );

    if tally.missing > 0 || tally.changed > 0 {
        bail!(
            "verification failed: {} missing, {} changed",
            tally.missing,
            tally.changed
        );
    }
    Ok(())
}

/// Classify one record, print its status line, and return the status to tally.
fn report(id_path: &str, kind: &str, rec: &RecordRef, fetched: Option<&str>) -> Status {
    let status = classify(&rec.cid, fetched);
    match status {
        Status::Ok => println!("  ok      {kind:8} {id_path}"),
        Status::Changed => println!("  CHANGED {kind:8} {id_path} (rkey={})", rec.rkey),
        Status::Missing => println!("  MISSING {kind:8} {id_path} (rkey={})", rec.rkey),
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_distinguishes_ok_changed_missing() {
        assert_eq!(classify("bafy", None), Status::Missing);
        assert_eq!(classify("bafy", Some("bafy")), Status::Ok);
        assert_eq!(classify("bafy", Some("bafz")), Status::Changed);
    }

    #[test]
    fn tally_counts_each_status() {
        let mut t = Tally::default();
        t.add(Status::Ok);
        t.add(Status::Ok);
        t.add(Status::Changed);
        t.add(Status::Missing);
        assert_eq!((t.ok, t.changed, t.missing), (2, 1, 1));
    }
}
