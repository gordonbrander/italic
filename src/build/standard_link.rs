//! Per-document standard.site ownership proof. standard.site verifies each
//! published page with a `<link rel="site.standard.document" href="at://…">` tag
//! in its `<head>`. The AT-URI is dynamic (assigned by the PDS, recorded in the
//! publish state file), so this pass injects it into each published doc's `data`
//! as `atproto_uri`, letting a theme emit:
//!
//! ```html
//! {% if page.data.atproto_uri %}
//! <link rel="site.standard.document" href="{{ page.data.atproto_uri | safe }}">
//! {% endif %}
//! ```
//!
//! (`| safe` keeps the `at://` slashes unescaped; without it Tera HTML-escapes
//! them — still a valid `href`, just noisier.)
//!
//! italic ships no default theme, so the tag itself is the theme author's to add;
//! this pass just provides the binding. It runs inside [`crate::build::build_index`]
//! before the index freezes, is gated on `publish.verification`, and is a no-op
//! until `italic publish` has written document records to the state file.

use crate::config::Config;
use crate::doc_index::DocIndex;
use crate::publish::state::{STATE_PATH, State};
use anyhow::Result;
use serde_yaml_ng::Value;
use std::path::Path;

/// Frontmatter/data key the AT-URI is exposed under (read by templates as
/// `page.data.atproto_uri`).
pub const DATA_KEY: &str = "atproto_uri";

pub fn run(config: &Config, index: &mut DocIndex) -> Result<()> {
    let Some(publish) = &config.publish else {
        return Ok(());
    };
    if !publish.verification {
        return Ok(());
    }
    let state = State::load(Path::new(STATE_PATH))?;
    inject(index, &state);
    Ok(())
}

/// Set `atproto_uri` on every doc the state has a document record for. Split from
/// [`run`] (which adds config gating + the state-file read) so it is unit-testable.
fn inject(index: &mut DocIndex, state: &State) {
    for (id, records) in &state.records {
        let Some(uri) = records.document.as_ref().map(|r| r.uri.as_str()) else {
            continue;
        };
        if uri.is_empty() {
            continue;
        }
        if let Some(doc) = index.doc_mut(Path::new(id)) {
            doc.data.insert(
                Value::String(DATA_KEY.to_string()),
                Value::String(uri.to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;
    use crate::publish::state::RecordRef;
    use std::path::PathBuf;

    fn index_with(id: &str) -> DocIndex {
        let mut index = DocIndex::new();
        index.insert(Doc {
            id_path: PathBuf::from(id),
            ..Doc::default()
        });
        index
    }

    fn state_with_document(id: &str, uri: &str) -> State {
        let mut state = State::default();
        state.doc_mut(Path::new(id)).document = Some(RecordRef {
            rkey: "r".into(),
            cid: "c".into(),
            uri: uri.into(),
        });
        state
    }

    #[test]
    fn injects_uri_into_doc_data() {
        let mut index = index_with("posts/hello.md");
        let state = state_with_document(
            "posts/hello.md",
            "at://did:plc:abc/site.standard.document/posts-hello",
        );
        inject(&mut index, &state);
        let doc = index.doc(Path::new("posts/hello.md")).unwrap();
        assert_eq!(
            doc.data.get(DATA_KEY).and_then(Value::as_str),
            Some("at://did:plc:abc/site.standard.document/posts-hello")
        );
    }

    #[test]
    fn skips_docs_without_a_document_record() {
        let mut index = index_with("posts/hello.md");
        // State references a different doc; hello stays untouched.
        let state = state_with_document("posts/other.md", "at://x/y/z");
        inject(&mut index, &state);
        let doc = index.doc(Path::new("posts/hello.md")).unwrap();
        assert!(doc.data.get(DATA_KEY).is_none());
    }

    #[test]
    fn empty_uri_is_ignored() {
        let mut index = index_with("posts/hello.md");
        let state = state_with_document("posts/hello.md", "");
        inject(&mut index, &state);
        let doc = index.doc(Path::new("posts/hello.md")).unwrap();
        assert!(doc.data.get(DATA_KEY).is_none());
    }
}
