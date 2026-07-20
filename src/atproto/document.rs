//! Hand-rolled `site.standard.document` / `site.standard.publication` record
//! types and the `Doc`/config → record mapping.
//!
//! The `site.standard.*` lexicons have no canonical Rust types in `atrium-api`,
//! and the record set is small, so we serialize plain serde structs to the
//! documented JSON shape (`$type` discriminator + camelCase fields, optionals
//! omitted). The PDS validates optimistically, and records are sent as
//! `Unknown` either way (see [`crate::atproto::client`]). Blob references reuse
//! `atrium_api::types::BlobRef`, which already serializes to the data-model blob
//! shape.

use crate::doc::Doc;
use crate::html;
use crate::permalink;
use atrium_api::types::{Blob, BlobRef, CidLink, TypedBlobRef};
use chrono::SecondsFormat;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const DOCUMENT_NSID: &str = "site.standard.document";
pub const PUBLICATION_NSID: &str = "site.standard.publication";
pub const THEME_BASIC_NSID: &str = "site.standard.theme.basic";
const THEME_COLOR_RGB_NSID: &str = "site.standard.theme.color#rgb";

/// External content lexicon ([markpub.at](https://markpub.at/)) used to carry
/// the full body as Markdown in the document's `content` open union.
const MARKDOWN_NSID: &str = "at.markpub.markdown";
const MARKDOWN_TEXT_NSID: &str = "at.markpub.text";

/// A `site.standard.document` record. Fields map directly from a [`Doc`]; see
/// [`document`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    /// AT-URI of the owning `site.standard.publication` record.
    pub site: String,
    pub title: String,
    /// RFC3339, e.g. `2024-01-20T14:30:00.000Z`.
    pub published_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Site-root-relative path (combined with the publication URL to build the
    /// canonical URL), e.g. `/blog/getting-started`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<BlobRef>,
    /// Full body as Markdown via the `at.markpub.markdown` open-union entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MarkdownContent>,
    /// Plaintext rendering of the body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Strong reference to the doc's Bluesky announcement post, when one has
    /// been created (see [`crate::atproto::state`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bsky_post_ref: Option<StrongRef>,
}

/// A `com.atproto.repo.strongRef` — a plain `{uri, cid}` object (no `$type`).
#[derive(Debug, Clone, Serialize)]
pub struct StrongRef {
    pub uri: String,
    pub cid: String,
}

impl From<&crate::atproto::state::PostRef> for StrongRef {
    fn from(post: &crate::atproto::state::PostRef) -> Self {
        Self {
            uri: post.uri.clone(),
            cid: post.cid.clone(),
        }
    }
}

/// An `at.markpub.markdown` content entry: the full document body as Markdown,
/// embedded in the document's `content` open union. `flavor`/`renderingRules`
/// are advisory hints for re-rendering (we run comrak with the GFM extension
/// set; see [`crate::tera_env`]).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownContent {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub flavor: &'static str,
    pub rendering_rules: &'static str,
    pub text: MarkdownText,
}

/// The `at.markpub.text` object nested inside [`MarkdownContent`], holding the
/// raw Markdown string.
#[derive(Debug, Clone, Serialize)]
pub struct MarkdownText {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub markdown: String,
}

/// A `site.standard.publication` record (the site/blog itself).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub url: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<BlobRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_theme: Option<BasicTheme>,
}

/// The `site.standard.theme.basic` object embedded on a [`Publication`] as
/// `basicTheme`. Built from the config-side colors by [`basic_theme`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicTheme {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub background: RgbColor,
    pub foreground: RgbColor,
    pub accent: RgbColor,
    pub accent_foreground: RgbColor,
}

/// A `site.standard.theme.color#rgb` object.
#[derive(Debug, Clone, Serialize)]
pub struct RgbColor {
    #[serde(rename = "$type")]
    pub type_: &'static str,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<crate::atproto::config::Rgb> for RgbColor {
    fn from(rgb: crate::atproto::config::Rgb) -> Self {
        Self {
            type_: THEME_COLOR_RGB_NSID,
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        }
    }
}

/// Map the config-side theme colors to the `site.standard.theme.basic` record
/// object.
pub fn basic_theme(theme: &crate::atproto::config::BasicTheme) -> BasicTheme {
    BasicTheme {
        type_: THEME_BASIC_NSID,
        background: theme.background.into(),
        foreground: theme.foreground.into(),
        accent: theme.accent.into(),
        accent_foreground: theme.accent_foreground.into(),
    }
}

/// Stable record key: the full SHA-256 of `input`, base32-encoded and lowercased
/// (52 chars). Deterministic and collision-resistant; the charset is rkey-safe
/// (lowercase `a`–`z`, `2`–`7`) and the length is well under the 512-char limit.
fn rkey_hash(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    data_encoding::BASE32_NOPAD.encode(&digest).to_lowercase()
}

