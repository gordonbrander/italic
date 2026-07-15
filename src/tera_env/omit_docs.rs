//! `docs | omit_docs(omit=[...])` — drop docs by `id_path` from a piped array.
//! The general-purpose complement to the `omit` kwarg baked into `collection`,
//! `backlinks`, `related`, and `filter_in_dir`: use it to exclude docs from any
//! list those functions don't cover (a `dirtree` input, a concatenation, the
//! current page from a hand-built array). Registered on both envs — it reads
//! `id_path` off each value and never touches the index, so it is pure data
//! shaping like `dirtree`/`filter_in_dir`.
//!
//! Input order is preserved (it is a subtraction, not a re-sort):
//!
//! ```html
//! {% set others = collection(name="all") | omit_docs(omit=[page.id_path]) %}
//! ```

use std::collections::HashSet;
use tera::{Error, Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera) {
    env.register_filter(
        "omit_docs",
        |docs: &[Value], kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            // `omit` is required — the filter's whole purpose, so a missing arg
            // is an author error. An empty array is a no-op passthrough.
            let omit = kwargs.must_get::<Vec<String>>("omit")?;
            let omit: HashSet<&str> = omit.iter().map(String::as_str).collect();
            omit_docs(docs, &omit)
        },
    );
}

/// Keep docs whose `id_path` is not in `omit`, in the original order. Kept free
/// of Tera plumbing so it can be unit-tested directly.
fn omit_docs(docs: &[Value], omit: &HashSet<&str>) -> TeraResult<Value> {
    let mut kept: Vec<Value> = Vec::new();
    for doc in docs {
        let id_path = doc
            .get_from_path("id_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Error::message("omit_docs filter: every doc must have a string `id_path`")
            })?;
        if !omit.contains(id_path) {
            kept.push(doc.clone());
        }
    }
    Ok(Value::from(kept))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A doc value with just the fields `omit_docs` reads/echoes.
    fn doc(id_path: &str, title: &str) -> Value {
        let map: BTreeMap<&str, &str> = [("id_path", id_path), ("title", title)]
            .into_iter()
            .collect();
        Value::from_serializable(&map)
    }

    fn ids(value: &Value) -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                d.get_from_path("id_path")
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn drops_listed_docs_preserving_order() {
        let docs = [doc("c.md", "C"), doc("a.md", "A"), doc("b.md", "B")];
        let omit: HashSet<&str> = ["a.md"].into_iter().collect();
        let out = omit_docs(&docs, &omit).unwrap();
        // Order preserved (not re-sorted); only `a.md` removed.
        assert_eq!(ids(&out), vec!["c.md", "b.md"]);
    }

    #[test]
    fn empty_omit_is_passthrough() {
        let docs = [doc("a.md", "A"), doc("b.md", "B")];
        let out = omit_docs(&docs, &HashSet::new()).unwrap();
        assert_eq!(ids(&out), vec!["a.md", "b.md"]);
    }

    #[test]
    fn missing_id_path_is_an_error() {
        let no_id = Value::from_serializable(&BTreeMap::from([("title", "No id")]));
        assert!(omit_docs(&[no_id], &HashSet::new()).is_err());
    }

    #[test]
    fn non_array_input_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("x", "not an array");
        assert!(
            env.render_str("{{ x | omit_docs(omit=[]) }}", &ctx, false)
                .is_err()
        );
    }

    #[test]
    fn missing_omit_argument_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("docs", &Vec::<u8>::new());
        assert!(
            env.render_str("{{ docs | omit_docs }}", &ctx, false)
                .is_err()
        );
    }
}
