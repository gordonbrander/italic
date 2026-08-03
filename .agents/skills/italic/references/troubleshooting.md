# Troubleshooting

Errors print as `Error: <message>`, sometimes followed by a `Caused by:` chain —
the **deepest** line is the real cause; grep this file for it. Template failures
are wrapped with where they happened: `` markup-phase Tera in <path> `` (a
document body) or `` rendering template `<name>` for <path> `` (a layout).

## `italic: command not found`

Not an Italic error — the binary isn't installed or isn't on `PATH`. This is
the normal state when the skill was installed before the binary:

```sh
command -v italic || cargo install italic
```

See [Installing](install.md) for the toolchain, `PATH`, and upgrade details.

## Template errors

### An index function or graph filter in a document body

```text
`collection` is not available in the markup phase (Markdown bodies); use it in a template
```

Document bodies render before the page index exists, so `collection()`,
`all()`, `taxonomy()`, `doc()`, `backlinks`, and `related` are template-phase
only. Move that logic into the layout. See
[the build pipeline](build-pipeline.md#consequences-worth-knowing).

### A URL filter got a path that isn't a document

```text
permalink filter: no doc with id_path `<path>`
```

(Same for `link`.) These filters resolve real documents' `id_path`s. For static
assets use `relative_url`/`absolute_url`, which prefix any path without a
lookup. For docs, check the `id_path` spelling (it is the source path, e.g.
`posts/hello.md`).

### Missing variables, unknown names

A missing variable is an error — guard optional values with `{% if %}`
(`pagination.prev_url`, a `doc(...)` lookup, custom `page.data` keys) or a
fallback (`{{ page.data.blurb or "" }}`). An unknown filter, function, or
component fails the whole build when templates load. `doc(id_path=...)`
returns none for unknown paths (guard it); `collection(name="typo")` returns
an empty list, not an error.

```text
backlinks: `order_by` must be one of title|date|updated (got `<value>`)
```

## Config and frontmatter errors

```text
could not parse YAML frontmatter
```

Malformed YAML inside a present `---` block. (A *missing* or unterminated
block is not an error — the file just has no frontmatter.)

```text
query: unknown key `<key>` (allowed: path, order_by, sort, omit)
query: `limit` is no longer a collection key — use the `collection()` filter's `limit=` argument, or an archive's `limit:`
related: `limit` is no longer a config key — pass it to the filter instead, e.g. `page.id_path | related(limit=5)`
related: unknown key `<key>` (allowed: weights)
```

Collection queries and the `related:` block check their keys strictly.

```text
sitemap: `<name>` does not name a collection (declare it under `collections:`)
```

(Same shape for `feed:`, `defaults:`, and `atproto:`.) These keys must name
declared collections — `all` always counts.

```text
archive `<path>` missing required `kind` field (collection|taxonomy)
archive `<path>` missing required `permalink` field
archive `<path>` has unknown kind `<kind>` (expected collection|taxonomy)
```

Every file in `archives/` needs `kind:` and `permalink:` frontmatter. See
[Archives](archives.md).

## CLI errors

```text
path already exists: <path>
```

`italic new` refuses **any** existing path, even an empty directory — there is
no merge or overwrite. Scaffold somewhere fresh and move files in.

```text
no theme set in config.yaml; set `theme:` to a theme directory before scaffolding
```

`italic scaffold` copies the configured theme's starter content; it needs
`theme:` set first. See [Themes](themes.md).

## ATProto errors

```text
no DID — set ITALIC_ATPROTO_DID (run `italic atproto did <your-handle>` to look it up)
no app password — set ITALIC_ATPROTO_APP_PASSWORD (create one at https://bsky.app/settings/app-passwords). Never put it in config.yaml.
```

Credentials come from the environment (a `.env` file is auto-loaded), never
from config. See [Publishing](atproto-publish.md).

```text
site.title is required to publish — it becomes the publication record's name (set it under `site:` in config.yaml)
site.url is required to publish documents — it disambiguates record keys so multiple sites can share one PDS
```

```text
atproto.publication.theme: `<key>` must be a 6-digit hex color like "#1a1a2e" (got `<value>`)
```

Quote hex colors — an unquoted `#` starts a YAML comment, so `background:
#1a1a2e` reaches italic as an empty value.

```text
<n> record(s) missing, changed, or pending — run `italic atproto publish`
```

Not a failure — `italic atproto status` exits nonzero when the PDS is out of
sync, so it can gate CI. See [Verifying](atproto-verify.md).

## Silent failures

The build succeeds but a change did nothing. There is no schema check on
**top-level** config keys, so these fail silently:

- **A misspelled or wrong-typed top-level key is ignored.** `taxonomy:` instead
  of `taxonomies:`, or `taxonomies: tags` (a string where a list is expected),
  produces no error and no effect. If a config change did nothing, diff your
  key names and shapes against the [config reference](config.md).
- **A scalar taxonomy value in frontmatter is ignored.** `tags: solo` yields
  no term; write `tags: [solo]`.
- **Unknown `:tokens` in a permalink stay literal.** Jekyll/Hugo habits like
  `:year` or `:title` are not expanded — you get a literal `:year` directory in
  the output. Only `:slug`, `:yyyy`, `:mm`, `:dd`, `:term` exist.
- **Date tokens in a non-taxonomy archive permalink expand against the Unix
  epoch** — `:yyyy` in an archive's `permalink:` yields `1970`.
- **A `redirect_from:` colliding with a real page is dropped** — redirect stubs
  write last and never overwrite; the real page wins with no warning.
- **A wikilink that doesn't resolve renders as `<span class="nolink">`**, not
  an error. Find them: `grep -r 'class="nolink"' public/`.

## Symptoms without errors

### A wikilink renders as plain text (`span.nolink`)

Usual causes: **typo or stem mismatch** (`[[My Note]]` needs a file whose stem
slugifies to `my-note`); **the target is a draft** (invisible to the link graph
in production builds, but resolving under `serve` — see
[Drafts](drafts.md)); **duplicate stems** (closest directory wins —
disambiguate with `[[dir/Name]]`, anchored at the content root; see
[Wikilinks](wikilinks.md#how-targets-resolve)). Wikilinks inside code
spans/fences are intentionally left literal.

### My page rendered without its layout

A document with no `template:` (and no collection default supplying one)
renders its body as the final output. Set `template:` in frontmatter or in
`defaults:` — and check the collection's `path:` glob actually matches the
file (globs match relative to `content/`).

### Styles or links break when deployed under a subpath

Hosting at `example.com/blog/` needs `base_path: /blog` under `site:`, and
templates must build URLs with the
[URL filters](url-filters.md) rather than hardcoded
`/`-prefixed paths.

### Two pages landed on the same output path

Permalinks don't collide-check across documents; the last write wins. Make
patterns more specific, or include `:yyyy/:mm/:dd`.

### Dates are wrong

Without frontmatter, `date` falls back to file *created* time, then modified
time — and file timestamps don't survive `git clone` or CI checkouts. For
anything date-sensitive, set `date:` in frontmatter or collection defaults.

### Drafts showed up where I didn't expect (or vanished where I did)

`serve` and `watch` always include drafts; `build` never does unless you pass
`--drafts`. See [Drafts](drafts.md).

### Stale output after changing permalinks or deleting pages

`italic build` writes into `output_dir` without clearing it, so renamed or
deleted pages leave orphans. Run `italic clean && italic build` (safe: `clean`
preserves [`keep_files`](config.md#keep_files) — `.git` by default),
and use `rsync --delete` or equivalent when deploying.

## Still stuck?

Open an issue at <https://github.com/gordonbrander/italic/issues> with the
command you ran and the full error output.
