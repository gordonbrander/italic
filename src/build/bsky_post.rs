//! Expose each doc's Bluesky announcement post to templates. `italic atproto
//! publish` records created posts in `.italic/bsky.yaml` (see
//! [`crate::atproto::state`]); this pass copies each recorded post's AT-URI
//! into the matching doc's `data` as `bsky_uri`, so templates can link the
//! post — or render its replies as comments — behind a
//! `{% if page.data.bsky_uri %}` guard.
//!
//! The state file is the small side, so the pass walks its entries rather than
//! the corpus. Docs never announced are untouched, which doubles as an escape
//! hatch: a post created outside italic can be wired up by setting `bsky_uri`
//! in frontmatter by hand. A missing state file loads as empty state (no-op);
//! a corrupt one is the same hard error `italic atproto` raises — a committed,
//! hand-editable file that fails to parse should be fixed, not skipped.

use crate::atproto::state::{self, State};
use crate::doc_index::DocIndex;
use anyhow::Result;
use serde_yaml_ng::Value;
use std::path::Path;

/// Data key the post's AT-URI is exposed under (read by templates as
/// `page.data.bsky_uri`).
pub const DATA_KEY: &str = "bsky_uri";

pub fn run(index: &mut DocIndex) -> Result<()> {
    let bsky = state::load(Path::new(state::STATE_PATH))?;
    inject(index, &bsky);
    Ok(())
}

/// Set `bsky_uri` on every doc with a recorded announcement post. State wins
/// over frontmatter for announced docs — the state file is the record of what
/// was actually posted. Split from [`run`] (which reads the file) so it is
/// unit-testable.
fn inject(index: &mut DocIndex, state: &State) {
    for (id, post) in &state.posts {
        let Some(doc) = index.doc_mut(Path::new(id)) else {
            continue;
        };
        doc.data.insert(
            Value::String(DATA_KEY.to_string()),
            Value::String(post.uri.clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atproto::state::PostRef;
    use crate::doc::Doc;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn doc(id: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id),
            ..Doc::default()
        }
    }

    fn index_with(docs: Vec<Doc>) -> DocIndex {
        let mut index = DocIndex::new();
        for doc in docs {
            index.insert(doc);
        }
        index
    }

    fn state_with(entries: &[(&str, &str)]) -> State {
        let mut posts = BTreeMap::new();
        for (id, rkey) in entries {
            posts.insert(
                id.to_string(),
                PostRef {
                    uri: format!("at://did:plc:abc/app.bsky.feed.post/{rkey}"),
                    cid: "bafyreib2aaa".into(),
                    created_at: "2026-07-20T18:04:11.000Z".into(),
                },
            );
        }
        State { version: 1, posts }
    }

    fn uri_of<'a>(index: &'a DocIndex, id: &str) -> Option<&'a str> {
        index
            .doc(Path::new(id))
            .unwrap()
            .data
            .get(DATA_KEY)
            .and_then(Value::as_str)
    }

    #[test]
    fn injects_the_recorded_uri() {
        let mut index = index_with(vec![doc("posts/hello.md")]);
        inject(&mut index, &state_with(&[("posts/hello.md", "3lwa")]));
        assert_eq!(
            uri_of(&index, "posts/hello.md"),
            Some("at://did:plc:abc/app.bsky.feed.post/3lwa")
        );
    }

    #[test]
    fn leaves_unannounced_docs_untouched() {
        let mut index = index_with(vec![doc("posts/hello.md"), doc("posts/quiet.md")]);
        inject(&mut index, &state_with(&[("posts/hello.md", "3lwa")]));
        assert!(uri_of(&index, "posts/quiet.md").is_none());
    }

    #[test]
    fn entry_matching_no_doc_is_a_no_op() {
        let mut index = index_with(vec![doc("posts/hello.md")]);
        inject(&mut index, &state_with(&[("posts/renamed.md", "3lwa")]));
        assert!(uri_of(&index, "posts/hello.md").is_none());
    }

    #[test]
    fn hand_set_frontmatter_survives_without_a_state_entry() {
        let mut announced = doc("posts/manual.md");
        announced.data.insert(
            Value::String(DATA_KEY.to_string()),
            Value::String("at://did:plc:me/app.bsky.feed.post/byhand".to_string()),
        );
        let mut index = index_with(vec![announced]);
        inject(&mut index, &state_with(&[]));
        assert_eq!(
            uri_of(&index, "posts/manual.md"),
            Some("at://did:plc:me/app.bsky.feed.post/byhand")
        );
    }

    #[test]
    fn state_wins_over_frontmatter_for_announced_docs() {
        let mut announced = doc("posts/hello.md");
        announced.data.insert(
            Value::String(DATA_KEY.to_string()),
            Value::String("at://did:plc:me/app.bsky.feed.post/stale".to_string()),
        );
        let mut index = index_with(vec![announced]);
        inject(&mut index, &state_with(&[("posts/hello.md", "3lwa")]));
        assert_eq!(
            uri_of(&index, "posts/hello.md"),
            Some("at://did:plc:abc/app.bsky.feed.post/3lwa")
        );
    }
}
