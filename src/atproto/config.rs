//! Non-secret `atproto:` configuration, parsed from `config.yaml`. Mirrors the
//! manual two-tier parse used for `related`/`collections` (see [`crate::config`]):
//! the structured block is removed from the raw YAML and parsed here with
//! unknown-key rejection so typos fail loudly. The block is entirely optional —
//! [`Atproto::default`] applies when a site declares no `atproto:` — so env
//! credentials plus `site:` metadata are enough to publish. The account identity
//! (DID) and the app password never live here — they come from the environment
//! (see [`crate::atproto::client`]).

use anyhow::{Result, anyhow};
use serde_yaml_ng::{Mapping, Value};
use std::path::PathBuf;

/// Default PDS host when `atproto.pds_host` is unset — the Bluesky-operated PDS
/// that most app-password accounts live on.
pub const DEFAULT_PDS_HOST: &str = "https://bsky.social";

/// The `atproto:` block. Selects what to sync to the PDS and how, but holds no
/// secrets. Always present on [`crate::config::Config`], defaulting when the
/// site declares no `atproto:` block. The publication record's `name`, `url`,
/// and `description` are not configured here — they derive from `site.title`,
/// `site.url` + `site.base_path`, and `site.description`.
#[derive(Debug, Clone)]
pub struct Atproto {
    /// PDS XRPC host, e.g. `https://bsky.social`. Defaults to [`DEFAULT_PDS_HOST`].
    pub pds_host: String,
    /// Which collections' docs get a `site.standard.document` record — the
    /// deduplicated union of their members. Defaults to the always-present `all`
    /// collection ([`crate::config::ALL`]).
    pub collections: Vec<String>,
    /// Emit the static verification artifacts during `build` (the
    /// `.well-known/site.standard.publication` file and the per-doc AT-URI
    /// binding). On by default; harmless before the first publish (the
    /// derived AT-URIs simply point at records that don't exist yet).
    pub verification: bool,
    /// `site.standard.publication` presentation (icon, theme).
    pub publication: Publication,
    /// Bluesky announcement posts (`app.bsky.feed.post`); see [`Bsky`].
    pub bsky: Bsky,
}

impl Default for Atproto {
    fn default() -> Self {
        Self {
            pds_host: DEFAULT_PDS_HOST.to_string(),
            collections: vec![crate::config::ALL.to_string()],
            verification: true,
            publication: Publication::default(),
            bsky: Bsky::default(),
        }
    }
}

/// The `bsky:` sub-block: publishing `app.bsky.feed.post` announcements for
/// docs that opt in via `bsky:` frontmatter. Off by default — creating posts
/// is outward-facing, so it takes an explicit `enabled: true`.
#[derive(Debug, Clone, Default)]
pub struct Bsky {
    /// Master switch for creating posts. Default false.
    pub enabled: bool,
    /// Docs dated before this never get posts — the guard against mass-posting
    /// an old archive on first publish. `None` means "3 days before now",
    /// resolved at publish time (the config stays a pure representation of the
    /// file).
    pub since: Option<chrono::NaiveDate>,
}

/// Presentation settings for the one `site.standard.publication` record. Its
/// textual fields (`name`/`url`/`description`) come from `site:`, so only what
/// `site:` doesn't already say lives here.
#[derive(Debug, Clone, Default)]
pub struct Publication {
    /// Path (relative to the working dir) to an icon image uploaded as a blob.
    pub icon: Option<PathBuf>,
    /// Colors for the record's embedded `site.standard.theme.basic` object.
    pub theme: Option<BasicTheme>,
}

/// The four colors of a `site.standard.theme.basic` theme, parsed from
/// `#rrggbb` hex strings at config-load time so bad values fail before any
/// network work. All four are required by the lexicon, so a partial block is a
/// parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicTheme {
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    pub accent_foreground: Rgb,
}

