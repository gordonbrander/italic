# Project layout

An italic project is a folder of conventional directories plus a `config.yaml`.
Everything is optional — a bare `content/` directory is a buildable site.

```
content/        # Your site content (.md, .html, .yaml) + co-located media
archives/       # Archive templates (tag pages, feeds, sitemaps — see Archives)
templates/      # Tera layouts, partials, and macros
data/           # YAML files surfaced to templates as {{ data.* }}
static/         # Copied verbatim into the output
themes/         # Conventional home for themes referenced via theme: in config.yaml
config.yaml     # Site config (optional)
public/         # Build output (created by italic build; removed by italic clean)
```

Every directory name except `themes/` is configurable via the `*_dir` keys in
`config.yaml` — see the [configuration reference](../reference/config.md#directories).
(`themes/` is just a convention; the `theme:` key takes any path.)

## Content structure is yours

Italic doesn't impose a layout on `content/`. Organize it however you like —
flat, deeply nested, mirroring an Obsidian vault — and use
[collections](../guides/collections.md) to define blogs, sections, and other
groupings as *queries* over that content rather than as directory requirements.
This is what lets one site host multiple blogs, news feeds, and portals at
once.

By default, a document's output location mirrors its source path
(`notes/foo.md` → `notes/foo.html`); [permalinks](../guides/permalinks.md)
override that per document or per collection.

Non-content files under `content/` (images, PDFs, …) are **co-located media**:
they're copied to the matching output path, and `![](…)`/`![[…]]`/`[[…]]`
references to them are resolved automatically — so Obsidian-style attachments
kept beside your notes just work. See
[co-located media](../guides/authoring.md#co-located-media-images-and-attachments).
Use `static/` instead for site-wide assets (CSS, fonts, favicons) that aren't
tied to a particular note.

## What each directory feeds

| Directory | Consumed by | Guide |
|-----------|------------|-------|
| `content/` | The build pipeline — every `.md`/`.html`/`.yaml` becomes a page; other files become [co-located media](../guides/authoring.md#co-located-media-images-and-attachments) | [Authoring](../guides/authoring.md) |
| `templates/` | The template phase; `templates/macros/` auto-imports into content | [Templates](../guides/templates.md) |
| `archives/` | The archive phase — collection/taxonomy listings, feeds, sitemaps | [Archives](../guides/archives.md) |
| `data/` | Loaded once, exposed to every template as `{{ data.* }}` | [Data files](../guides/data.md) |
| `static/` | Copied verbatim over the output as the last build step | [Project layout](#) |
| `themes/<name>/` | Overlaid beneath `templates/`, `archives/`, `static/` | [Themes](../guides/themes.md) |

## See also

- [Content model](content-model.md) — what counts as a document
- [The build pipeline](build-pipeline.md) — how the directories flow into output
