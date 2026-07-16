//! Jekyll-inspired URL filters. Registered on both the markup and template
//! envs (spec §11): all four are either 1:1 doc lookups or string composition,
//! never index listings.
//!
//! - `permalink` — `id_path` → absolute URL (`site_url + base_path + doc_url`).
//!   Falls back to root-relative output when `site_url` is `None`.
//! - `link` — `id_path` → root-relative URL (`base_path + doc_url`).
//! - `relative_url` — arbitrary relative path → `base_path + "/" + path`.
//! - `absolute_url` — arbitrary relative path → `site_url + base_path + "/" + path`.
//!   Falls back to root-relative when `site_url` is `None`.
//!
//! All four return *safe* values (`Value::safe_string`): they emit URLs, not
//! arbitrary text, so their output must not be HTML-escaped by Tera's
//! autoescape in `.html`/`.xml` templates (otherwise `/` becomes `&#x2F;`).

use crate::doc::DocMeta;
use crate::doc_index::DocIndex;
use crate::permalink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tera::{Error, Kwargs, State, Tera, TeraResult, Value};

/// Resolves a doc `id_path` to its URL (`permalink::to_url(output_path)`), or
/// `None` if no such doc exists. Lets `permalink`/`link` share one filter body
/// across the markup env (which only has a `DocMeta` snapshot) and the template
/// env (which reads through the shared `DocIndex`).
pub type DocUrlLookup = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Lookup backed by the markup phase's frozen `DocMeta` snapshot.
pub fn lookup_from_metas(docs: Arc<Vec<DocMeta>>) -> DocUrlLookup {
    Arc::new(move |id_path: &str| {
        let target = PathBuf::from(id_path);
        docs.iter()
            .find(|d| d.id_path == target)
            .map(|d| permalink::to_url(&d.output_path))
    })
}

/// Lookup backed by the shared template-phase `DocIndex` (O(1), no clone).
pub fn lookup_from_index(index: Arc<DocIndex>) -> DocUrlLookup {
    Arc::new(move |id_path: &str| index.output_path(Path::new(id_path)).map(permalink::to_url))
}

pub fn register(env: &mut Tera, lookup: DocUrlLookup, site_url: Option<String>, base_path: String) {
    // `permalink`: doc `id_path` → absolute URL (`site_url + base_path + doc_url`).
    let l = lookup.clone();
    let s = site_url.clone();
    let b = base_path.clone();
    env.register_filter(
        "permalink",
        move |id_path: &str, _: Kwargs, _: &State| -> TeraResult<Value> {
            let url = lookup_doc_url(&l, id_path, "permalink")?;
            let out = format!("{}{}{}", s.as_deref().unwrap_or(""), b, url);
            Ok(Value::safe_string(&out))
        },
    );

    // `link`: doc `id_path` → root-relative URL (`base_path + doc_url`).
    let b = base_path.clone();
    env.register_filter(
        "link",
        move |id_path: &str, _: Kwargs, _: &State| -> TeraResult<Value> {
            let url = lookup_doc_url(&lookup, id_path, "link")?;
            Ok(Value::safe_string(&format!("{}{}", b, url)))
        },
    );

    // `relative_url`: arbitrary path → `base_path + "/" + path`.
    let b = base_path.clone();
    env.register_filter(
        "relative_url",
        move |p: &str, _: Kwargs, _: &State| -> Value {
            let stripped = p.trim_start_matches('/');
            Value::safe_string(&format!("{}/{}", b, stripped))
        },
    );

    // `absolute_url`: arbitrary path → `site_url + base_path + "/" + path`.
    env.register_filter(
        "absolute_url",
        move |p: &str, _: Kwargs, _: &State| -> Value {
            let stripped = p.trim_start_matches('/');
            Value::safe_string(&format!(
                "{}{}/{}",
                site_url.as_deref().unwrap_or(""),
                base_path,
                stripped
            ))
        },
    );
}

fn lookup_doc_url(lookup: &DocUrlLookup, id_path: &str, filter_name: &str) -> TeraResult<String> {
    lookup(id_path).ok_or_else(|| {
        Error::message(format!(
            "{} filter: no doc with id_path `{}`",
            filter_name, id_path
        ))
    })
}
