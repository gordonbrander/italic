# Verifying your ATProto records

After `italic atproto publish`, your posts live in your PDS as
[`site.standard.document`](https://standard.site/docs/lexicons/document) and
[`site.standard.publication`](https://standard.site/docs/lexicons/publication)
records. This guide covers how to
confirm they're really there — first with the built-in `italic atproto status`
command, then by hand with `curl` for when you want to inspect records directly.

> The other side of this — *creating* the records — is covered in
> [Publishing to the ATmosphere](atproto-publish.md). This guide assumes
> you've already run `italic atproto publish` at least once.

## `italic atproto status`

The quickest check is the built-in command:

```sh
italic atproto status
```

It builds your site index (the same way `atproto publish` does — drafts
excluded, no HTML written) to derive what each record *should contain*, then
lists what your PDS actually holds via `com.atproto.repo.listRecords` and
compares. The PDS is the source of truth — there is no local state file:

```
authenticated as alice.example.com (did:plc:abc123…)
  ok       publication at://did:plc:abc123…/site.standard.publication/cadib…
  ok       posts/getting-started.md
  CHANGED  posts/edited-post.md (rkey=a7bfe…) — local content differs from the PDS
  MISSING  posts/second-post.md (rkey=c5oqy…)
  ORPHANED at://did:plc:abc123…/site.standard.document/xyz… (no matching local doc — deleted or renamed?)
1 published, 1 changed, 1 missing, 1 orphaned
```

Each expected record — the publication plus one document per doc in your
configured collection — is classified:

- **ok** — present on the PDS and identical to what your current local content
  produces.
- **CHANGED** — present, but its value differs from the locally built record:
  you edited content since the last publish, or the record was rewritten by
  another client. Run `italic atproto publish` to reconcile either way.
- **MISSING** — absent from the PDS. Run `italic atproto publish` to (re)create it.
- **ORPHANED** — a document record on the PDS that references your publication
  but has no matching local doc: you deleted or renamed the source since
  publishing it. See [removing orphans](#removing-orphans) below.

If anything is MISSING or CHANGED, `italic atproto status` **exits nonzero**,
so you can gate a CI step or a deploy script on it. Orphans only warn — they're
the normal aftermath of deletes and renames, and cleaning them up is a manual
step.

### What it needs

`italic atproto status` is networked, **authenticated**, and **read-only** — it
never writes a record. It needs:

- `site.url` set, since record keys are derived from your canonical URLs (the
  same requirement `publish` has), and `site.title` (the publication record's
  name). No [`atproto:`](config.md#atproto) block is needed.
- Credentials in the environment — the same
  `ITALIC_ATPROTO_DID` / `ITALIC_ATPROTO_APP_PASSWORD` you use to publish.
- Buildable, publishable content, since it builds the expected records exactly
  as `publish` would — including readable cover images.

See the [CLI reference](cli.md#command-notes) for details.

## Manual verification (under the hood)

`italic atproto status` calls the same public XRPC endpoints you can hit yourself. ATProto
reads are **public and unauthenticated**, so any record in a repo is fetchable
with plain `curl` — no app password, no session. Reach for this when you want to
inspect a record's actual fields, debug a mapping, or verify from a machine that
doesn't have italic installed.

You'll need `curl` and [`jq`](https://jqlang.github.io/jq/), plus your handle.

### Resolve handle → DID → PDS

Records are addressed by your account's **DID** and served by your **PDS**:

```sh
HANDLE=alice.example.com

# handle -> DID (or: italic atproto did $HANDLE)
DID=$(curl -s \
  "https://bsky.social/xrpc/com.atproto.identity.resolveHandle?handle=$HANDLE" \
  | jq -r .did)

# DID -> PDS service endpoint (works for did:plc and did:web)
PDS=$(curl -s "https://plc.directory/$DID" \
  | jq -r '.service[] | select(.id=="#atproto_pds") .serviceEndpoint')

echo "$DID @ $PDS"
```

The `$DID` should match the `ITALIC_ATPROTO_DID` you publish with.

### Confirm the collections exist

```sh
curl -s "$PDS/xrpc/com.atproto.repo.describeRepo?repo=$DID" | jq '.collections'
```

You're looking for `site.standard.document` and `site.standard.publication` in
the list.

### List your published records

`listRecords` returns each record's AT-URI, `cid`, and full `value`. This is
the same endpoint `italic atproto status` compares against:

```sh
# all document records, summarized
curl -s "$PDS/xrpc/com.atproto.repo.listRecords?repo=$DID&collection=site.standard.document&limit=100" \
  | jq -r '.records[] | "\(.value.title)\t\(.uri)"'

# the single publication record
curl -s "$PDS/xrpc/com.atproto.repo.listRecords?repo=$DID&collection=site.standard.publication" \
  | jq '.records[].value'
```

If you publish more than 100 records the response includes a `cursor`; pass it
back as `&cursor=…` to page through the rest.

### Inspect one record

Fetch a record by its **record key** (`rkey`) — the last segment of its AT-URI
(`at://<did>/<collection>/<rkey>`). italic derives document rkeys from a hash of
the canonical URL (and the publication rkey from the site origin), so they're
stable but not human-readable — copy the rkey from a `listRecords` result or an
`italic atproto status` MISSING line:

```sh
curl -s "$PDS/xrpc/com.atproto.repo.getRecord?repo=$DID&collection=site.standard.document&rkey=$RKEY" \
  | jq .
```

This is the call for verifying a specific field mapping — that `publishedAt`
matches the post's date, `description` is the summary, `coverImage` resolved to a
blob ref, and `site` points at your publication record's AT-URI. (See the
[field mapping table](atproto-publish.md#publishing-full-posts).)

### Removing orphans

When `italic atproto status` reports an ORPHANED record — a published document
whose local source you've since deleted or renamed — remove it with
`deleteRecord`. Deletes are writes, so unlike everything above this call is
authenticated (an app password session):

```sh
# session token (same app password you publish with)
JWT=$(curl -s -X POST "$PDS/xrpc/com.atproto.server.createSession" \
  -H 'Content-Type: application/json' \
  -d "{\"identifier\":\"$DID\",\"password\":\"$APP_PASSWORD\"}" | jq -r .accessJwt)

# delete by rkey — the last segment of the ORPHANED line's AT-URI
curl -s -X POST "$PDS/xrpc/com.atproto.repo.deleteRecord" \
  -H "Authorization: Bearer $JWT" -H 'Content-Type: application/json' \
  -d "{\"repo\":\"$DID\",\"collection\":\"site.standard.document\",\"rkey\":\"$RKEY\"}"
```

A renamed doc gets a fresh record at its new canonical URL on the next publish;
deleting the orphan at the old URL completes the move.

### Verification artifacts

standard.site verifies that you own the domain through artifacts italic emits
during `build` (gated on `atproto.verification`, on by default). Both AT-URIs
are **derived** — record keys are hashes of your canonical URLs — so the only
input beyond your config is `ITALIC_ATPROTO_DID`. `build` needs only the DID
(a public identifier), never the app password, so CI/deploy environments set
just the DID. Every such build emits:

1. **Publication proof** — a static file at
   `/.well-known/site.standard.publication` containing your publication's
   AT-URI. Generated automatically; nothing to template.
2. **Per-document proof** — a `<link rel="site.standard.document">` tag in
   each published page's `<head>`, emitted by the
   [metadata filters](metadata.md) (`{{ page | metadata }}` includes it, or
   compose `{{ page | standard_link }}`); the raw AT-URI is
   `page.data.atproto_uri`.
3. **[AT tags](metadata.md#at-tags)** — `<meta name="at:*">` tags
   generalizing the proof link. The build also exposes `site.atproto_did` and
   `site.atproto_publication_uri` for hand-rolled heads.

Because `atproto publish` derives record addresses the same way — and
authenticates *as* that same DID — build and deploy order doesn't matter:
HTML deployed before a publish already carries the right URIs, and
verification passes the moment the records exist.

### Verify the on-site proofs match

Confirm the emitted artifacts agree with what's actually in the PDS:

```sh
# publication proof — should equal the publication AT-URI from listRecords
curl -s https://example.com/.well-known/site.standard.publication

# per-document proof — the href should equal the document's AT-URI
curl -s https://example.com/posts/getting-started/ \
  | grep -o '<link rel="site.standard.document"[^>]*>'
```

The per-page tag is emitted by the built-in `standard_link` metadata filter
(included in the `{{ page | metadata }}` umbrella — see the
[Metadata guide](metadata.md)). Both proofs are *derived* from
`ITALIC_ATPROTO_DID` + `site.url` — record keys are hashes of your canonical
URLs — so they're present in every build and must simply agree with what the
PDS returns.

When the well-known file, the per-page `<link>`, and the PDS records all agree,
the round-trip is verified: your domain claims the records, and the records exist.

Every XRPC URL above is a plain unauthenticated `GET` — a browser works too,
as do repo browsers like [pdsls.dev](https://pdsls.dev/). For cryptographic
provenance (signed-repo CAR download and MST verification), see the
[ATProto primer](dev/at-proto.md) — `italic atproto status` is enough for
"did my publish work?".

## Quick reference

| Question | Endpoint |
|----------|----------|
| What's my DID? | `com.atproto.identity.resolveHandle?handle=…` |
| Where's my PDS? | `https://plc.directory/<DID>` → `#atproto_pds` endpoint |
| Which collections exist? | `com.atproto.repo.describeRepo?repo=<DID>` |
| List records in a collection | `com.atproto.repo.listRecords?repo=<DID>&collection=…` |
| Fetch one record | `com.atproto.repo.getRecord?repo=<DID>&collection=…&rkey=…` |
| Download the signed repo | `com.atproto.sync.getRepo?did=<DID>` |

All are unauthenticated `GET`s against your PDS (identity/PLC lookups go to
`bsky.social` and `plc.directory`).

## See also

- [Publishing to the ATmosphere](atproto-publish.md) — creating the records this guide verifies
- [CLI reference](cli.md#command-notes) — env vars and exit codes
- [ATProto HTTP reference](https://docs.bsky.app/docs/category/http-reference) · [standard.site lexicons](https://standard.site/docs/lexicons/document)
