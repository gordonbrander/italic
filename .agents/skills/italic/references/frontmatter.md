# Frontmatter reference

Frontmatter is a YAML block at the top of a document, delimited by `---` lines:

```markdown
---
title: Hello, world
template: base.html
date: 2026-01-01
tags: [intro]
---
The body of the post goes here.
```

`.md` and `.html` files take an optional frontmatter block; in a `.yaml` file
the whole file is frontmatter and the `content:` field (if present) is the
body. Missing or unterminated frontmatter is treated as no frontmatter;
malformed YAML inside a present block is a build error.

## Document keys

A few keys have special meaning and sensible defaults when absent:

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `title` | string | `""` | Document title, `{{ page.title }}`. |
| `summary` | string | `""` | Brief description, `{{ page.summary }}`. |
| `draft` | bool | `false` | Exclude from builds (see [Drafts](drafts.md)). |
| `template` | string | none | Template to wrap the body in. Without one, the rendered body is the final output. |
| `date` | date | file created time, falling back to modified time | Publication date. |
| `updated` | date | file modified time | Last-modified date. |
| `permalink` | string | mirror of the source path with `.html` | Output location pattern (see below). |
| `redirect_from` | array of strings | `[]` | Old URLs that should redirect here (see [Redirects](redirects.md)). |
| `<taxonomy>` | array of strings | `[]` | One field per declared taxonomy, e.g. `tags: [intro, rust]`. |
| `content` | string | `""` | `.yaml` files only: the body to render. |
| `image` | path | `site.image` | Social-card image for the [metadata filters](metadata.md) and the ATProto document's cover. |
| `image_alt` | string | none | Alt text for `image`. |
| `author` | string | `site.author` | Author name for meta/JSON-LD. |
| `author_did` | string \| list | `site.author_did` | Author DID(s) for AT-tags. |
| `keywords` | string \| list | none | `<meta name="keywords">` fallback when the doc has no tags (tags win). |
| `bsky` | string | none | Text for a Bluesky post announcing this doc, published by [`italic atproto publish`](atproto-bsky.md). ≤ 300 graphemes; requires `atproto.bsky.enabled`. |

The metadata keys (`image` through `keywords`) have no build-time behavior of
their own — they live in `page.data` like any custom key and are read by the
[metadata filters](metadata.md) and `atproto publish`.

Dates parse as RFC 3339 (`2026-01-01T12:00:00Z`) or plain `YYYY-MM-DD`.
Frontmatter dates win; the filesystem only fills in when the frontmatter value
is absent or unparseable.

**Any other key is preserved verbatim** and reachable in templates as
`{{ page.data.<key> }}`. (The special keys above are also still present in
`page.data`.) A document's taxonomy memberships are uplifted into
`page.terms` — a map of taxonomy → term slug → display text, e.g.
`page.terms.tags`.

## Permalink patterns

`permalink:` overrides the default output location (which mirrors the source
path: `notes/foo.md` → `notes/foo.html`).

| Variable | Expands to |
|----------|------------|
| `:slug` | Slugified stem of the source filename. |
| `:yyyy` | Four-digit year of `date`. |
| `:mm` | Two-digit month of `date`. |
| `:dd` | Two-digit day of `date`. |
| `:term` | Term slug — taxonomy archives only; left untouched elsewhere. |

A leading `/` is ignored; a trailing `/` writes `index.html` in that directory:

```yaml
permalink: /blog/:yyyy/:slug/   # → blog/2026/hello/index.html
```

See the [Permalinks guide](permalinks.md).

## Redirects

`redirect_from:` lists old URLs that should redirect to this document — useful
after renaming or reorganizing a published note:

```yaml
redirect_from:
  - /old-url/
  - /posts/legacy.html
```

Each entry is a literal historical URL (no pattern expansion) and emits a small
redirect page at that path. A trailing slash or extension-less entry writes
`index.html` in that directory; an entry with an extension is written as that
literal file. See the [Redirects guide](redirects.md).

## ATProto publishing

[`italic atproto publish`](atproto-publish.md) needs no dedicated
frontmatter: the document's `coverImage` blob comes from the page's `image:`
(then `site.image`) — the same fields the
[metadata filters](metadata.md) use for social cards.

The one opt-in key is `bsky:` — the text of a short Bluesky post announcing
the doc, published alongside its document record when
[`atproto.bsky.enabled`](config.md#atproto) is on:

```yaml
bsky: "New post: how I grow my digital garden 🌱"
```

Docs without a `bsky:` key never get a post — omitting the key is how you
deliberately skip one. The text is capped at 300 graphemes (Bluesky's limit)
and a post is **created once**: editing the text after the post exists does
nothing. See [Bluesky posts](atproto-bsky.md).

When the `ITALIC_ATPROTO_DID` env var is set, each document's standard.site
AT-URI is derived at build time and exposed to templates as
`page.data.atproto_uri`, for
emitting the verification `<link>` tag. See the
[Publishing guide](atproto-verify.md#verification-artifacts).

Once a doc's announcement post exists (recorded in `.italic/bsky.yaml`), its
AT-URI is likewise exposed as `page.data.bsky_uri`, so themes can link the
post or render its replies as comments. See
[Replies as comments](atproto-bsky.md#replies-as-comments).

## Setting defaults per collection

Rather than repeating frontmatter on every file, set collection-wide defaults
under `defaults:` in `config.yaml`. A document's own frontmatter always
overrides a default. See [`defaults`](config.md#defaults).

## Archive keys

Templates in `archives/` use their own frontmatter vocabulary (see the
[Archives guide](archives.md)):

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `kind` | `collection` \| `taxonomy` | **required** | What the archive iterates over. |
| `collection` | string | required with `kind: collection` | Name of the collection. |
| `taxonomy` | string | required with `kind: taxonomy` | Name of the taxonomy; emits one page-set per term. |
| `permalink` | string | **required** | Output pattern; `:term` available for taxonomy archives. Pages ≥ 2 get `page/N/` appended automatically. |
| `per_page` | integer | none — one page | Items per page. |
| `limit` | integer | none — no cap | Cap on items **before** pagination. For a collection archive it caps the total; for a taxonomy archive it caps items per term. |
| `query` | mapping | none | **Taxonomy archives only.** Scopes and re-orders each term's docs before pagination, using the same `path` / `order_by` / `sort` / `omit` keys as a collection [query](config.md#collections). An error on `kind: collection` archives. |
| `template` | string | none — the archive body **is** the final output | Layout to wrap each rendered archive page in, like a document's `template:`. |

A minimal, valid taxonomy archive — `archives/tags.html`:

```html
---
kind: taxonomy
taxonomy: tags
permalink: /tags/:term/
---
<h1>{{ term.text }}</h1>
<ul>{% for doc in pagination.items %}<li>{{ doc.title }}</li>{% endfor %}</ul>
```

The body renders through Tera whether or not `template:` is set. Worked
collection, feed, and pagination recipes are in the
[Archives guide](archives.md).

## See also

- [Authoring guide](authoring.md)
- [Configuration reference](config.md)
- [Template context](template-context.md) — how `page.*` is consumed