/// An RGB color (`site.standard.theme.color#rgb` on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Parse the `atproto:` mapping. Rejects unknown keys so typos surface
/// immediately, the way [`crate::config`]'s `parse_related` does, with pointed
/// migration errors for the removed `collection`/`publication.name`-era keys.
pub fn parse_atproto(map: &Mapping) -> Result<Atproto> {
    for key in map.keys() {
        match key.as_str() {
            Some("pds_host" | "collections" | "verification" | "publication" | "bsky") => {}
            Some("collection") => {
                return Err(anyhow!(
                    "atproto: `collection` has been replaced by `collections`, a list — \
                     e.g. `collections: [posts]` (defaults to `[all]`)"
                ));
            }
            other => {
                return Err(anyhow!(
                    "atproto: unknown key `{}` (allowed: pds_host, collections, \
                     verification, publication, bsky)",
                    other.unwrap_or("<non-string>")
                ));
            }
        }
    }

    let pds_host = string(map, "pds_host")?.unwrap_or_else(|| DEFAULT_PDS_HOST.to_string());
    let collections = parse_collections(map)?;
    let verification = bool_or(map, "verification", true)?;

    let publication = match submap(map, "publication")? {
        Some(m) => parse_publication(m)?,
        None => Publication::default(),
    };

    let bsky = match submap(map, "bsky")? {
        Some(m) => parse_bsky(m)?,
        None => Bsky::default(),
    };

    Ok(Atproto {
        pds_host,
        collections,
        verification,
        publication,
        bsky,
    })
}

/// Parse the `bsky:` sub-map into a [`Bsky`].
fn parse_bsky(map: &Mapping) -> Result<Bsky> {
    reject_unknown(map, &["enabled", "since"], "atproto.bsky")?;
    let enabled = match map.get(Value::String("enabled".into())) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(anyhow!("atproto.bsky: `enabled` must be a boolean")),
    };
    let since = match string(map, "since")? {
        None => None,
        Some(value) => Some(
            chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|_| {
                anyhow!("atproto.bsky: `since` must be a date like 2026-01-01 (got `{value}`)")
            })?,
        ),
    };
    Ok(Bsky { enabled, since })
}

/// Parse `collections:` — a list of collection names, mirroring the top-level
/// `feed:` key. Absent/null defaults to `[all]`; an explicit `[]` means "publish
/// only the publication record".
fn parse_collections(map: &Mapping) -> Result<Vec<String>> {
    match map.get(Value::String("collections".into())) {
        None | Some(Value::Null) => Ok(vec![crate::config::ALL.to_string()]),
        Some(Value::Sequence(seq)) => seq
            .iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    anyhow!("atproto: every `collections` entry must be a collection name (string)")
                })
            })
            .collect(),
        Some(_) => Err(anyhow!(
            "atproto: `collections` must be a list of collection names"
        )),
    }
}

fn parse_publication(map: &Mapping) -> Result<Publication> {
    for key in map.keys() {
        match key.as_str() {
            Some("icon" | "theme") => {}
            Some(removed @ ("name" | "url" | "description")) => {
                return Err(anyhow!(
                    "atproto.publication: `{removed}` was removed — the publication record \
                     now uses `site.title` (name), `site.url` + `site.base_path` (url), \
                     and `site.description` (description)"
                ));
            }
            other => {
                return Err(anyhow!(
                    "atproto.publication: unknown key `{}` (allowed: icon, theme)",
                    other.unwrap_or("<non-string>")
                ));
            }
        }
    }
    let theme = match submap(map, "theme")? {
        Some(m) => Some(parse_theme(m)?),
        None => None,
    };
    Ok(Publication {
        icon: string(map, "icon")?.map(PathBuf::from),
        theme,
    })
}

/// Parse the `theme:` sub-map into a [`BasicTheme`]. All four colors are
/// required (the lexicon requires them), each a `#rrggbb` hex string.
fn parse_theme(map: &Mapping) -> Result<BasicTheme> {
    reject_unknown(
        map,
        &["background", "foreground", "accent", "accent_foreground"],
        "atproto.publication.theme",
    )?;
    let color = |key: &str| -> Result<Rgb> {
        let value = string(map, key)?.ok_or_else(|| {
            anyhow!(
                "atproto.publication.theme: `{key}` is required (all four colors: \
                 background, foreground, accent, accent_foreground)"
            )
        })?;
        parse_hex_color(&value, key)
    };
    Ok(BasicTheme {
        background: color("background")?,
        foreground: color("foreground")?,
        accent: color("accent")?,
        accent_foreground: color("accent_foreground")?,
    })
}

