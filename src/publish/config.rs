//! Non-secret `publish:` configuration, parsed from `config.yaml`. Mirrors the
//! manual two-tier parse used for `related`/`collections` (see [`crate::config`]):
//! the structured block is removed from the raw YAML and parsed here with
//! unknown-key rejection so typos fail loudly. Secrets (handle/app password)
//! never live here — they come from the environment (see [`crate::publish::atproto`]).

use crate::doc::parse_date;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde_yaml_ng::{Mapping, Value};
use std::path::PathBuf;

/// Default PDS host when `publish.pds_host` is unset — the Bluesky-operated PDS
/// that most app-password accounts live on.
pub const DEFAULT_PDS_HOST: &str = "https://bsky.social";

/// The `publish:` block. Selects what to sync to the PDS and how, but holds no
/// secrets. Present only when the site declares `publish:` in `config.yaml`
/// (stored as `Option<Publish>` on [`crate::config::Config`]).
#[derive(Debug, Clone)]
pub struct Publish {
    /// PDS XRPC host, e.g. `https://bsky.social`. Defaults to [`DEFAULT_PDS_HOST`].
    pub pds_host: String,
    /// Account handle (e.g. `alice.example.com`). May be overridden by the
    /// `ITALIC_ATPROTO_HANDLE` env var at auth time; either source works.
    pub handle: Option<String>,
    /// Which collection's docs get a `site.standard.document` record. Defaults to
    /// the always-present `all` collection ([`crate::config::ALL`]).
    pub collection: String,
    /// Emit the static verification artifacts during `build` (the
    /// `.well-known/site.standard.publication` file and the per-doc AT-URI
    /// binding). On by default; harmless before the first publish (nothing is
    /// emitted until the state file records a publication URI).
    pub verification: bool,
    /// `site.standard.publication` metadata (the site/blog record).
    pub publication: Publication,
    /// `app.bsky.feed.post` summary settings (feature 2).
    pub bluesky: Bluesky,
}

/// Metadata for the one `site.standard.publication` record. `name`/`url` are
/// required to publish the publication record; they are optional here so an
/// otherwise-valid config parses, with the requirement enforced at publish time.
#[derive(Debug, Clone, Default)]
pub struct Publication {
    pub name: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    /// Path (relative to the working dir) to an icon image uploaded as a blob.
    pub icon: Option<PathBuf>,
}

/// `app.bsky.feed.post` summary settings. Off unless `bluesky.enabled: true`.
#[derive(Debug, Clone)]
pub struct Bluesky {
    pub enabled: bool,
    /// Collection whose docs get announced. Falls back to [`Publish::collection`]
    /// when unset (resolved at publish time).
    pub collection: Option<String>,
    /// Tera template for the post text, rendered per doc with `title`/`summary`
    /// in scope. Defaults to `doc.summary` when unset.
    pub post_template: Option<String>,
    /// Attach an `app.bsky.embed.external` link card pointing at the canonical URL.
    pub include_link_card: bool,
    /// Where the link-card thumbnail comes from.
    pub thumb: Thumb,
    /// Only announce docs dated on/after this instant. Guards a first publish of a
    /// large backlog from flooding the firehose.
    pub announce_after: Option<DateTime<Utc>>,
}

/// Source for the bsky link-card thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Thumb {
    /// Reuse the doc's `cover` frontmatter image (the same blob as the document
    /// record's `coverImage`).
    #[default]
    Cover,
    /// No thumbnail.
    None,
}

impl Default for Bluesky {
    fn default() -> Self {
        Bluesky {
            enabled: false,
            collection: None,
            post_template: None,
            include_link_card: true,
            thumb: Thumb::default(),
            announce_after: None,
        }
    }
}

/// Parse the `publish:` mapping. Rejects unknown top-level keys (and unknown keys
/// in the `publication`/`bluesky` sub-maps) so typos surface immediately, the way
/// [`crate::config`]'s `parse_related` does. `default_collection` is the
/// always-present `all` name, used when `collection` is omitted.
pub fn parse_publish(map: &Mapping, default_collection: &str) -> Result<Publish> {
    reject_unknown(
        map,
        &[
            "pds_host",
            "handle",
            "collection",
            "verification",
            "publication",
            "bluesky",
        ],
        "publish",
    )?;

    let pds_host = string(map, "pds_host")?.unwrap_or_else(|| DEFAULT_PDS_HOST.to_string());
    let handle = string(map, "handle")?;
    let collection = string(map, "collection")?.unwrap_or_else(|| default_collection.to_string());
    let verification = bool_or(map, "verification", true)?;

    let publication = match submap(map, "publication")? {
        Some(m) => parse_publication(m)?,
        None => Publication::default(),
    };
    let bluesky = match submap(map, "bluesky")? {
        Some(m) => parse_bluesky(m)?,
        None => Bluesky::default(),
    };

    Ok(Publish {
        pds_host,
        handle,
        collection,
        verification,
        publication,
        bluesky,
    })
}

fn parse_publication(map: &Mapping) -> Result<Publication> {
    reject_unknown(
        map,
        &["name", "url", "description", "icon"],
        "publish.publication",
    )?;
    Ok(Publication {
        name: string(map, "name")?,
        url: string(map, "url")?,
        description: string(map, "description")?,
        icon: string(map, "icon")?.map(PathBuf::from),
    })
}

