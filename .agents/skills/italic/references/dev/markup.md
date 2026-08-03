# Extending Comrak

Italic's Markdown flavor is [Comrak](https://github.com/kivikakk/comrak) plus a
handful of features it doesn't ship: wikilinks, block references, co-located
media, embeds, hashtags. This page explains how those are built. The code lives
in `src/build/markup.rs` (the orchestrator) and one module per feature under
`src/build/markup/`.

## Everything is a pass over the AST

The organizing idea, and the only one you really need: **every custom feature is
a pass over Comrak's parsed AST.** Nothing pre-processes the raw Markdown source,
nothing post-processes the rendered HTML, and there are no custom parse hooks or
broken-link callbacks. A pass takes the root node, walks it, mutates it, and
returns.

This is worth being stubborn about, because it buys three things:

**Code-awareness for free.** `[[foo]]` inside a fence never becomes a `WikiLink`
node, and `^abc` inside a code span never reaches a `Text` node. A string scanner
over the source would need a special case for every construct that quotes its
contents; a pass over the AST needs none. `wikilink.rs` pins this with
`wikilink_inside_code_fence_is_not_linked`.

**Structure, not text.** Passes see `Paragraph`, `Heading`, `Text`, so they can
reason about block boundaries. This is what makes a standalone `^blockid` marker
— a paragraph that tags the block *above* it — implementable at all.

**One parse, one render.** `parse_document` once into an arena, run the passes
over that tree, `format_html_with_plugins` once.

A corollary worth stating: there are **no regexes** in the subsystem, and the
`regex` crate is not a dependency. What little raw scanning remains happens
inside a single `Text` node, where a hand-written char walk is both clearer and
cheaper — `hashtag::scan`, `block_id::split_marker`, `media::render_embeds`.

## Where Comrak is configured

All of it in `markup_options()` in `src/tera_env.rs`. Two settings shape the
design more than the rest:

- `extension.wikilinks_title_after_pipe` — Comrak tokenizes `[[…]]` into a
  `NodeValue::WikiLink` for us. Italic never *parses* wikilink syntax; it only
  *resolves* the nodes Comrak hands it. (The `!` embed form, `![[img.png]]`, is
  not tokenized — see the media pass below.)
- `extension.header_id_prefix = Some(String::new())` — **heading anchors are
  entirely Comrak's built-in feature.** There is no custom heading-anchor pass.
  The empty prefix means the emitted `id` is the bare slug rather than
  `user-content-…`, so a `[[Note#Heading]]` fragment lands on it.

Also load-bearing: `render.r#unsafe = true`. Passes emit their output by swapping
a node's value for `NodeValue::HtmlInline` or `HtmlBlock`. Without raw-HTML
passthrough, none of it renders. The rest of the options — `table`, `footnotes`,
`autolink`, `alerts`, `math_dollars`, and friends — are plain runtime toggles
with no Italic code behind them. Syntax highlighting is a Comrak plugin
(`SyntectAdapter`), built once process-wide and shared.

## The pipeline

The body of `markup::render`, for `DocKind::Markdown`:

```rust
let arena = comrak::Arena::new();
let root = comrak::parse_document(&arena, &rendered, &env.options);
block_id::resolve_in_ast(&arena, root);
media::resolve_in_ast(root, doc, &env.asset_index, &env.base_path);
doc.links = wikilink::resolve_in_ast(root, doc, &env.stem_index);
if env.hashtags { /* hashtag::extract_in_ast(root) → doc.terms */ }
comrak::format_html_with_plugins(root, &env.options, &mut out, &plugins)?;
```

(Tera runs over the body string *before* this, so shortcodes are expanded by the
time Comrak sees the source.)

**Pass order is load-bearing**, for two reasons recorded in the source:

- `block_id` before `media`, because media collapses a whole `Text` node
  containing `![[` into one `HtmlInline` — which would swallow the trailing
  marker in `![[img.png]] ^abc`.
- `media` before `wikilink`, so media can claim the `[[…]]` references that name
  an asset (`[[report.pdf]]`) and leave plain note links for the wikilink pass.

## Collect first, then mutate

Every pass shares one discipline. Gather the target nodes into a `Vec`, *then*
mutate — detaching children or rewriting values mid-traversal would disturb the
`descendants()` iterator.

```rust
// Collect first, then mutate: detaching a node's children mid-traversal
// would disturb the `descendants()` iterator.
let wikilinks: Vec<&'a AstNode<'a>> = root
    .descendants()
    .filter(|node| matches!(node.data.borrow().value, NodeValue::WikiLink(_)))
    .collect();
```

Node values live behind a `RefCell` (`node.data.borrow()` /
`.borrow_mut().value`), so the second half of the discipline is keeping borrows
short. Where a pass needs to both inspect a node and call something that borrows
it again, it classifies with one borrow and acts with a fresh one — see the
`Kind` enum in `media.rs`, which exists so the pass never holds a borrow across
`node_text(node)`.

## Three ways to mutate

