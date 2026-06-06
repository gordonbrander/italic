use crate::config::Config;
use crate::doc::Doc;
use crate::doc_index::DocIndex;
use anyhow::{Context, Result};
use walkdir::WalkDir;

/// Scan `content_dir` into a [`DocIndex`]. When `include_drafts` is false (the
/// production `italic build` default), docs whose frontmatter sets `draft: true`
/// are skipped entirely, so they never reach collections, taxonomies, or
/// backlinks. Local `serve`/`watch` and `italic build --drafts` pass `true`.
pub fn run(config: &Config, include_drafts: bool) -> Result<DocIndex> {
    let mut index = DocIndex::new();
    if !config.content_dir.exists() {
        return Ok(index);
    }
    for entry in WalkDir::new(&config.content_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(ext, "md" | "html" | "yaml") {
            continue;
        }
        let id_path = path
            .strip_prefix(&config.content_dir)
            .with_context(|| format!("could not strip prefix from {}", path.display()))?
            .to_path_buf();
        let doc = Doc::load(&config.content_dir, &id_path, &config.taxonomies)?;
        if !include_drafts && doc.draft {
            continue;
        }
        index.insert(doc);
    }
    Ok(index)
}
