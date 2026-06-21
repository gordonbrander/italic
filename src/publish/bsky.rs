//! `app.bsky.feed.post` summary records (feature 2): a short Bluesky post that
//! announces a long-form document and links back to it via an
//! `app.bsky.embed.external` card.
//!
//! Like the `site.standard.*` records, these are hand-rolled serde structs sent
//! as `Unknown` — simpler than constructing atrium's typed unions, and uniform
//! with [`crate::publish::document`]. Two details drive the design:
//!
//! - **`text` is capped at 300 graphemes** (not bytes). Summaries often exceed
//!   this, so [`truncate_graphemes`] trims with an ellipsis. Facets (which use
//!   *byte* offsets) are intentionally avoided in v1 — the URL lives in the embed
//!   card, not the text.
//! - **Create-once.** Bsky posts are conventionally immutable, so the orchestrator
//!   creates the post once and records it in state; this module only builds the
//!   record.

use crate::doc::Doc;
use atrium_api::types::BlobRef;
use chrono::SecondsFormat;
use serde::Serialize;
use serde_yaml_ng::Value;
use unicode_segmentation::UnicodeSegmentation;

pub const FEED_POST_NSID: &str = "app.bsky.feed.post";
pub const EXTERNAL_EMBED_NSID: &str = "app.bsky.embed.external";

/// Bluesky's post-text limit, counted in grapheme clusters.
pub const MAX_GRAPHEMES: usize = 300;

/// An `app.bsky.feed.post` record.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPost {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub text: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<ExternalEmbed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub langs: Vec<String>,
}

