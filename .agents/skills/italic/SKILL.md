---
name: italic
description: >-
  Build, configure, and publish websites with Italic, a static site generator
  for Markdown notes, blogs, wikis, and digital gardens. Use when the user wants
  to make a website from Markdown or an Obsidian vault, when working in a
  project with a config.yaml and a content/ directory, or when they mention
  italic, wikilinks, backlinks, Tera templates, or publishing to Bluesky/ATProto.
license: AGPL-3.0
---

# Italic

Italic turns a directory of Markdown into a static website. It is a single Rust
binary with no runtime dependencies.

> **Note:** this skill is a stub. The workflow guidance below is enough to get a
> site built; the reference material in `references/` is complete.

## Install the binary

```sh
cargo install italic
```

This puts `italic` on the user's `PATH` (typically `~/.cargo/bin/italic`). If
`cargo` is missing, install Rust first from <https://rustup.rs>.

## Start a site

Always scaffold with `italic new` rather than hand-rolling the directory
layout — it writes the `content/`, `templates/`, `static/` structure and a
commented `config.yaml` that documents every option inline.

```sh
italic new my-site
cd my-site
```

Content goes in `content/` as Markdown files with YAML frontmatter.
`content/index.md` becomes the home page.

## The build loop

`italic build` is the test. It exits nonzero on a malformed `config.yaml`, a
broken template, or an unresolvable reference, so treat a clean exit as the
signal that a change is sound — never assume a template or config key works
without building.

```sh
italic build            # write static files to public/
italic build --drafts   # include draft: true documents
italic serve            # build + serve on :3000 with live reload
```

After changing `config.yaml` or anything in `templates/`, rebuild before
moving on.

## Looking things up

`italic --help` and `italic <command> --help` document every subcommand and
flag, and are always correct for the installed version. Prefer them over
recalling flags from memory.

For everything the binary cannot print, read the file you need from
`references/` next to this skill. If `references/` is missing or a broken
symlink, fetch the same path from
`https://raw.githubusercontent.com/gordonbrander/italic/main/docs/` instead.
Read specific files — do not crawl the whole directory:

| Question | File |
| :--- | :--- |
| What can `config.yaml` contain? | `references/reference/config.md` |
| What frontmatter keys does a document take? | `references/reference/frontmatter.md` |
| What filters and functions do templates have? | `references/reference/templates.md` |
| How do `[[wikilinks]]` and backlinks resolve? | `references/guides/wikilinks.md` |
| How do I group posts into collections or tags? | `references/guides/collections.md`, `references/guides/taxonomies.md` |
| How do I convert an Obsidian vault? | `references/guides/migration.md` |
| How do I deploy the output? | `references/guides/deployment.md` |
| How do I publish to Bluesky/ATProto? | `references/guides/publishing-atproto.md` |

`references/index.md` lists everything else. Skip `references/dev/` and
`references/contributing.md` — those are for people working on Italic itself,
not on a site built with it.
