# Template context — what is in scope where

Italic templates use [Tera](https://keats.github.io/tera/) (v2), a Jinja-style
template language. All of Tera's built-ins are available, plus the
`tera-contrib` set (`date`, `now()`, `slug`, `urlencode`, `urlencode_strict`,
`json_encode`, `striptags`, `spaceless`, `filesize_format`, `get_random()`,
`shuffle`), plus italic's own functions and filters:

- [Listing docs: `collection()`, `all()`, `taxonomy()`, `doc()`, `dir()`](template-functions.md)
- [Doc-list filters: `backlinks`, `related`, `dirtree`, …](doc-list-filters.md)
- [URL filters](url-filters.md) · [Text filters](text-filters.md)
- [Metadata filters](metadata.md) · [Components](components.md)

## Template files and autoescaping

Templates are any `.html`, `.xml`, `.tera`, `.json`, or `.txt` file under
`templates/`. Only `.html` and `.xml` are HTML-autoescaped — which is why
layouts write `{{ page.content | safe }}`. In `.tera`, `.json`, and `.txt`
templates values render verbatim (what JSON and plain text want).

## The two phases

Tera runs twice per build ([why](build-pipeline.md#consequences-worth-knowing)):

- **Markup phase** — each document's body renders as a Tera template *before*
  Markdown rendering. The page index doesn't exist yet, so index functions
  and graph filters error here:
  `` `<name>` is not available in the markup phase (Markdown bodies); use it in a template ``
- **Template phase** — layouts in `templates/` render each document and
  archive page. Everything is available.

| Phase | Names |
|-------|-------|
| Template only | `collection()`, `all()`, `taxonomy()`, `doc()` · `backlinks`, `related` · [metadata filters](metadata.md) |
| Both phases | `dir()` · `entries`, `dirtree`, `filter_in_dir`, `filter_by_id_path`, `omit_docs` · text filters · URL filters |

## Variables

What is in scope depends on what is rendering:

| Variable | Document (body & layout) | Collection archive | Taxonomy archive |
|----------|--------------------------|--------------------|------------------|
| `page` | the document | synthesized† | synthesized† |
| `site` | ✓ | ✓ | ✓ |
| `data` | ✓ | ✓ | ✓ |
| `pagination` | — | ✓ | ✓ |
| `term` | — | — | ✓ (`{slug, text}`) |

† An archive page's `page` is synthesized: `title` and `summary` are `""`,
`date`/`updated` are the Unix epoch (don't render them), `id_path` equals the
output path, and `page.data` holds the **archive's own frontmatter** (plus
injected `pagination`/`term`). Docs returned by `collection()`, `all()`,
`backlinks`, `related`, and `pagination.items` have the same shape as a
document's `page`.

## `page` fields

| Field | Contents |
|-------|----------|
| `page.title` | Title from frontmatter (`""` if unset). |
| `page.summary` | Summary from frontmatter (`""` if unset). |
| `page.content` | The rendered body (template phase). Pipe through `safe`. |
| `page.date`, `page.updated` | Dates (frontmatter, falling back to file times). |
| `page.id_path` | The document's source path — its canonical identity, used by `doc()`, `omit=`, and the URL filters. |
| `page.output_path` | Where the doc renders, relative to `output_dir` (after `permalink:` expansion). |
| `page.template` | The doc's `template:` frontmatter (unset when none). |
| `page.draft` | The doc's own `draft:` flag (drafts only appear at all under `serve`/`watch`/`--drafts`). |
| `page.terms` | Map of taxonomy → term slug → display text, e.g. `page.terms.tags`. |
| `page.links` | `id_path`s this doc's wikilinks resolve to (the outbound half of `backlinks`). |
| `page.redirect_from` | The doc's `redirect_from:` list. |
| `page.data` | The full frontmatter map — any custom key, e.g. `page.data.blurb`. |

## `pagination` fields

| Field | Contents |
|-------|----------|
| `pagination.items` | The docs on this page. |
| `pagination.current` | Current page number (1-indexed). |
| `pagination.total` | Total number of pages. |
| `pagination.prev_url` | Previous page's URL; **unset** on the first page. |
| `pagination.next_url` | Next page's URL; **unset** on the last page. |

Because `prev_url`/`next_url` are unset (not empty) at the ends, guard them:

```jinja
{% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">← Previous</a>{% endif %}
```

## See also

- [Layouts](layouts.md) — assigning templates, inheritance, a worked base.html
- [Frontmatter](frontmatter.md) — where `page.*` comes from