/// CIDv1 string (raw codec, sha-256) for blob bytes, exactly as the PDS mints
/// for `uploadBlob`: multibase `b` + base32-lower of the CID bytes
/// `[0x01 (v1), 0x55 (raw), 0x12 (sha2-256), 0x20 (32 bytes)] ++ sha256(bytes)`.
/// Lets publish/status derive a blob's address offline, without uploading.
pub fn blob_cid(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut cid = vec![0x01, 0x55, 0x12, 0x20];
    cid.extend_from_slice(&digest);
    format!(
        "b{}",
        data_encoding::BASE32_NOPAD.encode(&cid).to_lowercase()
    )
}

/// A [`BlobRef`] derived locally from file bytes — same `ref.$link` and `size`
/// the PDS would return from `uploadBlob`. The mimeType is a placeholder
/// (`*/*`, what atrium sends as Content-Type); record comparison ignores it
/// (see [`crate::atproto::compare`]).
pub fn derived_blob_ref(bytes: &[u8]) -> Result<BlobRef, anyhow::Error> {
    let cid = blob_cid(bytes);
    let link = CidLink::try_from(cid.as_str())
        .map_err(|e| anyhow::anyhow!("constructing CID link {cid}: {e}"))?;
    Ok(BlobRef::Typed(TypedBlobRef::Blob(Blob {
        r#ref: link,
        mime_type: "*/*".into(),
        size: bytes.len(),
    })))
}

/// Record key for a doc's `site.standard.document`, derived from its absolute
/// canonical URL (origin + path) so two sites published to one PDS never collide.
/// Pass the output of [`canonical_url`]. Deterministic — reconstructible from
/// config + the doc's output path, so no local bookkeeping is needed.
pub fn document_rkey(canonical_url: &str) -> String {
    rkey_hash(canonical_url)
}

/// Record key for the `site.standard.publication`, derived from the site origin
/// so each site gets its own publication record on a shared PDS.
pub fn publication_rkey(site_url: &str) -> String {
    rkey_hash(site_url)
}

/// AT-URI of a doc's `site.standard.document` record. Fully derivable from the
/// account DID (`ITALIC_ATPROTO_DID`), `site.url`, and the doc's output path.
/// The build-time verification `<link>` and `italic
/// atproto` both construct record addresses through here, so they can never
/// drift.
pub fn document_uri(did: &str, canonical_url: &str) -> String {
    format!(
        "at://{did}/{DOCUMENT_NSID}/{}",
        document_rkey(canonical_url)
    )
}

/// AT-URI of the site's `site.standard.publication` record, derived the same
/// way as [`document_uri`]. Emitted into `.well-known` at build time.
pub fn publication_uri(did: &str, site_url: &str) -> String {
    format!(
        "at://{did}/{PUBLICATION_NSID}/{}",
        publication_rkey(site_url)
    )
}

/// Site-root-relative path for a doc, e.g. `/blog/post/` or `/posts/p.html`. This
/// is the document record's `path` field; combined with the publication URL it
/// yields the canonical URL.
pub fn canonical_path(doc: &Doc, base_path: &str) -> String {
    format!("{}{}", base_path, permalink::to_url(&doc.output_path))
}

/// Full canonical URL for a doc, e.g. `https://example.com/blog/post/`.
/// `site_url` should have no trailing slash (as normalized by
/// [`crate::config`]); falls back to the root-relative path when absent.
pub fn canonical_url(doc: &Doc, site_url: Option<&str>, base_path: &str) -> String {
    let path = canonical_path(doc, base_path);
    match site_url {
        Some(origin) => format!("{origin}{path}"),
        None => path,
    }
}

