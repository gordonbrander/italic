# URL filters — turning paths into URLs

All four work in **both phases**.

| Filter | Input | Output |
|--------|-------|--------|
| `permalink` | `id_path` | Absolute URL: `site.url` + output path. |
| `link` | `id_path` | Root-relative URL. |
| `relative_url` | any path | `base_path` + `/` + path. |
| `absolute_url` | any path | `site.url` + `base_path` + `/` + path. |

`permalink` and `link` resolve a **document's** `id_path` to where it actually
renders (honoring its `permalink:` frontmatter); `relative_url`/`absolute_url`
prefix arbitrary paths. When `site.url` is unset, the absolute forms degrade
gracefully to root-relative.

**`permalink` and `link` fail the build on an unknown path**:

```text
permalink filter: no doc with id_path `<path>`
```

For static assets (CSS, images) use `relative_url`/`absolute_url`, which
prefix any path without a lookup.

```jinja
<a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
<link rel="stylesheet" href="{{ "css/style.css" | relative_url }}">
```

Rule of thumb: doc links → `permalink`/`link`; assets and hand-built paths →
`relative_url`/`absolute_url`. Feeds and social tags want absolute forms;
in-site navigation is happy with relative ones. Hosting under a subpath needs
[`base_path`](config.md#site) and these filters everywhere — see
[Permalinks](permalinks.md).