/// An `app.bsky.embed.external` link card.
#[derive(Debug, Clone, Serialize)]
pub struct ExternalEmbed {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub external: External,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct External {
    pub uri: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb: Option<BlobRef>,
}

/// True when a doc opts out of being announced via `bsky: false` frontmatter.
pub fn opted_out(doc: &Doc) -> bool {
    doc.data.get("bsky").and_then(Value::as_bool) == Some(false)
}

/// Truncate `text` to at most `max` grapheme clusters, appending `…` (one
/// grapheme) when truncation occurs. Bluesky counts graphemes, not bytes, so an
/// emoji or combining sequence counts once. Trailing whitespace before the
/// ellipsis is trimmed.
pub fn truncate_graphemes(text: &str, max: usize) -> String {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    if graphemes.len() <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let kept: String = graphemes[..max - 1].concat();
    format!("{}…", kept.trim_end())
}

/// Render the announcement text for `doc`. A per-post `bsky_text:` frontmatter
/// override wins; otherwise the optional Tera `template` is rendered with
/// `title`/`summary` in scope; otherwise the text is `doc.summary`. The result is
/// always grapheme-truncated to [`MAX_GRAPHEMES`].
pub fn render_text(doc: &Doc, template: Option<&str>) -> anyhow::Result<String> {
    if let Some(override_text) = doc.data.get("bsky_text").and_then(Value::as_str) {
        return Ok(truncate_graphemes(override_text.trim(), MAX_GRAPHEMES));
    }
    let raw = match template {
        Some(tpl) => {
            let mut ctx = tera::Context::new();
            ctx.insert("title", &doc.title);
            ctx.insert("summary", &doc.summary);
            // Plaintext, not HTML — autoescape off.
            tera::Tera::one_off(tpl, &ctx, false)
                .map_err(|e| anyhow::anyhow!("rendering bsky post_template: {e}"))?
        }
        None => doc.summary.clone(),
    };
    Ok(truncate_graphemes(raw.trim(), MAX_GRAPHEMES))
}

/// Build the `app.bsky.embed.external` link card for `doc`.
pub fn external_embed(url: String, doc: &Doc, thumb: Option<BlobRef>) -> ExternalEmbed {
    ExternalEmbed {
        type_: EXTERNAL_EMBED_NSID,
        external: External {
            uri: url,
            title: doc.title.clone(),
            description: doc.summary.clone(),
            thumb,
        },
    }
}

/// Assemble the `app.bsky.feed.post` record.
pub fn feed_post(doc: &Doc, text: String, embed: Option<ExternalEmbed>) -> FeedPost {
    FeedPost {
        type_: FEED_POST_NSID,
        text,
        created_at: doc.date.to_rfc3339_opts(SecondsFormat::Millis, true),
        embed,
        langs: vec!["en".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};
    use serde_yaml_ng::Mapping;
    use std::path::PathBuf;

    fn at(date: &str) -> DateTime<Utc> {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap()
            .and_utc()
    }

    fn doc() -> Doc {
        Doc {
            id_path: PathBuf::from("blog/hello.md"),
            title: "Hello World".into(),
            summary: "A short summary.".into(),
            date: at("2024-01-20"),
            ..Doc::default()
        }
    }

    #[test]
    fn truncate_under_limit_is_unchanged() {
        assert_eq!(truncate_graphemes("hello", 300), "hello");
        assert_eq!(truncate_graphemes("", 300), "");
    }

    #[test]
    fn truncate_counts_graphemes_not_bytes() {
        // Family emoji is a single grapheme cluster of many bytes.
        let s = "👩‍👩‍👧‍👦 a";
        assert_eq!(s.graphemes(true).count(), 3); // emoji, space, 'a'
        assert_eq!(truncate_graphemes(s, 300), s);
    }

    #[test]
    fn truncate_adds_ellipsis_when_over_limit() {
        let s = "abcdef";
        let out = truncate_graphemes(s, 4);
        // 3 kept graphemes + ellipsis = 4 graphemes.
        assert_eq!(out, "abc…");
        assert_eq!(out.graphemes(true).count(), 4);
    }

    #[test]
    fn truncate_trims_trailing_space_before_ellipsis() {
        let out = truncate_graphemes("ab cd", 4);
        assert_eq!(out, "ab…");
    }

    #[test]
    fn truncate_emoji_boundary_not_split() {
        // 5 flags = 5 graphemes; limit 3 keeps 2 + ellipsis, never half a flag.
        let flags = "🇺🇸🇬🇧🇫🇷🇩🇪🇯🇵";
        let out = truncate_graphemes(flags, 3);
        assert_eq!(out.graphemes(true).count(), 3);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn render_text_defaults_to_summary() {
        let t = render_text(&doc(), None).unwrap();
        assert_eq!(t, "A short summary.");
    }

    #[test]
    fn render_text_uses_template() {
        let t = render_text(&doc(), Some("{{ title }} — {{ summary }}")).unwrap();
        assert_eq!(t, "Hello World — A short summary.");
    }

    #[test]
    fn render_text_frontmatter_override_wins() {
        let mut d = doc();
        d.data.insert(
            Value::String("bsky_text".into()),
            Value::String("Custom!".into()),
        );
        let t = render_text(&d, Some("{{ title }}")).unwrap();
        assert_eq!(t, "Custom!");
    }

    #[test]
    fn opted_out_detects_false_flag() {
        let mut d = doc();
        assert!(!opted_out(&d));
        d.data
            .insert(Value::String("bsky".into()), Value::Bool(false));
        assert!(opted_out(&d));
        // A non-false value does not opt out.
        let mut d2 = doc();
        d2.data
            .insert(Value::String("bsky".into()), Value::Bool(true));
        assert!(!opted_out(&d2));
    }

    #[test]
    fn feed_post_serializes_with_external_embed() {
        let d = doc();
        let embed = external_embed("https://example.com/blog/hello/".into(), &d, None);
        let post = feed_post(&d, "Hello World — read more".into(), Some(embed));
        let v = serde_json::to_value(&post).unwrap();
        assert_eq!(v["$type"], "app.bsky.feed.post");
        assert_eq!(v["text"], "Hello World — read more");
        assert_eq!(v["createdAt"], "2024-01-20T14:30:00.000Z");
        assert_eq!(v["langs"], serde_json::json!(["en"]));
        assert_eq!(v["embed"]["$type"], "app.bsky.embed.external");
        assert_eq!(
            v["embed"]["external"]["uri"],
            "https://example.com/blog/hello/"
        );
        assert_eq!(v["embed"]["external"]["title"], "Hello World");
        assert_eq!(v["embed"]["external"]["description"], "A short summary.");
        assert!(v["embed"]["external"].get("thumb").is_none());
    }

    #[test]
    fn feed_post_without_embed_omits_field() {
        let _ = Mapping::new();
        let post = feed_post(&doc(), "text".into(), None);
        let v = serde_json::to_value(&post).unwrap();
        assert!(v.get("embed").is_none());
    }
}
