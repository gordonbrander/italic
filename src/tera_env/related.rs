//! Tera adapter for the `related` filter. Takes the site's configured
//! [`Related`] weights, adds the call's `limit`/`omit` kwargs, forwards into
//! [`DocIndex::related`], and serializes the ranked docs.

use crate::doc_index::DocIndex;
use crate::related::Related;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tera::{Tera, Value};

const KNOWN_KEYS: &[&str] = &["limit", "omit"];

/// Register `related` as a Tera filter on `env`. Usage:
/// `{% for doc in page.id_path | related(limit=5) %}`. The piped value is the
/// query doc's `id_path`; returns the docs most related to it, ranked best
/// first, using the configured per-namespace `weights`. `limit` and `omit` are
/// per-call kwargs (not config). Template-env only — spec §11 forbids
/// index-listing filters in the markup env. Reads the shared [`DocIndex`].
pub fn register(env: &mut Tera, index: Arc<DocIndex>, config: Related) {
    env.register_filter(
        "related",
        move |value: &Value, args: &HashMap<String, Value>| -> tera::Result<Value> {
            let id_path_str = value.as_str().ok_or_else(|| {
                tera::Error::msg("related filter: input must be a string id_path")
            })?;
            let target = Path::new(id_path_str);
            let opts = from_kwargs(&config, args)?;
            let results = index.related(target, &opts);
            tera::to_value(results).map_err(tera::Error::from)
        },
    );
}

/// Build the per-call [`Related`] from the configured base plus kwargs. Weights
/// come from config; `limit` and `omit` come from the call.
fn from_kwargs(config: &Related, args: &HashMap<String, Value>) -> tera::Result<Related> {
    for key in args.keys() {
        if !KNOWN_KEYS.contains(&key.as_str()) {
            return Err(tera::Error::msg(format!(
                "related: unknown argument `{}` (allowed: {})",
                key,
                KNOWN_KEYS.join(", ")
            )));
        }
    }

    let mut opts = config.clone();

    if let Some(v) = args.get("limit") {
        let n = v
            .as_u64()
            .ok_or_else(|| tera::Error::msg("related: `limit` must be a non-negative integer"))?;
        opts.limit = Some(n as usize);
    }

    if let Some(v) = args.get("omit") {
        let arr = v
            .as_array()
            .ok_or_else(|| tera::Error::msg("related: `omit` must be an array of strings"))?;
        opts.omit = arr
            .iter()
            .map(|e| {
                e.as_str()
                    .map(PathBuf::from)
                    .ok_or_else(|| tera::Error::msg("related: `omit` entries must be strings"))
            })
            .collect::<tera::Result<Vec<_>>>()?;
    }

    Ok(opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::related::LINKS;

    // The configured base: weights only. `limit`/`omit` arrive as call kwargs.
    fn base() -> Related {
        Related {
            weights: vec![("tags".to_string(), 1.0), (LINKS.to_string(), 1.0)],
            ..Default::default()
        }
    }

    fn val_str(s: &str) -> Value {
        Value::String(s.to_string())
    }

    #[test]
    fn from_kwargs_empty_keeps_config_weights_and_no_limit() {
        let opts = from_kwargs(&base(), &HashMap::new()).unwrap();
        assert_eq!(opts.weights.len(), 2);
        // No config limit; without a kwarg the result is unlimited.
        assert!(opts.limit.is_none());
        assert!(opts.omit.is_empty());
    }

    #[test]
    fn from_kwargs_limit_comes_from_kwarg() {
        let mut args = HashMap::new();
        args.insert("limit".to_string(), Value::from(2u64));
        let opts = from_kwargs(&base(), &args).unwrap();
        assert_eq!(opts.limit, Some(2));
    }

    #[test]
    fn from_kwargs_parses_omit() {
        let mut args = HashMap::new();
        args.insert(
            "omit".to_string(),
            Value::Array(vec![val_str("a.md"), val_str("b.md")]),
        );
        let opts = from_kwargs(&base(), &args).unwrap();
        assert_eq!(
            opts.omit,
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")]
        );
    }

    #[test]
    fn from_kwargs_unknown_key_errors() {
        let mut args = HashMap::new();
        args.insert("weights".to_string(), val_str("x"));
        assert!(from_kwargs(&base(), &args).is_err());
    }

    #[test]
    fn from_kwargs_limit_not_integer_errors() {
        let mut args = HashMap::new();
        args.insert("limit".to_string(), val_str("3"));
        assert!(from_kwargs(&base(), &args).is_err());
    }

    #[test]
    fn from_kwargs_omit_not_array_errors() {
        let mut args = HashMap::new();
        args.insert("omit".to_string(), val_str("a.md"));
        assert!(from_kwargs(&base(), &args).is_err());
    }
}
