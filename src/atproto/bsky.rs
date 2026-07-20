//! Hand-rolled `app.bsky.feed.post` record types and the `Doc` → post mapping,
//! following the same pattern as [`crate::atproto::document`]: plain serde
//! structs serialized to the documented JSON shape (`$type` discriminator +
//! camelCase, optionals omitted).
//!
//! A post is the social announcement for a doc: the author-written `bsky:`
//! frontmatter text plus an `app.bsky.embed.external` link card pointing at
//! the canonical URL (so the link needs no facets). Posts are create-once —
//! see [`crate::atproto::state`].

use crate::doc::Doc;
use anyhow::{Result, anyhow};
use atrium_api::types::BlobRef;
use serde::Serialize;
use unicode_segmentation::UnicodeSegmentation;

pub const POST_NSID: &str = "app.bsky.feed.post";
const EMBED_EXTERNAL_NSID: &str = "app.bsky.embed.external";

/// Bluesky's post-length cap, counted in extended grapheme clusters (not bytes
/// or chars).
pub const MAX_GRAPHEMES: usize = 300;

/// An `app.bsky.feed.post` record.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub text: String,
    /// RFC3339, e.g. `2026-07-20T14:30:00.000Z` — wall-clock time at create.
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<ExternalEmbed>,
}

/// An `app.bsky.embed.external` link card.
#[derive(Debug, Clone, Serialize)]
pub struct ExternalEmbed {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub external: External,
}

/// The card contents: canonical URL + title + description, with the doc's
/// cover image as the optional thumbnail.
#[derive(Debug, Clone, Serialize)]
pub struct External {
    pub uri: String,
    pub title: String,
    /// Required by the lexicon — empty string when the doc has no summary.
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb: Option<BlobRef>,
}

/// Build a post: `text` from `bsky:` frontmatter, link card from the doc's
/// canonical URL/title/summary, `thumb` from the cover machinery (derived for
/// dry runs, uploaded for real writes).
pub fn post(
    text: &str,
    url: &str,
    title: &str,
    description: &str,
    thumb: Option<BlobRef>,
    created_at: &str,
) -> Post {
    Post {
        type_: POST_NSID,
        text: text.to_string(),
        created_at: created_at.to_string(),
        embed: Some(ExternalEmbed {
            type_: EMBED_EXTERNAL_NSID,
            external: External {
                uri: url.to_string(),
                title: title.to_string(),
                description: description.to_string(),
                thumb,
            },
        }),
    }
}

/// The doc's `bsky:` frontmatter text, validated. `Ok(None)` when the key is
/// absent — the author deliberately opted out. A present key must be a
/// non-empty string within Bluesky's grapheme cap; anything else is a hard
/// error naming the doc, raised before any network work.
pub fn frontmatter_text(doc: &Doc) -> Result<Option<String>> {
    let Some(value) = doc.data.get("bsky") else {
        return Ok(None);
    };
    let text = value.as_str().map(str::trim).ok_or_else(|| {
        anyhow!(
            "{}: `bsky:` must be a string (the post text)",
            doc.id_path.display()
        )
    })?;
    if text.is_empty() {
        return Err(anyhow!(
            "{}: `bsky:` is empty — write the post text, or remove the key to skip the post",
            doc.id_path.display()
        ));
    }
    let graphemes = text.graphemes(true).count();
    if graphemes > MAX_GRAPHEMES {
        return Err(anyhow!(
            "{}: `bsky:` text is {graphemes} graphemes — Bluesky caps posts at {MAX_GRAPHEMES}",
            doc.id_path.display()
        ));
    }
    Ok(Some(text.to_string()))
}

