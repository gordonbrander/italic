use crate::html;
use crate::slug;
use comrak::Arena;
use comrak::nodes::{Ast, AstNode, LineColumn, NodeValue};
use std::cell::RefCell;
use std::collections::HashSet;

/// Strip Obsidian block-id markers (`^blockid`) from a parsed comrak AST and
/// plant an anchor in their place, so `[[Note#^blockid]]` has somewhere to land.
///
/// A marker is a `^` at a word boundary followed by a run of `[A-Za-z0-9-]`,
/// sitting at the very end of a paragraph or heading. The marker is removed from
/// the text and the block gains a trailing
/// `<span class="block-anchor" id="…"></span>`, rendered verbatim because the
/// markup env sets `render.unsafe_`.
///
/// The id is the marker body run through [`slug::slugify`] — the same slugifier
/// [`super::wikilink`] applies to a link fragment. `slugify` drops the `^`, so
/// `[[Note#^Abc-1]]` already emits `href="…#abc-1"` with no special case, and
/// this pass only has to make the anchor agree. Slugifying also makes block ids
/// case-insensitive, as in Obsidian.
///
/// Two deliberate limits, both documented in `docs/guides/migration.md`:
/// - Only a *trailing* marker is recognized. Obsidian also allows one on its own
///   line after a code fence, table, or blockquote; such a line stays literal.
/// - Block ids share the anchor namespace with heading slugs, so a `^overview`
///   marker and an `## Overview` heading collide.
///
/// A marker that isn't preceded by whitespace (`page^abc`) or that stands alone
/// as a whole paragraph is left literal — the quiet failure `super::media` chose
/// for `![[missing.png]]`. Scanning `Text` nodes rather than the raw source
/// makes the pass code-aware for free: a `^abc` inside a fence or code span
/// never reaches a `Text` node.
pub fn resolve_in_ast<'a>(arena: &'a Arena<'a>, root: &'a AstNode<'a>) {
    // Collect first, then mutate: detaching a node's children mid-traversal
    // would disturb the `descendants()` iterator — the discipline every pass in
    // this module shares.
    let blocks: Vec<&'a AstNode<'a>> = root
        .descendants()
        .filter(|node| {
            matches!(
                node.data.borrow().value,
                NodeValue::Paragraph | NodeValue::Heading(_)
            )
        })
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    for block in blocks {
        // A list item wraps its content in a Paragraph, so `- item ^xyz` needs
        // no `Item` special case. Appending an *inline* node (not a block
        // sibling) keeps the output valid inside tight lists, where comrak
        // renders `<li>` with no wrapping `<p>`.
        let Some(last) = block.last_child() else {
            continue;
        };
        let text = match &last.data.borrow().value {
            NodeValue::Text(t) => t.clone(),
            _ => continue,
        };
        let Some((rest, body)) = split_marker(&text) else {
            continue;
        };

        // Strip the marker whether or not it earns an anchor: a duplicate id is
        // still a marker, and leaving it as visible text is the bug this pass
        // exists to fix.
        if rest.is_empty() {
            last.detach();
        } else {
            last.data.borrow_mut().value = NodeValue::Text(rest.to_string().into());
        }

        let id = slug::slugify(body);
        if !seen.insert(id.clone()) {
            eprintln!("duplicate block id ^{id}: anchored the first block only");
            continue;
        }
        let anchor = format!(
            r#"<span class="block-anchor" id="{}"></span>"#,
            html::escape(&id)
        );
        block.append(arena.alloc(AstNode::new(RefCell::new(Ast::new(
            NodeValue::HtmlInline(anchor),
            LineColumn { line: 0, column: 0 },
        )))));
    }
}

