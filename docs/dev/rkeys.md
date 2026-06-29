# rkey generation

## `site.standard` rkeys

Record keys for `site.standard.document` and `site.standard.publication` are
**hashes of the site's absolute URLs**, so two Italic sites published to the same
PDS account never collide.

An AT-Proto record is uniquely identified by `(DID, collection, rkey)`. A single
PDS account is one DID, so if the rkey carried no site identity, two sites sharing
that account would clobber each other's records. Folding the origin into the rkey
makes records unique per site.

### The generator

`src/publish/document.rs`

```rust
fn rkey_hash(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    data_encoding::BASE32_NOPAD.encode(&digest).to_lowercase()
}

pub fn document_rkey(canonical_url: &str) -> String {
    rkey_hash(canonical_url)
}

pub fn publication_rkey(site_url: &str) -> String {
    rkey_hash(site_url)
}
```

- **Document rkey** = `rkey_hash` of the doc's absolute canonical URL — origin +
  base path + the doc's web path — built by `canonical_url(doc, site_url,
  base_path)` (`document.rs`). e.g.
  `https://example.com/blog/getting-started/` → `c5oqyxkz4pfia2zmhhye62t42vdzpiwjdmtphglnfxwpg2y5v4ba`.
- **Publication rkey** = `rkey_hash` of the site origin (`site.url`). Replaces the
  old hardcoded `"self"`, which would otherwise collide for two sites on one PDS.

The hash is the full SHA-256, base32-encoded and lowercased: 52 chars, all in the
rkey-safe charset (`a`–`z`, `2`–`7`), well under AT-Proto's 512-char limit. The
full digest is used (not truncated) so collisions are negligible.

### `site.url` is required

Publishing documents **requires** `site.url` to be configured — it's the origin
that disambiguates rkeys. `run()` (`publish.rs`) errors early if documents are
enabled and `site.url` is unset.

### Deterministic and reconstructible

Because the rkey is a pure function of config (`site.url`, `base_path`) plus the
doc's output path, it's reconstructible even if the state file
(`.italic/atproto.yaml`) is lost. Publishing uses `putRecord` (not
`createRecord`), so re-publishing a document updates it in place.

Note the rkey now tracks the **published URL** (via `output_path`/permalink), not
the source `id_path`. Changing a doc's `permalink:` frontmatter changes its rkey.

This contrasts with Bluesky posts in the same repo, which use **server-assigned
TID rkeys** via `createRecord` (create-once) — see `atproto.rs` and `state.rs`.

### Migration note (breaking change)

Switching from the old slug-of-path rkeys to URL hashes is **breaking** for sites
already published with the old scheme. On the next publish, records are written at
the new rkeys and the old records are left **orphaned** on the PDS — they are not
auto-deleted. Delete stale records manually if desired. (A future `publish
--prune` could diff PDS records against state.)