/// The first `max` graphemes of `text`, with an ellipsis when truncated — for
/// confirmation prompts and dry-run listings.
pub fn preview(text: &str, max: usize) -> String {
    let mut graphemes = text.graphemes(true);
    let head: String = graphemes.by_ref().take(max).collect();
    match graphemes.next() {
        Some(_) => format!("{head}…"),
        None => head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml_ng::{Mapping, Value};
    use std::path::PathBuf;

    fn doc_with_bsky(value: Value) -> Doc {
        let mut data = Mapping::new();
        data.insert(Value::String("bsky".into()), value);
        Doc {
            id_path: PathBuf::from("blog/hello.md"),
            data,
            ..Doc::default()
        }
    }

    #[test]
    fn post_serializes_to_lexicon_shape() {
        let rec = post(
            "New post!",
            "https://example.com/blog/hello/",
            "Hello",
            "A greeting.",
            None,
            "2026-07-20T14:30:00.000Z",
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["$type"], "app.bsky.feed.post");
        assert_eq!(v["text"], "New post!");
        assert_eq!(v["createdAt"], "2026-07-20T14:30:00.000Z");
        assert_eq!(v["embed"]["$type"], "app.bsky.embed.external");
        let ext = &v["embed"]["external"];
        assert_eq!(ext["uri"], "https://example.com/blog/hello/");
        assert_eq!(ext["title"], "Hello");
        assert_eq!(ext["description"], "A greeting.");
        // No thumb → omitted entirely.
        assert!(ext.get("thumb").is_none());
    }

    #[test]
    fn post_description_empty_string_is_present() {
        // The lexicon requires `description`, so no-summary docs send "".
        let rec = post(
            "t",
            "https://e.com/",
            "T",
            "",
            None,
            "2026-01-01T00:00:00.000Z",
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["embed"]["external"]["description"], "");
    }

    #[test]
    fn post_thumb_serializes_as_blob() {
        let thumb = crate::atproto::document::derived_blob_ref(b"png bytes").unwrap();
        let rec = post(
            "t",
            "https://e.com/",
            "T",
            "",
            Some(thumb),
            "2026-01-01T00:00:00.000Z",
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["embed"]["external"]["thumb"]["$type"], "blob");
    }

    #[test]
    fn frontmatter_absent_is_none() {
        let doc = Doc::default();
        assert_eq!(frontmatter_text(&doc).unwrap(), None);
    }

    #[test]
    fn frontmatter_string_is_trimmed_and_returned() {
        let doc = doc_with_bsky(Value::String("  New post!  ".into()));
        assert_eq!(
            frontmatter_text(&doc).unwrap().as_deref(),
            Some("New post!")
        );
    }

    #[test]
    fn frontmatter_non_string_errors_with_doc_path() {
        let doc = doc_with_bsky(Value::Bool(true));
        let err = format!("{:#}", frontmatter_text(&doc).unwrap_err());
        assert!(err.contains("blog/hello.md"), "{err}");
        assert!(err.contains("must be a string"), "{err}");
    }

    #[test]
    fn frontmatter_empty_errors() {
        let doc = doc_with_bsky(Value::String("   ".into()));
        let err = format!("{:#}", frontmatter_text(&doc).unwrap_err());
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn grapheme_cap_is_boundary_exact() {
        // 300 graphemes: fine. 301: error. A multi-codepoint emoji counts as 1.
        let family = "👨\u{200d}👩\u{200d}👧\u{200d}👦"; // ZWJ sequence, 1 grapheme
        assert_eq!(family.graphemes(true).count(), 1);
        let at_cap = format!("{}{}", family, "a".repeat(MAX_GRAPHEMES - 1));
        let doc = doc_with_bsky(Value::String(at_cap.clone()));
        assert_eq!(frontmatter_text(&doc).unwrap(), Some(at_cap));
        let over = "a".repeat(MAX_GRAPHEMES + 1);
        let doc = doc_with_bsky(Value::String(over));
        let err = format!("{:#}", frontmatter_text(&doc).unwrap_err());
        assert!(err.contains("301 graphemes"), "{err}");
    }

    #[test]
    fn preview_truncates_on_grapheme_boundary() {
        assert_eq!(preview("short", 60), "short");
        assert_eq!(preview("abcdef", 3), "abc…");
        // Exactly at the limit: no ellipsis.
        assert_eq!(preview("abc", 3), "abc");
    }
}
