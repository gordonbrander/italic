# Bluesky announcement posts

Besides the long-form document record, [`italic atproto
publish`](atproto-publish.md) can announce a doc with a short
[`app.bsky.feed.post`](https://docs.bsky.app/docs/advanced-guides/posts) — a
regular Bluesky post from your account, carrying your text plus a link card
back to the article. The document record cross-links it via its `bskyPostRef`
field, so apps that read standard.site documents can find the announcement
(and its replies) — comments for your post, tracked off-platform.

## Doubly opt-in

Turn the feature on in config:

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
summary, with the doc's [cover image](atproto-publish.md#cover-images) as the
thumbnail.

## Posts are created once

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

## Replies as comments

During `build`, each announced doc's post AT-URI is exposed to templates as
`page.data.bsky_uri`, so a theme can link the announcement — or fetch its
replies and render them as comments:

```html
{% if page.data.bsky_uri %}
<jardin-comments uri="{{ page.data.bsky_uri }}"></jardin-comments>
{% endif %}
```

Docs without a recorded post simply lack the key. A post created outside
italic can be wired up by setting `bsky_uri:` in that doc's frontmatter by
hand. Note that `italic serve` does not watch `.italic/`, so a freshly
published post appears in templates on the next restart or build.

## Guard rails

Two guards prevent accidentally blasting posts:

- **A date cutoff.** Docs dated before `atproto.bsky.since` never get posts;
  when `since` is unset it defaults to **3 days before now**, so enabling the
  feature over an old archive announces nothing by accident. Set `since`
  explicitly to announce older docs.
- **A confirmation prompt.** Before creating anything, `italic atproto
  publish` lists every pending post and asks. Pass `--yes` to skip the prompt
  (required in CI, where stdin isn't a terminal).

`--dry-run` shows pending posts too, without touching the network.

## See also

- [Publishing to the ATmosphere](atproto-publish.md) — the document records
- [Verifying your records](atproto-verify.md) — `status`, POST PENDING, STALE
