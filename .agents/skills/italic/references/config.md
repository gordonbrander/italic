# Configuration reference

Site-wide configuration lives in `config.yaml` at the project root. **Every key
is optional** — italic builds with no config file at all (and a config file
containing only comments is treated the same as a missing one).

A complete `config.yaml` with every key at its default:

```yaml
# Directories (relative to the project root)
content_dir: content
output_dir: public
templates_dir: templates
static_dir: static
data_dir: data
archives_dir: archives

# Globs under output_dir that `italic clean` must not delete.
keep_files:
  - .git

# Optional theme; no default.
# theme: themes/my-theme

# Extract inline `#hashtags` into the `tags` taxonomy. Off by default.
hashtags: false

site: {}          # site metadata, reachable as {{ site.* }} in templates
collections: {}   # named queries
taxonomies: []    # declared taxonomy field names
defaults: {}      # per-collection default frontmatter
# related:        # weights for the related filter; defaults derived (see below)
#   weights: {}

sitemap: all      # collection the auto sitemap covers; null to disable
feed:             # one /feed/<name>.xml per collection; `[]` to disable
  - all

# atproto:        # `italic atproto publish` settings; whole block optional
#   ...           # (see the `atproto` section below — secrets go in env vars)
```

> **Unknown top-level keys are silently ignored** — there is no schema check at
> the top level, so a misspelled key (`taxonomy:` for `taxonomies:`) or a
> wrong-typed block (`taxonomies: tags` instead of a list) produces **no error
> and no effect**. A clean build does not prove a config key took effect.
> Inside known blocks the rules are stricter: unknown *query* keys under
> `collections:`, unknown keys under `related:`/`atproto:`, and undeclared
> collection names in `sitemap:`/`feed:`/`defaults:` all fail the build. See
> [Troubleshooting § Silent failures](troubleshooting.md#silent-failures).

## Directories

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `content_dir` | path | `content` | Source content (`.md`, `.html`, `.yaml`). |
| `output_dir` | path | `public` | Build output. Emptied by `italic clean` (see [`keep_files`](#keep_files)). |
| `templates_dir` | path | `templates` | Tera layouts, partials, and components. |
| `static_dir` | path | `static` | Copied verbatim into the output. |
| `data_dir` | path | `data` | YAML files surfaced as `{{ data.* }}`. |
| `archives_dir` | path | `archives` | Archive templates (see [Archives](archives.md)). |

All paths are relative to the working directory.

## `keep_files`

List of globs (default `[".git"]`): paths under `output_dir` that
[`italic clean`](cli.md#command-notes) must not delete. Patterns match
relative to `output_dir` with collection-glob semantics (`*` never crosses
`/`; a matching **directory is kept whole**, which is why bare `.git`
preserves a whole repo — the point of the default, for
[worktree deploys](deployment.md#deploy-branch-as-a-git-worktree)).

> **Setting `keep_files` replaces the default.** `keep_files: ["CNAME"]`
> leaves `.git` unprotected — include `.git` yourself. `keep_files: []` keeps
> nothing. Site-only: a theme's `keep_files` is ignored.

## `theme`

Path to a theme folder (no default), e.g. `theme: themes/obsidian`. The
theme's `templates/`, `archives/`, and `static/` overlay **beneath** your
site's, and its `config.yaml` supplies defaults yours overrides. Themes don't
nest. Full layering rules: [Themes](themes.md#how-a-theme-layers).

## `hashtags`

Bool, default `false`. When `true`, the markup phase scans Markdown bodies
for inline `#hashtags`, adds them to each doc's `tags` taxonomy, and strips
them from the rendered HTML. Either the theme or the site setting it turns it
on.

## `site`

A free-form map of site metadata. Everything under `site:` is reachable in
templates as `{{ site.<key> }}`. These keys also have built-in meaning:

| Key | Type | Default | Consumed by |
|-----|------|---------|-------------|
| `site.url` | string | none | Origin for absolute URLs, e.g. `https://example.com`. Trailing slash trimmed; when unset, absolute-URL filters degrade to root-relative. **Required by `atproto publish`.** |
| `site.base_path` | string | `""` | Subpath the site is hosted under, e.g. `/blog`. Normalized to start with `/` and not end with one. |
| `site.title` | string | none | Fallback page title for the metadata filters; `<title>` of built-in feeds. **Required by `atproto publish`** (becomes the publication record's name). |
| `site.description` | string | none | Fallback page description (metadata filters, built-in feeds, publication record). |
| `site.image` | path | none | Default social-card image when a page sets no `image:`. |
| `site.author` | string | none | Fallback author for meta/JSON-LD when a page sets no `author:`. |
| `site.author_did` | string \| list | none | Fallback author DID(s) for AT-tags; below `page.data.author_did`, above `site.atproto_did`. |
| `site.atproto_did` | string | none | The site's own ATProto identity, for AT-tags. |
| `site.twitter` | string | none | `twitter:site` handle, e.g. `@handle`. |
| `site.locale` | string | `en_US` | `og:locale`. |

## `collections`

A map of name → query. Each collection is a saved query over your content,
evaluated once per build and readable in templates via
`collection(name="...")`. Order in `config.yaml` is preserved.

```yaml
collections:
  posts:
    path: "posts/*.md"
    order_by: date
    sort: desc
```

Query keys (`path`, `order_by`, `sort`, `omit`), their defaults, and the
no-`limit` rule are in [Collections](collections.md#query-keys) — unknown
query keys are an error.

### The `all` collection

A collection named `all` always exists. If you don't declare one, the build
injects it with the default query (every doc, date descending). It backs the
`all()` function and is also readable as `collection(name="all")` — handy for a
`sitemap.xml` or full archive. Declare your own `all` under `collections:` to
change its order or contents (a `path`/`omit` may narrow it below every doc); a
site `all` overrides a theme's, like any other collection.

## `taxonomies`

An array of frontmatter field names to treat as taxonomies, e.g.
`taxonomies: [tags, category, series]`. There are no built-in defaults —
declare `tags` like any other taxonomy. Declaration order is preserved. See
[Taxonomies](taxonomies.md).

## `defaults`

A map of collection name → default frontmatter. Values fill in keys that
members of that collection did not set themselves; a document's own frontmatter
always wins. Every entry must name a declared collection (a theme's collection
counts) — an unknown name is an error.

```yaml
defaults:
  posts:
    permalink: /blog/:yyyy/:mm/:dd/:slug/
    template: post.html
```

When a document belongs to multiple collections with overlapping defaults, the
later entry (in config order) wins.

## `related`

Weights for the [`related`](doc-list-filters.md#related--pages-related-to-this-page)
filter; `weights` is the only allowed key (an unknown or stale key is an
error). The weights block, its defaults, and the scoring model live in
[Related pages](related.md#configuring-weights).

## `sitemap`

The collection the auto-generated `/sitemap.xml` covers. Defaults to
[`all`](#the-all-collection); name another collection to scope it, or set it
null (an empty `sitemap:`) to disable. Customize the markup with your own
`archives/sitemap.xml` — a disk archive shadows the built-in. The named
collection must be declared, or the build fails.

## `feed`

A list of collections, each getting an RSS feed at `/feed/<name>.xml`
(25 most recent items). Defaults to `[all]`; `feed: []` disables. Override a
feed's markup with a disk archive at the matching path
(`archives/feed/posts.xml`). Every listed collection must be declared.

## `atproto`

Non-secret settings for `italic atproto publish`. **The whole block is
optional**, and **secrets never go here** — the DID and app password come from
the environment. Unknown keys (in the block or its sub-maps) are an error.
Every key, its default, and the full shape:
[the `atproto:` config block](atproto-publish.md#the-atproto-config-block).

## Theme config merging

When `theme:` is set, the theme's own `config.yaml` is loaded and layered
beneath yours, with per-key merge rules — see
[Themes](themes.md#how-a-theme-layers).

## See also

- [Themes](themes.md) · [Collections](collections.md)
- [Frontmatter](frontmatter.md)
