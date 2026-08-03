# Metadata & social cards

Every site needs the same `<head>` metadata — a meta description, a canonical
link, Open Graph and Twitter cards so links unfurl nicely when shared, JSON-LD
for search engines, and `<link>`s that point feed readers at your RSS. Italic
ships built-in **metadata filters** so your theme doesn't hand-roll (and keep in
sync) all of that from `page` and `site`.

These are template-phase filters (they belong in layouts, not Markdown bodies).
They're *safe* — their markup isn't HTML-escaped — and they degrade gracefully:
when `site.url` is unset, URLs fall back to root-relative; when a field is
missing, the corresponding tag is simply omitted.

## The one-liner

For a complete, sensible `<head>`, pipe `page` through `metadata`:

```jinja
<head>
  <title>{{ page.title }} · {{ site.title }}</title>
  {{ page | metadata }}
</head>
```

The filters read the `site` context variable themselves, so there's nothing to
pass in.

That emits, in order: the generator tag
(`<meta name="generator" content="italic <version>">`), the description,
keywords, `robots noindex` for [drafts](drafts.md), the canonical link, the
standard.site proof link (for [published](atproto-publish.md) pages), the
[AT tags](#at-tags), Open Graph tags, the Twitter card, JSON-LD, and a
feed-discovery `<link>` for each configured [feed](archives.md).

The umbrella covers *generated* metadata only. Document-level tags your theme
owns — `<title>`, `<meta charset>`, and the viewport — are yours to write, one
static line each:

```jinja
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{{ page.title }} · {{ site.title }}</title>
  {{ page | metadata }}
</head>
```

On non-article pages (a home or landing page), pass `type="website"`:

```jinja
{{ page | metadata(type="website") }}
```

## Composing individual filters

When you want control over what goes in `<head>`, use the filters individually:

```jinja
<head>
  <title>{{ page.title }} · {{ site.title }}</title>
  {{ page | meta_description }}
  {{ page | meta_keywords }}
  {{ page | canonical_link }}
  {{ page | standard_link }}
  {{ page | at_meta }}
  {{ page | open_graph(type="article") }}
  {{ page | twitter_card }}
  {{ page | json_ld }}
  {{ page | system_meta }}
  {{ site | feed_links }}
</head>
```

What each filter emits:

| Filter | Pipe | Emits |
|--------|------|-------|
| `metadata` | `page` | The umbrella: everything below in order. Generated metadata only — `<title>`, `<meta charset>`, and viewport are the theme's to write. |
| `system_meta` | `page` | Italic/system-controlled tags. Today the generator tag; the home for future engine-owned `<head>` metadata. |
| `meta_description` | `page` | `<meta name="description">` (`page.summary`, else `site.description`). |
| `meta_keywords` | `page` | `<meta name="keywords">` from `tags`, else `page.data.keywords`. |
| `canonical_link` | `page` | `<link rel="canonical">`. |
| `standard_link` | `page` | `<link rel="site.standard.document">` from `page.data.atproto_uri` — the standard.site per-page proof; empty unless [`ITALIC_ATPROTO_DID`](atproto-verify.md#verification-artifacts) is set. |
| `at_meta` | `page` | [AT tags](#at-tags) — each omitted when its source is absent. |
| `open_graph` | `page` | `og:*` (and `article:*` when `type="article"`, the default). |
| `twitter_card` | `page` | `twitter:*` (`summary_large_image` when an image exists). |
| `json_ld` | `page` | `<script type="application/ld+json">` (`BlogPosting`, or `WebSite` for `type="website"`). |
| `feed_links` | `site` | One `<link rel="alternate">` per configured `feed:`. |

The filters read site-wide fallbacks from the `site` context variable
automatically — no kwarg needed (a legacy `site=site` kwarg is accepted and
ignored). `open_graph`/`json_ld`/`metadata` accept `type=` (default
`"article"`).

## AT tags

Nothing on a web page tells a reader — human or machine — which atproto records
and identities it corresponds to. The [AT tags
proposal](https://tangled.org/chrisshank.com/at-tags/) fixes that with an `at:`
namespace on `<meta>` tags, so link previews, clients and crawlers can find the
underlying records without scraping. It uses `<meta>` rather than `<link>`
because AT URIs are not valid in a `<link href>`.

`{{ page | at_meta }}` (included in the umbrella) emits them:

```html
<meta name="at:canonical" content="at://did:plc:abc/site.standard.document/rkey">
<meta name="at:alternate" content="at://did:plc:abc/site.standard.publication/rkey">
<meta name="at:author" content="at://did:plc:abc">
<meta name="at:me" content="at://did:plc:abc">
<meta name="at:blog:comments" content="at://did:plc:abc/app.bsky.feed.post/rkey">
```

| Tag | Means | Comes from |
|-----|-------|------------|
| `at:canonical` | the record this page *is* — delete it and the page has no reason to exist | `page.data.atproto_uri`, the [document record](atproto-publish.md) |
| `at:alternate` | a record this page merely references | `site.atproto_publication_uri`, the publication record |
| `at:author` | who wrote the page | `page.data.author_did`, else `site.author_did`, else `site.atproto_did` |
| `at:me` | the identity behind the site | `site.atproto_did` |
| `at:blog:comments` | the post carrying this page's comments | `page.data.bsky_uri`, the [Bluesky announcement post](atproto-publish.md) |

Every property has *array semantics* — repeat a tag to give it several values.
So `author_did:` accepts a list for a co-authored page:

```yaml
---
title: A joint post
author_did: [did:plc:ada, did:plc:grace]
---
```

`site.atproto_did` and `site.atproto_publication_uri` are derived during the
build from `ITALIC_ATPROTO_DID` plus `site.url` — the same inputs as the
[verification artifacts](atproto-verify.md#verification-artifacts) — so
there is nothing to configure. Without them you simply get fewer tags; each one
is omitted when its source is missing. Both are ordinary context variables, so a
hand-rolled `<head>` can read them directly.

## Configuration

Set these once under `site:` in `config.yaml`; the filters read them as
fallbacks and defaults:

```yaml
site:
  title: My Site
  description: A short tagline used as the fallback description.
  url: https://example.com        # required for absolute og:url / og:image
  author: Ada Lovelace            # article:author / JSON-LD author fallback
  twitter: "@mysite"              # twitter:site / twitter:creator
  locale: en_US                   # og:locale (default en_US)
  image: /img/social-card.png     # default social image, site-wide
```

The feed `<link>`s come from your [`feed:` config](archives.md) — one per
generated `/feed/<name>.xml`.

## Per-page overrides

Frontmatter fields refine the metadata for a single page:

```yaml
---
title: Hello World
summary: A short post — used as the description and og:description.
image: /img/hello.png       # this page's social image (page.data.image); also
                            # the ATProto coverImage fallback when publishing
image_alt: A friendly hello # alt text for the social image
author: Guest Author        # overrides site.author
author_did: did:plc:ada     # at:author (a list for co-authors); see AT tags
keywords: [rust, ssg]       # used when the page has no tags
---
```

Keywords come from the page's `tags` ([taxonomy](taxonomies.md)) when present,
otherwise from `keywords:`. A page's [`date`/`updated`](frontmatter.md)
become `article:published_time` / `article:modified_time`.

## See also

- [Layouts](layouts.md) — the `<head>` these filters live in
- [Frontmatter](frontmatter.md) — the per-page override keys
- [Archives, feeds & sitemaps](archives.md) — what `feed_links` discovers
