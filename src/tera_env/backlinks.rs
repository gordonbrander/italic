//! Tera adapter for backlink listing. Parses kwargs into [`Backlinks`] options,
//! forwards into [`DocIndex::list_backlinks`], serializes the result.

use crate::backlinks::Backlinks;
use crate::doc_index::DocIndex;
use crate::query::{OrderKey, SortDir};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tera::{Error, Kwargs, State, Tera, TeraResult, Value};

/// Register `backlinks` as a Tera filter on `env`. Usage:
/// `{{ doc.id_path | backlinks(order_by="title", sort="asc") }}`. The piped
/// value is the target doc's `id_path`; returns a list of docs whose
/// `links` contain it. Template-env only — spec §11 forbids index-listing
/// filters in the markup env. Reads through the shared [`DocIndex`].
pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_filter(
        "backlinks",
        move |id_path: &str, kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            let target = Path::new(id_path);
            let b = from_kwargs(&kwargs)?;
            let results = index.list_backlinks(target, &b);
            Value::try_from_serializable(&results)
        },
    );
}

fn from_kwargs(kwargs: &Kwargs) -> TeraResult<Backlinks> {
    let mut b = Backlinks::default();

    if let Some(s) = kwargs.get::<&str>("order_by")? {
        b.order_by = match s {
            "title" => OrderKey::Title,
            "date" => OrderKey::Date,
            "updated" => OrderKey::Updated,
            other => {
                return Err(Error::message(format!(
                    "backlinks: `order_by` must be one of title|date|updated (got `{}`)",
                    other
                )));
            }
        };
    }

    if let Some(s) = kwargs.get::<&str>("sort")? {
        b.sort = match s {
            "asc" => SortDir::Asc,
            "desc" => SortDir::Desc,
            other => {
                return Err(Error::message(format!(
                    "backlinks: `sort` must be one of asc|desc (got `{}`)",
                    other
                )));
            }
        };
    }

    if let Some(omit) = kwargs.get::<Vec<String>>("omit")? {
        b.omit = omit.into_iter().map(PathBuf::from).collect();
    }

    if let Some(n) = kwargs.get::<usize>("limit")? {
        b.limit = Some(n);
    }

    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kwargs<const N: usize>(pairs: [(&'static str, Value); N]) -> Kwargs {
        Kwargs::from(pairs)
    }

    #[test]
    fn from_kwargs_empty_is_default() {
        let b = from_kwargs(&kwargs([])).unwrap();
        assert_eq!(b.order_by, OrderKey::Date);
        assert_eq!(b.sort, SortDir::Desc);
    }

    #[test]
    fn from_kwargs_all_fields() {
        let b = from_kwargs(&kwargs([
            ("order_by", Value::from("title")),
            ("sort", Value::from("asc")),
        ]))
        .unwrap();
        assert_eq!(b.order_by, OrderKey::Title);
        assert_eq!(b.sort, SortDir::Asc);
    }

    #[test]
    fn from_kwargs_parses_limit() {
        let b = from_kwargs(&kwargs([("limit", Value::from(3u64))])).unwrap();
        assert_eq!(b.limit, Some(3));
    }

    #[test]
    fn from_kwargs_limit_not_integer_errors() {
        assert!(from_kwargs(&kwargs([("limit", Value::from("3"))])).is_err());
    }

    #[test]
    fn from_kwargs_parses_omit() {
        let b = from_kwargs(&kwargs([(
            "omit",
            Value::from(vec![Value::from("a.md"), Value::from("b.md")]),
        )]))
        .unwrap();
        assert_eq!(
            b.omit,
            vec![
                std::path::PathBuf::from("a.md"),
                std::path::PathBuf::from("b.md")
            ]
        );
    }

    #[test]
    fn from_kwargs_omit_not_array_errors() {
        assert!(from_kwargs(&kwargs([("omit", Value::from("a.md"))])).is_err());
    }

    #[test]
    fn from_kwargs_bad_order_by_errors() {
        assert!(from_kwargs(&kwargs([("order_by", Value::from("nope"))])).is_err());
    }

    #[test]
    fn from_kwargs_bad_sort_errors() {
        assert!(from_kwargs(&kwargs([("sort", Value::from("upward"))])).is_err());
    }
}
