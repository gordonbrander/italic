use crate::html;
use crate::slug;
use comrak::Arena;
use comrak::nodes::{Ast, AstNode, LineColumn, NodeHtmlBlock, NodeValue};
use std::cell::RefCell;
use std::collections::HashSet;

/// Strip Obsidian block-id markers (`^blockid`) from a parsed comrak AST and
/// plant an anchor in their place, so `[[Note#^blockid]]` has somewhere to land.
/// Two marker positions, mirroring Obsidian:
///
/// - **Trailing** — `^blockid` ending a paragraph or heading (a list item's
///   content is a paragraph, so items are covered). The marker is stripped and
///   the block gains a trailing `<span class="block-anchor" id="…"></span>`.
/// - **Standalone** — a paragraph consisting of nothing but `^blockid`, which
///   tags the block *above* it. The only way to reference a table, code fence, or
///   blockquote, none of which have anywhere to put a trailing marker. The
///   marker paragraph is removed and the anchor is inserted **before** its
///   previous sibling.
///
/// Both anchors render verbatim because the markup env sets `render.unsafe_`.
/// A standalone anchor goes *before* the block rather than replacing the marker
/// after it: the scroll target is the whole point, and an anchor below a long
/// table lands the reader at its bottom edge. The trailing form appends inside
/// its block instead, which is fine precisely because a paragraph, heading, or
/// list item is short enough that the anchor's position within it is invisible.
///
/// The id is the marker body run through [`slug::slugify`] — the same slugifier
/// [`super::wikilink`] applies to a link fragment. `slugify` drops the `^`, so
/// `[[Note#^Abc-1]]` already emits `href="…#abc-1"` with no special case, and
/// this pass only has to make the anchor agree. Slugifying also makes block ids
/// case-insensitive, as in Obsidian.
///
/// Markers left literal, all quiet failures in the spirit of `super::media`'s
/// `![[missing.png]]`:
/// - one not preceded by whitespace (`page^abc`), or not ending its block;
/// - a standalone marker with no previous sibling — there is nothing to tag;
/// - a lazy continuation, so `> quoted` immediately followed by `^q1` (no blank
///   line) keeps `^q1` inside the quote's paragraph, where it is neither
///   trailing-after-whitespace nor a standalone paragraph.
///
/// Scanning `Text` nodes rather than the raw source makes the pass code-aware
/// for free: a `^abc` inside a fence or code span never reaches a `Text` node.
///
/// One parse-level trap the pass cannot see, documented in
/// `docs/guides/wikilinks.md`: a marker on the line directly after a **table**
/// is swallowed by GFM as another table row. Tables need a blank line before
/// the marker; fences don't care.
///
/// Block ids share the anchor namespace with heading slugs, so a `^overview`
/// marker collides with an `## Overview` heading. A duplicate id within one
/// document anchors the first block only.
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
        // Standalone first: a paragraph that is nothing but a marker can't also
        // carry a trailing one, and `split_marker` would reject it anyway.
        if let Some(body) = standalone_marker(block) {
            let Some(target) = block.previous_sibling() else {
                // Nothing above to tag — leave the marker literal.
                continue;
            };
            block.detach();
            if let Some(id) = claim(&mut seen, &body) {
                target.insert_before(new_node(
                    arena,
                    NodeValue::HtmlBlock(NodeHtmlBlock {
                        block_type: 0,
                        literal: anchor_html(&id),
                    }),
                ));
            }
            continue;
        }

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

        if let Some(id) = claim(&mut seen, body) {
            block.append(new_node(arena, NodeValue::HtmlInline(anchor_html(&id))));
        }
    }
}

/// Slugify a marker body and claim the id for this document, or `None` if some
/// earlier block already took it. Either way the caller has already stripped the
/// marker text — a duplicate loses its anchor, not its cleanup.
fn claim(seen: &mut HashSet<String>, body: &str) -> Option<String> {
    let id = slug::slugify(body);
    if !seen.insert(id.clone()) {
        eprintln!("duplicate block id ^{id}: anchored the first block only");
        return None;
    }
    Some(id)
}

fn anchor_html(id: &str) -> String {
    format!(
        r#"<span class="block-anchor" id="{}"></span>"#,
        html::escape(id)
    )
}

fn new_node<'a>(arena: &'a Arena<'a>, value: NodeValue) -> &'a AstNode<'a> {
    arena.alloc(AstNode::new(RefCell::new(Ast::new(
        value,
        LineColumn { line: 0, column: 0 },
    ))))
}

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

/// The marker body of a paragraph that consists of *nothing but* `^blockid` —
/// Obsidian's way of tagging the block above. Emptiness is checked on the node,
/// not the text: `*em*\n^abc` also ends in a `Text` of `"^abc"`, but it has
/// siblings, and must stay literal.
fn standalone_marker<'a>(block: &'a AstNode<'a>) -> Option<String> {
    if !matches!(block.data.borrow().value, NodeValue::Paragraph) {
        return None;
    }
    let only = block.first_child()?;
    if only.next_sibling().is_some() {
        return None;
    }
    let text = match &only.data.borrow().value {
        NodeValue::Text(t) => t.clone(),
        _ => return None,
    };
    let body = text.trim().strip_prefix('^')?;
    if body.is_empty() || !body.chars().all(is_id_char) {
        return None;
    }
    Some(body.to_string())
}

