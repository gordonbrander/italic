//! Non-secret `publish:` configuration, parsed from `config.yaml`. Mirrors the
//! manual two-tier parse used for `related`/`collections` (see [`crate::config`]):
//! the structured block is removed from the raw YAML and parsed here with
//! unknown-key rejection so typos fail loudly. Secrets (handle/app password)
//! never live here — they come from the environment (see [`crate::publish::atproto`]).

use anyhow::{Result, anyhow};
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

/// Parse the `publish:` mapping. Rejects unknown top-level keys (and unknown keys
/// in the `publication` sub-map) so typos surface immediately, the way
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

    Ok(Publish {
        pds_host,
        handle,
        collection,
        verification,
        publication,
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

/// Error if `map` has any key outside `allowed`. `ctx` names the block for the
/// message (e.g. `publish.publication`).
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
    }

    #[test]
    fn full_block_parses() {
        let p = parse(
            "pds_host: https://pds.example\n\
             handle: alice.example.com\n\
             collection: posts\n\
             verification: false\n\
             publication:\n  name: My Garden\n  url: https://example.com\n  icon: static/icon.png\n",
        )
        .unwrap();
        assert_eq!(p.pds_host, "https://pds.example");
        assert_eq!(p.collection, "posts");
        assert!(!p.verification);
        assert_eq!(p.publication.name.as_deref(), Some("My Garden"));
        assert_eq!(p.publication.icon, Some(PathBuf::from("static/icon.png")));
    }

    #[test]
    fn unknown_top_level_key_errors() {
        let err = format!("{:#}", parse("pds_hsot: x\n").unwrap_err());
        assert!(err.contains("unknown key"), "{err}");
    }
}
