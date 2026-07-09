# Wikilinks & backlinks

Wikilinks are the connective tissue of a digital garden: link notes by title,
and italic resolves the target, builds the backlink graph, and feeds the
[related-pages](related.md) engine.

## Syntax

```markdown
[[Page Title]]
[[Page Title|Display text]]
[[reference/Glossary]]          # path-prefixed form
[[Page Title#Some Heading]]     # heading reference
[[Page Title#^abc123]]          # block reference
```

- `[[Page Title]]` links to the page whose filename stem slugifies to
  `page-title`, displaying the authored text.
- `[[Page Title|Display text]]` links the same way but shows `Display text`.
- `[[dir/sub/Name]]` restricts matching to docs whose parent directory equals
  that prefix, anchored at the content root. A leading slash
  (`[[/Name]]`) requires a top-level document.

Wikilinks inside code spans and fenced code blocks stay literal — they are
resolved after Markdown parsing, so `` `[[not a link]]` `` renders as written.

## Linking into a page

A `#fragment` after the target links to a spot *within* the page. The note is
resolved first, then the fragment is appended to the URL — so `[[Note#Heading]]`
and `[[Note#Heading|Display text]]` both work, and a fragment naming something
that doesn't exist still links to the note (it just lands at the top).

**Headings.** Every heading emits an `id` anchor, so `[[Guide#Getting Started]]`
becomes `/guide.html#getting-started`. Hand-written `[Markdown](/guide.html#getting-started)`
links land too.

**Blocks.** Tag a block with a `^blockid` marker — Obsidian's block-id syntax —
and link to it with `[[Note#^blockid]]`. A marker goes at the **end of a
paragraph, heading, or list item**, preceded by a space:

```markdown
A claim worth citing. ^abc123

- a list item ^xyz789
```

Blocks with nowhere to put a trailing marker — tables, code fences, blockquotes,
whole lists — take one **alone on its own line**, tagging the block above it:

```markdown
| col | val |
| --- | --- |
| a   | 1   |

^table1
```

Either way the marker is stripped and replaced with an anchor. A trailing marker
anchors inside its block; a standalone one anchors just before the block it tags,
so a link lands at the top of a long table rather than its bottom edge:

```html
<p>A claim worth citing.<span class="block-anchor" id="abc123"></span></p>

<span class="block-anchor" id="table1"></span>
<table>…</table>
```

Link to any of them from anywhere:

```markdown
See [[note#^abc123]], the [[note#^xyz789|list item]], and [[note#^table1]].
```

Block ids may contain letters, digits, and dashes, and are case-insensitive
(`^Abc-1` and `^abc-1` are the same id). Four things to know:

- **A standalone marker needs a blank line before it after a table** — without
  one, Markdown reads the marker as another table row. After a code fence the
  blank line is optional; after a blockquote or a list it's required (a marker on
  the line right after `> quoted` becomes part of the quote).
- A standalone marker with nothing above it — the first thing in a file — tags
  nothing and renders literally.
- Block ids share the anchor namespace with heading slugs, so a `^overview`
  marker collides with an `## Overview` heading on the same page.
- A duplicate id within one page anchors the first block only.

Markers inside code spans and fenced code blocks stay literal, same as
wikilinks.

## How targets resolve

Resolution mirrors Obsidian's behavior:

1. The target stem is slugified and matched against the slugified filename
   stems of **all** documents.
2. Among matches, the winner is the one with the smallest directory distance
   from the linking document — your own folder beats a sibling folder beats a
   distant one.
3. Remaining ties break by lexicographically smallest `id_path`, so builds are
   deterministic.

A resolved link renders as an anchor; an unresolved one as a span:

```html
<a class="wikilink" href="…">Display text</a>
<span class="nolink">Display text</span>
```

Style `.wikilink` and `.nolink` in your CSS to make link state visible —
gardens often render unresolved links in a muted color.

## Backlinks

Every resolved wikilink registers an edge in the site's link graph. Only
`[[wikilinks]]` count — a plain Markdown `[label](other.md)` link does not.
The wikilink syntax is the intentional "this is a cross-document reference"
signal, and backlinks reflect that.

Render a page's backlinks in its layout with the `backlinks` filter:

```jinja
<h2>Linked from</h2>
<ul>
{% for src in page.id_path | backlinks(order_by="title", sort="asc") %}
  <li><a href="{{ src.id_path | link }}">{{ src.title }}</a></li>
{% endfor %}
</ul>
```

Kwargs (`order_by`, `sort`, `omit`, `limit`) are in the
[template reference](../reference/templates.md#backlinks--pages-that-link-to-this-one).

The link graph is also one of the namespaces the [related-pages](related.md)
engine scores — including co-citations (two pages linking to the same third
page), which backlinks alone don't capture.

## See also

- [Related pages](related.md)
- [Template reference: backlinks](../reference/templates.md#backlinks--pages-that-link-to-this-one)
- [Tutorial: publish your Obsidian vault](../getting-started/tutorial.md)
