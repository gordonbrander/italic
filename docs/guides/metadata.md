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
  {{ page | metadata(site=site) }}
</head>
```

That emits, in order: `<meta charset>`, viewport, the generator tag
(`<meta name="generator" content="italic <version>">`), `<title>` (`Page · Site`),
the description, keywords, `robots noindex` for [drafts](drafts.md), the canonical
link, the standard.site proof link (for [published](publishing-atproto.md)
pages), Open Graph tags, the Twitter card, JSON-LD, and a feed-discovery
`<link>` for each configured [feed](archives.md).

On non-article pages (a home or landing page), pass `type="website"`:

```jinja
{{ page | metadata(site=site, type="website") }}
```

## Composing individual filters

When you want control over what goes in `<head>`, use the filters individually:

```jinja
<head>
  <title>{{ page.title }} · {{ site.title }}</title>
  {{ page | meta_description(site=site) }}
  {{ page | meta_keywords }}
  {{ page | canonical_link }}
  {{ page | standard_link }}
  {{ page | open_graph(site=site, type="article") }}
  {{ page | twitter_card(site=site) }}
  {{ page | json_ld(site=site) }}
  {{ page | system_meta }}
  {{ site | feed_links }}
</head>
```

`{{ page | system_meta }}` emits italic's own engine-controlled tags — currently
just `<meta name="generator" content="italic <version>">`, and the home for more
system tags later. The umbrella already includes it.

See the [template reference](../reference/templates.md#metadata-filters) for the
full table of filters and what each emits.

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
keywords: [rust, ssg]       # used when the page has no tags
---
```

Keywords come from the page's `tags` ([taxonomy](taxonomies.md)) when present,
otherwise from `keywords:`. A page's [`date`/`updated`](../reference/frontmatter.md)
become `article:published_time` / `article:modified_time`.

## See also

- [Templates](templates.md) — layouts and the two render phases
- [Template reference](../reference/templates.md#metadata-filters) — every filter
- [Archives, feeds & sitemaps](archives.md) — what `feed_links` discovers
