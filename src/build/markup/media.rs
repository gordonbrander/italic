//! Co-located media resolution (spec §8, the asset half). Runs on the parsed
//! comrak AST *before* [`wikilink`](super::wikilink) so it can claim the
//! references that point at media files, leaving plain note links for the
//! wikilink pass.
//!
//! Three reference shapes are rewritten, all to **root-relative, base-path-aware**
//! URLs pointing at the asset's mirrored output location (see
//! [`content_assets`](crate::build::content_assets)). Because the URLs are
//! root-relative they stay correct no matter where the *page* lands — a default
//! `foo.html` or a relocating `foo/index.html` permalink:
//!
//! - **Standard markdown** `![](image.png)` / `[label](file.pdf)` — the `url` is
//!   resolved *relative to the source note's directory* and rewritten in place
//!   when it lands on a known asset. (`Image` → comrak still renders `<img>`;
//!   `Link` → `<a>`.)
//! - **Obsidian embeds** `![[image.png]]` — comrak does *not* tokenize the `!`
//!   form as a wikilink (it stays a `Text` node), so embeds are found by scanning
//!   `Text` nodes. Text nodes never occur inside code spans/blocks, so this is
//!   code-aware for free. An image-extension target becomes `<img>`; anything
//!   else becomes a download `<a>`.
//! - **Obsidian attachment links** `[[report.pdf]]` — a real `WikiLink` node
//!   whose target matches an asset; rewritten to an `<a>`.
//!
//! Embeds and attachment links match by **slugified full filename** (extension
//! included) across the whole vault — Obsidian's attachment semantics — reusing
//! the wikilink prefix/distance/lexical tiebreak helpers.
//!
//! References that don't resolve to a known asset are left untouched: external
//! URLs (`https:`, `mailto:`, `//…`), already-root-absolute paths (`/img.png`),
//! and relative paths with no matching file (e.g. a hand-placed `static/` asset).

use super::wikilink::{dir_distance, node_text, prefix_matches, split_prefix_stem};
use crate::doc::Doc;
use crate::html;
use comrak::nodes::{AstNode, NodeValue};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

/// Extensions rendered as inline images for embeds. Anything else embeds as a
/// download link.
const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp", "ico",
];

/// Lookup tables over the vault's co-located media, built once per markup env
/// (cloned per Rayon worker alongside the wikilink stem index).
#[derive(Clone, Default)]
pub struct AssetIndex {
    /// Every asset's exact `content/`-relative path, for resolving relative
    /// markdown refs (`![](sub/x.png)`) by literal path.
    by_path: HashSet<PathBuf>,
    /// Slugified full filename (extension included) → candidate paths, for
    /// resolving `[[…]]`/`![[…]]` refs by name à la Obsidian attachments.
    by_name: HashMap<String, Vec<PathBuf>>,
}

impl AssetIndex {
    /// Build the lookup tables from the read phase's asset list.
    pub fn build(assets: &[PathBuf]) -> AssetIndex {
        let mut by_path = HashSet::new();
        let mut by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for path in assets {
            by_path.insert(path.clone());
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                by_name
                    .entry(slug::slugify(name))
                    .or_default()
                    .push(path.clone());
            }
        }
        AssetIndex { by_path, by_name }
    }

    fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }
}

