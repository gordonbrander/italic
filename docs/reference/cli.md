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

## `italic atproto publish`

Build the site and sync it to your ATProto PDS as standard.site documents.
Unlike the other commands this one is networked and authenticated, and it
writes **no HTML** — it reuses the build only to obtain your rendered
documents. Records that are identical to what the PDS already holds are
skipped (no blob upload, no repo commit); the summary reports the split
(`done: 2 put, 40 unchanged`). Requires `site.title`, `site.url`, and
credentials — no [`atproto:`](config.md#atproto) block is needed (see the
[Publishing guide](../guides/publishing-atproto.md)).

| Flag | Default | Meaning |
|------|---------|---------|
| `--dry-run` | off | Build records and report what would change, making no network calls. |
| `--yes` | off | Skip the confirmation prompt before creating new [Bluesky posts](../guides/publishing-atproto.md#bluesky-posts). Required in CI when posts are pending (stdin is not a terminal). |

With [`atproto.bsky.enabled`](config.md#atproto), docs carrying `bsky:`
frontmatter also get an `app.bsky.feed.post` announcement — created once, then
recorded in the committed `.italic/bsky.yaml`, and never touched again.

Drafts are never published (the build runs with drafts excluded). Your account
DID and app password come from the environment (`ITALIC_ATPROTO_DID`,
`ITALIC_ATPROTO_APP_PASSWORD`) — never `config.yaml`. Look your DID up with
[`italic atproto did`](#italic-atproto-did-handle).

```sh
italic atproto publish --dry-run   # preview — safe, no network
italic atproto publish             # sync document + publication records
```

## `italic atproto status`

Compare the records your site *should* have against what your PDS actually
holds (via `com.atproto.repo.listRecords`) — the PDS is the source of truth
for documents. Networked, authenticated, and **read-only** — it
never writes a record. Like `atproto publish` it builds the site index (drafts
excluded, no HTML written) to derive the expected records, so it requires the
same inputs: `site.title`, `site.url`, and credentials — no
[`atproto:`](config.md#atproto) block needed.

For each expected record it reports `ok` (present and identical to the locally
built record), `CHANGED` (present but differing — unpublished local edits, or
rewritten by another client; re-publish to reconcile), or `MISSING` (absent —
re-publish to fix), plus `ORPHANED` for records on the PDS that reference your
publication but have no matching local doc (deleted or renamed sources). With
[Bluesky posts](../guides/publishing-atproto.md#bluesky-posts) enabled it also
reports `POST PENDING` for opted-in docs whose post hasn't been created, and
`STALE` for `.italic/bsky.yaml` entries whose doc has gone away. If anything is
MISSING, CHANGED, or POST PENDING the command **exits nonzero**, so it can gate
a CI step; orphans and stale entries only warn.

```sh
italic atproto status   # check every published record
```

See the [Verifying guide](../guides/verifying-atproto.md) for the full workflow,
including manual verification with `curl`.

## `italic atproto did <handle>`

Resolve an ATProto handle (e.g. `alice.bsky.social`) to its DID — the permanent
account identifier that `ITALIC_ATPROTO_DID` expects. Networked but
**unauthenticated**: it calls the public `com.atproto.identity.resolveHandle`
endpoint; no config or credentials needed.

The bare DID is printed to stdout (so it's scriptable); an `export` hint goes
to stderr.

```sh
italic atproto did alice.bsky.social
# did:plc:abc123…

export ITALIC_ATPROTO_DID=$(italic atproto did alice.bsky.social)
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
