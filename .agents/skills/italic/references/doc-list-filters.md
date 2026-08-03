# Doc-list filters — sorting, scoping, and pruning lists of docs

`backlinks` and `related` read the link graph, so they are **template-phase
only**; the rest work in both phases (see
[context](template-context.md#the-two-phases)).

## `backlinks` — pages that link to this one

Pipe an `id_path`; returns the docs whose wikilinks resolve to it.

| Kwarg | Default | Meaning |
|-------|---------|---------|
| `order_by` | `date` | `title` \| `date` \| `updated`. |
| `sort` | `desc` | `asc` \| `desc`. |
| `omit` | `[]` | `id_path`s to exclude (e.g. `omit=[page.id_path]` drops a self-link). |
| `limit` | unlimited | Max items. |

```jinja
{% for src in page.id_path | backlinks(order_by="title", sort="asc") %}
  <li>{{ src.title }}</li>
{% endfor %}
```

## `related` — pages related to this page

Pipe an `id_path`; returns the most related pages, best-match first, scored by
the weights configured under [`related:`](related.md). The page is always
excluded from its own results; ties break by `date` descending, then
`id_path`.

| Kwarg | Default | Meaning |
|-------|---------|---------|
| `limit` | unlimited | Max items. |
| `omit` | `[]` | `id_path`s to exclude. |

```jinja
{% for doc in page.id_path | related(limit=5) %}
  <li><a href="{{ doc.id_path | link }}">{{ doc.title }}</a></li>
{% endfor %}
```

## `entries` — iterate a map in key order

Turns a map into an array of `{key, value}` objects sorted by key (Tera's
`sort` only takes arrays). `sort` is `asc` (default) or `desc`.

```jinja
{% for entry in taxonomy(name="tags") | entries(sort="desc") %}
  {{ entry.key }}: {{ entry.value | length }}
{% endfor %}
```

## `dirtree` — fold docs into a directory tree

Groups an array of docs by output path and returns the content root's children
as a tree. Each node has `name` (path segment), `path` (accumulated output
path), and `kind`: directories (`"dir"`) carry `children`; files (`"file"`)
carry the original `doc`. Children sort by `name`. Walk it with a recursive
[component](components.md):

```jinja
{% component tree(nodes) %}
<ul>
  {% for n in nodes %}
    {% if n.kind == "dir" %}
      <li>{{ n.name }}{{<tree nodes={n.children} />}}</li>
    {% else %}
      <li><a href="{{ n.doc.id_path | link }}">{{ n.doc.title }}</a></li>
    {% endif %}
  {% endfor %}
</ul>
{% endcomponent tree %}

{{<tree nodes={collection(name="posts") | dirtree} />}}
```

## `filter_in_dir` — keep docs in one directory

Keeps only the docs whose `id_path` is an **immediate** child of `dir` (nested
subdirectories excluded), sorted by `id_path`.

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `dir` | yes | A literal directory; `""` for top-level docs. Not auto-derived from a file path — wrap one with `dir(...)`. |
| `omit` | no | `id_path`s to exclude. |

```jinja
{% set siblings = collection(name="all")
     | filter_in_dir(dir=dir(path=page.id_path), omit=[page.id_path]) %}
```

## `filter_by_id_path` — keep docs matching a path glob

Keeps only the docs whose `id_path` matches `path`, a glob with the same
semantics as a collection [query](collections.md): `literal_separator` is on,
so `posts/*.md` stays within one directory while `posts/**` descends. Unlike
`filter_in_dir`, it **preserves input order** — it filters, never re-sorts.

A taxonomy term is global (it aggregates docs from every path), so this is how
you scope a shared tag to one section at render time (build-phase equivalent:
an archive's [`query:`](archives.md#scoping-a-taxonomy-archive-with-query)):

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `path` | yes | A glob matched against each doc's `id_path`. |
| `omit` | no | `id_path`s to exclude. |

```jinja
{% set posts = taxonomy(name="tags")["rust"] | filter_by_id_path(path="posts/**") %}
```

## `omit_docs` — drop docs from a list by `id_path`

Removes docs whose `id_path` appears in `omit`, preserving input order. The
general-purpose complement to the `omit` kwargs built into `collection`,
`backlinks`, `related`, and `filter_in_dir`.

| Kwarg | Required | Meaning |
|-------|----------|---------|
| `omit` | yes | Array of `id_path` strings; an empty array is a passthrough. |

```jinja
{% set others = collection(name="all") | omit_docs(omit=[page.id_path]) %}
```

## See also

- [Template functions](template-functions.md) — where the lists come from
- [Related pages](related.md) — how relatedness is scored and weighted