/// Rewrite media references in a parsed comrak AST, in place. No return value —
/// assets don't feed the backlink graph. A no-op when the vault has no assets.
pub fn resolve_in_ast<'a>(
    root: &'a AstNode<'a>,
    source: &Doc,
    assets: &AssetIndex,
    base_path: &str,
) {
    if assets.is_empty() {
        return;
    }
    // Collect first, then mutate: rewriting a node's value (or detaching its
    // children) mid-traversal would disturb the `descendants()` iterator — the
    // same discipline the wikilink pass uses.
    let nodes: Vec<&'a AstNode<'a>> = root.descendants().collect();
    for node in nodes {
        // Classify with a short immutable borrow; act with a fresh borrow after,
        // so we never hold the borrow across `node_text`/replacement.
        let kind = match &node.data.borrow().value {
            NodeValue::Image(link) | NodeValue::Link(link) => Kind::Inline(link.url.clone()),
            NodeValue::WikiLink(w) => Kind::Wiki(w.url.clone()),
            NodeValue::Text(t) if t.contains("![[") => Kind::Text(t.to_string()),
            _ => Kind::Skip,
        };
        match kind {
            Kind::Skip => {}
            Kind::Inline(url) => {
                if let Some(new_url) = resolve_relative(&url, source, assets, base_path)
                    && let NodeValue::Image(link) | NodeValue::Link(link) =
                        &mut node.data.borrow_mut().value
                {
                    link.url = new_url;
                }
            }
            Kind::Wiki(target) => {
                // Only claim wikilinks that name an asset; plain note links fall
                // through to the wikilink pass untouched.
                if let Some(id) = resolve_by_name(&target, &source.id_path, assets) {
                    let display = node_text(node);
                    let label = if display.is_empty() { target } else { display };
                    let url = asset_url(&id, base_path);
                    let html = render_anchor(&url, &label, "wikilink");
                    for child in node.children().collect::<Vec<_>>() {
                        child.detach();
                    }
                    node.data.borrow_mut().value = NodeValue::HtmlInline(html);
                }
            }
            Kind::Text(text) => {
                if let Some(html) = render_embeds(&text, source, assets, base_path) {
                    node.data.borrow_mut().value = NodeValue::HtmlInline(html);
                }
            }
        }
    }
}

enum Kind {
    Skip,
    /// A standard markdown `Image`/`Link` `url` to resolve relative to the note.
    Inline(String),
    /// A `[[target]]` wikilink target to resolve by name against the asset set.
    Wiki(String),
    /// A `Text` node that contains at least one `![[` embed to expand.
    Text(String),
}

/// Resolve a relative markdown `url` against the source note's directory; return
/// the rewritten root-relative URL iff it lands on a known asset, preserving any
/// `#fragment`/`?query` suffix. Returns `None` for external/absolute URLs and
/// for relative paths that match no asset (left untouched).
fn resolve_relative(
    url: &str,
    source: &Doc,
    assets: &AssetIndex,
    base_path: &str,
) -> Option<String> {
    let (path_part, suffix) = split_suffix(url);
    if path_part.is_empty() || is_external(path_part) || path_part.starts_with('/') {
        return None;
    }
    let source_dir = source.id_path.parent().unwrap_or(Path::new(""));
    let normalized = normalize(&source_dir.join(path_part))?;
    if assets.by_path.contains(&normalized) {
        Some(format!("{}{}", asset_url(&normalized, base_path), suffix))
    } else {
        None
    }
}

/// Expand every `![[…]]` embed in a text run, escaping the literal segments
/// between them. Returns `None` (leave the `Text` node as-is) when no embed
/// actually resolves — including when an `![[…]]` names no asset.
fn render_embeds(text: &str, source: &Doc, assets: &AssetIndex, base_path: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = text;
    let mut replaced = false;
    while let Some(start) = rest.find("![[") {
        let after = &rest[start + 3..];
        let Some(end) = after.find("]]") else { break };
        let inner = &after[..end];
        let (target, alias) = match inner.split_once('|') {
            Some((t, a)) => (t.trim(), Some(a.trim())),
            None => (inner.trim(), None),
        };
        if let Some(id) = resolve_by_name(target, &source.id_path, assets) {
            out.push_str(&html::escape(&rest[..start]));
            out.push_str(&render_embed(&id, alias.unwrap_or(target), base_path));
            replaced = true;
        } else {
            // Unresolved embed: keep the literal `![[…]]` text verbatim.
            out.push_str(&html::escape(&rest[..start + 3 + end + 2]));
        }
        rest = &after[end + 2..];
    }
    if !replaced {
        return None;
    }
    out.push_str(&html::escape(rest));
    Some(out)
}

