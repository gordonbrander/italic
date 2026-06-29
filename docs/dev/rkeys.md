# rkey generation

## `standard.site` rkeys

`standard.site` rkeys are generated **deterministically from the document's file path** — they're slug-derived, not random or TID-based.

### The generator

`src/publish/document.rs:76`

```rust
pub fn document_rkey(id_path: &Path) -> String {
    let stem = id_path.with_extension("");
    let key = slug::slugify(stem.to_string_lossy());
    if key.is_empty() {
        "index".to_string()
    } else {
        key
    }
}
```

Steps:
1. Strip the extension off the document's `id_path`.
2. `slug::slugify` it (lowercase, spaces/special chars → hyphens, ASCII-only) via the `slug` crate (`Cargo.toml:34`).
3. Fall back to `"index"` if the slug is empty.

Examples (from tests at `document.rs:218`):
- `blog/Getting Started.md` → `blog-getting-started`
- `index.md` → `index`

The collection is `site.standard.document` (`document.rs:20`); the publication record uses a hardcoded rkey `"self"` (`publish.rs:300`).

### Why stable slugs

Because rkeys are derived from the path, they're **reconstructible even if the state file is lost**, and publishing uses `putRecord` (not `createRecord`), so re-publishing a document updates it in place rather than duplicating. Two paths that slugify identically would collide — rare, and a collision just updates rather than duplicates.

This contrasts with Bluesky posts in the same repo, which use **server-assigned TID rkeys** via `createRecord` (create-once) — see `atproto.rs:168` and `state.rs:11`.

Generated rkeys are persisted in the sidecar state file `.italic/atproto.yaml`, keyed by `id_path` (`state.rs:29`).

One note: it's the `site.standard.*` lexicon (NSID `site.standard.document`), not `standard.site` — the call sites are `publish.rs:274` (publish) and `publish.rs:157` (dry-run).