/// Parse a `#rrggbb` hex color (case-insensitive). Exactly one canonical form —
/// no 3-digit shorthand — so the error message can show the expected shape.
fn parse_hex_color(value: &str, key: &str) -> Result<Rgb> {
    let bad = || {
        anyhow!(
            "atproto.publication.theme: `{key}` must be a 6-digit hex color \
             like \"#1a1a2e\" (got `{value}`)"
        )
    };
    let hex = value.strip_prefix('#').ok_or_else(bad)?;
    // Explicit digit check: `from_str_radix` alone would admit a leading `+`.
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let channel = |range| u8::from_str_radix(&hex[range], 16).map_err(|_| bad());
    Ok(Rgb {
        r: channel(0..2)?,
        g: channel(2..4)?,
        b: channel(4..6)?,
    })
}

/// Error if `map` has any key outside `allowed`. `ctx` names the block for the
/// message (e.g. `atproto.publication.theme`).
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
        Some(_) => Err(anyhow!("atproto: `{key}` must be a string")),
    }
}

/// Read an optional bool field, falling back to `default`; error if non-bool.
fn bool_or(map: &Mapping, key: &str, default: bool) -> Result<bool> {
    match map.get(Value::String(key.into())) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(anyhow!("atproto: `{key}` must be a boolean")),
    }
}

