# Content model

Every file in `content/` with a recognized extension becomes a **document** —
the unit everything else operates on. Collections query documents, taxonomies
group them, templates render them, archives list them.

## `id_path`: a document's identity

A document's **`id_path`** is its path relative to `content/`, e.g.
`posts/hello.md`. It is the canonical identity used everywhere:

- `doc(id_path="about.md")` looks a document up by it.
- `omit=[page.id_path]` excludes documents from listings by it.
- The URL filters (`| link`, `| permalink`) resolve it to the document's
  rendered location.
- Collection `path:` globs match against it.

Distinct from `id_path` is the **output path** — where the document renders.
By default the output path mirrors the `id_path` with an `.html` extension; a
[`permalink:`](permalinks.md) changes the output path but never the `id_path`.

## Three content types

| Type | Frontmatter | Body |
|------|-------------|------|
| `.md` | Optional YAML block | Markdown → rendered to HTML |
| `.html` | Optional YAML block | Raw HTML → passed through |
| `.yaml` | The whole file | `content:` field rendered as HTML |

`.md` and `.html` use the conventional `---`-delimited frontmatter block. A
`.yaml` document is *all* frontmatter — useful for data-heavy pages — with the
optional `content:` string field as its body.

Frontmatter is *uplifted* into typed fields (`page.title`, `page.date`,
`page.terms`, …) with everything else preserved verbatim on `page.data`; dates
fall back to file timestamps. The key-by-key rules are in the
[frontmatter reference](frontmatter.md), and a collection's
[`defaults:`](config.md#defaults) fills any key its members didn't set.

## Documents vs. archive pages

Archive templates in `archives/` generate *view pages* — paginated listings,
feeds, sitemaps — from collections and taxonomies. View pages are rendered
output only: they are never classified back into collections, taxonomies, or
backlinks, so a tag page can't tag itself. See [Archives](archives.md).

## See also

- [The build pipeline](build-pipeline.md) — how documents flow to output
- [Authoring](authoring.md) — writing the files themselves
