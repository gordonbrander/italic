# Drafts

Mark work-in-progress pages as drafts to keep them out of your published site
while still seeing them locally.

## Marking a draft

```markdown
---
title: Work in progress
draft: true
---
Not ready to publish yet.
```

## How drafts behave

Drafts are dropped at the very start of the build, so they never appear in the
output — and never show up in collections, taxonomies, backlinks, related
pages, archives, or feeds either. It's as if the file weren't there.

| Command | Drafts included? |
|---------|------------------|
| `italic build` | No |
| `italic build --drafts` | Yes |
| `italic serve` | Always |
| `italic watch` | Always |

`serve` and `watch` always include drafts so you can preview while writing.
Use `build --drafts` for a one-off build that includes them — e.g. a staging
deploy for review.

One consequence worth knowing: because a draft is invisible to the link graph,
wikilinks *to* a draft render as unresolved (`<span class="nolink">`) in a
production build, and resolve again when the draft ships.

## What does *not* hide a page

`draft: true` is the only hiding mechanism. These have no effect:

- **`publish: false`** (Obsidian Publish) — inert; the page builds normally.
- **A future `date:`** — future-dated posts build and publish like any other
  (unlike Jekyll/Hugo).
- **Underscore or dot prefixes** on filenames — no special meaning; even
  `.hidden.md` builds into a page.

**Verify:** `italic build`, then confirm the draft's output file is absent
from `public/`.

## See also

- [CLI reference](cli.md)
- [The build pipeline](build-pipeline.md) — why drafts vanish everywhere at once
