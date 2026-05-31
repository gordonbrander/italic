//! The classify phase. After markup, build the *frozen classification* — a
//! snapshot of the source docs with collections and taxonomies defined on it —
//! and hand it to the archives and template phases as a shared `Arc<DocIndex>`.
//!
//! Classification is built from source content only and never mutated again, so
//! the parallel phases downstream can read it by reference. Crucially,
//! archive-generated pages are appended to the *live* index but are not present
//! here, so `collection()`/`taxonomy()` only ever list authored docs (no
//! feedback loop, no ordering between archives).

use crate::config::Config;
use crate::doc_index::DocIndex;
use std::sync::Arc;

/// Clone the post-markup index and define every configured collection and
/// taxonomy on the clone, returning it frozen behind an `Arc`.
pub fn run(config: &Config, index: &DocIndex) -> Arc<DocIndex> {
    let mut snapshot = index.clone();
    for (name, query) in &config.collections {
        snapshot.define_collection(name, query);
    }
    snapshot.define_taxonomies(&config.taxonomies);
    Arc::new(snapshot)
}
