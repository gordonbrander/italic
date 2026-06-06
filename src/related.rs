//! Options + rarity-weighting seam for the `related` doc-similarity query. The
//! scorer itself lives on [`DocIndex::related`](crate::doc_index::DocIndex::related)
//! because it reads the inverted taxonomy and link indices; this module owns the
//! `Related` options struct and the `idf` seam that shapes scoring.

use std::path::PathBuf;

/// The reserved namespace name for the link graph. As a `related` weight key it
/// means "score by the link graph" (forward links, backlinks, and co-citation)
/// rather than a taxonomy. Because the link graph lives in its own index (not
/// the taxonomy map), a site may still *declare* a taxonomy literally named
/// `links`; it simply can't be addressed through `related` weights, where the
/// name is reserved for the graph.
pub const LINKS: &str = "links";

/// Per-query options for `related`.
#[derive(Debug, Clone, Default)]
pub struct Related {
    /// `(namespace, weight)` pairs. A namespace is either a taxonomy name —
    /// whose shared term slugs drive overlap — or the special [`LINKS`]
    /// namespace. Config order; an unknown namespace contributes nothing.
    pub weights: Vec<(String, f64)>,
    /// Docs to exclude from the result (besides the query doc itself, which is
    /// always excluded). Applied before `limit`.
    pub omit: Vec<PathBuf>,
    pub limit: Option<usize>,
}

/// Inverse-document-frequency weight for a shared term with document frequency
/// `df`. Phase 1 stub: flat `1.0`, so every shared term counts equally. The seam
/// exists so Phase 2 can down-weight common terms (e.g. `1.0 / (1.0 + ln df)`)
/// against a real vault without disturbing the scorer's structure.
pub fn idf(_df: usize) -> f64 {
    1.0
}
