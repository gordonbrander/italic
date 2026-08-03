# Recipe: notes + blog + tags + feed, end to end

The complete path from a folder of Markdown (an Obsidian vault or any notes
directory) to a published site with backlinks, related pages, tag pages, a
blog, and RSS. Every block is pastable; after each step, `italic build` (or
leave `italic serve` running — it reloads live, drafts included).

## 1. Site + notes

```sh
italic new my-garden && cd my-garden
cp -R ~/vault/* content/     # keep your existing structure
```

## 2. One layout for everything

`templates/base.html` — backlinks and related included:

```html
<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>{{ page.title }} | {{ site.title }}</title>
  {{ page | metadata }}
</head>
<body>
  <main>
    <h1>{{ page.title }}</h1>
    {{ page.content | safe }}
  </main>
  <aside>
    <h2>Linked from</h2>
    <ul>
    {% for src in page.id_path | backlinks %}
      <li><a href="{{ src.id_path | link }}">{{ src.title }}</a></li>
    {% endfor %}
    </ul>
    <h2>Related</h2>
    <ul>
    {% for doc in page.id_path | related(limit=5) %}
      <li><a href="{{ doc.id_path | link }}">{{ doc.title }}</a></li>
    {% endfor %}
    </ul>
  </aside>
</body>
</html>
```

`config.yaml` — a catch-all collection assigns it, plus tags and hashtags:

```yaml
site:
  title: My Garden
  url: https://example.com   # enables absolute URLs + built-in feed links

collections:
  notes:
    path: "**/*.md"
  posts:
    path: "posts/*.md"       # dated posts live in content/posts/

defaults:
  notes:
    template: base.html
  posts:
    template: base.html
    permalink: /blog/:yyyy/:slug/

taxonomies:
  - tags

hashtags: true               # inline #hashtags feed the tags taxonomy too
feed:
  - posts                    # built-in RSS at /feed/posts.xml
```

## 3. Tag pages

`archives/tags.html` — one page per term:

```yaml
---
kind: taxonomy
taxonomy: tags
permalink: /tags/:term/
template: base.html
---
{% for post in pagination.items %}
  <p><a href="{{ post.id_path | permalink }}">{{ post.title }}</a></p>
{% endfor %}
```

## 4. The blog index

`archives/blog.html` — reverse-chronological, paginated:

```yaml
---
kind: collection
collection: posts
permalink: /blog/
per_page: 10
template: base.html
---
{% for post in pagination.items %}
  <p>
    <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
    <time>{{ post.date | date(format="%Y-%m-%d") }}</time>
  </p>
{% endfor %}
```

## 5. Feeds — already done

The `feed: [posts]` line above emits `/feed/posts.xml` (25 most recent), and
`/sitemap.xml` and `/feed/all.xml` exist by default. Hand-write an
`archives/feed.xml` only for custom markup or a custom path — see
[built-in feeds](archives.md#built-in-feeds-and-sitemap).

## 6. Verify and ship

```sh
italic build && ls public/tags/ public/blog/ public/feed/
```

Everything in `public/` is plain static files; drafts are excluded. Hosting
recipes: [Deployment](deployment.md).

## Where next

- Style it, or adopt a [theme](themes.md).
- [Permalinks](permalinks.md) · [Components](components.md) ·
  [Migration](migration.md) — Obsidian/Jekyll/Hugo/Quartz mappings.