fn rfc3339(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Map a [`Doc`] to a `site.standard.document` record. `site_uri` is the
/// publication AT-URI; `cover` is the pre-uploaded cover blob (if any);
/// `bsky_post_ref` is the doc's announcement post from the bsky state file
/// (if one has been created).
pub fn document(
    doc: &Doc,
    site_uri: &str,
    base_path: &str,
    cover: Option<BlobRef>,
    bsky_post_ref: Option<StrongRef>,
) -> Document {
    let text = html::strip_tags(&doc.content);
    let text_content = if text.trim().is_empty() {
        None
    } else {
        Some(text)
    };
    let description = if doc.summary.is_empty() {
        None
    } else {
        Some(doc.summary.clone())
    };
    let updated_at = if doc.updated > doc.date {
        Some(rfc3339(&doc.updated))
    } else {
        None
    };
    let tags = doc
        .terms
        .get("tags")
        .map(|bucket| bucket.keys().cloned().collect())
        .unwrap_or_default();
    // Full body as Markdown (Markdown docs only; `None` for Raw/Yaml).
    let content = doc.markdown.as_ref().map(|md| MarkdownContent {
        type_: MARKDOWN_NSID,
        flavor: "gfm",
        rendering_rules: "comrak",
        text: MarkdownText {
            type_: MARKDOWN_TEXT_NSID,
            markdown: md.clone(),
        },
    });

    Document {
        type_: DOCUMENT_NSID,
        site: site_uri.to_string(),
        title: doc.title.clone(),
        published_at: rfc3339(&doc.date),
        updated_at,
        path: Some(canonical_path(doc, base_path)),
        description,
        cover_image: cover,
        content,
        text_content,
        tags,
        bsky_post_ref,
    }
}

/// Build the `site.standard.publication` record from plain values. The values
/// are derived from `site:` config by `crate::atproto::publication_record`,
/// which owns the requiredness checks (`site.title`, `site.url`).
pub fn publication(
    name: String,
    url: String,
    description: Option<String>,
    icon: Option<BlobRef>,
    basic_theme: Option<BasicTheme>,
) -> Publication {
    Publication {
        type_: PUBLICATION_NSID,
        url,
        name,
        description,
        icon,
        basic_theme,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn blob_cid_matches_known_vector() {
        // The well-known CIDv1 raw sha-256 CID for "hello world" — cross-checks
        // the hand-rolled encoding against the IPFS/ATProto ecosystem.
        assert_eq!(
            blob_cid(b"hello world"),
            "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e"
        );
    }

    #[test]
    fn derived_blob_ref_serializes_to_blob_shape() {
        let blob = derived_blob_ref(b"hello world").unwrap();
        let v = serde_json::to_value(&blob).unwrap();
        assert_eq!(v["$type"], "blob");
        assert_eq!(
            v["ref"]["$link"],
            "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e"
        );
        assert_eq!(v["size"], 11);
    }

    fn at(date: &str) -> DateTime<Utc> {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap()
            .and_utc()
    }

    fn doc() -> Doc {
        let mut terms: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let tags = terms.entry("tags".into()).or_default();
        tags.insert("tutorial".into(), "Tutorial".into());
        tags.insert("atproto".into(), "atproto".into());
        Doc {
            id_path: PathBuf::from("blog/getting-started.md"),
            output_path: PathBuf::from("blog/getting-started/index.html"),
            title: "Getting Started".into(),
            summary: "Learn how to publish.".into(),
            content: "<p>Full <em>body</em> text.</p>".into(),
            markdown: Some("Full *body* text.".into()),
            terms,
            date: at("2024-01-20"),
            updated: at("2024-01-20"),
            ..Doc::default()
        }
    }

    #[test]
    fn document_rkey_hashes_canonical_url() {
        // Deterministic: pinned to the full base32 SHA-256 of the canonical URL.
        assert_eq!(
            document_rkey("https://example.com/blog/getting-started/"),
            "c5oqyxkz4pfia2zmhhye62t42vdzpiwjdmtphglnfxwpg2y5v4ba"
        );
        // 52 chars, all rkey-safe base32 (lowercase a–z, 2–7).
        let rkey = document_rkey("https://example.com/blog/getting-started/");
        assert_eq!(rkey.len(), 52);
        assert!(rkey.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7')));
        // Origin-sensitive: same path on different origins → different rkeys.
        assert_ne!(
            document_rkey("https://a.com/p/"),
            document_rkey("https://b.com/p/")
        );
    }

    #[test]
    fn uris_compose_did_nsid_and_derived_rkey() {
        assert_eq!(
            document_uri("did:plc:abc", "https://example.com/blog/getting-started/"),
            "at://did:plc:abc/site.standard.document/\
             c5oqyxkz4pfia2zmhhye62t42vdzpiwjdmtphglnfxwpg2y5v4ba"
        );
        assert_eq!(
            publication_uri("did:plc:abc", "https://example.com"),
            format!(
                "at://did:plc:abc/site.standard.publication/{}",
                publication_rkey("https://example.com")
            )
        );
    }

    #[test]
    fn publication_rkey_hashes_origin() {
        assert_eq!(
            publication_rkey("https://example.com"),
            "cadiblkunttkk57uf5jn6m5uz7okovuftztexdl54mu3cugqttuq"
        );
        assert_ne!(
            publication_rkey("https://a.com"),
            publication_rkey("https://b.com")
        );
    }

    #[test]
    fn canonical_path_joins_base_path_and_url() {
        let d = doc();
        assert_eq!(canonical_path(&d, ""), "/blog/getting-started/");
        assert_eq!(
            canonical_path(&d, "/garden"),
            "/garden/blog/getting-started/"
        );
    }

    #[test]
    fn canonical_url_prefixes_origin() {
        let d = doc();
        assert_eq!(
            canonical_url(&d, Some("https://example.com"), ""),
            "https://example.com/blog/getting-started/"
        );
        assert_eq!(canonical_url(&d, None, ""), "/blog/getting-started/");
    }

    #[test]
    fn document_serializes_to_lexicon_shape() {
        let d = doc();
        let rec = document(
            &d,
            "at://did:plc:abc/site.standard.publication/self",
            "",
            None,
            None,
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["$type"], "site.standard.document");
        assert_eq!(v["site"], "at://did:plc:abc/site.standard.publication/self");
        assert_eq!(v["title"], "Getting Started");
        assert_eq!(v["publishedAt"], "2024-01-20T14:30:00.000Z");
        assert_eq!(v["path"], "/blog/getting-started/");
        assert_eq!(v["description"], "Learn how to publish.");
        // HTML stripped to plaintext.
        assert_eq!(v["textContent"], "Full body text.");
        // Full body as a markpub Markdown content-union entry.
        assert_eq!(v["content"]["$type"], "at.markpub.markdown");
        assert_eq!(v["content"]["flavor"], "gfm");
        assert_eq!(v["content"]["renderingRules"], "comrak");
        assert_eq!(v["content"]["text"]["$type"], "at.markpub.text");
        assert_eq!(v["content"]["text"]["markdown"], "Full *body* text.");
        // tags come from the `tags` taxonomy bucket keys (sorted).
        assert_eq!(v["tags"], json!(["atproto", "tutorial"]));
        // No updatedAt (updated == date), no coverImage.
        assert!(v.get("updatedAt").is_none());
        assert!(v.get("coverImage").is_none());
    }

    #[test]
    fn document_bsky_post_ref_serializes_as_strong_ref() {
        let d = doc();
        let rec = document(
            &d,
            "at://did:plc:abc/site.standard.publication/self",
            "",
            None,
            Some(StrongRef {
                uri: "at://did:plc:abc/app.bsky.feed.post/3lwa".into(),
                cid: "bafyreib2".into(),
            }),
        );
        let v = serde_json::to_value(&rec).unwrap();
        // A plain {uri, cid} object — strongRef has no $type.
        assert_eq!(
            v["bskyPostRef"],
            json!({
                "uri": "at://did:plc:abc/app.bsky.feed.post/3lwa",
                "cid": "bafyreib2",
            })
        );
    }

    #[test]
    fn document_omits_content_when_no_markdown() {
        // Raw/Yaml docs carry no Markdown source, so the content union is omitted.
        let mut d = doc();
        d.markdown = None;
        let rec = document(
            &d,
            "at://did:plc:abc/site.standard.publication/self",
            "",
            None,
            None,
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert!(v.get("content").is_none());
    }

    #[test]
    fn document_includes_updated_when_later_than_published() {
        let mut d = doc();
        d.updated = at("2024-02-01");
        let rec = document(
            &d,
            "at://did:plc:abc/site.standard.publication/self",
            "",
            None,
            None,
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["updatedAt"], "2024-02-01T14:30:00.000Z");
    }

    #[test]
    fn publication_serializes_to_lexicon_shape() {
        let rec = publication(
            "My Garden".into(),
            "https://example.com".into(),
            None,
            None,
            None,
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["$type"], "site.standard.publication");
        assert_eq!(v["name"], "My Garden");
        assert_eq!(v["url"], "https://example.com");
        assert!(v.get("description").is_none());
        assert!(v.get("icon").is_none());
        assert!(v.get("basicTheme").is_none());
    }

    #[test]
    fn publication_theme_serializes_to_lexicon_shape() {
        use crate::atproto::config::{BasicTheme as ThemeConfig, Rgb};
        let theme = ThemeConfig {
            background: Rgb {
                r: 0x1a,
                g: 0x1a,
                b: 0x2e,
            },
            foreground: Rgb {
                r: 0xee,
                g: 0xee,
                b: 0xee,
            },
            accent: Rgb {
                r: 0xe9,
                g: 0x45,
                b: 0x60,
            },
            accent_foreground: Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
        };
        let rec = publication(
            "My Garden".into(),
            "https://example.com".into(),
            Some("A digital garden.".into()),
            None,
            Some(basic_theme(&theme)),
        );
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(v["description"], "A digital garden.");
        let t = &v["basicTheme"];
        assert_eq!(t["$type"], "site.standard.theme.basic");
        // camelCase field name per the lexicon.
        assert_eq!(t["accentForeground"]["r"], 255);
        assert_eq!(t["background"]["$type"], "site.standard.theme.color#rgb");
        assert_eq!(t["background"]["r"], 0x1a);
        assert_eq!(t["background"]["g"], 0x1a);
        assert_eq!(t["background"]["b"], 0x2e);
        assert_eq!(t["accent"]["r"], 0xe9);
    }
}
