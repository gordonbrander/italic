//! The publish sidecar state file (`.italic/atproto.yaml`).
//!
//! This is the crux that makes `publish` different from `build`: build holds no
//! memory between runs, but publish must. The state remembers which PDS records
//! map to which docs (keyed by `id_path`) plus the publication record's AT-URI,
//! so re-running *updates* records via `putRecord` instead of duplicating them.
//!
//! It is serialized as YAML so users can read and hand-edit it — to inspect what
//! was published, fix a bad entry, or recover.
//!
//! It is load-bearing for **correctness**, not just efficiency: `app.bsky.feed.post`
//! records are create-once (server-assigned TID rkeys, treated as immutable by
//! clients), so the only thing preventing a duplicate post on the next run is the
//! `bsky` entry recorded here. Lose the file and you risk re-announcing every post.
//! Document rkeys, by contrast, are slug-derived and reconstructible.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Default location of the state file, relative to the working directory.
pub const STATE_PATH: &str = ".italic/atproto.yaml";

/// A written record's address: its AT-URI and content hash (CID). The CID enables
/// optimistic `swapRecord` concurrency later; for now it's recorded for parity
/// and debugging.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordRef {
    pub rkey: String,
    pub cid: String,
    /// AT-URI (`at://did/collection/rkey`). Always present for created records;
    /// recorded explicitly so cross-links don't have to reconstruct it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uri: String,
}

/// Per-doc record bookkeeping: the long-form document record and the optional
/// Bluesky announcement post.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocRecords {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<RecordRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bsky: Option<RecordRef>,
}

/// The whole sidecar. `records` is keyed by `id_path` (as a string for stable
/// YAML keys). A `BTreeMap` keeps the file deterministic/diff-friendly.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    /// The account DID this state was written against. Lets `publish` warn if the
    /// configured handle resolves to a different repo than the records belong to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub did: Option<String>,
    /// AT-URI of the one `site.standard.publication` record, once bootstrapped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_uri: Option<String>,
    #[serde(default)]
    pub records: BTreeMap<String, DocRecords>,
}

impl State {
    /// Load the state file, or return an empty state if it doesn't exist yet
    /// (first publish). A present-but-corrupt file is a hard error rather than a
    /// silent reset, since resetting risks duplicate Bluesky posts.
    pub fn load(path: &Path) -> Result<State> {
        if !path.exists() {
            return Ok(State::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading publish state {}", path.display()))?;
        serde_yaml_ng::from_str(&raw)
            .with_context(|| format!("parsing publish state {}", path.display()))
    }

    /// Write the state file, creating the parent directory if needed. Called after
    /// each record write so a mid-run crash never loses a created post.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let yaml = serde_yaml_ng::to_string(self).context("serializing publish state")?;
        fs::write(path, yaml).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// The records for `id_path`, if any have been published.
    pub fn doc(&self, id_path: &Path) -> Option<&DocRecords> {
        self.records.get(&key(id_path))
    }

    /// Mutable per-doc records, inserting an empty entry on first access.
    pub fn doc_mut(&mut self, id_path: &Path) -> &mut DocRecords {
        self.records.entry(key(id_path)).or_default()
    }
}

/// State map keys are `id_path` rendered with forward slashes, so the file is
/// stable across platforms.
fn key(id_path: &Path) -> String {
    id_path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};

    #[test]
    fn missing_file_is_empty_state() {
        let s = State::load(Path::new("/no/such/atproto.yaml")).unwrap();
        assert!(s.publication_uri.is_none());
        assert!(s.records.is_empty());
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempdir("pubstate");
        let path = dir.join(".italic/atproto.yaml");
        let mut s = State {
            did: Some("did:plc:abc".into()),
            publication_uri: Some("at://did:plc:abc/site.standard.publication/self".into()),
            ..State::default()
        };
        s.doc_mut(Path::new("posts/hello.md")).document = Some(RecordRef {
            rkey: "hello".into(),
            cid: "bafyrei".into(),
            uri: "at://did:plc:abc/site.standard.document/hello".into(),
        });
        s.doc_mut(Path::new("posts/hello.md")).bsky = Some(RecordRef {
            rkey: "3lwa".into(),
            cid: "bafycid".into(),
            uri: "at://did:plc:abc/app.bsky.feed.post/3lwa".into(),
        });
        s.save(&path).unwrap();

        let loaded = State::load(&path).unwrap();
        assert_eq!(loaded, s);
        assert_eq!(
            loaded
                .doc(Path::new("posts/hello.md"))
                .and_then(|r| r.document.as_ref())
                .map(|r| r.rkey.as_str()),
            Some("hello")
        );
        cleanup(&dir);
    }

    #[test]
    fn corrupt_file_errors() {
        let dir = tempdir("pubstate");
        let path = dir.join("atproto.yaml");
        // Valid YAML, but a sequence — not the mapping `State` deserializes from,
        // so loading is a hard error rather than a silent reset.
        fs::write(&path, "- not a map\n").unwrap();
        assert!(State::load(&path).is_err());
        cleanup(&dir);
    }

    #[test]
    fn doc_mut_inserts_then_reuses() {
        let mut s = State::default();
        s.doc_mut(Path::new("a.md")).document = Some(RecordRef::default());
        assert!(s.doc(Path::new("a.md")).unwrap().document.is_some());
        // Second access reuses the same entry.
        s.doc_mut(Path::new("a.md")).bsky = Some(RecordRef::default());
        let d = s.doc(Path::new("a.md")).unwrap();
        assert!(d.document.is_some() && d.bsky.is_some());
        assert_eq!(s.records.len(), 1);
    }
}
