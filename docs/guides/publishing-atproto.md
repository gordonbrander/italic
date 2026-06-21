# Publishing to Bluesky & the ATmosphere

italic can publish your site to your [ATProto](https://atproto.com/) Personal
Data Server (PDS) — the same account server that backs your Bluesky handle. Two
related things ship:

1. **Long-form documents** — each post becomes a
   [`site.standard.document`](https://standard.site/docs/lexicons/document)
   record (the [standard.site](https://standard.site/) long-form lexicon), under
   one [`site.standard.publication`](https://standard.site/docs/lexicons/publication)
   record that represents your site. Other ATProto apps (Leaflet, Pckt,
   Offprint, AppViews) can then discover, index, recommend, and port your
   writing.
2. **Bluesky summaries** — optionally, each post also gets a short
   [`app.bsky.feed.post`](https://docs.bsky.app/docs/api/app-bsky-feed-post): a
   teaser with a link card back to the canonical article, announced to your
   followers.

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

Secrets never live in `config.yaml` (which you check into git). italic resolves
each value from, in order of precedence:

1. an **environment variable**,
2. a gitignored **credentials file** at `.italic/credentials`,
3. for the non-secret host/handle only, the `publish:` config.

| Setting | Env var | File key | Config fallback |
|---------|---------|----------|-----------------|
| PDS host | `ITALIC_ATPROTO_PDS_HOST` | `pds_host` | `publish.pds_host` (default `https://bsky.social`) |
| Handle | `ITALIC_ATPROTO_HANDLE` | `handle` | `publish.handle` |
| App password | `ITALIC_ATPROTO_APP_PASSWORD` | `app_password` | **never** |

The credentials file is a simple `KEY=VALUE` list (blank lines and `#` comments
ignored):

```
# .italic/credentials  — add `.italic/` to .gitignore
handle = alice.example.com
app_password = xxxx-xxxx-xxxx-xxxx
```

> **Add `.italic/` to your `.gitignore`.** It holds both your credentials file
> (if you use one) and the publish state file described below.

## The state file

italic records what it published in `.italic/atproto.json`: the publication's
AT-URI, your account DID, and a per-document map of `id_path → { document, bsky }`
record keys and CIDs.

This file is **load-bearing for correctness**, not just speed:

- **Documents** use stable, slug-derived record keys, so they update in place
  (`putRecord`) every run — reconstructible even if the state is lost.
- **Bluesky posts** are create-once. The post entry in the state file is the
  *only* thing that stops a re-run from posting the same announcement again.
  Lose the file and you risk re-announcing every post.

So keep `.italic/atproto.json` — commit it to a private repo, or back it up.
Don't commit it to a public one (it isn't secret, but it's noise). It is written
incrementally, after each record, so an interrupted run never loses a post it
already created.

## Publishing documents (feature 1)

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
| `coverImage` | the `cover:` frontmatter image (uploaded as a blob) |
| `site` | your publication record's AT-URI |

The **publication** record comes from `publish.publication` — `name` and `url`
are required to publish it (the build fails loudly if either is missing). An
optional `icon:` path is uploaded as a blob.

## Bluesky summaries (feature 2)

Turn announcements on under `publish.bluesky`:

```yaml
publish:
  collection: posts
  publication:
    name: My Garden
    url: https://example.com
  bluesky:
    enabled: true
    post_template: "{{ title }} — {{ summary }}"
    include_link_card: true
    thumb: cover               # cover | none
    announce_after: 2026-01-01 # skip older posts on a first run
```

For each eligible post italic creates an `app.bsky.feed.post` with an
`app.bsky.embed.external` **link card** pointing at the canonical article
(`uri` + `title` + `description`, plus an optional thumbnail). The post is tied
back to the long-form record via the document's `bskyPostRef`, so a reader who
lands on either can find the other.

### Post text

The text comes from, in order:

1. a per-post `bsky_text:` frontmatter override, else
2. the `post_template` (a small [Tera](https://keats.github.io/tera/) string with
   `title` and `summary` in scope), else
3. the post's `summary`.

Bluesky caps post text at **300 graphemes** (not bytes), so longer text is
truncated with an ellipsis on a grapheme boundary — emoji and combining
sequences are never split. The article link lives in the embed card, not the
text, so no facets are needed.

### Create-once

Bluesky posts are treated as roughly immutable by clients, so italic's contract
is **create the announcement once, never repost**. A post already recorded in
the state file is skipped on later runs. (Editing a post on a later run is a
possible future enhancement.)

### Per-post control

Two frontmatter keys override the defaults on a single post:

```yaml
---
title: A Quiet Note
bsky: false                       # never announce this post
# or:
bsky_text: "New: A Quiet Note →"  # custom announcement text
---
```

## Verification artifacts

standard.site verifies that you own the domain two ways, and italic emits both
during `build` (gated on `publish.verification`, on by default). They only
appear **after** your first `publish`, since both need the AT-URIs that
publishing assigns:

1. **Publication proof** — a static file at
   `/.well-known/site.standard.publication` containing your publication's AT-URI.
   Generated automatically; nothing to template.

2. **Per-document proof** — a `<link rel="site.standard.document">` tag in each
   published page's `<head>`. italic ships no default theme, so you add the tag
   yourself; italic provides the AT-URI as `page.data.atproto_uri`:

   ```html
   {% if page.data.atproto_uri %}
   <link rel="site.standard.document" href="{{ page.data.atproto_uri | safe }}">
   {% endif %}
   ```

So the recommended flow is: `italic publish` once (to mint the records and write
state), then `italic build` (to emit the proofs), then deploy your HTML.

## Selective and partial runs

```sh
italic publish --dry-run          # build records, diff against state, no network
italic publish --documents-only   # site.standard.document/publication only
italic publish --bsky-only        # app.bsky.feed.post summaries only
```

`--dry-run` is the safe preview — it renders every record and reports what it
would *create* vs. *update* (and which posts it would skip as already-announced)
without touching the network. Reach for it whenever you change config or
templates.

Drafts are never published: `publish` builds with drafts excluded, so a
`draft: true` post stays out of the PDS just as it stays out of `italic build`.

## Rate limits

A first publish of a large site creates many records at once, and Bluesky
enforces write rate limits. italic spaces out writes with a small throttle, and
`bluesky.announce_after` lets you cap a first run to recent posts so you don't
flood your followers' feeds with your entire back catalogue.

## Configuration summary

The full `publish:` block (see the [config reference](../reference/config.md#publish)
for details):

```yaml
publish:
  pds_host: https://bsky.social   # optional
  handle: alice.example.com       # or via env / credentials file
  collection: posts               # docs to publish; defaults to `all`
  verification: true              # emit the .well-known + <link> proofs
  publication:
    name: My Garden               # required to publish
    url: https://example.com      # required to publish
    description: A digital garden.
    icon: static/icon.png
  bluesky:
    enabled: false
    collection: posts             # defaults to publish.collection
    post_template: "{{ title }} — {{ summary }}"
    include_link_card: true
    thumb: cover                  # cover | none
    announce_after: 2026-01-01
```

## See also

- [CLI reference](../reference/cli.md#italic-publish) — the `publish` command and its flags
- [Configuration reference](../reference/config.md#publish) — every `publish:` key
- [Frontmatter reference](../reference/frontmatter.md#bluesky-publishing-keys) — `bsky`, `bsky_text`, `cover`
- [Deployment](deployment.md) — hosting your static HTML
- [standard.site](https://standard.site/) · [Bluesky API](https://docs.bsky.app/)