/// Split a trailing `^blockid` marker off a block's final text run:
/// `"A claim. ^abc-1"` → `Some(("A claim.", "abc-1"))`. `None` when the run
/// doesn't end in a marker, when the `^` isn't preceded by whitespace
/// (`page^abc`), or when the marker is the whole run (a lone `^abc` paragraph).
fn split_marker(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim_end();
    let caret = trimmed.rfind('^')?;
    let body = &trimmed[caret + 1..];
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    // `next_back()` is `None` when the caret opens the run, which is what keeps a
    // standalone `^abc` paragraph literal.
    if !trimmed[..caret].chars().next_back()?.is_whitespace() {
        return None;
    }
    Some((trimmed[..caret].trim_end(), body))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse `body` as Markdown, run the block-id pass, and render to HTML —
    /// the same parse→strip→render path `markup::render` drives.
    fn render_md(body: &str) -> String {
        let arena = comrak::Arena::new();
        let mut options = comrak::Options::default();
        options.render.r#unsafe = true;
        let root = comrak::parse_document(&arena, body, &options);
        resolve_in_ast(&arena, root);
        let mut out = String::new();
        comrak::format_html(root, &options, &mut out).unwrap();
        out
    }

    #[test]
    fn anchors_a_paragraph() {
        let html = render_md("A claim worth citing. ^abc123");
        assert_eq!(
            html,
            "<p>A claim worth citing.<span class=\"block-anchor\" id=\"abc123\"></span></p>\n"
        );
    }

    #[test]
    fn anchors_a_heading() {
        let html = render_md("## Some Heading ^intro");
        assert!(html.contains(r#"<span class="block-anchor" id="intro"></span></h2>"#));
        assert!(!html.contains('^'));
    }

    #[test]
    fn anchors_a_list_item() {
        let html = render_md("- a list item ^xyz789\n- plain item");
        assert!(html.contains(r#"a list item<span class="block-anchor" id="xyz789"></span>"#));
        assert!(html.contains("<li>plain item</li>"));
    }

    #[test]
    fn slugifies_the_id() {
        // Lowercased and dash-preserving, so `[[Note#^Abc-1]]` (which slugifies
        // to `abc-1` in the wikilink pass) finds this anchor.
        let html = render_md("Claim. ^Abc-1");
        assert!(html.contains(r#"id="abc-1""#));
    }

    #[test]
    fn anchors_after_trailing_inline_markup() {
        let html = render_md("A *stressed* claim. ^m1");
        assert!(html.contains(r#"claim.<span class="block-anchor" id="m1"></span>"#));
    }

    #[test]
    fn empty_run_detaches_rather_than_leaving_blank_text() {
        let html = render_md("*only emphasis* ^m2");
        assert_eq!(
            html,
            "<p><em>only emphasis</em><span class=\"block-anchor\" id=\"m2\"></span></p>\n"
        );
    }

    #[test]
    fn code_fence_stays_literal() {
        let html = render_md("```\ncode ^abc\n```");
        assert!(html.contains("code ^abc"));
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn inline_code_stays_literal() {
        let html = render_md("use `x ^abc` here");
        assert!(html.contains("x ^abc"));
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn lone_marker_paragraph_stays_literal() {
        // The standalone-line form Obsidian uses for tables and fences is not
        // supported; it must not silently eat the paragraph.
        let html = render_md("^abc123");
        assert_eq!(html, "<p>^abc123</p>\n");
    }

    #[test]
    fn requires_a_word_boundary() {
        let html = render_md("see page^abc for more");
        assert!(html.contains("page^abc"));
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn marker_must_be_last() {
        let html = render_md("a ^abc trailing words");
        assert!(html.contains("^abc trailing words"));
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn rejects_non_id_characters() {
        let html = render_md("caret at end ^");
        assert!(html.contains('^'));
        assert!(!html.contains("block-anchor"));
        let html = render_md("exponent 2^n");
        assert!(html.contains("2^n"));
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn duplicate_id_anchors_the_first_block_only() {
        let html = render_md("first. ^dup\n\nsecond. ^dup");
        assert_eq!(html.matches("block-anchor").count(), 1);
        // Both markers are stripped even though only one anchor is planted.
        assert!(!html.contains('^'));
        assert!(html.contains("<p>second.</p>"));
    }

    #[test]
    fn splits_marker() {
        assert_eq!(split_marker("A claim. ^abc-1"), Some(("A claim.", "abc-1")));
        assert_eq!(split_marker("trailing ws ^m  "), Some(("trailing ws", "m")));
        assert_eq!(split_marker("^lone"), None);
        assert_eq!(split_marker("page^abc"), None);
        assert_eq!(split_marker("no marker"), None);
        assert_eq!(split_marker("bad ^ab_c"), None);
    }
}
