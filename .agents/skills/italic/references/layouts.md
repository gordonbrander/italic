# Layouts

Layouts live in `templates/` and use
[Tera](https://keats.github.io/tera/) (v2), a Jinja-style template language
with inheritance, includes, and components. If you've used Jinja2, Liquid, or
Nunjucks, it will feel familiar.

## Assigning a template

Set a layout with the `template` frontmatter key, or via collection defaults:

```yaml
---
template: post.html
---
```

```yaml
# config.yaml — every post gets the layout without per-file frontmatter
defaults:
  posts:
    template: post.html
```

A document without a template renders its body as the final output.

**Bootstrapping a fresh site** (`italic new` ships no templates): a catch-all
collection plus a default wires every doc to one layout —

```yaml
# config.yaml
collections:
  notes:
    path: "**/*.md"
defaults:
  notes:
    template: base.html
```

— then write `templates/base.html` as below and `italic build`.

## A worked base layout

```html
<!-- templates/base.html -->
<!doctype html>
<html>
<head>
  <title>{{ page.title }} | {{ site.title or "My Site" }}</title>
  <link rel="stylesheet" href="{{ "css/style.css" | relative_url }}">
</head>
<body>
  <main>
    {% block content %}
    <h1>{{ page.title }}</h1>
    {{ page.content | safe }}
    {% endblock %}
  </main>
</body>
</html>
```

Three things to notice:

- `page.content` is the document's already-rendered HTML body — pipe it
  through `safe` so it isn't escaped.
- `site.*` is whatever you put under `site:` in `config.yaml` — and only
  that, so guard optional keys with `or` (undefined variables fail the
  build): `{{ site.title or "My Site" }}` works even with no `site:` block.
- Static assets get URL-prefixed with `relative_url` so the site works under a
  [`base_path`](permalinks.md#urls-site-url-and-base-path).

## Inheritance

Use Tera's `{% extends %}`/`{% block %}` for layout inheritance and
`{% include %}` for partials, exactly as in the
[Tera docs](https://keats.github.io/tera/). A child layout overrides just the
blocks it names — here, `base.html`'s `content` block above:

```html
<!-- templates/post.html -->
{% extends "base.html" %}
{% block content %}
  <article>
    <h1>{{ page.title }}</h1>
    <p>{{ page.date | date(format="%B %e, %Y") }} · {{ page.content | reading_time }}</p>
    {{ page.content | safe }}
  </article>
{% endblock %}
```

## Tera 2 notes

Italic uses Tera 2, which tightened a few things over Tera 1 — worth knowing
if you're porting templates from an older italic site or another Tera 1 tool:

- **Undefined variables error.** `{{ page.missing }}` fails the build instead
  of printing nothing. Use a fallback (`{{ page.missing or "" }}`), optional
  chaining (`{{ a?.b?.c or "default" }}`), or guard with `{% if %}` — one level
  of undefined is allowed in conditions, so `{% if page.missing %}` is fine.
- **Macros are gone**, replaced by globally-registered
  [components](components.md) — `{{ youtube::embed(id="x") }}` becomes
  `{{<youtube.embed id="x" />}}` with no `{% import %}` anywhere.
- **Filter renames/removals**: `escape` → `escape_html`, `as_str` → `str`,
  `slugify` → `slug`, `filesizeformat` → `filesize_format`,
  `linebreaksbr` → `newlines_to_br`, `divisibleby` → `divisible_by`;
  `concat` and `slice` are gone in favor of native spread
  (`[...items, extra]`) and Python-style slicing (`items[:5]`, `items[::-1]`);
  `truncate` now requires `length=`.
- **New goodies**: map literals (`{"a": 1}`), list comprehensions, ternaries
  (`{{ "yes" if x else "no" }}`), `{% set %}` blocks with filters, and
  error messages that point at the exact template span.

## Available context

Every variable in scope (`page`, `site`, `data`, `pagination`, `term`), every
italic function and filter, and the file-extension autoescaping rules are in
[Template context](template-context.md).

## See also

- [Template context](template-context.md) — variables, phases, autoescaping
- [Components (shortcodes)](components.md)
- [Archives, feeds & sitemaps](archives.md) — templates that generate pages
