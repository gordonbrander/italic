# Verifying your ATProto records

After `italic publish`, your posts live in your PDS as
[`site.standard.document`](https://standard.site/docs/lexicons/document) and
[`site.standard.publication`](https://standard.site/docs/lexicons/publication)
records. This guide covers how to
confirm they're really there and still match what italic published — first with
the built-in `italic pubstatus` command, then by hand with `curl` for when you want
to inspect records directly.

> The other side of this — *creating* the records — is covered in
> [Publishing to the ATmosphere](publishing-atproto.md). This guide assumes
> you've already run `italic publish` at least once.

## `italic pubstatus`

The quickest check is the built-in command:

```sh
italic pubstatus
```

It reads back every record italic recorded in `.italic/atproto.yaml`, fetches
each one from your PDS, and reports its status:

```
authenticated as alice.example.com (did:plc:abc123…)
  ok      publication
  ok      document posts/getting-started.md
  ok      document posts/second-post.md
3 ok, 0 missing, 0 changed
```

Each record is classified as:

- **ok** — present on the PDS, and its content hash (CID) matches local state.
- **CHANGED** — present, but the live CID differs from state. The record was
  edited or re-written since italic last published it (e.g. by another client, or
  a `publish` run you haven't recorded).
- **MISSING** — italic recorded the record, but it's no longer on the PDS.

All records, including `site.standard.publication`, are checked for content drift.
(State files written before the publication's CID was recorded fall back to an
existence-only check for that one record — re-run `italic publish` to record it.)

If anything is MISSING or CHANGED, `italic pubstatus` **exits nonzero**, so you can
gate a CI step or a deploy script on it.

### What it needs

`italic pubstatus` is networked, **authenticated**, and **read-only** — it never
writes a record or modifies the state file. It needs:

- A [`publish:`](../reference/config.md#publish) block in `config.yaml`.
- Credentials in the environment — the same
  `ITALIC_ATPROTO_HANDLE` / `ITALIC_ATPROTO_APP_PASSWORD` you use to publish.
- A prior `italic publish`, since it verifies what's recorded in
  `.italic/atproto.yaml`. With no state there's nothing to verify, and the
  command says so.

Unlike `publish`, `pubstatus` does **not** build your site — it only reads config
and state — so it still works while your content is mid-edit.

See the [CLI reference](../reference/cli.md#italic-pubstatus) for details.

## Manual verification (under the hood)

`italic pubstatus` calls the same public XRPC endpoints you can hit yourself. ATProto
reads are **public and unauthenticated**, so any record in a repo is fetchable
with plain `curl` — no app password, no session. Reach for this when you want to
inspect a record's actual fields, debug a mapping, or verify from a machine that
doesn't have italic installed.

You'll need `curl` and [`jq`](https://jqlang.github.io/jq/), plus your handle.

### Resolve handle → DID → PDS

Records are addressed by your account's **DID** and served by your **PDS**:

```sh
HANDLE=alice.example.com

# handle -> DID
DID=$(curl -s \
  "https://bsky.social/xrpc/com.atproto.identity.resolveHandle?handle=$HANDLE" \
  | jq -r .did)

# DID -> PDS service endpoint (works for did:plc and did:web)
PDS=$(curl -s "https://plc.directory/$DID" \
  | jq -r '.service[] | select(.id=="#atproto_pds") .serviceEndpoint')

echo "$DID @ $PDS"
```

The `$DID` should match the `did` recorded in your `.italic/atproto.yaml`.

### Confirm the collections exist

```sh
curl -s "$PDS/xrpc/com.atproto.repo.describeRepo?repo=$DID" | jq '.collections'
```

You're looking for `site.standard.document` and `site.standard.publication` in
the list.

### List your published records

`listRecords` returns each record's AT-URI, `cid`, and full `value`:

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
stable but not human-readable — copy the rkey from `.italic/atproto.yaml` or a
`listRecords` result:

```sh
curl -s "$PDS/xrpc/com.atproto.repo.getRecord?repo=$DID&collection=site.standard.document&rkey=$RKEY" \
  | jq .
```

This is the call for verifying a specific field mapping — that `publishedAt`
matches the post's date, `description` is the summary, `coverImage` resolved to a
blob ref, and `site` points at your publication record's AT-URI. (See the
[field mapping table](publishing-atproto.md#publishing-full-posts).)

### Verify the on-site proofs match

standard.site ownership rests on two artifacts italic emits during `build`.
Confirm they agree with what's actually in the PDS:

```sh
# publication proof — should equal the publication AT-URI from listRecords
curl -s https://example.com/.well-known/site.standard.publication

# per-document proof — the href should equal the document's AT-URI
curl -s https://example.com/posts/getting-started/ \
  | grep -o '<link rel="site.standard.document"[^>]*>'
```

The per-page tag is emitted by the built-in `standard_link` metadata filter
(included in the `{{ page | metadata(site=site) }}` umbrella — see the
[Metadata guide](metadata.md)). Both proofs are *derived* from `publish.did` +
`site.url` — record keys are hashes of your canonical URLs — so they're present
in every build, independent of the publish state file, and must simply agree
with what the PDS returns.

When the well-known file, the per-page `<link>`, and the PDS records all agree,
the round-trip is verified: your domain claims the records, and the records exist.

### Browser & GUI alternatives

Every XRPC URL above is a plain `GET` — paste it into a browser (literal values
swapped in for `$VAR`s) for raw JSON. For a friendlier whole-repo view:

- **[pdsls.dev](https://pdsls.dev/)** — paste `at://<DID>` (or your handle) for a
  navigable repo browser.
- **[atproto-browser.vercel.app](https://atproto-browser.vercel.app/)** — walk
  records by handle in a web UI.

These call the same public endpoints — nothing privileged.

### Cryptographic verification

The JSON above is what the PDS *serves*. To verify records are genuinely signed
into your repo's Merkle tree, download the full signed repository as a CAR file:

```sh
curl -s "$PDS/xrpc/com.atproto.sync.getRepo?did=$DID" -o repo.car
```

Then inspect it with [`goat`](https://github.com/bluesky-social/indigo/tree/main/cmd/goat),
the official ATProto CLI:

```sh
goat repo unpack repo.car        # extract records to a directory tree
goat repo verify repo.car        # check the commit signature & MST integrity
```

For routine "did my publish work?" checks, `italic pubstatus` is enough; reach for
the CAR file when you care about cryptographic provenance.

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

- [Publishing to the ATmosphere](publishing-atproto.md) — creating the records this guide verifies
- [CLI reference](../reference/cli.md#italic-pubstatus) — the `pubstatus` command and its flags
- [ATProto HTTP reference](https://docs.bsky.app/docs/category/http-reference) · [standard.site lexicons](https://standard.site/docs/lexicons/document)
