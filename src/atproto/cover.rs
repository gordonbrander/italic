//! Cover-image resolution for the `site.standard.document` record's
//! `coverImage` blob. Coordinates with the metadata filters
//! (`crate::tera_env::meta`), which resolve the social image as
//! `page.data.image` → `site.image`: the blob comes from the same fields, so
//! the ATProto cover always matches the page's og:/twitter: image.
//!
//! `image:`/`site.image` are site-root-relative *URL paths* — they resolve to
//! files by walking the static source roots (italic serves images only from
//! `static/`, so every locally-served image path is findable there). Pure
//! (filesystem existence checks only); the upload/caching orchestration lives
//! in [`crate::atproto`].

use crate::doc::Doc;
use serde_yaml_ng::Value;
use std::path::{Path, PathBuf};

/// How a doc's cover image resolved. Anything but `Resolved` skips the blob;
/// problems warn rather than fail (matching the HTML side, where a broken
/// social image doesn't fail the build either).
#[derive(Debug, PartialEq)]
pub enum Cover {
    /// A local file to upload; the `&'static str` names the source field for
    /// dry-run/warning text (`"image: frontmatter"` / `"site.image"`).
    Resolved(PathBuf, &'static str),
    /// The selected field is an external URL — it can't be uploaded as a blob.
    External(String),
    /// The selected field's file exists under no static root; carries the raw
    /// field value and its source name.
    Missing(String, &'static str),
    /// No image field anywhere.
    None,
}

/// Resolve a doc's cover: `image:` frontmatter → `site_image`. The first
/// *present* field wins; if it can't be blobbed (external URL, missing file) we
/// do NOT cascade further — the ATProto cover should match the page's declared
/// social image, never silently substitute a different one.
///
/// `roots` must be in lookup priority order — site first, then theme — the
/// *reverse* of [`crate::config::Config::static_roots`]'s copy order (the copy
/// phase walks theme-then-site so the site overlays the theme).
pub fn resolve(doc: &Doc, site_image: Option<&str>, roots: &[PathBuf]) -> Cover {
    let (raw, source) = match data_field(doc, "image") {
        Some(raw) => (raw, "image: frontmatter"),
        None => match site_image.map(str::trim).filter(|s| !s.is_empty()) {
            Some(raw) => (raw, "site.image"),
            None => return Cover::None,
        },
    };
    if is_external(raw) {
        return Cover::External(raw.to_string());
    }
    match find_in_roots(raw, roots) {
        Some(path) => Cover::Resolved(path, source),
        None => Cover::Missing(raw.to_string(), source),
    }
}

/// A trimmed, non-empty string field of the doc's raw frontmatter (mirrors the
/// metadata filters' `data_field` semantics).
fn data_field<'a>(doc: &'a Doc, key: &str) -> Option<&'a str> {
    doc.data
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// True for `http://`, `https://`, and protocol-relative `//` URLs (mirrors
/// the metadata filters' `abs_url` passthrough test).
fn is_external(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

/// Find the file serving a site-root-relative URL path: try each root in
/// order, joining the path with its leading `/` trimmed.
fn find_in_roots(url_path: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let rel = Path::new(url_path.trim_start_matches('/'));
    roots
        .iter()
        .map(|root| root.join(rel))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};
    use serde_yaml_ng::Mapping;
    use std::fs;

    fn doc_with(fields: &[(&str, &str)]) -> Doc {
        let mut data = Mapping::new();
        for (k, v) in fields {
            data.insert(Value::String((*k).into()), Value::String((*v).into()));
        }
        Doc {
            data,
            ..Doc::default()
        }
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"png").unwrap();
    }

    #[test]
    fn image_resolves_in_static_root() {
        let dir = tempdir("cover_image_resolves");
        touch(&dir.join("static/img/i.png"));
        let doc = doc_with(&[("image", "/img/i.png")]);
        let roots = vec![dir.join("static")];
        assert_eq!(
            resolve(&doc, None, &roots),
            Cover::Resolved(dir.join("static/img/i.png"), "image: frontmatter")
        );
        cleanup(&dir);
    }

    #[test]
    fn site_root_shadows_theme_root() {
        let dir = tempdir("cover_site_shadows_theme");
        touch(&dir.join("static/img/i.png"));
        touch(&dir.join("theme/static/img/i.png"));
        let doc = doc_with(&[("image", "/img/i.png")]);
        // Lookup order: site first, then theme.
        let roots = vec![dir.join("static"), dir.join("theme/static")];
        assert_eq!(
            resolve(&doc, None, &roots),
            Cover::Resolved(dir.join("static/img/i.png"), "image: frontmatter")
        );
        cleanup(&dir);
    }

    #[test]
    fn image_falls_through_to_theme_root() {
        let dir = tempdir("cover_theme_fallthrough");
        touch(&dir.join("theme/static/img/i.png"));
        let doc = doc_with(&[("image", "img/i.png")]); // no leading slash
        let roots = vec![dir.join("static"), dir.join("theme/static")];
        assert_eq!(
            resolve(&doc, None, &roots),
            Cover::Resolved(dir.join("theme/static/img/i.png"), "image: frontmatter")
        );
        cleanup(&dir);
    }

    #[test]
    fn missing_image_does_not_cascade_to_site_image() {
        let dir = tempdir("cover_no_cascade");
        touch(&dir.join("static/img/default.png"));
        let doc = doc_with(&[("image", "/img/nope.png")]);
        let roots = vec![dir.join("static")];
        assert_eq!(
            resolve(&doc, Some("/img/default.png"), &roots),
            Cover::Missing("/img/nope.png".into(), "image: frontmatter")
        );
        cleanup(&dir);
    }

    #[test]
    fn site_image_is_the_fallback() {
        let dir = tempdir("cover_site_image");
        touch(&dir.join("static/img/default.png"));
        let doc = doc_with(&[]);
        let roots = vec![dir.join("static")];
        assert_eq!(
            resolve(&doc, Some("/img/default.png"), &roots),
            Cover::Resolved(dir.join("static/img/default.png"), "site.image")
        );
        cleanup(&dir);
    }

    #[test]
    fn external_image_field_is_skipped() {
        let doc = doc_with(&[("image", "//cdn.example/i.png")]);
        assert_eq!(
            resolve(&doc, None, &[]),
            Cover::External("//cdn.example/i.png".into())
        );
    }

    #[test]
    fn empty_fields_are_ignored() {
        let doc = doc_with(&[("image", "")]);
        assert_eq!(resolve(&doc, Some("   "), &[]), Cover::None);
        assert_eq!(resolve(&doc, None, &[]), Cover::None);
    }
}
