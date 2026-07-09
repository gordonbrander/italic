# Migrating to italic

Italic reads plain Markdown with YAML frontmatter, so most migrations are
mostly a matter of pointing it at your existing files and mapping config.

## From an Obsidian vault

The happy path — italic is built for this. Copy (or symlink) your vault into
`content/` and build:

- `[[Wikilinks]]` and `[[Wikilinks|aliases]]` resolve with the same fuzzy
  matching algorithm Obsidian uses; backlinks come for free.
- Heading references (`[[Note#Heading]]`) and block references
  (`[[Note#^abc123]]`) both resolve, and a trailing `^blockid` marker becomes an
  anchor rather than visible text. Two differences from Obsidian, both in
  [wikilinks](wikilinks.md#linking-into-a-page): a block marker must end a
  paragraph, heading, or list item (a marker alone on a line — how Obsidian tags
  tables and code fences — renders literally), and block ids share the anchor
  namespace with heading slugs.
- Inline `#hashtags` lift into the `tags` taxonomy with `hashtags: true`.
- **Attachments kept beside your notes just work.** Drop images and other media
  anywhere under `content/` — next to the note that uses them, or in a shared
  `attachments/` folder — and reference them however you do in Obsidian:
  - a standard image, `![caption](diagram.png)`, resolved relative to the note;
  - an embed, `![[diagram.png]]`, matched by filename across the vault;
  - an attachment link, `[[report.pdf]]`.

  Each resolves to the file's published location, so references stay correct
  even for notes with a custom `permalink:`. See
  [co-located media](authoring.md#co-located-media-images-and-attachments).
- Notes without frontmatter are fine — `title` defaults to empty (set it, or
  derive headings from your H1s in the layout), dates fall back to file
  timestamps.

What doesn't carry over: Obsidian plugins, dataview queries, canvas files, and
note *transclusion* — `![[Some Note]]` embeds a media file, but it won't inline
another note's rendered body. Start with the
[tutorial](../getting-started/tutorial.md), which walks this exact path.

## From Jekyll

| Jekyll | Italic |
|--------|--------|
| `_posts/` with dated filenames | A `posts` collection; put the date in frontmatter (or keep it in the filename and set `permalink:` per file). |
| `_config.yml` | `config.yaml` — `site:` holds your metadata. |
| `permalink: /blog/:year/:title/` | `permalink: /blog/:yyyy/:slug/` in collection `defaults:`. |
| `layout: post` | `template: post.html`. |
| Liquid (`{{ page.title }}`, `{% for %}`) | Tera — nearly identical interpolation/block syntax; filters differ in spots ([Tera built-ins](https://keats.github.io/tera/docs/#built-ins)). |
| `_data/*.yml` | `data/*.yaml`, as `{{ data.* }}`. |
| `categories`/`tags` | Declare both under `taxonomies:`. |
| `redirect_from:` (jekyll-redirect-from plugin) | [`redirect_from:`](redirects.md) — same key, built in. |

## From Hugo

| Hugo | Italic |
|------|--------|
| `content/` sections | Keep the folder structure; define [collections](collections.md) by glob instead of section. |
| `hugo.toml` | `config.yaml`. |
| `[permalinks]` patterns | `permalink:` in collection `defaults:` (`:yyyy`, `:mm`, `:dd`, `:slug`). |
| Go templates (`{{ .Title }}`) | Tera (`{{ page.title }}`) — syntax differs substantially; layouts need rewriting. |
| `layouts/_default/list.html` | An [archive template](archives.md). |
| Taxonomies in config | Same idea: `taxonomies:` array. |
| Shortcodes | [Tera macros](macros.md). |
| `aliases:` frontmatter | Rename to [`redirect_from:`](redirects.md) — same redirect-stub behavior, different key. |

## From Zola

The closest relative — Zola also uses Tera, so templates mostly port directly.
Differences to mind:

- Context names differ: Zola's `page.permalink`/`section` model vs. italic's
  `page.id_path` + URL filters; there are no "sections" — use
  [collections](collections.md).
- Zola's `_index.md` section pages become [archives](archives.md).
- `taxonomies` move from per-page config syntax to a plain `taxonomies:` array
  plus frontmatter fields.

## General checklist

1. Copy content into `content/`; don't restructure yet.
2. Declare your taxonomies, then your collections (globs over the existing
   layout).
3. Recreate permalinks with `defaults:` so URLs don't break; spot-check old
   URLs against the new output. Where a URL *does* change, add the old one to
   the page's [`redirect_from:`](redirects.md) so existing links redirect
   instead of 404.
4. Port layouts to Tera one at a time, starting with `base.html`.
5. Wire archives for listings and feeds.

## See also

- [Tutorial: publish your Obsidian vault](../getting-started/tutorial.md)
- [Collections](collections.md) · [Permalinks](permalinks.md) · [Templates](templates.md)
