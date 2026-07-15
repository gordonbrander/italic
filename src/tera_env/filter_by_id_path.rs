//! `docs | filter_by_id_path(path="posts/**", omit=[...])` — keep only the docs
//! whose `id_path` matches a glob. Registered on both envs: it reads `id_path`
//! off each piped doc value and never touches the index, so it is pure data
//! shaping like `filter_in_dir`.
//!
//! Unlike `filter_in_dir` (a single literal directory level), this matches a
//! full glob with the same semantics as a collection `Query` (`crate::query`):
//! `literal_separator` is on, so `posts/*.md` does not cross `/` while
//! `posts/**` does. This is the render-time counterpart that lets a template
//! scope a shared taxonomy to a path, e.g.
//!
//! ```html
//! {% set posts = taxonomy(name="tags")["rust"] | filter_by_id_path(path="posts/**") %}
//! ```
//!
//! Matching is against `id_path` (the source path). Input order is **preserved**
//! — it is a pure filter, not a reorder (this is the one behavioral difference
//! from `filter_in_dir`, which sorts).

use globset::GlobBuilder;
use std::collections::HashSet;
use tera::{Error, Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera) {
    env.register_filter(
        "filter_by_id_path",
        |docs: &[Value], kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            let path = kwargs.must_get::<&str>("path")?;
            let omit = kwargs.get::<Vec<String>>("omit")?.unwrap_or_default();
            let omit: HashSet<&str> = omit.iter().map(String::as_str).collect();
            filter_by_id_path(docs, path, &omit)
        },
    );
}

/// Keep docs whose `id_path` matches `glob` and that are not in `omit`,
/// preserving input order. Glob semantics match `crate::query::Query`
/// (`literal_separator` on). Kept free of Tera plumbing so it can be
/// unit-tested directly.
fn filter_by_id_path(docs: &[Value], glob: &str, omit: &HashSet<&str>) -> TeraResult<Value> {
    let matcher = GlobBuilder::new(glob)
        .literal_separator(true)
        .build()
        .map_err(|e| Error::message(format!("filter_by_id_path: invalid glob `{glob}`: {e}")))?
        .compile_matcher();

    let mut kept: Vec<Value> = Vec::new();
    for doc in docs {
        let id_path = doc
            .get_from_path("id_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Error::message("filter_by_id_path filter: every doc must have a string `id_path`")
            })?;
        if matcher.is_match(id_path) && !omit.contains(id_path) {
            kept.push(doc.clone());
        }
    }
    Ok(Value::from(kept))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A doc value with just the fields `filter_by_id_path` reads/echoes.
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

    fn empty_omit() -> HashSet<&'static str> {
        HashSet::new()
    }

    #[test]
    fn double_star_matches_across_directories() {
        let docs = [
            doc("posts/a.md", "A"),
            doc("posts/sub/b.md", "B"),
            doc("notes/x.md", "X"), // different top dir — excluded
        ];
        let out = filter_by_id_path(&docs, "posts/**", &empty_omit()).unwrap();
        assert_eq!(ids(&out), vec!["posts/a.md", "posts/sub/b.md"]);
    }

    #[test]
    fn single_star_does_not_cross_separator() {
        // `literal_separator` is on, so `*` stops at `/`.
        let docs = [doc("posts/a.md", "A"), doc("posts/sub/b.md", "B")];
        let out = filter_by_id_path(&docs, "posts/*.md", &empty_omit()).unwrap();
        assert_eq!(ids(&out), vec!["posts/a.md"]);
    }

    #[test]
    fn preserves_input_order() {
        // Fed out of id_path order; the filter must not re-sort.
        let docs = [
            doc("posts/c.md", "C"),
            doc("posts/a.md", "A"),
            doc("posts/b.md", "B"),
        ];
        let out = filter_by_id_path(&docs, "posts/*.md", &empty_omit()).unwrap();
        assert_eq!(ids(&out), vec!["posts/c.md", "posts/a.md", "posts/b.md"]);
    }

    #[test]
    fn omit_drops_listed_id_paths() {
        let docs = [doc("posts/a.md", "A"), doc("posts/b.md", "B")];
        let omit: HashSet<&str> = ["posts/a.md"].into_iter().collect();
        let out = filter_by_id_path(&docs, "posts/*.md", &omit).unwrap();
        assert_eq!(ids(&out), vec!["posts/b.md"]);
    }

    #[test]
    fn invalid_glob_is_an_error() {
        let docs = [doc("posts/a.md", "A")];
        assert!(filter_by_id_path(&docs, "posts/[", &empty_omit()).is_err());
    }

    #[test]
    fn missing_id_path_is_an_error() {
        let no_id = Value::from_serializable(&BTreeMap::from([("title", "No id")]));
        assert!(filter_by_id_path(&[no_id], "**", &empty_omit()).is_err());
    }

    #[test]
    fn non_array_input_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("x", "not an array");
        assert!(
            env.render_str("{{ x | filter_by_id_path(path=\"**\") }}", &ctx, false)
                .is_err()
        );
    }

    #[test]
    fn missing_path_argument_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("docs", &Vec::<u8>::new());
        assert!(
            env.render_str("{{ docs | filter_by_id_path }}", &ctx, false)
                .is_err()
        );
    }
}
