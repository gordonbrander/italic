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

Everything else italic does is pure, offline, and stateless: `content/` in,
`public/` out, no memory between runs. Publishing is the one exception — it is
**networked, stateful, and authenticated**:

- It talks to your PDS over HTTP.
- It remembers what it published last time in a small **state file** so
  re-running *updates* records in place instead of creating duplicates.
- It needs **credentials**.

`italic publish` reuses the normal build pipeline to get your fully-rendered
documents, then syncs records — it does **not** write any HTML. Run `italic
build` to update your site; run `italic publish` to update the PDS.

## Quick start

1. **Create an app password.** In Bluesky, go to
   *Settings → App passwords* and make one. (App-password auth is the v1 path;
   OAuth is a planned follow-up.)

2. **Provide credentials** via environment variables — never `config.yaml`:

   ```sh
   export ITALIC_ATPROTO_HANDLE=alice.example.com
   export ITALIC_ATPROTO_APP_PASSWORD=xxxx-xxxx-xxxx-xxxx
   ```

3. **Configure `publish:`** in `config.yaml`:

   ```yaml
   collections:
     posts:
       path: "posts/*.md"

   publish:
     collection: posts            # which collection becomes documents
     publication:
       name: My Garden
       url: https://example.com   # where your HTML actually lives
   ```

4. **Preview, then publish:**

   ```sh
   italic publish --dry-run   # show what would change — no network calls
   italic publish             # do it
   ```

The first run bootstraps the `site.standard.publication` record and creates a
document per post. Re-running updates the changed records in place.

## Credentials

Your **app password is a secret and never lives in `config.yaml`** (which you
check into git) — it comes only from the environment. The non-secret host and
handle come from the environment too, falling back to the `publish:` config:

| Setting | Env var | Config fallback |
|---------|---------|-----------------|
| PDS host | `ITALIC_ATPROTO_PDS_HOST` | `publish.pds_host` (default `https://bsky.social`) |
| Handle | `ITALIC_ATPROTO_HANDLE` | `publish.handle` |
| App password | `ITALIC_ATPROTO_APP_PASSWORD` | **never** |

Export the env vars in your shell before publishing (or pass them inline on the
command, or set them in your CI secrets):

```sh
export ITALIC_ATPROTO_HANDLE=alice.example.com
export ITALIC_ATPROTO_APP_PASSWORD=xxxx-xxxx-xxxx-xxxx
```

> **Add `.italic/` to your `.gitignore`.** It holds the publish state file
> described below.

## The state file

italic records what it published in `.italic/atproto.yaml`: the publication's
AT-URI, your account DID, and a per-document map of `id_path → { document }`
record keys and CIDs. It's plain, human-readable YAML — you can open it to inspect
what was published, or hand-edit an entry to recover from a mistake.

Documents use stable record keys derived from their canonical URL, so they
update in place (`putRecord`) every run — and the keys are reconstructible from
config + content even if the state file is lost. The state file mainly lets
`pubstatus` verify what's on the PDS against what was last published.

Keep `.italic/atproto.yaml` out of public repos (it isn't secret, but it's
noise). It is written incrementally, after each record, so an interrupted run
never loses track of what it already wrote.

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

The **publication** record comes from `publish.publication` — `name` and `url`
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
during `build` (gated on `publish.verification`, on by default). Both AT-URIs
are **derived** — record keys are hashes of your canonical URLs, so the only
input that isn't already in your config is your account DID. Set it once:

```yaml
publish:
  did: did:plc:…   # printed by `italic publish` on login
```

With `did` and `site.url` set, every build — including CI builds that have no
publish state file — emits:

1. **Publication proof** — a static file at
   `/.well-known/site.standard.publication` containing your publication's AT-URI.
   Generated automatically; nothing to template.

2. **Per-document proof** — a `<link rel="site.standard.document">` tag in each
   published page's `<head>`. The built-in [metadata filters](metadata.md) emit
   it automatically: `{{ page | metadata(site=site) }}` includes it, or compose
   `{{ page | standard_link }}` yourself. Hand-rolled heads can still read the
   raw AT-URI from `page.data.atproto_uri`.

Because `italic publish` derives record addresses the same way, build and
deploy order doesn't matter: HTML deployed before a publish already carries the
right URIs, and verification passes the moment the records exist. As a
safeguard, `publish` errors if the account you authenticate as doesn't match
`publish.did` — otherwise the deployed proofs would point at a different repo
than the records land in.

## Previewing a run

```sh
italic publish --dry-run   # build records, diff against state, no network
```

`--dry-run` is the safe preview — it renders every record and reports what it
would *create* vs. *update* without touching the network. Reach for it whenever
you change config or templates.

Drafts are never published: `publish` builds with drafts excluded, so a
`draft: true` post stays out of the PDS just as it stays out of `italic build`.

## Rate limits

A first publish of a large site creates many records at once, and PDS hosts
enforce write rate limits. italic spaces out writes with a small throttle.

## Configuration summary

The full `publish:` block (see the [config reference](../reference/config.md#publish)
for details):

```yaml
publish:
  pds_host: https://bsky.social   # optional
  handle: alice.example.com       # or via ITALIC_ATPROTO_HANDLE
  did: did:plc:abc123             # enables build-time verification artifacts
  collection: posts               # docs to publish; defaults to `all`
  verification: true              # emit the .well-known + <link> proofs
  publication:
    name: My Garden               # required to publish
    url: https://example.com      # required to publish
    description: A digital garden.
    icon: static/icon.png
```

## See also

- [CLI reference](../reference/cli.md#italic-publish) — the `publish` command and its flags
- [Configuration reference](../reference/config.md#publish) — every `publish:` key
- [Deployment](deployment.md) — hosting your static HTML
- [standard.site](https://standard.site/)
