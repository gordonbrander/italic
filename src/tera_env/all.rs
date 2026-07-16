//! Tera adapter for the whole corpus. `all()` returns the always-present
//! [`all`](crate::config::ALL) collection from the frozen [`DocIndex`] shared
//! across the template phase. That collection is guaranteed to exist: when a
//! site/theme does not declare its own `all` under `collections:`,
//! `Config::load_with_theme` injects one with the default [`Query`], so `all()`
//! lists every doc in date-desc order out of the box. It is the zero-config
//! escape hatch for "list everything" that needs no `collections:` entry.
//!
//! Because it is backed by a collection, a site that *does* declare `all:` under
//! `collections:` reorders, omits, or filters what `all()` returns.
//!
//! It takes no arguments by design: ordering, limiting, and filtering are what
//! collections are for (`collection(name=...)` with an `order_by`/`sort`/`omit`
//! definition — including redefining `all` itself), or what the array filters
//! do at render time (`omit_docs`, `dirtree`, `filter_in_dir`, array slicing).

use crate::config::ALL;
use crate::doc_index::DocIndex;
use std::sync::Arc;
use tera::{Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function("all", move |_: Kwargs, _: &State| -> TeraResult<Value> {
        let docs: Vec<&crate::doc::Doc> = index.get_collection(ALL).collect();
        Value::try_from_serializable(&docs)
    });
}
