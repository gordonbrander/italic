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


- It talks to your PDS over HTTP.
- It needs **credentials**.
- standard.site rkeys are deterministic hashes of your canonical URLs, so re-running
  *updates* records in place instead of creating duplicates — no local
  bookkeeping needed. The PDS itself is the record of what's published.

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

Export the env vars via a `.env` file in your site directory (or pass them inline on the
command, or set them in your CI secrets):

```sh
# .env  (gitignored — never commit your app password)
ITALIC_ATPROTO_DID=did:plc:abc123…
ITALIC_ATPROTO_APP_PASSWORD=xxxx-xxxx-xxxx-xxxx
```

A value exported in the shell takes precedence over the `.env`
file.

## No local state (for documents)

italic's standard.site document rkeys are pure functions of
`site.url` (+ `base_path`) and each document's output path. This makes publishing
standard.site records idempotent. Every run derives the same addresses and
`putRecord` updates them in place; an interrupted run is simply re-run.
To see what's actually published, ask the PDS via `italic atproto status`
(see [Verifying your records](verifying-atproto.md)).

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

Besides the long-form document record, italic can announce a doc with a short
[`app.bsky.feed.post`](https://docs.bsky.app/docs/advanced-guides/posts) — a
regular Bluesky post from your account, carrying your text plus a link card
back to the article. The document record cross-links it via its `bskyPostRef`
field, so apps that read standard.site documents can find the announcement
(and its replies) — comments for your post, tracked off-platform.

Posting is doubly opt-in. Turn the feature on in config:

```yaml
atproto:
  bsky:
    enabled: true
```

…and give each doc you want announced a `bsky:` frontmatter key with the post
text (≤ 300 graphemes — Bluesky's cap):

```yaml
---
title: Composting for beginners
bsky: "New post: composting for beginners. Everything I wish I'd known 🌱"
---
```

Docs without a `bsky:` key are simply skipped — omitting the key is how you
deliberately not-announce something. The post carries a link card
(`app.bsky.embed.external`) built from the doc's canonical URL, title, and
summary, with the doc's [cover image](#cover-images) as the thumbnail.

### Posts are created once

Documents update in place, but a Bluesky post is a social artifact — people
reply to it, like it, repost it — so italic **creates each doc's post exactly
once and never updates or deletes it**. Editing the `bsky:` text after the
post exists does nothing. Created posts are recorded in `.italic/bsky.yaml`, a
human-readable file mapping each doc to its post:

```yaml
version: 1
posts:
  posts/composting.md:
    uri: at://did:plc:abc123/app.bsky.feed.post/3lwabc22xyz
    cid: bafyreib2…
    createdAt: 2026-07-20T18:04:11.000Z
```

**Commit this file** — it is what prevents duplicate posts. If you rename a
doc, move its entry to the new id_path, or the renamed doc will look new and
get a second post. `italic atproto status` reports docs whose post is still
pending (`POST PENDING`) and state entries whose doc has gone away (`STALE`).

### Guard rails

Two guards prevent accidentally blasting posts:

- **A date cutoff.** Docs dated before `atproto.bsky.since` never get posts;
  when `since` is unset it defaults to **3 days before now**, so enabling the
  feature over an old archive announces nothing by accident. Set `since`
  explicitly to announce older docs.
- **A confirmation prompt.** Before creating anything, `italic atproto
  publish` lists every pending post and asks. Pass `--yes` to skip the prompt
  (required in CI, where stdin isn't a terminal).

`--dry-run` shows pending posts too, without touching the network.

## Verification artifacts

standard.site verifies that you own the domain two ways, and italic emits both
during `build` (gated on `atproto.verification`, on by default). Both AT-URIs
are **derived** — record keys are hashes of your canonical URLs, so the only
input that isn't already in your config is your account DID, read from the same
`ITALIC_ATPROTO_DID` env var publishing uses. Note that `build` needs only the
DID (a public identifier), never the app password — set just
`ITALIC_ATPROTO_DID` in your CI/deploy environment.

With `ITALIC_ATPROTO_DID` and `site.url` set, every build — including CI builds
— emits:

1. **Publication proof** — a static file at
   `/.well-known/site.standard.publication` containing your publication's AT-URI.
   Generated automatically; nothing to template.

2. **Per-document proof** — a `<link rel="site.standard.document">` tag in each
   published page's `<head>`. The built-in [metadata filters](metadata.md) emit
   it automatically: `{{ page | metadata }}` includes it, or compose
   `{{ page | standard_link }}` yourself. Hand-rolled heads can still read the
   raw AT-URI from `page.data.atproto_uri`.

Because `italic atproto publish` derives record addresses the same way — and
authenticates *as* that same DID — build and deploy order doesn't matter: HTML
deployed before a publish already carries the right URIs, and verification
passes the moment the records exist. The proofs and the records can't point at
different repos, because both come from the one `ITALIC_ATPROTO_DID`.

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

## Configuration summary

The `atproto:` block is entirely optional; the full shape (see the
[config reference](../reference/config.md#atproto) for details):

```yaml
atproto:
  pds_host: https://bsky.social   # optional
  collections: [posts]            # docs to publish; defaults to [all]
  verification: true              # emit the .well-known + <link> proofs
  publication:
    icon: static/icon.png
    theme:                        # standard.site basic theme colors
      background: "#1a1a2e"       # quote hex values — # starts a YAML comment
      foreground: "#eeeeee"
      accent: "#e94560"
      accent_foreground: "#ffffff"
  bsky:
    enabled: true                 # publish Bluesky announcement posts
    since: 2026-01-01             # cutoff; defaults to 3 days before now
```

The publication record's `name`/`url`/`description` come from `site.title`,
`site.url` + `site.base_path`, and `site.description`. Your identity
(`ITALIC_ATPROTO_DID`) and app password (`ITALIC_ATPROTO_APP_PASSWORD`) live in
the environment, never in config.

## See also

- [CLI reference](../reference/cli.md#italic-atproto-publish) — the `atproto publish` command and its flags
- [Configuration reference](../reference/config.md#atproto) — every `atproto:` key
- [Deployment](deployment.md) — hosting your static HTML
- [standard.site](https://standard.site/)
