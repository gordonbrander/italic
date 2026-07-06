# AtProto in brief

The **AT Protocol** (Authenticated Transfer Protocol) is the federated networking protocol behind Bluesky. Its core idea: your identity and your data are decoupled from any particular server. Data lives in **records** (typed JSON, e.g. a `app.bsky.feed.post`) stored in per-user repositories — signed Merkle trees — so they're portable and verifiable. Servers federate by streaming these records to each other.

## The core roles

- **PDS (Personal Data Server)** — Hosts a user's *repository*: the actual store of their records (posts, likes, follows, profile). It signs records, serves them over the `com.atproto.*` XRPC API, and emits changes. Think of it as "your home server." A user can migrate their repo from one PDS to another without losing identity.

- **DID PLC** — Your **DID** (Decentralized Identifier, e.g. `did:plc:abc123...`) is your permanent identity. **PLC** ("Public Ledger of Credentials") is the directory service that maps that DID to your current signing keys and your current PDS endpoint. Because identity (DID) is separate from hosting (PDS), you can move servers and update the PLC record while keeping the same DID — followers still find you. A human-readable handle (`alice.bsky.social`) just resolves *to* a DID; the DID is the real identity.

- **Firehose** — The PDS (and relays aggregating many PDSes) exposes a real-time stream of every repository commit — every new/updated/deleted record across the network. This is `com.atproto.sync.subscribeRepos`, a WebSocket of signed events. Indexers, feed generators, and analytics consume it to build views of the network.

- **Client** — Any app (the Bluesky app, a CLI, a bot) that reads and writes records on a user's behalf by calling the PDS's XRPC API. The client authenticates, then creates/updates/deletes records in the user's repo.

## How the pieces fit

```
Client ──auth──▶ PDS ──signs & stores records──▶ Repository
  │                │
  │                └──emits commits──▶ Firehose ──▶ Relays / Indexers / Feeds
  │
  └── resolves handle ──▶ DID ──▶ PLC directory ──▶ (current keys + PDS URL)
```

## Authenticating a CLI to post records to a PDS

1. **Resolve identity (optional but typical).** Turn a handle into a DID and find the user's PDS via the PLC directory (`com.atproto.identity.resolveHandle`, then the DID doc gives the PDS service endpoint). If you already know the PDS host, you can skip ahead. (italic ships this as `italic atproto resolve-did <handle>` — italic is DID-only elsewhere, so this is the on-ramp from a handle to `ITALIC_ATPROTO_DID`.)

2. **Create a session.** Call `com.atproto.server.createSession` with an *identifier* (handle or DID) and a **password**. Best practice: use an **App Password** (generated in Bluesky settings) rather than the account's main password — it's revocable and scoped.

   ```bash
   curl -X POST https://bsky.social/xrpc/com.atproto.server.createSession \
     -H 'Content-Type: application/json' \
     -d '{"identifier":"alice.bsky.social","password":"xxxx-xxxx-xxxx-xxxx"}'
   ```

   This returns an `accessJwt` (short-lived bearer token), a `refreshJwt`, plus your `did` and PDS info.

3. **Write a record.** Call `com.atproto.repo.createRecord` with the access JWT as a bearer token, specifying the repo (your DID), the collection (the record type), and the record body:

   ```bash
   curl -X POST https://bsky.social/xrpc/com.atproto.repo.createRecord \
     -H "Authorization: Bearer $ACCESS_JWT" \
     -H 'Content-Type: application/json' \
     -d '{
       "repo":"did:plc:abc123...",
       "collection":"app.bsky.feed.post",
       "record":{
         "$type":"app.bsky.feed.post",
         "text":"Hello from the CLI",
         "createdAt":"2026-06-21T12:00:00Z"
       }
     }'
   ```

   The PDS validates the record against its Lexicon schema, signs it into your repo, and emits the commit on the firehose. The post is now live and federating.

4. **Refresh when needed.** Access JWTs expire quickly; use `com.atproto.server.refreshSession` with the refresh token to get a new one without re-sending the password.

The newer path replaces step 2 with **OAuth** (DPoP-bound tokens, no password handling) — preferable for production clients — but the `createSession` + App Password flow is the simplest for a CLI and is still widely used.