/// Read an optional sub-mapping field, erroring if present but not a mapping.
fn submap<'a>(map: &'a Mapping, key: &str) -> Result<Option<&'a Mapping>> {
    match map.get(Value::String(key.into())) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Mapping(m)) => Ok(Some(m)),
        Some(_) => Err(anyhow!("atproto: `{key}` must be a mapping")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Atproto> {
        let map: Mapping = serde_yaml_ng::from_str(yaml).unwrap();
        parse_atproto(&map)
    }

    #[test]
    fn defaults_apply_for_minimal_block() {
        let p = parse("verification: true\n").unwrap();
        assert_eq!(p.pds_host, DEFAULT_PDS_HOST);
        assert_eq!(p.collections, vec!["all"]);
        assert!(p.verification);
        assert!(p.publication.icon.is_none());
        assert!(p.publication.theme.is_none());
    }

    #[test]
    fn empty_block_matches_struct_default() {
        let p = parse("{}\n").unwrap();
        let d = Atproto::default();
        assert_eq!(p.pds_host, d.pds_host);
        assert_eq!(p.collections, d.collections);
        assert_eq!(p.verification, d.verification);
        assert_eq!(p.publication.icon, d.publication.icon);
        assert_eq!(p.publication.theme, d.publication.theme);
    }

    #[test]
    fn full_block_parses() {
        let p = parse(
            "pds_host: https://pds.example\n\
             collections: [posts, notes]\n\
             verification: false\n\
             publication:\n\
             \x20 icon: static/icon.png\n\
             \x20 theme:\n\
             \x20   background: \"#1a1a2e\"\n\
             \x20   foreground: \"#EEeeEE\"\n\
             \x20   accent: \"#e94560\"\n\
             \x20   accent_foreground: \"#ffffff\"\n",
        )
        .unwrap();
        assert_eq!(p.pds_host, "https://pds.example");
        assert_eq!(p.collections, vec!["posts", "notes"]);
        assert!(!p.verification);
        assert_eq!(p.publication.icon, Some(PathBuf::from("static/icon.png")));
        let theme = p.publication.theme.unwrap();
        assert_eq!(
            theme.background,
            Rgb {
                r: 0x1a,
                g: 0x1a,
                b: 0x2e
            }
        );
        // Case-insensitive hex.
        assert_eq!(
            theme.foreground,
            Rgb {
                r: 0xee,
                g: 0xee,
                b: 0xee
            }
        );
        assert_eq!(
            theme.accent_foreground,
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
    }

    #[test]
    fn collections_empty_list_is_allowed() {
        // `collections: []` publishes only the publication record, like `feed: []`.
        let p = parse("collections: []\n").unwrap();
        assert!(p.collections.is_empty());
    }

    #[test]
    fn collections_bare_string_errors() {
        let err = format!("{:#}", parse("collections: posts\n").unwrap_err());
        assert!(err.contains("must be a list"), "{err}");
    }

    #[test]
    fn collections_non_string_entry_errors() {
        let err = format!("{:#}", parse("collections: [5]\n").unwrap_err());
        assert!(err.contains("collection name"), "{err}");
    }

    #[test]
    fn legacy_collection_key_errors_with_hint() {
        let err = format!("{:#}", parse("collection: posts\n").unwrap_err());
        assert!(err.contains("`collections`"), "{err}");
    }

    #[test]
    fn legacy_publication_keys_error_with_hint() {
        for key in [
            "name: My Garden",
            "url: https://example.com",
            "description: A blog",
        ] {
            let err = format!(
                "{:#}",
                parse(&format!("publication:\n  {key}\n")).unwrap_err()
            );
            assert!(err.contains("site.title"), "{key}: {err}");
        }
    }

    #[test]
    fn theme_missing_color_errors() {
        let err = format!(
            "{:#}",
            parse(
                "publication:\n  theme:\n    background: \"#ffffff\"\n\
                 \x20   foreground: \"#000000\"\n    accent: \"#ff0000\"\n"
            )
            .unwrap_err()
        );
        assert!(err.contains("accent_foreground"), "{err}");
    }

    #[test]
    fn theme_bad_hex_errors() {
        for bad in ["ffffff", "#fff", "#gggggg", "#ffffff00", "#ff ff0"] {
            let yaml = format!(
                "publication:\n  theme:\n    background: \"{bad}\"\n\
                 \x20   foreground: \"#000000\"\n    accent: \"#ff0000\"\n\
                 \x20   accent_foreground: \"#ffffff\"\n"
            );
            let err = format!("{:#}", parse(&yaml).unwrap_err());
            assert!(err.contains("6-digit hex color"), "{bad}: {err}");
        }
    }

    #[test]
    fn theme_unknown_key_errors() {
        let err = format!(
            "{:#}",
            parse("publication:\n  theme:\n    bakground: \"#ffffff\"\n").unwrap_err()
        );
        assert!(err.contains("unknown key"), "{err}");
    }

    #[test]
    fn publication_unknown_key_errors() {
        let err = format!("{:#}", parse("publication:\n  ikon: x\n").unwrap_err());
        assert!(err.contains("unknown key"), "{err}");
    }

    #[test]
    fn unknown_top_level_key_errors() {
        let err = format!("{:#}", parse("pds_hsot: x\n").unwrap_err());
        assert!(err.contains("unknown key"), "{err}");
    }

    #[test]
    fn bsky_defaults_to_disabled() {
        let p = parse("{}\n").unwrap();
        assert!(!p.bsky.enabled);
        assert!(p.bsky.since.is_none());
    }

    #[test]
    fn bsky_block_parses() {
        let p = parse("bsky:\n  enabled: true\n  since: 2026-01-15\n").unwrap();
        assert!(p.bsky.enabled);
        assert_eq!(p.bsky.since, chrono::NaiveDate::from_ymd_opt(2026, 1, 15));
    }

    #[test]
    fn bsky_bad_since_errors() {
        let err = format!("{:#}", parse("bsky:\n  since: yesterday\n").unwrap_err());
        assert!(err.contains("2026-01-01"), "{err}");
    }

    #[test]
    fn bsky_non_bool_enabled_errors() {
        let err = format!("{:#}", parse("bsky:\n  enabled: yes please\n").unwrap_err());
        assert!(err.contains("boolean"), "{err}");
    }

    #[test]
    fn bsky_unknown_key_errors() {
        let err = format!("{:#}", parse("bsky:\n  enable: true\n").unwrap_err());
        assert!(err.contains("unknown key"), "{err}");
    }
}
