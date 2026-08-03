# Template functions — listing and looking up docs

`collection()`, `all()`, `taxonomy()`, and `doc()` read the page index, so
they are **template-phase only** — inside a document body they error (see
[context](template-context.md#the-two-phases)). `dir()` works in both phases.

## `collection(name=...)` — list a named collection

Returns the members of a collection declared under `collections:` in
`config.yaml`, in the collection's configured order.

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `name` | yes | Collection name. `collection(name="typo")` returns an empty list, not an error. |
| `omit` | no | Array of `id_path` strings to exclude; layers on top of the collection's definition-time `omit`. |
| `limit` | no | Max items; applied after `omit`. |

```jinja
{% for post in collection(name="recent_posts", omit=[page.id_path], limit=5) %}
  <a href="{{ post.id_path | permalink }}">{{ post.title }}</a>
{% endfor %}
```

## `all()` — list every doc

Returns the always-present [`all` collection](config.md#the-all-collection) —
every document, date-descending by default. Takes no arguments; to reorder or
filter, redeclare `all` in config, pipe through
[doc-list filters](doc-list-filters.md), or slice (`all()[:5]`).

```jinja
{% for doc in all() %}
  <a href="{{ doc.id_path | link }}">{{ doc.title }}</a>
{% endfor %}
```

## `taxonomy(name=...)` — list a taxonomy's terms

Returns a map of term slug → docs for a declared taxonomy. Iterate
deterministically with [`entries`](doc-list-filters.md#entries--iterate-a-map-in-key-order).

```jinja
{% for slug, docs in taxonomy(name="tags") %}
  <h2>{{ slug }}</h2>
  {% for post in docs %}<a href="{{ post.id_path | permalink }}">{{ post.title }}</a>{% endfor %}
{% endfor %}
```

## `doc(id_path=...)` — look up a single doc

Fetch one document by `id_path`. Returns none for an unknown path, so guard
with `{% if %}` rather than failing the build:

```jinja
{% set about = doc(id_path="about.md") %}
{% if about %}<a href="{{ about.id_path | link }}">{{ about.title }}</a>{% endif %}
```

## `dir(path=...)` — parent directory of a path

**Both phases.** Returns the parent directory of a `/`-separated path
(`dir(path="foo/bar/baz.png")` → `"foo/bar"`). A path with no directory yields
`""`. Pair with [`filter_in_dir`](doc-list-filters.md#filter_in_dir--keep-docs-in-one-directory).

## See also

- [Doc-list filters](doc-list-filters.md) — sort, scope, and prune the results
- [Template context](template-context.md) — what else is in scope
