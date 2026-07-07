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

**italic stays a static generator.** Publishing is a *client* operation — it
writes records into a PDS you already own — not a server you have to run. Keep
hosting your HTML wherever you host it today (GitHub Pages, Netlify, …);
publishing to the ATmosphere is additive.

## How it differs from `build`

Everything italic does is pure, offline, and stateless: `content/` in,
`public/` out, no memory between runs. Publishing keeps the stateless part but
is **networked and authenticated**:

- It talks to your PDS over HTTP.
- It needs **credentials**.
- Record keys are deterministic hashes of your canonical URLs, so re-running
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

3. **Provide credentials** via environment variables — never `config.yaml`:

   ```sh
   export ITALIC_ATPROTO_DID=did:plc:abc123…
   export ITALIC_ATPROTO_APP_PASSWORD=xxxx-xxxx-xxxx-xxxx
   ```

4. **Configure `atproto:`** in `config.yaml`:

   ```yaml
   collections:
     posts:
       path: "posts/*.md"

   atproto:
     collection: posts            # which collection becomes documents
     publication:
       name: My Garden
       url: https://example.com   # where your HTML actually lives
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

Export the env vars in your shell before publishing (or pass them inline on the
command, or set them in your CI secrets):

```sh
export ITALIC_ATPROTO_DID=did:plc:abc123…
export ITALIC_ATPROTO_APP_PASSWORD=xxxx-xxxx-xxxx-xxxx
```

## No local state

Publishing keeps no local state. Record keys are pure functions of `site.url`
(+ `base_path`) and each document's output path, so every run derives the same
addresses and `putRecord` updates them in place; an interrupted run is simply
re-run. To see what's actually published, ask the PDS —
`italic atproto status` does exactly that (see
[Verifying your records](verifying-atproto.md)).

Re-publishing is also cheap: each run reads back what the PDS holds and
compares it to the freshly built records, skipping any that are unchanged — no
blob upload, no repo commit, nothing on the firehose. The summary reports the
split (`done: 2 put, 40 unchanged`), so publishing after editing one post
writes exactly one record.

> Earlier versions of italic recorded publishes in `.italic/atproto.yaml`. That
> file is no longer read or written — you can delete it.

## Publishing full posts

Each document in your configured `collection` maps straight from its existing
fields — no new content modeling:

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

The **publication** record comes from `atproto.publication` — `name` and `url`
are required to publish it (the build fails loudly if either is missing). An
optional `icon:` path is uploaded as a blob.

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
   it automatically: `{{ page | metadata(site=site) }}` includes it, or compose
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

The full `atproto:` block (see the [config reference](../reference/config.md#atproto)
for details):

```yaml
atproto:
  pds_host: https://bsky.social   # optional
  collection: posts               # docs to publish; defaults to `all`
  verification: true              # emit the .well-known + <link> proofs
  publication:
    name: My Garden               # required to publish
    url: https://example.com      # required to publish
    description: A digital garden.
    icon: static/icon.png
```

Your identity (`ITALIC_ATPROTO_DID`) and app password
(`ITALIC_ATPROTO_APP_PASSWORD`) live in the environment, never in config.

## See also

- [CLI reference](../reference/cli.md#italic-atproto-publish) — the `atproto publish` command and its flags
- [Configuration reference](../reference/config.md#atproto) — every `atproto:` key
- [Deployment](deployment.md) — hosting your static HTML
- [standard.site](https://standard.site/)