/// Render a resolved embed: `<img>` for image extensions, otherwise a download
/// `<a>` (Obsidian embeds non-images as an inline viewer; a link is the faithful
/// static-site analogue).
fn render_embed(id: &Path, label: &str, base_path: &str) -> String {
    let url = asset_url(id, base_path);
    if is_image(id) {
        format!(
            r#"<img class="embed" src="{}" alt="{}">"#,
            html::escape(&url),
            html::escape(label)
        )
    } else {
        render_anchor(&url, label, "embed")
    }
}

fn render_anchor(url: &str, label: &str, class: &str) -> String {
    format!(
        r#"<a class="{}" href="{}">{}</a>"#,
        html::escape(class),
        html::escape(url),
        html::escape(label)
    )
}

/// Resolve a `[[…]]`/`![[…]]` target to an asset by slugified full filename,
/// mirroring wikilink resolution: optional `dir/Name` prefix, then minimum
/// directory distance to the source, then lexicographically smallest path.
fn resolve_by_name(target: &str, source_id: &Path, assets: &AssetIndex) -> Option<PathBuf> {
    let (prefix, name) = split_prefix_stem(target);
    let name_slug = slug::slugify(name);
    if name_slug.is_empty() {
        return None;
    }
    let candidates = assets.by_name.get(&name_slug)?;
    let empty = Path::new("");
    let source_dir = source_id.parent().unwrap_or(empty);
    let mut best: Option<(&PathBuf, usize)> = None;
    for cand in candidates {
        let cand_dir = cand.parent().unwrap_or(empty);
        if let Some(p) = prefix
            && !prefix_matches(cand_dir, p)
        {
            continue;
        }
        let dist = dir_distance(source_dir, cand_dir);
        best = match best {
            None => Some((cand, dist)),
            Some((curr, curr_dist)) => {
                if dist < curr_dist || (dist == curr_dist && cand < curr) {
                    Some((cand, dist))
                } else {
                    Some((curr, curr_dist))
                }
            }
        };
    }
    best.map(|(cand, _)| cand.clone())
}

/// The mirrored output URL for an asset: `base_path` + `/` + its `content/`-
/// relative path, with `/` separators on every platform. Root-relative so it is
/// correct regardless of the referencing page's permalink.
fn asset_url(id_path: &Path, base_path: &str) -> String {
    let rel = id_path
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("{}/{}", base_path.trim_end_matches('/'), rel)
}

/// Split a URL into its path portion and a trailing `#fragment`/`?query` suffix
/// (whichever comes first), so the suffix can be preserved across a rewrite.
fn split_suffix(url: &str) -> (&str, &str) {
    match url.find(['#', '?']) {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    }
}

