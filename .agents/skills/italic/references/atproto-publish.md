# Publishing to the ATmosphere

italic can publish your site to your [ATProto](https://atproto.com/) Personal
Data Server (PDS) — the same account server that backs your Bluesky handle.
Each post becomes a
[`site.standard.document`](https://standard.site/docs/lexicons/document)
record (the [standard.site](https://standard.site/) long-form lexicon), under
one [`site.standard.publication`](https://standard.site/docs/lexicons/publication)
record that represents your site. Other ATProto apps (Leaflet, Pckt,
Offprint, AppViews) can then discover, index, recommend, and port your
writing.

## How it differs from `build`

Unlike every other italic command, `atproto publish` talks to your PDS over
HTTP and needs **credentials**. It is safe to re-run: standard.site rkeys are
deterministic hashes of your canonical URLs, so re-running *updates* records
in place instead of creating duplicates — no local bookkeeping needed. The PDS
itself is the record of what's published.

`italic atproto publish` reuses the normal build pipeline to get your fully-rendered
documents, then syncs records — it does **not** write any HTML. Run `italic
build` to update your site; run `italic atproto publish` to update the PDS.

## Quick start

1. **Create an app password.** In Bluesky, go to
   *Settings → App passwords* and make one. (App-password auth is the v1 path;
   OAuth is a planned follow-up.)

2. **Look up your DID** — your account's permanent identifier (handles can
   change; DIDs can't):

   ```sh
   italic atproto did alice.example.com
   # did:plc:abc123…
   ```

3. **Provide credentials** Put the following environment variables in a gitignored
   `.env` file in your project root (italic loads `.env` automatically):

   ```sh
   # .env  (gitignored — never commit your app password)
   ITALIC_ATPROTO_DID=did:plc:abc123…
   ITALIC_ATPROTO_APP_PASSWORD=xxxx-xxxx-xxxx-xxxx
   ```

4. **Set your site metadata** in `config.yaml` — the publication record derives
   from it (`site.title` → name, `site.url` + `site.base_path` → url,
   `site.description` → description), so there is nothing atproto-specific to
   configure:

   ```yaml
   site:
     title: My Garden
     url: https://example.com     # where your HTML actually lives
   ```

   An `atproto:` block is only needed to change the defaults — e.g. to publish
   specific collections instead of every doc:

   ```yaml
   collections:
     posts:
       path: "posts/*.md"

   atproto:
     collections: [posts]         # which collections become documents
   ```

5. **Preview, then publish:**

   ```sh
   italic atproto publish --dry-run   # show what would change — no network calls
   italic atproto publish             # do it
   ```

The first run bootstraps the `site.standard.publication` record and creates a
document per post. Re-running updates the changed records in place.

## Credentials

Your account is identified by its **DID**, not its handle — the atproto spec
treats handles as mutable aliases that "need to be resolved to a DID in almost
all situations", so italic uses the DID everywhere. Look yours up once with
`italic atproto did <handle>`.

Your **app password is a secret and never lives in `config.yaml`** (which you
check into git) — it comes only from the environment. The DID comes only from
the environment too (it also drives the build-time verification artifacts —
see below); the non-secret host falls back to the `atproto:` config:

| Setting | Env var | Config fallback |
|---------|---------|-----------------|
| PDS host | `ITALIC_ATPROTO_PDS_HOST` | `atproto.pds_host` (default `https://bsky.social`) |
| DID | `ITALIC_ATPROTO_DID` | **never** |
| App password | `ITALIC_ATPROTO_APP_PASSWORD` | **never** |

Export the env vars via a gitignored `.env` file as in the quickstart above
(or inline on the command, or in your CI secrets). A value exported in the
shell takes precedence over the `.env` file.

## No local state (for documents)

italic's standard.site document rkeys are pure functions of
`site.url` (+ `base_path`) and each document's output path. This makes publishing
standard.site records idempotent. Every run derives the same addresses and
`putRecord` updates them in place; an interrupted run is simply re-run.
To see what's actually published, ask the PDS via `italic atproto status`
(see [Verifying your records](atproto-verify.md)).

Re-publishing is also cheap: each run reads back what the PDS holds and
compares it to the freshly built records, skipping any that are unchanged — no
blob upload, no repo commit, nothing on the firehose. The summary reports the
split (`done: 2 put, 40 unchanged`), so publishing after editing one post
writes exactly one record.

The one exception is [Bluesky posts](#bluesky-posts): their record keys are
assigned by the PDS at create time and a post must never be created twice, so
created posts are remembered in a committed YAML file, `.italic/bsky.yaml`.

## Publishing full posts

Each document in your configured `collections` (their deduplicated union) maps
straight from its existing fields — no new content modeling:

| `site.standard.document` field | Source |
|--------------------------------|--------|
| `title` | `page.title` |
| `publishedAt` | `page.date` |
| `updatedAt` | `page.updated` (only when newer than `date`) |
| `description` | `page.summary` |
| `path` | the document's URL path (`base_path` + permalink) |
| `tags` | the `tags` taxonomy |
| `textContent` | plaintext of the rendered body |
| `coverImage` | the page's `image:` social image, else `site.image` (uploaded as a blob) |
| `site` | your publication record's AT-URI |

The **publication** record derives from `site:` — `site.title` becomes its
`name` (required to publish; the run fails loudly if missing), `site.url` +
`site.base_path` its `url`, and `site.description` its `description`.
`atproto.publication` adds presentation: an optional `icon:` path uploaded as a
blob, and an optional `theme:` (four `#rrggbb` colors) embedded as the record's
[`basicTheme`](https://standard.site/docs/lexicons/theme/).

### Cover images

`coverImage` shares its source with the [social-card metadata](metadata.md):
the same `image:` frontmatter (then `site.image`) that feeds `og:image` and
`twitter:image`, so the ATProto cover always matches the page's social card.
These are site-root-relative **URL paths** (e.g. `/img/cover.png`), resolved to
files through your `static/` sources (the site's `static/` first, then the
theme's). External URLs and paths that match no static file are skipped with a
warning instead of failing.

A shared image (typically the `site.image` default) is uploaded once per run,
not once per document. `--dry-run` shows each document's resolved cover source.

## Bluesky posts

Docs can opt in (via `bsky:` frontmatter, with `atproto.bsky.enabled` on) to a
one-time `app.bsky.feed.post` announcing them, cross-linked from the document
record. Posts are **created once, never updated**, tracked in the committed
`.italic/bsky.yaml`. Full behavior, replies-as-comments, and the guard rails:
[Bluesky announcement posts](atproto-bsky.md).

## Verification artifacts

With `ITALIC_ATPROTO_DID` and `site.url` set, every `build` (gated on
`atproto.verification`, on by default) emits the domain-ownership proofs: the
`/.well-known/site.standard.publication` file, the per-page
`<link rel="site.standard.document">`, and the [AT tags](metadata.md#at-tags).
`build` needs only the DID, never the app password. Full details:
[verification artifacts](atproto-verify.md#verification-artifacts).

## Previewing a run

```sh
italic atproto publish --dry-run   # build records, show what would be put, no network
```

`--dry-run` is the safe preview — it renders every record and lists each `put`
(create-or-update at its stable record key) without touching the network. Reach
for it whenever you change config or templates.

Drafts are never published: `atproto publish` builds with drafts excluded, so a
`draft: true` post stays out of the PDS just as it stays out of `italic build`.

## Rate limits

A first publish of a large site creates many records at once, and PDS hosts
enforce write rate limits. italic spaces out writes with a small throttle.

## The `atproto:` config block

The whole block is optional — credentials in the environment plus your
existing `site:` metadata are enough to publish. Unknown keys (in the block or
its sub-maps) are an error. The full shape:

```yaml
atproto:
  pds_host: https://bsky.social   # optional
  collections: [posts]            # docs to publish; defaults to [all]
  verification: true              # emit the .well-known + <link> proofs
  publication:
    icon: static/icon.png         # uploaded as a blob
    theme:                        # standard.site basic theme colors
      background: "#1a1a2e"       # quote hex values — # starts a YAML comment
      foreground: "#eeeeee"
      accent: "#e94560"
      accent_foreground: "#ffffff"
  bsky:
    enabled: true                 # publish Bluesky announcement posts
    since: 2026-01-01             # cutoff; defaults to 3 days before now
```

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `pds_host` | string | `https://bsky.social` | PDS XRPC host. Overridden by `ITALIC_ATPROTO_PDS_HOST`. |
| `collections` | list | `[all]` | Collections whose docs become `site.standard.document` records — the deduplicated union of their members. Each must be a declared collection. `[]` publishes only the publication record. |
| `verification` | bool | `true` | Emit the static ownership proofs during `build` (the `.well-known` file and the per-doc `<link>` binding). Needs `ITALIC_ATPROTO_DID` and `site.url`. |
| `publication.icon` | path | none | Uploaded as a blob for the publication record. |
| `publication.theme` | mapping | none | Four `#rrggbb` colors (`background`, `foreground`, `accent`, `accent_foreground`), embedded as the record's [`basicTheme`](https://standard.site/docs/lexicons/theme/). All four required when present; quote them — `#` starts a YAML comment. |
| `bsky.enabled` | bool | `false` | Turn on [Bluesky announcement posts](atproto-bsky.md). |
| `bsky.since` | `YYYY-MM-DD` | 3 days before now | Cutoff — docs dated before it never get posts. |

The publication record's `name`/`url`/`description` come from `site.title`
(required), `site.url` + `site.base_path` (required), and `site.description`.
Your identity (`ITALIC_ATPROTO_DID`) and app password
(`ITALIC_ATPROTO_APP_PASSWORD`) live in the environment, never in config.

Migrating from older configs: `collection:` (singular) became `collections:`
(a list), and `publication.name`/`url`/`description` were removed in favor of
the `site:` fields.

## See also

- [CLI reference](cli.md#command-notes) — env vars, exit codes, `--dry-run`/`--yes`
- [Bluesky announcement posts](atproto-bsky.md)
- [Verifying your records](atproto-verify.md)
- [standard.site](https://standard.site/)
