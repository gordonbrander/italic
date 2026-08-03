# CLI reference

`italic --help` and `italic <command> --help` are authoritative for subcommands
and flags — this page covers only what `--help` cannot print. There is **no
`--version` flag** and no `check`/`lint` command: `italic build`'s exit code is
the validity check.

## Environment

A `.env` file in the working directory is loaded automatically at startup.

| Variable | Used by | Meaning |
|----------|---------|---------|
| `ITALIC_ATPROTO_DID` | `atproto publish`/`status`; `build` (verification artifacts) | Your account DID. Look it up: `italic atproto did <handle>`. |
| `ITALIC_ATPROTO_APP_PASSWORD` | `atproto publish`/`status` | App password (create at bsky.app/settings/app-passwords). Never in `config.yaml`. |
| `ITALIC_ATPROTO_PDS_HOST` | `atproto publish`/`status` | Overrides `atproto.pds_host`. |

## Output and exit codes

- `italic build` prints **nothing on success** — the exit code is the only
  signal. Nonzero means the build failed; the error goes to stderr.
- `italic serve` prints `serving http://<addr>` and, on each rebuild,
  `rebuilt in <duration>` or `build failed: <error>` — **it keeps running and
  keeps serving the last good output after a failed rebuild.** Watch stderr;
  don't infer success from the process staying alive. Same for `italic watch`.
- Watchers are registered once at startup. Changing `theme:` or a `*_dir` in
  `config.yaml` rebuilds but does not watch the new locations — restart
  `serve`/`watch`.
- `italic clean` prints `cleaned <dir> (<n> removed, <n> kept)`.
- `italic atproto status` **exits nonzero** when any record is MISSING,
  CHANGED, or POST PENDING — usable as a CI gate. Token meanings in
  [Verifying](atproto-verify.md).
- `italic atproto did <handle>` prints the bare DID to stdout (scriptable;
  the `export` hint goes to stderr):
  `export ITALIC_ATPROTO_DID=$(italic atproto did alice.bsky.social)`

## Command notes

- **`build`** — without `--drafts`, drafts are dropped at read time and never
  appear anywhere (collections, taxonomies, backlinks). `serve` and `watch`
  **always** include drafts. See [Drafts](drafts.md).
- **`new <path>`** — refuses **any** existing path, even an empty directory;
  there is no merge. The scaffold is a fully commented `config.yaml` showing
  every key, plus empty `content/`, `templates/`, `data/`, `static/`,
  `archives/`, `themes/` dirs — **no templates and no content**.
- **`scaffold`** — copies the configured theme's starter content into
  `content/`; requires `theme:`; skips existing files, so it's safe to re-run.
  See [Themes](themes.md).
- **`clean`** — empties the output directory in place, preserving
  [`keep_files`](config.md#keep_files) matches (`.git` by default). `build`
  writes over the top and never removes, so `italic clean && italic build` is
  how you drop orphans after renames or deletions.
- **`atproto publish`** — networked and authenticated; writes **no HTML** (it
  builds only to derive records, drafts excluded). Idempotent: unchanged
  records are skipped (`done: 2 put, 40 unchanged`); `--dry-run` makes no
  network calls at all. `--yes` skips the Bluesky-post confirmation prompt —
  required in CI when posts are pending (stdin is not a terminal). Requires
  `site.title`, `site.url`, and the env credentials; no `atproto:` block
  needed. See [Publishing](atproto-publish.md).
- **`atproto status`** — networked, authenticated, **read-only**; same inputs
  as `publish`.

## See also

- [Configuration reference](config.md) — directories the commands read/write
- [Troubleshooting](troubleshooting.md) — verbatim error strings
