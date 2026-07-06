//! Per-document standard.site ownership proof. standard.site verifies each
//! published page with a `<link rel="site.standard.document" href="at://…">` tag
//! in its `<head>`. The AT-URI is fully derivable from the inputs at hand —
//! `at://` + the account DID (`ITALIC_ATPROTO_DID`) + the collection NSID + an
//! rkey hashed from the doc's canonical URL (see
//! [`crate::atproto::document::document_uri`]) — so this pass computes it
//! directly; no publish state, no network, and the proofs are present in every
//! build (including CI, where the state file typically doesn't exist).
//!
//! The pass injects the URI into each published doc's `data` as `atproto_uri`;
//! the tag itself is emitted by the built-in `standard_link` metadata filter
//! (and by the `metadata` umbrella — see `crate::tera_env::meta`), so themes
//! using `{{ page | metadata(site=site) }}` get verification for free;
//! hand-rolled heads can still read `page.data.atproto_uri` directly.
//!
//! It runs inside [`crate::build::build_index`] before the index freezes, and is
//! a no-op unless `atproto.verification` (default on), the `ITALIC_ATPROTO_DID`
//! env var, and `site.url` are all present. Because `italic atproto publish` derives
//! record addresses through the same functions, a page's proof link and its
//! record can only disagree between a URL change and the next atproto.

use crate::atproto::document;
use crate::config::Config;
use crate::doc_index::DocIndex;
use anyhow::Result;
use serde_yaml_ng::Value;
use std::path::PathBuf;

/// Frontmatter/data key the AT-URI is exposed under (read by templates as
/// `page.data.atproto_uri`).
pub const DATA_KEY: &str = "atproto_uri";

pub fn run(config: &Config, did: Option<&str>, index: &mut DocIndex) -> Result<()> {
    let Some(atproto) = &config.atproto else {
        return Ok(());
    };
    if !atproto.verification {
        return Ok(());
    }
    let (Some(did), Some(site_url)) = (did, &config.site_url) else {
        return Ok(());
    };
    inject(index, did, site_url, &config.base_path, &atproto.collection);
    Ok(())
}

/// Derive and set `atproto_uri` on every non-draft doc in the publish
/// collection. Drafts are skipped — they are never published (publish builds
/// without drafts), so a derived URI would assert a record that will never
/// exist. Split from [`run`] (which adds the config gating) so it is
/// unit-testable.
fn inject(index: &mut DocIndex, did: &str, site_url: &str, base_path: &str, collection: &str) {
    let ids: Vec<PathBuf> = index
        .get_collection(collection)
        .filter(|doc| !doc.draft)
        .map(|doc| doc.id_path.clone())
        .collect();
    for id in ids {
        let Some(doc) = index.doc_mut(&id) else {
            continue;
        };
        let url = document::canonical_url(doc, Some(site_url), base_path);
        doc.data.insert(
            Value::String(DATA_KEY.to_string()),
            Value::String(document::document_uri(did, &url)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;
    use crate::query::Query;
    use serde_yaml_ng::Mapping;
    use std::path::{Path, PathBuf};

    const DID: &str = "did:plc:testabc";
    const SITE_URL: &str = "https://example.com";

    fn doc(id: &str, output: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id),
            output_path: PathBuf::from(output),
            ..Doc::default()
        }
    }

    /// An index of `docs` whose `posts` collection matches `posts/*`.
    fn index_with(docs: Vec<Doc>) -> DocIndex {
        let mut index = DocIndex::new();
        for doc in docs {
            index.insert(doc);
        }
        let m: Mapping = serde_yaml_ng::from_str("path: posts/*").unwrap();
        index.define_collection("posts", &Query::from_yaml_mapping(&m).unwrap());
        index
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
    fn injects_derived_uri_for_collection_members() {
        let mut index = index_with(vec![doc("posts/hello.md", "posts/hello/index.html")]);
        inject(&mut index, DID, SITE_URL, "", "posts");
        // Exactly the URI atproto would write: same canonical_url + rkey fns.
        let expected = document::document_uri(DID, "https://example.com/posts/hello/");
        assert_eq!(uri_of(&index, "posts/hello.md"), Some(expected.as_str()));
    }

    #[test]
    fn skips_drafts() {
        let mut draft = doc("posts/wip.md", "posts/wip/index.html");
        draft.draft = true;
        let mut index = index_with(vec![draft]);
        inject(&mut index, DID, SITE_URL, "", "posts");
        assert!(uri_of(&index, "posts/wip.md").is_none());
    }

    #[test]
    fn skips_docs_outside_the_collection() {
        let mut index = index_with(vec![
            doc("posts/hello.md", "posts/hello/index.html"),
            doc("about.md", "about/index.html"),
        ]);
        inject(&mut index, DID, SITE_URL, "", "posts");
        assert!(uri_of(&index, "posts/hello.md").is_some());
        assert!(uri_of(&index, "about.md").is_none());
    }

    #[test]
    fn base_path_participates_in_the_derived_url() {
        let mut index = index_with(vec![doc("posts/hello.md", "posts/hello/index.html")]);
        inject(&mut index, DID, SITE_URL, "/blog", "posts");
        let expected = document::document_uri(DID, "https://example.com/blog/posts/hello/");
        assert_eq!(uri_of(&index, "posts/hello.md"), Some(expected.as_str()));
    }
}
