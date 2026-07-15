//! Tera adapter for the `related` filter. Takes the site's configured
//! [`Related`] weights, adds the call's `limit`/`omit` kwargs, forwards into
//! [`DocIndex::related`], and serializes the ranked docs.

use crate::doc_index::DocIndex;
use crate::related::Related;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tera::{Kwargs, State, Tera, TeraResult, Value};

/// Register `related` as a Tera filter on `env`. Usage:
/// `{% for doc in page.id_path | related(limit=5) %}`. The piped value is the
/// query doc's `id_path`; returns the docs most related to it, ranked best
/// first, using the configured per-namespace `weights`. `limit` and `omit` are
/// per-call kwargs (not config). Template-env only — spec §11 forbids
/// index-listing filters in the markup env. Reads the shared [`DocIndex`].
pub fn register(env: &mut Tera, index: Arc<DocIndex>, config: Related) {
    env.register_filter(
        "related",
        move |id_path: &str, kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            let target = Path::new(id_path);
            let opts = from_kwargs(&config, &kwargs)?;
            let results = index.related(target, &opts);
            Value::try_from_serializable(&results)
        },
    );
}

/// Build the per-call [`Related`] from the configured base plus kwargs. Weights
/// come from config; `limit` and `omit` come from the call.
fn from_kwargs(config: &Related, kwargs: &Kwargs) -> TeraResult<Related> {
    let mut opts = config.clone();

    if let Some(n) = kwargs.get::<usize>("limit")? {
        opts.limit = Some(n);
    }

    if let Some(omit) = kwargs.get::<Vec<String>>("omit")? {
        opts.omit = omit.into_iter().map(PathBuf::from).collect();
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

    fn kwargs<const N: usize>(pairs: [(&'static str, Value); N]) -> Kwargs {
        Kwargs::from(pairs)
    }

    #[test]
    fn from_kwargs_empty_keeps_config_weights_and_no_limit() {
        let opts = from_kwargs(&base(), &kwargs([])).unwrap();
        assert_eq!(opts.weights.len(), 2);
        // No config limit; without a kwarg the result is unlimited.
        assert!(opts.limit.is_none());
        assert!(opts.omit.is_empty());
    }

    #[test]
    fn from_kwargs_limit_comes_from_kwarg() {
        let opts = from_kwargs(&base(), &kwargs([("limit", Value::from(2u64))])).unwrap();
        assert_eq!(opts.limit, Some(2));
    }

    #[test]
    fn from_kwargs_parses_omit() {
        let opts = from_kwargs(
            &base(),
            &kwargs([(
                "omit",
                Value::from(vec![Value::from("a.md"), Value::from("b.md")]),
            )]),
        )
        .unwrap();
        assert_eq!(
            opts.omit,
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")]
        );
    }

    #[test]
    fn from_kwargs_limit_not_integer_errors() {
        assert!(from_kwargs(&base(), &kwargs([("limit", Value::from("3"))])).is_err());
    }

    #[test]
    fn from_kwargs_omit_not_array_errors() {
        assert!(from_kwargs(&base(), &kwargs([("omit", Value::from("a.md"))])).is_err());
    }
}