/// True for URLs we must never rewrite: those carrying a scheme (`https:`,
/// `mailto:`, `data:`, …) or protocol-relative (`//host/…`).
fn is_external(url: &str) -> bool {
    if url.starts_with("//") {
        return true;
    }
    // A `:` before the first `/` means a scheme (`mailto:x`, `https://…`).
    match (url.find(':'), url.find('/')) {
        (Some(colon), Some(slash)) => colon < slash,
        (Some(_), None) => true,
        _ => false,
    }
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Normalize `.`/`..` components without touching the filesystem. Returns `None`
/// if the path escapes the vault root (a leading `..`) or is rooted/absolute.
fn normalize(p: &Path) -> Option<PathBuf> {
    let mut out: Vec<&std::ffi::OsStr> = Vec::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop()?;
            }
            Component::Normal(s) => out.push(s),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out.iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_doc(id_path: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id_path),
            output_path: PathBuf::from(id_path).with_extension("html"),
            ..Default::default()
        }
    }

    fn source_with_output(id_path: &str, output_path: &str) -> Doc {
        Doc {
            id_path: PathBuf::from(id_path),
            output_path: PathBuf::from(output_path),
            ..Default::default()
        }
    }

    /// Parse `body`, run the media pass with an asset set, and render to HTML —
    /// the same path `markup::render` drives.
    fn render_md(body: &str, source: &Doc, assets: &[&str], base_path: &str) -> String {
        let arena = comrak::Arena::new();
        let mut options = comrak::Options::default();
        options.render.r#unsafe = true;
        options.extension.wikilinks_title_after_pipe = true;
        let root = comrak::parse_document(&arena, body, &options);
        let index = AssetIndex::build(&assets.iter().map(PathBuf::from).collect::<Vec<_>>());
        resolve_in_ast(root, source, &index, base_path);
        let mut out = String::new();
        comrak::format_html(root, &options, &mut out).unwrap();
        out
    }

    #[test]
    fn rewrites_co_located_markdown_image() {
        let source = source_doc("blog/post.md");
        let out = render_md("![diagram](image.png)", &source, &["blog/image.png"], "");
        assert!(out.contains(r#"src="/blog/image.png""#), "got: {out}");
        assert!(out.contains(r#"alt="diagram""#), "got: {out}");
    }

    #[test]
    fn relocating_permalink_still_resolves_to_mirrored_asset() {
        // The headline case: even though the page lands at blog/post/index.html,
        // the root-relative asset URL points at the mirrored file, not a path
        // relative to the deeper page dir.
        let source = source_with_output("blog/post.md", "blog/post/index.html");
        let out = render_md("![](image.png)", &source, &["blog/image.png"], "");
        assert!(out.contains(r#"src="/blog/image.png""#), "got: {out}");
    }

    #[test]
    fn rewrites_markdown_link_to_asset() {
        let source = source_doc("notes/n.md");
        let out = render_md(
            "[the report](report.pdf)",
            &source,
            &["notes/report.pdf"],
            "",
        );
        assert!(out.contains(r#"href="/notes/report.pdf""#), "got: {out}");
        assert!(out.contains(">the report<"), "got: {out}");
    }

    #[test]
    fn normalizes_parent_dir_in_relative_ref() {
        let source = source_doc("blog/2025/post.md");
        let out = render_md("![](../img/x.png)", &source, &["blog/img/x.png"], "");
        assert!(out.contains(r#"src="/blog/img/x.png""#), "got: {out}");
    }

    #[test]
    fn obsidian_embed_image_becomes_img() {
        let source = source_doc("blog/post.md");
        let out = render_md("![[diagram.png]]", &source, &["blog/diagram.png"], "");
        assert!(
            out.contains(r#"<img class="embed" src="/blog/diagram.png" alt="diagram.png">"#),
            "got: {out}"
        );
    }

    #[test]
    fn obsidian_embed_uses_alias_as_alt() {
        let source = source_doc("blog/post.md");
        let out = render_md(
            "![[diagram.png|A diagram]]",
            &source,
            &["blog/diagram.png"],
            "",
        );
        assert!(out.contains(r#"alt="A diagram""#), "got: {out}");
    }

    #[test]
    fn obsidian_embed_non_image_becomes_link() {
        let source = source_doc("notes/n.md");
        let out = render_md("![[report.pdf]]", &source, &["notes/report.pdf"], "");
        assert!(
            out.contains(r#"<a class="embed" href="/notes/report.pdf">report.pdf</a>"#),
            "got: {out}"
        );
    }

    #[test]
    fn embed_preserves_surrounding_text() {
        let source = source_doc("a.md");
        let out = render_md("before ![[x.png]] after", &source, &["x.png"], "");
        assert!(out.contains("before "), "got: {out}");
        assert!(out.contains("after"), "got: {out}");
        assert!(out.contains(r#"<img class="embed" src="/x.png""#), "got: {out}");
    }

    #[test]
    fn attachment_wikilink_becomes_link() {
        let source = source_doc("notes/n.md");
        let out = render_md("see [[report.pdf]]", &source, &["notes/report.pdf"], "");
        assert!(
            out.contains(r#"<a class="wikilink" href="/notes/report.pdf">report.pdf</a>"#),
            "got: {out}"
        );
    }

    #[test]
    fn attachment_wikilink_uses_alias_label() {
        let source = source_doc("notes/n.md");
        let out = render_md(
            "[[report.pdf|the report]]",
            &source,
            &["notes/report.pdf"],
            "",
        );
        assert!(out.contains(r#"href="/notes/report.pdf""#), "got: {out}");
        assert!(out.contains(">the report<"), "got: {out}");
    }

    #[test]
    fn base_path_is_prepended() {
        let source = source_doc("blog/post.md");
        let out = render_md("![](image.png)", &source, &["blog/image.png"], "/garden");
        assert!(
            out.contains(r#"src="/garden/blog/image.png""#),
            "got: {out}"
        );
    }

    #[test]
    fn external_url_left_untouched() {
        let source = source_doc("a.md");
        let out = render_md("![](https://example.com/x.png)", &source, &["x.png"], "");
        assert!(
            out.contains(r#"src="https://example.com/x.png""#),
            "got: {out}"
        );
    }

    #[test]
    fn root_absolute_url_left_untouched() {
        let source = source_doc("blog/post.md");
        // /image.png is author-pinned even though blog/image.png is an asset.
        let out = render_md("![](/image.png)", &source, &["blog/image.png"], "");
        assert!(out.contains(r#"src="/image.png""#), "got: {out}");
    }

    #[test]
    fn unresolved_relative_ref_left_untouched() {
        let source = source_doc("a.md");
        // logo.png is not an asset (maybe a hand-placed static/ file): leave it.
        let out = render_md("![](logo.png)", &source, &["other.png"], "");
        assert!(out.contains(r#"src="logo.png""#), "got: {out}");
    }

    #[test]
    fn unresolved_embed_kept_literal() {
        let source = source_doc("a.md");
        let out = render_md("![[missing.png]]", &source, &["other.png"], "");
        assert!(out.contains("![[missing.png]]"), "got: {out}");
        assert!(!out.contains("<img"), "got: {out}");
    }

    #[test]
    fn fragment_suffix_preserved() {
        let source = source_doc("a.md");
        let out = render_md("[doc](report.pdf#page=2)", &source, &["report.pdf"], "");
        assert!(out.contains(r#"href="/report.pdf#page=2""#), "got: {out}");
    }

    #[test]
    fn embed_in_code_fence_stays_literal() {
        // Text nodes never occur inside code, so the embed scan can't touch it.
        let source = source_doc("a.md");
        let out = render_md("```\n![[x.png]]\n```", &source, &["x.png"], "");
        assert!(!out.contains("<img"), "got: {out}");
        assert!(out.contains("![[x.png]]"), "got: {out}");
    }

    #[test]
    fn resolve_by_name_prefers_nearest_then_lexical() {
        let source_id = PathBuf::from("blog/2025/post.md");
        let index = AssetIndex::build(&[
            PathBuf::from("reference/x.png"),
            PathBuf::from("blog/x.png"),
        ]);
        let hit = resolve_by_name("x.png", &source_id, &index).unwrap();
        assert_eq!(hit, PathBuf::from("blog/x.png"));
    }

    #[test]
    fn resolve_by_name_honors_dir_prefix() {
        let source_id = PathBuf::from("a.md");
        let index = AssetIndex::build(&[PathBuf::from("img/x.png"), PathBuf::from("other/x.png")]);
        let hit = resolve_by_name("img/x.png", &source_id, &index).unwrap();
        assert_eq!(hit, PathBuf::from("img/x.png"));
    }

    #[test]
    fn is_external_classifies_schemes_and_protocol_relative() {
        assert!(is_external("https://x/y.png"));
        assert!(is_external("mailto:a@b.com"));
        assert!(is_external("//cdn/x.png"));
        assert!(!is_external("img/x.png"));
        assert!(!is_external("x.png"));
    }
}
