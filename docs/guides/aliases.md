# Aliases (redirects)

Gardens get reorganized — that's the point. Moving a note inside italic is
painless ([wikilinks](wikilinks.md) resolve by stem, so internal links never
break), but once a note has been shared, bookmarked, or indexed under a URL,
moving it 404s that old URL for the outside world. `aliases:` fixes that: it
emits a tiny redirect page at each old URL that sends visitors to the note's
current location.

## Adding aliases

List the old URLs in frontmatter:

```yaml
---
title: My Reorganized Note
permalink: /notes/reorganized/
aliases:
  - /old-url/
  - /2019/an-even-older-url/
  - /posts/legacy.html
---
```

Each entry becomes a small HTML file at that path that redirects to the note's
canonical URL (here, `/notes/reorganized/`). Nothing else is required — the
feature is always on, and a note with no `aliases:` emits nothing.

## Old URL → output file

An alias is a **literal historical URL**, written verbatim (no `:slug`/`:yyyy`
pattern expansion — a stray `:` stays as-is). It maps to an output file using
the same trailing convention as [permalinks](permalinks.md), plus a rule for
bare paths:

| `aliases:` entry     | File written         | Serves at        |
|----------------------|----------------------|------------------|
| `/old-url/`          | `old-url/index.html` | `/old-url/`      |
| `/old-url`           | `old-url/index.html` | `/old-url/`      |
| `/posts/legacy.html` | `posts/legacy.html`  | `/posts/legacy.html` |
| `/feed.xml`          | `feed.xml`           | `/feed.xml`      |

In short: a trailing slash **or** no file extension writes `index.html` in that
directory (clean URLs); anything with an extension is written as that literal
file. A leading `/` is ignored either way.

## What the redirect page does

Each stub is a minimal page that redirects three ways, so it works for every
visitor and crawler:

- `<link rel="canonical">` points at the new URL — this is what search engines
  consolidate on, passing the old URL's ranking to the new page. (There's
  deliberately no `noindex`, which would *drop* the page instead of forwarding
  its ranking.)
- `<meta http-equiv="refresh">` redirects browsers with JavaScript disabled.
- A small script calls `location.replace`, preserving the URL fragment so a
  deep link like `/old-url/#a-heading` lands on the right heading.

The stub is a fixed built-in; there's no template to override.

## Hosting under a subpath

If your site is served under a `base_path` (e.g. `site.base_path: /blog`), the
redirect *target* is automatically prefixed — an alias points at
`/blog/notes/reorganized/`, not `/notes/reorganized/`. The alias *file* is still
written at the literal path you wrote (`old-url/index.html`), because that path
already represents the on-disk URL your host serves under the subpath. So write
aliases as the host sees them and don't add `/blog` yourself — italic won't
double-prefix.

## Collisions

A redirect stub never overwrites a real page. If an alias resolves to a path
that's already produced by a real document (or another alias), the real output
wins and the stub is skipped with a warning naming both files:

```
duplicate output path 'collide/index.html' from old-note.md — already written by real.md; skipping
```

If two notes claim the *same* alias, the one whose source path sorts first wins,
deterministically. Either way the build still succeeds — a stale alias is a
warning, never a hard error.

## Drafts

Aliases on a [draft](drafts.md) emit nothing in a production build, because the
draft itself is dropped before any output is generated. Publish the draft and
its aliases ship with it.

## See also

- [Permalinks](permalinks.md) — changed a permalink? Add the old one as an alias.
- [Migrating to italic](migration.md) — preserve a site's existing URLs on import.
- [Frontmatter reference](../reference/frontmatter.md#document-keys)
