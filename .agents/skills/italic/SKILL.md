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

## Install the binary

This skill may be installed before the binary is. Check first — every command
below fails with `italic: command not found` otherwise:

```sh
command -v italic || cargo install italic
```

`cargo install` compiles from source (a few minutes, needs Rust 1.95+) and
puts `italic` at `~/.cargo/bin/italic`. If `cargo` itself is missing, install
Rust from <https://rustup.rs> first. There is no `--version` flag; check the
installed version with `cargo install --list | grep italic`. Toolchain, PATH,
upgrade, and CI details: `references/install.md`.

## Start a site

```sh
italic new my-site     # refuses ANY existing path, even an empty dir
cd my-site
```

The scaffold is a fully commented `config.yaml` (every key, inline docs) plus
empty `content/`, `templates/`, `data/`, `static/`, `archives/`, `themes/`
dirs — **no templates and no content**. Markdown goes in `content/`
(`content/index.md` is the home page); then either:

- **Hand-roll the first layout** — the catch-all `collections:`/`defaults:`
  pairing and a worked `base.html` are in `references/layouts.md`; the full
  blog-and-tags path is `references/recipe-blog-and-tags.md`.
- **Adopt a theme**:

  ```sh
  git clone --depth 1 https://github.com/gordonbrander/italic_themes.git themes/
  # set `theme: themes/<name>` in config.yaml, then:
  italic scaffold      # copies the theme's starter content in
  ```

## The build loop

`italic build` is the test: nonzero exit on a bad config, template, or
reference — and silence on success. Rebuild after every config or template
change; never assume a key or filter works without building.

```sh
italic build            # write static files to public/
italic build --drafts   # include draft: true documents
italic serve            # build + serve on :3000 with live reload
italic clean            # empty public/ in place (keeps .git & keep_files)
```

Sharp edges: `serve`/`watch` **always** include drafts (`build` never does
without `--drafts`), so links that resolve while serving can 404 in
production. After renames or permalink changes run `italic clean && italic
build` — build never deletes stale output. `serve`/`watch` keep serving stale
output after a failed rebuild (watch stderr for `build failed:`).

## Publishing to ATProto (optional)

Credentials come from env vars (a gitignored `.env` is auto-loaded):
`ITALIC_ATPROTO_DID` (find it: `italic atproto did <handle>`) and
`ITALIC_ATPROTO_APP_PASSWORD`. Publishing requires `site.title` and
`site.url`. `italic atproto publish --dry-run` previews with zero network
calls; `italic atproto status` is read-only and exits nonzero when out of
sync (a CI gate). Details: `references/atproto-publish.md`.

## Looking things up

`italic --help` and `italic <command> --help` document every subcommand and
flag, and are always correct for the installed version. The scaffolded
`config.yaml` documents every config key inline. For everything else, read
the file for your question from `references/` next to this skill — this table
is the complete listing. If `references/` is missing, fetch the same path
from
`https://raw.githubusercontent.com/gordonbrander/italic/main/.agents/skills/italic/references/`.

| Question | File |
| :--- | :--- |
| How do I install/upgrade the binary? Rust toolchain? `command not found`? | `references/install.md` |
| A build error or wrong output — what does this message mean? | `references/troubleshooting.md` (grep it for the verbatim stderr text) |
| What can `config.yaml` contain? | `references/config.md` |
| What frontmatter keys does a document take? | `references/frontmatter.md` |
| What do the commands print? Env vars? Exit codes? | `references/cli.md` |
| What is a document / an `id_path`? | `references/content-model.md` |
| Build stages — why can't my template see X? | `references/build-pipeline.md` |
| What variables exist in this template? Which phase am I in? | `references/template-context.md` |
| How do I list docs — `collection()`, `all()`, `taxonomy()`, `doc()`? | `references/template-functions.md` |
| How do I sort/scope doc lists — `backlinks`, `related`, `dirtree`…? | `references/doc-list-filters.md` |
| How do I turn a path into a URL? | `references/url-filters.md` |
| How do I truncate, get reading time, render Markdown strings? | `references/text-filters.md` |
| How do layouts and inheritance work? First template? | `references/layouts.md` |
| How do I emit `<head>` metadata / social cards? | `references/metadata.md` |
| How do I define a component (shortcode)? | `references/components.md` |
| How do `[[wikilinks]]`, block refs, embeds, backlinks work? | `references/wikilinks.md` |
| What Markdown syntax works? Where do images live? | `references/authoring.md` |
| How do I group docs into collections? | `references/collections.md` |
| How do tags/taxonomies/hashtags work? | `references/taxonomies.md` |
| How do I make listing pages, tag pages, feeds, sitemaps? | `references/archives.md` |
| How do I customize URLs? | `references/permalinks.md` |
| How do I redirect old URLs? | `references/redirects.md` |
| How do drafts behave? | `references/drafts.md` |
| How do I use or write a theme? | `references/themes.md` |
| How does related-pages ranking work? | `references/related.md` |
| How do I add site data files (`{{ data.* }}`)? | `references/data.md` |
| How do I deploy the output? | `references/deployment.md` |
| Migrating from Obsidian/Jekyll/Hugo/Quartz/Zola? | `references/migration.md` |
| Build me a blog with tags and a feed, end to end | `references/recipe-blog-and-tags.md` |
| How do I publish to my ATProto PDS? | `references/atproto-publish.md` |
| How do I announce posts on Bluesky? | `references/atproto-bsky.md` |
| Did my publish work? Verify/inspect/delete records? | `references/atproto-verify.md` |

`references/dev/` (markup internals, rkey derivation, an ATProto primer,
contributing) is for working on Italic itself — skip it when building a site.

## What italic does NOT do

Genre habits that silently fail here — check these before debugging:

- **No `check`/`lint` subcommand and no `--version` flag.** `italic build`'s
  exit code is the only validity signal, and success prints **nothing**.
- **Unknown top-level `config.yaml` keys are silently ignored** — a clean
  build does not prove a config key took effect.
- **Unknown permalink `:tokens` stay literal** (`:year` from Jekyll/Hugo
  yields a literal `:year` directory). Only `:slug :yyyy :mm :dd :term` exist.
- **`![[Note]]` does not transclude** — embeds are for media files only.
  Obsidian `aliases:` and `publish:` frontmatter are inert.
- **`collection()`, `all()`, `taxonomy()`, `doc()`, `backlinks`, `related`
  work only in layout templates**, never inside a document body — the error
  says "is not available in the markup phase".
