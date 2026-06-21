# CLI reference

The `italic` binary has the following subcommands. Run `italic --help` or
`italic <command> --help` for the same information at the terminal.

## `italic build`

Run the full build pipeline once, writing the site into the output directory
(`public/` by default; see [`output_dir`](config.md#directories)).

| Flag | Default | Meaning |
|------|---------|---------|
| `--drafts` | off | Include documents marked `draft: true` in the output. |

Without `--drafts`, drafts are dropped at the start of the build and never
appear in the output — nor in collections, taxonomies, or backlinks. See
[Drafts](../guides/drafts.md).

```sh
italic build            # production build
italic build --drafts   # staging build that includes drafts
```

## `italic serve`

Build the site, serve it locally with live reload, and rebuild on every change
to the source directories. Drafts are always included while serving.

| Flag | Default | Meaning |
|------|---------|---------|
| `--port <PORT>` | `3000` | Port to bind. |
| `--host <HOST>` | `127.0.0.1` | Host address to bind. |

```sh
italic serve
italic serve --port 8080
italic serve --host 0.0.0.0 --port 8080   # reachable from other devices on your network
```

## `italic watch`

Rebuild on every change to the source directories, without running a server.
Drafts are always included while watching. Useful when another tool is serving
the output directory.

```sh
italic watch
```

## `italic publish`

Build the site and sync it to your ATProto PDS as standard.site documents and
(optionally) Bluesky summaries. Unlike the other commands this one is networked,
stateful, and authenticated, and it writes **no HTML** — it reuses the build only
to obtain your rendered documents. Requires a [`publish:`](config.md#publish)
block and credentials (see the
[Publishing guide](../guides/publishing-atproto.md)).

| Flag | Default | Meaning |
|------|---------|---------|
| `--dry-run` | off | Build records and report what would change, making no network calls. |
| `--documents-only` | off | Sync only `site.standard.document`/`publication` records (skip Bluesky). |
| `--bsky-only` | off | Create only `app.bsky.feed.post` summaries (skip document records). |

Drafts are never published (the build runs with drafts excluded). The app
password comes from the environment (`ITALIC_ATPROTO_APP_PASSWORD`) — never
`config.yaml`.

```sh
italic publish --dry-run        # preview — safe, no network
italic publish                  # documents + (if enabled) Bluesky summaries
italic publish --documents-only # long-form records only
```

## `italic new <path>`

Scaffold an empty starter site at `<path>`. The path must not already exist.
The scaffold includes a fully commented `config.yaml` showing every available
key with its default.

```sh
italic new my-site
```

## `italic scaffold`

Copy the configured theme's starter content into your `content/` directory.
Requires a `theme:` key in `config.yaml`. Existing files are skipped, so it is
safe to run in a project that already has content. See
[Themes](../guides/themes.md).

```sh
italic scaffold
```

## `italic clean`

Remove the output directory (`public/` by default).

```sh
italic clean
```

## See also

- [Configuration reference](config.md) — directories the commands read and write
- [Quickstart](../getting-started/quickstart.md) — the commands in context
