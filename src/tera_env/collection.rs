//! Tera adapter for named collections. `collection(name="posts")` returns the
//! docs of the collection precomputed on the [`DocIndex`] during the template
//! phase. An unknown name returns an empty list (no error), so a misspelled name
//! renders nothing rather than failing the build.
//!
//! Optional `omit` and `limit` kwargs filter the cached collection at render
//! time — the cached docs are omit-filtered then truncated, no re-query, no
//! re-sort. `omit` layers on top of the collection's own definition-time `omit`;
//! `limit` is a render-only cap (a collection has no definition-time count — that
//! is deliberately the filter's job). This is what lets a page exclude itself
//! from a collection it belongs to, e.g.
//! `collection(name="posts", omit=[page.id_path], limit=5)`. Omit is applied
//! before limit.

use crate::doc_index::DocIndex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tera::{Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "collection",
        move |kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            // A missing or non-string `name` is an author error (unlike an
            // unknown-but-well-formed name, which simply yields nothing).
            let name = kwargs.must_get::<&str>("name")?;
            let omit: Vec<PathBuf> = kwargs
                .get::<Vec<String>>("omit")?
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect();
            let limit = kwargs.get::<usize>("limit")?;
            let omit: HashSet<&Path> = omit.iter().map(PathBuf::as_path).collect();
            let docs: Vec<&crate::doc::Doc> = index
                .get_collection(name)
                .filter(|d| !omit.contains(d.id_path.as_path()))
                .take(limit.unwrap_or(usize::MAX))
                .collect();
            Value::try_from_serializable(&docs)
        },
    );
}