- **Rewrite in place.** Mutate the node's value and let Comrak render it as it
  normally would. `media` does this for standard `![](x.png)` / `[l](f.pdf)`
  references: it edits `link.url` and the `<img>` / `<a>` falls out for free.
- **Collapse to raw HTML.** Detach the node's children and swap its value for
  `HtmlInline`. This is the common case — `wikilink::replace_node_html` is the
  canonical form, and `media` reuses the shape for embeds and attachment links.
- **Insert new nodes.** Requires the arena, which is why `block_id`'s entry point
  is `resolve_in_ast(arena, root)` while the others take only `root`. Nodes are
  built through a small helper:

```rust
fn new_node<'a>(arena: &'a Arena<'a>, value: NodeValue) -> &'a AstNode<'a> {
    arena.alloc(AstNode::new(RefCell::new(Ast::new(
        value,
        LineColumn { line: 0, column: 0 },
    ))))
}
```

## The passes at a glance

| Feature | File | Matches | Mechanism | Emits |
| --- | --- | --- | --- | --- |
| Wikilinks | `wikilink.rs` | `WikiLink` node | collapse to HTML | `<a class="wikilink">` / `<span class="nolink">` |
| Block ids | `block_id.rs` | `Text` in `Paragraph`/`Heading` | strip text, insert node | `<span class="block-anchor" id="…">` |
| Media refs | `media.rs` | `Image` / `Link` node | rewrite `link.url` | unchanged `<img>` / `<a>` |
| Attachments | `media.rs` | `WikiLink` naming an asset | collapse to HTML | `<a class="wikilink">` |
| Embeds | `media.rs` | `Text` containing `![[` | collapse to HTML | `<img class="embed">` / `<a class="embed">` |
| Hashtags | `hashtag.rs` | `Text` node | rewrite `Text` (strip) | nothing — tags are harvested |
| Heading anchors | *none* | — | Comrak option | Comrak's own `id` |

That last row is the informative one. Before adding a pass, check whether Comrak
already does it.

## Passes can harvest, not just transform

Two passes return data alongside their edits. `wikilink::resolve_in_ast` returns
the deduplicated `id_path`s it resolved, which the caller assigns to `doc.links`
— that is the input to the backlink graph. `hashtag::extract_in_ast` returns the
tag texts it stripped, which land in `doc.terms`. This is the markup phase's only
cross-doc output; everything else a pass does is confined to one document's tree.

## Shared helpers

Resolution logic is shared rather than duplicated. `wikilink.rs` owns
`node_text`, `split_prefix_stem`, `dir_distance`, and `prefix_matches`, and
`media.rs` imports all four — so `![[diagram.png]]` and `[[Some Note]]` break ties
by the same nearest-directory-then-lexicographic rule.

Two helpers matter to any new pass:

- `slug::slugify` (`src/slug.rs`) is the **canonical** slugifier — permalinks,
  taxonomy terms, wikilink stems, block ids, heading fragments. It is a std-only
  port of Comrak's `Anchorizer` character rules, and a contract test renders a
  heading through real Comrak and asserts the two agree. That test is what makes
  `[[Note#Heading]]` reliably land on the `id` Comrak emitted. Block references
  need no slugifier code of their own: `slugify` drops the `^`, so
  `[[Note#^Abc-1]]` already produces `href="…#abc-1"`.
- `html::escape` (`src/html.rs`) escapes the five XML characters. Because passes
  emit raw HTML into an `unsafe`-rendering document, **every interpolated value
  goes through it** — no exceptions. `display_does_not_emit_raw_html` in
  `wikilink.rs` guards the obvious hole.

## Concurrency

`markup::run` renders docs in parallel with Rayon, cloning the markup env once
per worker (`try_for_each_init`). A pass therefore sees exactly one document's
arena and must not reach for global mutable state. State that a pass needs across
a whole document — `block_id`'s set of claimed ids, for instance — lives as a
local in the pass, scoped to that one call.

## Failure is quiet, by design

The rest of Italic fails loudly on a bad reference (see
[Contributing](../contributing.md)). Markup passes are the exception: an
unresolved `![[missing.png]]` stays literal, a `^orphan` marker with nothing
above it stays literal, an unresolved `[[Note]]` renders as a `nolink` span.
Authors write prose containing carets and brackets, and a build that dies on a
stray `2^n` would be unusable. When a pass declines to transform something, it
leaves the source text alone rather than guessing.

## Testing

Two levels, as everywhere in the repo (details in
[Contributing](../contributing.md)). Each pass has an inline `#[cfg(test)] mod
tests` with a local `render_md` helper that runs parse → pass → format and
asserts on the HTML; these cover the transform logic and its edge cases. The
end-to-end behavior is pinned by fixtures — `09_wikilinks`, `28_heading_links`,
`29_block_references` — which build a whole mini-site and diff the output tree
byte-for-byte.

Some tests pin behavior Italic doesn't choose. `table_without_blank_line_is_a_row`
in `block_id.rs` records that GFM swallows a `^marker` on the line after a table
before any pass can see it. Pinning it keeps the docs honest.
