# Taxonomies & hashtags

Taxonomies categorize documents. Tags are the familiar example, but italic has
no built-ins — any frontmatter field can become a taxonomy: category, series,
publication, phase of the moon.

## Declaring taxonomies

List the frontmatter fields to treat as taxonomies in `config.yaml`:

```yaml
taxonomies:
  - tags
  - category
  - series
```

Then assign terms in any document's frontmatter:

```yaml
---
title: Building a second brain
tags: [pkm, writing]
series: [garden-notes]
---
```

A document's memberships are available in templates as `page.terms` — a map of
taxonomy → term slug → display text (e.g. `page.terms.tags`).

## Hashtags

With `hashtags: true` in `config.yaml`, italic lifts inline `#hashtags` out of
Markdown bodies into the `tags` taxonomy and strips them from the rendered
HTML:

```markdown
Quick thought about composting ideas. #pkm #gardening
```

…tags the page `pkm` and `gardening`, with the hashtags gone from the output.
This happens during markup, so hashtag-derived terms count everywhere
frontmatter tags do: tag archives, `taxonomy()`, and
[related-page](related.md) scoring. It's off by default so literal `#`
characters in prose are untouched.

## Using taxonomies in templates

List a taxonomy's terms and their documents with `taxonomy()`:

```jinja
{% for slug, docs in taxonomy(name="tags") %}
  <h2>{{ slug }}</h2>
  {% for post in docs %}
    <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
  {% endfor %}
{% endfor %}
```

For a deterministic order, pipe through `entries`:

```jinja
{% for entry in taxonomy(name="tags") | entries %}
  {{ entry.key }} ({{ entry.value | length }})
{% endfor %}
```

## Term archive pages

Generate one page per term — `/tags/rust/`, `/tags/tools/` — with a
`kind: taxonomy` archive whose `permalink:` uses `:term` (the term's slug).
Worked example in [Archives, feeds & sitemaps](archives.md#taxonomy-archives).

## Gotchas

- **A taxonomy field must be a list.** `tags: solo` (a bare string) is
  silently ignored — no error, no term. Write `tags: [solo]`.
- **Nested hashtags don't nest.** With `hashtags: true`, `#parent/child` is a
  single flat term (text `parent/child`, slug `parentchild`).
- Terms that slugify identically collapse into one (`Rust` and `rust`).

**Verify:** `italic build`, then `ls public/tags/` (or your archive's path) —
one directory per term.

A term is global — it gathers docs from every path — so to scope a shared tag to
one section (e.g. only its `posts/**` docs), filter by path: a taxonomy archive
takes a [`query:`](archives.md#scoping-a-taxonomy-archive-with-query) sub-mapping,
and inside any template you can pipe a term's docs through
[`filter_by_id_path`](doc-list-filters.md#filter_by_id_path--keep-docs-matching-a-path-glob).

## See also

- [Configuration reference: taxonomies](config.md#taxonomies)
- [Related pages](related.md) — taxonomies as relatedness signals
- [Template reference: taxonomy()](template-functions.md#taxonomyname--list-a-taxonomys-terms)
