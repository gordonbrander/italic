# Migrating to italic

Italic reads plain Markdown with YAML frontmatter, so most migrations are
mostly a matter of pointing it at your existing files and mapping config.

## From an Obsidian vault

Copy (or symlink) your vault into `content/` and build:

- `[[Wikilinks]]` and `[[Wikilinks|aliases]]` resolve with the same fuzzy
  matching algorithm Obsidian uses; backlinks come for free.
- Heading references (`[[Note#Heading]]`) and block references
  (`[[Note#^abc123]]`) both resolve. `^blockid` markers become anchors rather
  than visible text, whether they trail a paragraph, heading, or list item, or
  sit alone on a line tagging the table, code fence, or blockquote above them.
  Two differences from Obsidian, both in
  [wikilinks](wikilinks.md#linking-into-a-page): a standalone marker needs a
  blank line after a table (Markdown would otherwise read it as a row), and
  block ids share the anchor namespace with heading slugs.
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

What does **not** carry over (each verified against the current binary):

| Obsidian feature | In italic |
|------------------|-----------|
| Note transclusion `![[Some Note]]` | Not supported — media embeds only; a note embed is left as literal text. See [wikilinks](wikilinks.md#differences-from-obsidian). |
| `aliases:` frontmatter | Inert — `[[Some Alias]]` renders `nolink`. Use real stems; for URL aliases use [`redirect_from:`](redirects.md). |
| `%%comments%%` | Not stripped — they publish literally. Delete them. |
| `[[note.md]]` extension links | Don't resolve — link by stem, `[[note]]`. |
| Nested tags `#parent/child` | One flat term (text `parent/child`, slug `parentchild`) — no hierarchy. |
| `publish: false` | Inert — the page builds. Use [`draft: true`](drafts.md). |
| `cssclasses:` | Inert — plain `page.data`; consume it in your layout if wanted. |
| Plugins, dataview, canvas | Not supported. |

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
| Shortcodes | [Tera components](components.md). |
| `aliases:` frontmatter | Rename to [`redirect_from:`](redirects.md) — same redirect-stub behavior, different key. |

## From Quartz

Quartz content is an Obsidian-flavored vault, so the content path is the
Obsidian one above (including its divergence table). Beyond that:

| Quartz | Italic |
|--------|--------|
| `quartz.config.ts` | `config.yaml` — `site:` holds title/baseUrl-style metadata. |
| Quartz components/layout (TypeScript) | Tera [layouts](layouts.md) and [components](components.md) — rewritten, not ported. |
| Built-in tag pages, RSS | An [archive template](archives.md) per listing; feeds/sitemap are [built in](archives.md#built-in-feeds-and-sitemap). |
| `aliases:` frontmatter | Rename to [`redirect_from:`](redirects.md). |

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

- [Recipe: notes + blog + tags + feed](recipe-blog-and-tags.md)
- [Collections](collections.md) · [Permalinks](permalinks.md) · [Layouts](layouts.md)
