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

Build the site and sync it to your ATProto PDS as standard.site documents.
Unlike the other commands this one is networked, stateful, and authenticated,
and it writes **no HTML** — it reuses the build only to obtain your rendered
documents. Requires a [`publish:`](config.md#publish) block and credentials
(see the [Publishing guide](../guides/publishing-atproto.md)).

| Flag | Default | Meaning |
|------|---------|---------|
| `--dry-run` | off | Build records and report what would change, making no network calls. |

Drafts are never published (the build runs with drafts excluded). Your account
DID and app password come from the environment (`ITALIC_ATPROTO_DID`,
`ITALIC_ATPROTO_APP_PASSWORD`) — never `config.yaml`. Look your DID up with
[`italic atproto resolve-did`](#italic-atproto-resolve-did-handle).

```sh
italic publish --dry-run   # preview — safe, no network
italic publish             # sync document + publication records
```

## `italic pubstatus`

Read back the records `italic publish` wrote and confirm they still exist on your
PDS and match local state. Networked, authenticated, and **read-only** — it never
writes a record or touches the state file. Unlike `publish` it does **not** build
the site, so it works even while your content is mid-edit. Requires a
[`publish:`](config.md#publish) block, credentials, and a prior `publish` (it
checks what's recorded in `.italic/atproto.yaml`).

For each recorded record it reports `ok` (present, content hash matches),
`CHANGED` (present, but the live CID differs — edited or re-written since publish),
or `MISSING` (absent). If anything is MISSING or CHANGED the command **exits
nonzero**, so it can gate a CI step.

```sh
italic pubstatus   # check every published record
```

See the [Verifying guide](../guides/verifying-atproto.md) for the full workflow,
including manual verification with `curl`.

## `italic atproto resolve-did <handle>`

Resolve an ATProto handle (e.g. `alice.bsky.social`) to its DID — the permanent
account identifier that `ITALIC_ATPROTO_DID` expects. Networked but
**unauthenticated**: it calls the public `com.atproto.identity.resolveHandle`
endpoint; no config or credentials needed.

The bare DID is printed to stdout (so it's scriptable); an `export` hint goes
to stderr.

```sh
italic atproto resolve-did alice.bsky.social
# did:plc:abc123…

export ITALIC_ATPROTO_DID=$(italic atproto resolve-did alice.bsky.social)
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