fn parse_bluesky(map: &Mapping) -> Result<Bluesky> {
    reject_unknown(
        map,
        &[
            "enabled",
            "collection",
            "post_template",
            "include_link_card",
            "thumb",
            "announce_after",
        ],
        "publish.bluesky",
    )?;
    let thumb = match string(map, "thumb")?.as_deref() {
        None | Some("cover") => Thumb::Cover,
        Some("none") => Thumb::None,
        Some(other) => {
            return Err(anyhow!(
                "publish.bluesky.thumb: unknown value `{other}` (allowed: cover, none)"
            ));
        }
    };
    let announce_after = match map.get(Value::String("announce_after".into())) {
        Some(v) => Some(parse_date(Some(v)).ok_or_else(|| {
            anyhow!("publish.bluesky.announce_after: must be a date or RFC3339 datetime")
        })?),
        None => None,
    };
    Ok(Bluesky {
        enabled: bool_or(map, "enabled", false)?,
        collection: string(map, "collection")?,
        post_template: string(map, "post_template")?,
        include_link_card: bool_or(map, "include_link_card", true)?,
        thumb,
        announce_after,
    })
}

/// Error if `map` has any key outside `allowed`. `ctx` names the block for the
/// message (e.g. `publish.bluesky`).
fn reject_unknown(map: &Mapping, allowed: &[&str], ctx: &str) -> Result<()> {
    for key in map.keys() {
        let name = key.as_str().unwrap_or("<non-string>");
        if !allowed.contains(&name) {
            return Err(anyhow!(
                "{ctx}: unknown key `{name}` (allowed: {})",
                allowed.join(", ")
            ));
        }
    }
    Ok(())
}

/// Read an optional string field, erroring if present but not a string.
fn string(map: &Mapping, key: &str) -> Result<Option<String>> {
    match map.get(Value::String(key.into())) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(anyhow!("publish: `{key}` must be a string")),
    }
}

/// Read an optional bool field, falling back to `default`; error if non-bool.
fn bool_or(map: &Mapping, key: &str, default: bool) -> Result<bool> {
    match map.get(Value::String(key.into())) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(anyhow!("publish: `{key}` must be a boolean")),
    }
}

/// Read an optional sub-mapping field, erroring if present but not a mapping.
fn submap<'a>(map: &'a Mapping, key: &str) -> Result<Option<&'a Mapping>> {
    match map.get(Value::String(key.into())) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Mapping(m)) => Ok(Some(m)),
        Some(_) => Err(anyhow!("publish: `{key}` must be a mapping")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Publish> {
        let map: Mapping = serde_yaml_ng::from_str(yaml).unwrap();
        parse_publish(&map, "all")
    }

    #[test]
    fn defaults_apply_for_minimal_block() {
        let p = parse("handle: alice.example.com\n").unwrap();
        assert_eq!(p.pds_host, DEFAULT_PDS_HOST);
        assert_eq!(p.handle.as_deref(), Some("alice.example.com"));
        assert_eq!(p.collection, "all");
        assert!(p.verification);
        assert!(!p.bluesky.enabled);
        assert!(p.bluesky.include_link_card);
        assert_eq!(p.bluesky.thumb, Thumb::Cover);
    }

    #[test]
    fn full_block_parses() {
        let p = parse(
            "pds_host: https://pds.example\n\
             handle: alice.example.com\n\
             collection: posts\n\
             verification: false\n\
             publication:\n  name: My Garden\n  url: https://example.com\n  icon: static/icon.png\n\
             bluesky:\n  enabled: true\n  collection: notes\n  post_template: \"{{ title }}\"\n  thumb: none\n  announce_after: 2026-01-01\n",
        )
        .unwrap();
        assert_eq!(p.pds_host, "https://pds.example");
        assert_eq!(p.collection, "posts");
        assert!(!p.verification);
        assert_eq!(p.publication.name.as_deref(), Some("My Garden"));
        assert_eq!(p.publication.icon, Some(PathBuf::from("static/icon.png")));
        assert!(p.bluesky.enabled);
        assert_eq!(p.bluesky.collection.as_deref(), Some("notes"));
        assert_eq!(p.bluesky.post_template.as_deref(), Some("{{ title }}"));
        assert_eq!(p.bluesky.thumb, Thumb::None);
        assert!(p.bluesky.announce_after.is_some());
    }

    #[test]
    fn unknown_top_level_key_errors() {
        let err = format!("{:#}", parse("pds_hsot: x\n").unwrap_err());
        assert!(err.contains("unknown key"), "{err}");
    }

    #[test]
    fn unknown_bluesky_key_errors() {
        let err = format!("{:#}", parse("bluesky:\n  enabeld: true\n").unwrap_err());
        assert!(err.contains("unknown key"), "{err}");
    }

    #[test]
    fn bad_thumb_value_errors() {
        let err = format!("{:#}", parse("bluesky:\n  thumb: gravatar\n").unwrap_err());
        assert!(err.contains("thumb"), "{err}");
    }

    #[test]
    fn bad_announce_after_errors() {
        let err = format!(
            "{:#}",
            parse("bluesky:\n  announce_after: someday\n").unwrap_err()
        );
        assert!(err.contains("announce_after"), "{err}");
    }
}
