//! Italic's canonical slugifier. A std-only port of comrak 0.52's `Anchorizer`
//! character rules (`comrak/src/html/anchorizer.rs`), minus its per-document
//! dedup counter. Used for ALL slugs — permalinks, taxonomy terms, wikilink
//! stems, asset names, and heading-link fragments — so a `[[Note#Heading]]`
//! fragment equals the `id` comrak emits on that heading. The contract test
//! below pins this to comrak's real output.

/// Slugify text the way comrak anchorizes heading ids: lowercase; keep
/// alphanumerics / `-` / `_`; turn each space into `-`; drop everything else.
/// No run-collapsing and no leading/trailing trim — matching comrak, not the
/// old `slug` crate.
pub fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(is_permitted_char)
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// comrak keeps space, `-`, Unicode letters, marks, numbers, and connector
/// punctuation (`_`). std's `is_alphanumeric` covers letters + numbers; we add
/// `-`, `_`, and space explicitly. Divergence from comrak is confined to exotic
/// Unicode combining marks — accepted for the std-only (ASCII-safe) port.
fn is_permitted_char(c: &char) -> bool {
    *c == ' ' || *c == '-' || *c == '_' || c.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render `# {heading}` through real comrak with heading anchors enabled and
    /// return the `id` comrak emits — the source of truth we must match.
    fn comrak_heading_id(heading: &str) -> String {
        let mut options = comrak::Options::default();
        options.extension.header_id_prefix = Some(String::new());
        let html = comrak::markdown_to_html(&format!("# {heading}\n"), &options);
        // Emitted shape: <h1><a href="#..." ... id="{ID}"></a>{heading}</h1>
        let marker = r#" id=""#;
        let start = html.find(marker).expect("comrak emitted an id") + marker.len();
        let end = html[start..].find('"').unwrap() + start;
        html[start..end].to_string()
    }

    /// The linchpin: our slugifier must equal comrak's emitted heading id for a
    /// corpus of ASCII + accented-Latin headings (where std and comrak agree).
    /// Exotic-Unicode combining marks are intentionally excluded — the std-only
    /// port may diverge there.
    #[test]
    fn slugify_matches_comrak_heading_ids() {
        for heading in [
            "Hello World",
            "Isn't it grand?",
            "Foo: Bar / Baz",
            "Café",
            "Multiple   spaces",
            "under_score",
            "Trailing punctuation!",
            "Getting Started",
        ] {
            assert_eq!(
                slugify(heading),
                comrak_heading_id(heading),
                "slugify diverged from comrak for {heading:?}"
            );
        }
    }

    #[test]
    fn drops_apostrophes_and_punctuation() {
        assert_eq!(slugify("Isn't it grand?"), "isnt-it-grand");
        assert_eq!(slugify("Foo: Bar / Baz"), "foo-bar--baz");
    }

    #[test]
    fn does_not_collapse_runs() {
        assert_eq!(slugify("a  b"), "a--b");
    }

    #[test]
    fn does_not_trim_separators() {
        assert_eq!(slugify(" foo "), "-foo-");
    }

    #[test]
    fn keeps_unicode_letters() {
        assert_eq!(slugify("Café"), "café");
    }

    #[test]
    fn keeps_underscore_and_hyphen() {
        assert_eq!(slugify("under_score-dash"), "under_score-dash");
    }

    #[test]
    fn common_case_is_unchanged() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn empty_and_all_dropped() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("!!!"), "");
    }
}
