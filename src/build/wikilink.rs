use crate::doc::{Doc, DocMeta};
use crate::html;
use crate::permalink;
use std::path::{Component, Path, PathBuf};

/// Scan `body` for `[[Wiki Link]]` / `[[Wiki Link|Display]]` and replace each
/// occurrence with either `<a class="wikilink" href="…">display</a>` (resolved)
/// or `<span class="nolink">display</span>` (unresolved, logged to stderr).
///
/// Returns the rewritten body and the deduplicated list of resolved target
/// `id_path`s — the source doc's outlinks (consumed by Phase 10 backlinks).
///
/// Spec §8: global stem-slug match across all docs; ties broken by minimum
/// directory distance to the source, then by lexicographically smallest
/// `id_path`. An optional path prefix `[[dir/sub/Name]]` (anchored at the
/// vault root, slugified componentwise) restricts the candidate set to docs
/// whose parent directory matches that prefix exactly.
pub fn expand(body: &str, source: &Doc, docs: &[DocMeta]) -> (String, Vec<PathBuf>) {
    let mut out = String::with_capacity(body.len());
    let mut outlinks: Vec<PathBuf> = Vec::new();
    let mut rest = body;

    while let Some(open) = rest.find("[[") {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];

        let close = find_close(after_open);
        match close {
            Some(end) => {
                let inside = &after_open[..end];
                let (target, display) = split_target_display(inside);

                if let Some(doc) = resolve(target, &source.id_path, docs) {
                    let url = permalink::to_url(&doc.output_path);
                    out.push_str(r#"<a class="wikilink" href=""#);
                    out.push_str(&html::escape(&url));
                    out.push_str(r#"">"#);
                    out.push_str(&html::escape(display));
                    out.push_str("</a>");
                    if !outlinks.contains(&doc.id_path) {
                        outlinks.push(doc.id_path.clone());
                    }
                } else {
                    eprintln!(
                        "warning: unresolved wikilink [[{}]] in {}",
                        target,
                        source.id_path.display()
                    );
                    out.push_str(r#"<span class="nolink">"#);
                    out.push_str(&html::escape(display));
                    out.push_str("</span>");
                }
                rest = &after_open[end + 2..];
            }
            None => {
                // No close on this opener — emit `[[` literally and keep
                // scanning so a later valid `[[…]]` still gets picked up.
                out.push_str("[[");
                rest = after_open;
            }
        }
    }
    out.push_str(rest);

    (out, outlinks)
}

/// Find the byte offset of the first `]]` that closes a wikilink opening,
/// rejecting the link if a `\n` or another `[` appears first. Returns the
/// offset of the leading `]` of the closing pair, or `None` if no valid close
/// exists.
fn find_close(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut j = 0;
    while j + 1 < bytes.len() {
        let c = bytes[j];
        if c == b'\n' || c == b'[' {
            return None;
        }
        if c == b']' && bytes[j + 1] == b']' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn split_target_display(inside: &str) -> (&str, &str) {
    match inside.find('|') {
        Some(idx) => (inside[..idx].trim(), inside[idx + 1..].trim()),
        None => {
            let t = inside.trim();
            (t, t)
        }
    }
}

/// Split a wikilink target into an optional path prefix and a stem segment.
/// `"reference/Glossary"` → `(Some("reference"), "Glossary")`,
/// `"Hello"`              → `(None, "Hello")`.
/// A leading `/` survives in the prefix as the empty string, which causes
/// `prefix_matches` to require a root-level candidate.
fn split_prefix_stem(target: &str) -> (Option<&str>, &str) {
    match target.rfind('/') {
        Some(idx) => (Some(&target[..idx]), &target[idx + 1..]),
        None => (None, target),
    }
}

/// Edge count between two directory paths in the tree: drop the longest
/// shared component prefix, sum the lengths of what remains on each side.
/// `("blog/2025", "reference")` → 3 (up 2, down 1).
fn dir_distance(a: &Path, b: &Path) -> usize {
    let ac: Vec<Component> = a.components().collect();
    let bc: Vec<Component> = b.components().collect();
    let common = ac.iter().zip(bc.iter()).take_while(|(x, y)| x == y).count();
    (ac.len() - common) + (bc.len() - common)
}

/// True iff `parent`'s normalized components — slugified individually —
/// equal `prefix`'s slash-separated components, also slugified. Empty
/// segments (e.g. from a leading `/`) are ignored on the prefix side.
fn prefix_matches(parent: &Path, prefix: &str) -> bool {
    let prefix_slugs: Vec<String> = prefix
        .split('/')
        .filter(|s| !s.is_empty())
        .map(slug::slugify)
        .collect();
    let parent_slugs: Vec<String> = parent
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str().map(slug::slugify),
            _ => None,
        })
        .collect();
    parent_slugs == prefix_slugs
}

/// Resolve a wikilink target to a doc, per spec §8.
///
/// Slugify the stem (and prefix components, if a `dir/Name` form is used)
/// and scan the full doc set. Among candidates whose stem-slug matches —
/// and, if a prefix is given, whose parent dir also matches — pick the one
/// with the smallest directory distance from the source. Ties are broken
/// by the lexicographically smallest `id_path` so output is deterministic
/// across runs and platforms.
fn resolve<'a>(target: &str, source_id_path: &Path, docs: &'a [DocMeta]) -> Option<&'a DocMeta> {
    let (prefix, stem) = split_prefix_stem(target);
    let stem_slug = slug::slugify(stem);
    if stem_slug.is_empty() {
        return None;
    }
    let empty = Path::new("");
    let source_dir = source_id_path.parent().unwrap_or(empty);

    let mut best: Option<(&DocMeta, usize)> = None;
    for doc in docs {
        let cand_stem = doc
            .id_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if slug::slugify(cand_stem) != stem_slug {
            continue;
        }
        let cand_dir = doc.id_path.parent().unwrap_or(empty);
        if let Some(p) = prefix {
            if !prefix_matches(cand_dir, p) {
                continue;
            }
        }
        let dist = dir_distance(source_dir, cand_dir);
        best = match best {
            None => Some((doc, dist)),
            Some((curr, curr_dist)) => {
                let better = dist < curr_dist
                    || (dist == curr_dist && doc.id_path < curr.id_path);
                if better {
                    Some((doc, dist))
                } else {
                    Some((curr, curr_dist))
                }
            }
        };
    }
    best.map(|(doc, _)| doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_doc(id_path: &str) -> Doc {
        let mut d = Doc::default();
        d.id_path = PathBuf::from(id_path);
        d.output_path = PathBuf::from(id_path).with_extension("html");
        d
    }

    fn doc_at(id_path: &str) -> DocMeta {
        DocMeta::from(&source_doc(id_path))
    }

    fn doc_with_permalink(id_path: &str, output_path: &str) -> DocMeta {
        let mut d = Doc::default();
        d.id_path = PathBuf::from(id_path);
        d.output_path = PathBuf::from(output_path);
        DocMeta::from(&d)
    }

    #[test]
    fn expand_resolves_same_dir() {
        let source = source_doc("blog/a.md");
        let docs = vec![DocMeta::from(&source), doc_at("blog/b.md")];
        let (out, outlinks) = expand("see [[b]]", &source, &docs);
        assert_eq!(out, r#"see <a class="wikilink" href="/blog/b.html">b</a>"#);
        assert_eq!(outlinks, vec![PathBuf::from("blog/b.md")]);
    }

    #[test]
    fn expand_walks_to_parent() {
        let source = source_doc("blog/2025/deep.md");
        let docs = vec![DocMeta::from(&source), doc_at("hello.md")];
        let (out, outlinks) = expand("see [[hello]]", &source, &docs);
        assert!(out.contains(r#"href="/hello.html""#));
        assert_eq!(outlinks, vec![PathBuf::from("hello.md")]);
    }

    #[test]
    fn expand_walks_multiple_levels() {
        let source = source_doc("a/b/c/d.md");
        let docs = vec![DocMeta::from(&source), doc_at("top.md")];
        let (out, _) = expand("[[top]]", &source, &docs);
        assert!(out.contains(r#"href="/top.html""#));
    }

    #[test]
    fn expand_uses_display_text() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, _) = expand("[[b|click here]]", &source, &docs);
        assert_eq!(
            out,
            r#"<a class="wikilink" href="/b.html">click here</a>"#
        );
    }

    #[test]
    fn expand_unresolved_emits_nolink_span() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source)];
        let (out, outlinks) = expand("[[Missing]]", &source, &docs);
        assert_eq!(out, r#"<span class="nolink">Missing</span>"#);
        assert!(outlinks.is_empty());
    }

    #[test]
    fn expand_records_outlinks() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md"), doc_at("c.md")];
        let (_, outlinks) = expand("[[b]] and [[c]]", &source, &docs);
        assert_eq!(
            outlinks,
            vec![PathBuf::from("b.md"), PathBuf::from("c.md")]
        );
    }

    #[test]
    fn expand_dedups_outlinks() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (_, outlinks) = expand("[[b]] [[b]] [[b|again]]", &source, &docs);
        assert_eq!(outlinks, vec![PathBuf::from("b.md")]);
    }

    #[test]
    fn expand_escapes_html_in_display() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, _) = expand("[[b|<script>]]", &source, &docs);
        assert!(out.contains("&lt;script&gt;"));
        assert!(!out.contains("<script>"));
    }

    #[test]
    fn expand_ignores_newline_in_brackets() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, _) = expand("[[b\nfoo]]", &source, &docs);
        assert_eq!(out, "[[b\nfoo]]");
    }

    #[test]
    fn expand_ignores_nested_open_bracket() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source), doc_at("b.md")];
        let (out, _) = expand("[[[b]]", &source, &docs);
        // First `[[` aborts (sees `[` inside); next scan starts at `[b]]` which has no `[[`.
        assert_eq!(out, "[[[b]]");
    }

    #[test]
    fn expand_handles_unmatched_open() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source)];
        let (out, _) = expand("text [[ no close", &source, &docs);
        assert_eq!(out, "text [[ no close");
    }

    #[test]
    fn expand_passes_through_no_wikilinks() {
        let source = source_doc("a.md");
        let docs = vec![DocMeta::from(&source)];
        let (out, outlinks) = expand("plain markdown text", &source, &docs);
        assert_eq!(out, "plain markdown text");
        assert!(outlinks.is_empty());
    }

    #[test]
    fn resolve_first_match_wins_closer_dir() {
        // Two docs share stem `hello`; the one in the same dir as source should win
        // over the one in a parent dir.
        let source = doc_at("blog/post.md");
        let docs = vec![
            doc_at("hello.md"),       // parent-dir candidate
            doc_at("blog/hello.md"),  // same-dir candidate (should win)
        ];
        let hit = resolve("hello", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("blog/hello.md"));
    }

    #[test]
    fn resolve_slugifies_target() {
        let source = doc_at("a.md");
        let docs = vec![doc_at("hello-world.md")];
        let hit = resolve("Hello World", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("hello-world.md"));
    }

    #[test]
    fn resolve_returns_none_when_no_match() {
        let source = doc_at("a.md");
        let docs = vec![doc_at("other.md")];
        assert!(resolve("missing", &source.id_path, &docs).is_none());
    }

    #[test]
    fn expand_uses_to_url_for_index_html_dirs() {
        // A doc with a permalink-style output_path (trailing /index.html) should
        // render as a dir URL via to_url.
        let source = source_doc("a.md");
        let docs = vec![
            DocMeta::from(&source),
            doc_with_permalink("b.md", "blog/b/index.html"),
        ];
        let (out, _) = expand("[[b]]", &source, &docs);
        assert!(out.contains(r#"href="/blog/b/""#));
    }

    #[test]
    fn resolve_finds_cross_subtree_neighbor() {
        // Source's ancestor chain has no match, but a deep neighbor in a sibling
        // subtree does — the new global lookup must find it.
        let source = doc_at("blog/2025/post.md");
        let docs = vec![doc_at("reference/glossary.md")];
        let hit = resolve("Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("reference/glossary.md"));
    }

    #[test]
    fn resolve_picks_nearest_when_ambiguous() {
        // Same stem in two subtrees; the one closer to source by directory
        // distance wins. blog/glossary.md (dist 1) beats reference/glossary.md
        // (dist 3) from blog/2025/post.md.
        let source = doc_at("blog/2025/post.md");
        let docs = vec![
            doc_at("reference/glossary.md"),
            doc_at("blog/glossary.md"),
        ];
        let hit = resolve("Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("blog/glossary.md"));
    }

    #[test]
    fn resolve_lexicographic_tiebreaker_on_equal_distance() {
        // Two candidates equidistant from source — pick the lexicographically
        // smaller id_path for determinism.
        let source = doc_at("root.md");
        let docs = vec![
            doc_at("zeta/glossary.md"),
            doc_at("alpha/glossary.md"),
        ];
        let hit = resolve("Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("alpha/glossary.md"));
    }

    #[test]
    fn resolve_explicit_prefix_picks_prefix_match_over_closer() {
        // Even though blog/glossary.md is closer, [[reference/Glossary]] must
        // resolve only to the prefix-matching candidate.
        let source = doc_at("blog/2025/post.md");
        let docs = vec![
            doc_at("blog/glossary.md"),
            doc_at("reference/glossary.md"),
        ];
        let hit = resolve("reference/Glossary", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("reference/glossary.md"));
    }

    #[test]
    fn resolve_explicit_prefix_no_fallback_to_bare_stem() {
        // [[missing/Hello]] must NOT fall back to a bare-stem `hello.md` at
        // the root — explicit prefixes are absolute.
        let source = doc_at("a.md");
        let docs = vec![doc_at("hello.md")];
        assert!(resolve("missing/Hello", &source.id_path, &docs).is_none());
    }

    #[test]
    fn resolve_slugifies_prefix_components() {
        // Prefix `Blog Posts` slugifies to `blog-posts`, matching the
        // candidate directory of the same slug.
        let source = doc_at("a.md");
        let docs = vec![doc_at("blog-posts/hello.md")];
        let hit = resolve("Blog Posts/Hello", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("blog-posts/hello.md"));
    }

    #[test]
    fn resolve_root_anchored_prefix() {
        // `[[/Hello]]` (leading slash → empty prefix) matches only a
        // root-level doc, not a nested one.
        let source = doc_at("blog/post.md");
        let docs = vec![doc_at("blog/hello.md"), doc_at("hello.md")];
        let hit = resolve("/Hello", &source.id_path, &docs).unwrap();
        assert_eq!(hit.id_path, PathBuf::from("hello.md"));
    }

    #[test]
    fn dir_distance_basics() {
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("blog/2025")), 0);
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("blog")), 1);
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("")), 2);
        assert_eq!(dir_distance(Path::new("blog/2025"), Path::new("reference")), 3);
        assert_eq!(dir_distance(Path::new("a/b/c"), Path::new("a/x/y")), 4);
    }
}
