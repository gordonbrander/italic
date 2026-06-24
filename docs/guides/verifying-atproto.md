# Verifying your ATProto records

After `italic publish`, your posts live in your PDS as
[`site.standard.document`](https://standard.site/docs/lexicons/document) and
[`site.standard.publication`](https://standard.site/docs/lexicons/publication)
records. This guide shows how to confirm they're really there, that their
contents are correct, and that the on-site proofs point back at them — all from
the command line.

The key fact: **ATProto reads are public and unauthenticated.** Every record in
a repo is served as plain JSON over the `com.atproto.*` XRPC API. You don't need
your app password, a session, or any italic tooling to inspect what was
published — just `curl` and `jq` (or a browser). Writes need auth; reads don't.

> The opposite side of this — *creating* the records — is covered in
> [Publishing to Bluesky & ATProto](publishing-atproto.md). This guide assumes
> you've already run `italic publish` at least once.

## What you'll need

- `curl` and [`jq`](https://jqlang.github.io/jq/) for the JSON examples.
- Your **handle** (e.g. `alice.example.com`) — the same one in
  `ITALIC_ATPROTO_HANDLE` / `publish.handle`.

Nothing else. No secrets are involved in reading.

## Step 1 — resolve handle → DID → PDS

Records are addressed by your account's **DID**, and served by your **PDS**.
Resolve both from your handle:

```sh
HANDLE=alice.example.com

# handle -> DID
DID=$(curl -s \
  "https://bsky.social/xrpc/com.atproto.identity.resolveHandle?handle=$HANDLE" \
  | jq -r .did)
echo "$DID"          # e.g. did:plc:abc123...

# DID -> PDS service endpoint (works for did:plc and did:web)
PDS=$(curl -s "https://plc.directory/$DID" \
  | jq -r '.service[] | select(.id=="#atproto_pds") .serviceEndpoint')
echo "$PDS"          # e.g. https://bsky.social  (or your self-hosted PDS)
```

If you publish to a custom `publish.pds_host`, that host serves the records and
`$PDS` will reflect it. The DID is the stable identity; the PDS can change if you
migrate, which is exactly why you resolve it fresh rather than hard-coding it.

The DID you get here should match the `did` recorded in your
`.italic/atproto.yaml` state file — a quick first sanity check.

## Step 2 — confirm the collections exist

Before looking at individual records, verify the repo even has the standard.site
collections:

```sh
curl -s "$PDS/xrpc/com.atproto.repo.describeRepo?repo=$DID" | jq '.collections'
```

You're looking for both NSIDs in the output:

```json
[
  "site.standard.document",
  "site.standard.publication",
  "app.bsky.feed.post"
]
```

Seeing `site.standard.document` and `site.standard.publication` confirms a
publish landed. (`app.bsky.feed.post` shows up too if you enabled Bluesky
announcements.)

## Step 3 — list your published documents

`listRecords` returns every record in a collection — each with its AT-URI,
`cid`, and full `value` (the exact JSON italic wrote):

```sh
curl -s "$PDS/xrpc/com.atproto.repo.listRecords?repo=$DID&collection=site.standard.document&limit=100" \
  | jq '.records[] | {uri, title: .value.title, path: .value.path}'
```

A compact way to confirm count and titles at a glance:

```sh
curl -s "$PDS/xrpc/com.atproto.repo.listRecords?repo=$DID&collection=site.standard.document&limit=100" \
  | jq -r '.records[] | "\(.value.title)\t\(.uri)"'
```

If you publish more than 100 records, the response includes a `cursor`; pass it
back as `&cursor=...` to page through the rest.

Check the publication record the same way — there should be exactly one:

```sh
curl -s "$PDS/xrpc/com.atproto.repo.listRecords?repo=$DID&collection=site.standard.publication" \
  | jq '.records[].value'
```

Confirm its `name` and `url` match your `publish.publication` config.

## Step 4 — inspect a single record

To check one document closely, fetch it by **record key** (`rkey`) — the last
segment of its AT-URI (`at://<did>/<collection>/<rkey>`). italic derives
document rkeys from the slug, so they're stable and human-readable:

```sh
curl -s "$PDS/xrpc/com.atproto.repo.getRecord?repo=$DID&collection=site.standard.document&rkey=getting-started" \
  | jq .
```

This is the call to reach for when you want to verify a specific field mapping —
that `publishedAt` matches the post's `date`, that `description` is the summary,
that `coverImage` resolved to a blob ref, that `site` points at your publication
record's AT-URI. (See the [field mapping
table](publishing-atproto.md#publishing-full-posts) for what each field should
contain.)

## Step 5 — verify the on-site proofs match

standard.site ownership rests on two artifacts italic emits during `build`, and
verification means checking that they agree with what's actually in the PDS.

**Publication proof** — fetch the well-known file from your live site and
confirm the AT-URI it advertises is the same one `listRecords` returned in
step 3:

```sh
curl -s https://example.com/.well-known/site.standard.publication
# -> at://did:plc:abc123.../site.standard.publication/self
```

**Per-document proof** — each published page carries a
`<link rel="site.standard.document">` in its `<head>`. Pull it out and check it
against the record's AT-URI:

```sh
curl -s https://example.com/posts/getting-started/ \
  | grep -o '<link rel="site.standard.document"[^>]*>'
```

The `href` should be the AT-URI you saw for that document in step 4. When the
well-known file, the per-page `<link>`, and the PDS records all agree, the
round-trip is verified: your domain claims the records, and the records exist.

## Browser & GUI alternatives

Every XRPC URL above is a plain `GET` — paste it into a browser (with literal
values swapped in for the `$VAR`s) and you'll get the raw JSON. For a friendlier
view of an entire repo:

- **[pdsls.dev](https://pdsls.dev/)** — paste `at://<DID>` (or your handle) for a
  navigable repo browser; drills into any collection and record.
- **[atproto-browser.vercel.app](https://atproto-browser.vercel.app/)** — enter
  your handle to walk records in a web UI.

These are convenient for eyeballing, but they call the same public endpoints the
`curl` examples do — nothing privileged.

## Cryptographic verification (optional)

The JSON above is what the PDS *serves*. To verify the records are genuinely
signed into your repo's Merkle tree — not just present in an API response —
download the full signed repository as a CAR file:

```sh
curl -s "$PDS/xrpc/com.atproto.sync.getRepo?did=$DID" -o repo.car
```

Then inspect it with [`goat`](https://github.com/bluesky-social/indigo/tree/main/cmd/goat),
the official ATProto CLI:

```sh
goat repo unpack repo.car        # extract records to a directory tree
goat repo verify repo.car        # check the commit signature & MST integrity
```

This proves the records are committed and signed by your account's key, which is
the strongest form of verification short of resolving the signing key from the
PLC directory yourself. For routine "did my publish work?" checks, steps 1–5 are
enough; reach for the CAR when you care about cryptographic provenance.

## Quick reference

| Question | Endpoint |
|----------|----------|
| What's my DID? | `com.atproto.identity.resolveHandle?handle=…` |
| Where's my PDS? | `https://plc.directory/<DID>` → `#atproto_pds` endpoint |
| Which collections exist? | `com.atproto.repo.describeRepo?repo=<DID>` |
| List records in a collection | `com.atproto.repo.listRecords?repo=<DID>&collection=…` |
| Fetch one record | `com.atproto.repo.getRecord?repo=<DID>&collection=…&rkey=…` |
| Download the signed repo | `com.atproto.sync.getRepo?did=<DID>` |

All are unauthenticated `GET`s against your PDS (the identity/PLC lookups against
`bsky.social` and `plc.directory`).

## See also

- [Publishing to Bluesky & ATProto](publishing-atproto.md) — creating the records this guide verifies
- [CLI reference](../reference/cli.md#italic-publish) — the `publish` command and its flags
- [ATProto HTTP reference](https://docs.bsky.app/docs/category/http-reference) · [standard.site lexicons](https://standard.site/docs/lexicons/document)