/// Split a trailing `^blockid` marker off a block's final text run:
/// `"A claim. ^abc-1"` → `Some(("A claim.", "abc-1"))`. `None` when the run
/// doesn't end in a marker, when the `^` isn't preceded by whitespace
/// (`page^abc`), or when the caret opens the run — the last of which is what
/// keeps `*em*\n^abc` literal, since [`standalone_marker`] has already declined
/// that paragraph.
fn split_marker(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim_end();
    let caret = trimmed.rfind('^')?;
    let body = &trimmed[caret + 1..];
    if body.is_empty() || !body.chars().all(is_id_char) {
        return None;
    }
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
        options.extension.table = true;
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
    fn standalone_marker_anchors_a_table() {
        // A blank line is required: GFM otherwise reads the marker as another
        // table row. See `table_without_blank_line_is_a_row`.
        let html = render_md("| a |\n| - |\n| 1 |\n\n^table1");
        let anchor = r#"<span class="block-anchor" id="table1"></span>"#;
        assert!(html.contains(anchor), "got: {html}");
        // The anchor must precede the table — an anchor below it would scroll
        // the reader to the table's bottom edge.
        assert!(
            html.find(anchor).unwrap() < html.find("<table>").unwrap(),
            "anchor must come before the table: {html}"
        );
        assert!(!html.contains('^'));
    }

    #[test]
    fn standalone_marker_anchors_a_code_fence() {
        // Fences need no blank line — the marker line closes nothing.
        let html = render_md("```\ncode\n```\n^fence1");
        let anchor = r#"<span class="block-anchor" id="fence1"></span>"#;
        assert!(html.contains(anchor), "got: {html}");
        assert!(html.find(anchor).unwrap() < html.find("<pre>").unwrap());
        assert!(html.contains("code"));
    }

    #[test]
    fn standalone_marker_anchors_a_blockquote() {
        let html = render_md("> quoted\n\n^q1");
        let anchor = r#"<span class="block-anchor" id="q1"></span>"#;
        assert!(html.contains(anchor), "got: {html}");
        assert!(html.find(anchor).unwrap() < html.find("<blockquote>").unwrap());
    }

    #[test]
    fn standalone_marker_anchors_a_paragraph() {
        let html = render_md("Some claim.\n\n^p1");
        assert!(html.contains(r#"<span class="block-anchor" id="p1"></span>"#));
        assert_eq!(html.matches("<p>").count(), 1, "marker <p> removed: {html}");
    }

    #[test]
    fn standalone_marker_with_nothing_above_stays_literal() {
        // First node in the document — there is no block to tag.
        let html = render_md("^orphan\n\ntext");
        assert!(html.contains("<p>^orphan</p>"), "got: {html}");
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn soft_break_before_marker_stays_literal() {
        // `Paragraph(Emph, SoftBreak, Text("^abc"))` — the marker text is the
        // last child but the paragraph is not *only* the marker. This is the
        // regression the node-level only-child check exists to prevent.
        let html = render_md("*em*\n^abc");
        assert!(html.contains("^abc"), "got: {html}");
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn table_without_blank_line_is_a_row() {
        // Not a behavior we choose — GFM swallows the marker line as a row
        // before the pass ever sees it. Pinned so the docs stay honest.
        let html = render_md("| a |\n| - |\n| 1 |\n^table1");
        assert!(html.contains("^table1"), "got: {html}");
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn lazy_continuation_into_blockquote_stays_literal() {
        // No blank line: `^q1` folds into the quote's paragraph after a
        // SoftBreak, so it is neither standalone nor trailing-after-whitespace.
        let html = render_md("> quoted\n^q1");
        assert!(html.contains("^q1"), "got: {html}");
        assert!(!html.contains("block-anchor"));
    }

    #[test]
    fn standalone_marker_after_a_list_keeps_it_tight() {
        // The blank line ends the list rather than loosening it, so items stay
        // `<li>a</li>` and the whole list gets the anchor.
        let html = render_md("- a\n- b\n\n^list1");
        assert!(html.contains("<li>a</li>"), "list went loose: {html}");
        let anchor = r#"<span class="block-anchor" id="list1"></span>"#;
        assert!(html.find(anchor).unwrap() < html.find("<ul>").unwrap());
    }

    #[test]
    fn standalone_marker_is_slugified_like_a_trailing_one() {
        let html = render_md("Some claim.\n\n^Cap-1");
        assert!(html.contains(r#"id="cap-1""#), "got: {html}");
    }

    #[test]
    fn duplicate_across_both_forms_anchors_the_first() {
        let html = render_md("Trailing. ^dup\n\nSome claim.\n\n^dup");
        assert_eq!(html.matches("block-anchor").count(), 1, "got: {html}");
        // The losing marker is still stripped, not left as visible text.
        assert!(!html.contains('^'), "got: {html}");
    }

    #[test]
    fn consecutive_standalone_markers_both_anchor_the_block_above() {
        let html = render_md("Some claim.\n\n^a\n\n^b");
        assert!(html.contains(r#"id="a""#) && html.contains(r#"id="b""#));
        assert_eq!(
            html.matches("<p>").count(),
            1,
            "both markers removed: {html}"
        );
        assert!(!html.contains('^'));
    }

    #[test]
    fn standalone_marker_rejects_non_id_characters() {
        let html = render_md("Some claim.\n\n^ab_c");
        assert!(html.contains("^ab_c"), "got: {html}");
        assert!(!html.contains("block-anchor"));
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
