//! Published-post state for Bluesky announcements: `.italic/bsky.yaml`.
//!
//! Documents are stateless — deterministic rkeys, `putRecord`, the PDS as
//! source of truth. Bluesky posts are the one exception: the lexicon requires
//! TID rkeys (PDS-assigned at create time) and posts are create-once, so
//! italic must remember which docs already have one. That memory is this
//! committed, human-editable YAML file mapping each doc's id_path to its
//! post's `{uri, cid, createdAt}` — the same pair the document record's
//! `bskyPostRef` carries. Losing the file risks duplicate posts, which is why
//! it belongs in version control.

use crate::doc::Doc;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Where the state file lives, relative to the working directory (like
/// `config.yaml` and everything else in italic).
pub const STATE_PATH: &str = ".italic/bsky.yaml";

/// Comment header written above the YAML body (serde can't emit comments).
const HEADER: &str = "\
# Bluesky posts created by `italic atproto publish`.
# Commit this file. Each doc gets at most one post, created once and never
# updated or deleted by italic. If you rename a doc, move its entry to the
# new id_path to avoid a duplicate post.
";

/// The parsed state file.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Format version; always 1. Present so a future format change (or a
    /// hand-edited file) fails loudly instead of misparsing.
    pub version: u32,
    /// id_path (forward-slash normalized, see [`state_key`]) → created post.
    #[serde(default)]
    pub posts: BTreeMap<String, PostRef>,
}

/// A created `app.bsky.feed.post`, as recorded at create time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostRef {
    /// AT-URI, e.g. `at://did:plc:abc/app.bsky.feed.post/3lwabc22xyz`.
    pub uri: String,
    /// The record CID — with `uri`, the `com.atproto.repo.strongRef` pair.
    pub cid: String,
    /// RFC3339 instant, same value as the post record's `createdAt`.
    pub created_at: String,
}

/// The state-map key for a doc: its id_path, forward-slash normalized so the
/// file is portable across platforms.
pub fn state_key(doc: &Doc) -> String {
    doc.id_path.to_string_lossy().replace('\\', "/")
}

/// Load the state file. A missing file is simply first-run (empty state); an
/// unparseable or wrong-version file is a hard error — silently starting fresh
/// would re-create every post.
pub fn load(path: &Path) -> Result<State> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(State {
                version: 1,
                posts: BTreeMap::new(),
            });
        }
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let state: State = serde_yaml_ng::from_str(&source).with_context(|| {
        format!(
            "{} is not a valid bsky state file — fix it or delete it \
             (deleting means already-announced docs would get duplicate posts)",
            path.display()
        )
    })?;
    if state.version != 1 {
        return Err(anyhow!(
            "{}: unsupported state version {} (this italic understands version 1)",
            path.display(),
            state.version
        ));
    }
    Ok(state)
}

/// Save the state file: comment header + YAML body, written to a temp file and
/// renamed into place so a crash mid-write can't corrupt it (a lost state file
/// means duplicate posts).
pub fn save(state: &State, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let body = serde_yaml_ng::to_string(state).context("serializing bsky state")?;
    let tmp = path.with_extension("yaml.tmp");
    fs::write(&tmp, format!("{HEADER}{body}"))
        .with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("moving {} into place at {}", tmp.display(), path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};

    fn post_ref(rkey: &str) -> PostRef {
        PostRef {
            uri: format!("at://did:plc:abc/app.bsky.feed.post/{rkey}"),
            cid: "bafyreib2aaa".into(),
            created_at: "2026-07-20T18:04:11.000Z".into(),
        }
    }

    #[test]
    fn missing_file_is_empty_state() {
        let state = load(Path::new("no/such/dir/bsky.yaml")).unwrap();
        assert_eq!(state.version, 1);
        assert!(state.posts.is_empty());
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempdir("state_round_trip");
        let path = dir.join(".italic/bsky.yaml");
        let mut state = State {
            version: 1,
            posts: BTreeMap::new(),
        };
        state.posts.insert("blog/hello.md".into(), post_ref("3lwa"));
        save(&state, &path).unwrap();
        // The written file leads with the human-facing header comment.
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.starts_with("# Bluesky posts"), "{written}");
        assert_eq!(load(&path).unwrap(), state);
        cleanup(&dir);
    }

    #[test]
    fn corrupt_file_errors_with_path() {
        let dir = tempdir("state_corrupt");
        let path = dir.join("bsky.yaml");
        std::fs::write(&path, "posts: [not, a, map]\n").unwrap();
        let err = format!("{:#}", load(&path).unwrap_err());
        assert!(err.contains("bsky.yaml"), "{err}");
        assert!(err.contains("duplicate posts"), "{err}");
        cleanup(&dir);
    }

    #[test]
    fn wrong_version_errors() {
        let dir = tempdir("state_version");
        let path = dir.join("bsky.yaml");
        std::fs::write(&path, "version: 2\nposts: {}\n").unwrap();
        let err = format!("{:#}", load(&path).unwrap_err());
        assert!(err.contains("version 2"), "{err}");
        cleanup(&dir);
    }
}
