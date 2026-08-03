# Deployment

`italic build` produces plain static files in `public/` — any static host
works. The recipes below cover the common ones.

Two settings matter everywhere:

```yaml
site:
  url: https://example.com   # so feeds and social tags get absolute URLs
  base_path: ""              # set when hosting under a subpath
```

## GitHub Pages

For a **project site** served at `username.github.io/repo/`, set
`base_path: /repo` and use the [URL filters](permalinks.md#urls-site-url-and-base-path)
in your templates. For a **user site** (`username.github.io`) or a custom
domain, leave `base_path` empty.

`.github/workflows/deploy.yml`:

```yaml
name: Deploy
on:
  push:
    branches: [main]
permissions:
  contents: read
  pages: write
  id-token: write
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install italic
      - run: italic build
      - uses: actions/upload-pages-artifact@v3
        with:
          path: public
  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

(Caching `~/.cargo` or installing a prebuilt binary will speed up the install
step considerably.)

## Netlify

`netlify.toml` at the repo root:

```toml
[build]
command = "cargo install italic && italic build"
publish = "public"
```

Netlify's build image includes the Rust toolchain via its standard tooling;
alternatively build in CI and deploy the `public/` folder with
`netlify deploy --prod --dir=public`.

## Cloudflare Pages

In the Pages project settings:

- **Build command**: `cargo install italic && italic build`
- **Build output directory**: `public`

Or skip remote builds entirely: build locally or in CI and push with
`wrangler pages deploy public`.

## Deploy branch as a git worktree

Many hosts — GitHub Pages in "deploy from branch" mode, DigitalOcean App Platform,
Cloudflare Pages — will serve a branch of built output directly. A git worktree
lets you keep that branch checked out *at* `public/`, so `italic build` writes
straight into it and publishing needs no CI.

One-time setup, from your source repo:

```sh
git worktree add public deploy      # or: git worktree add -b deploy public
```

`public/` is now a checkout of the `deploy` branch sharing the same `.git` store —
no second clone, no extra remote. Then a deploy is:

```sh
#!/bin/sh
set -eu
italic clean
italic build
cd public
git add -A
git commit -m "Deploy $(git -C .. rev-parse --short HEAD)" || { echo "no changes"; exit 0; }
git push
```

The `clean` step matters: `italic build` writes over the top and never removes, so
without it a deleted page's HTML lingers in `public/` and git keeps publishing it.
`clean` preserves the worktree's `.git` and leaves the directory in place — that is
what [`keep_files`](../reference/config.md#keep_files) defaults to — so the worktree
stays registered across the cycle.

Keep `public/` out of the source branch so `main` stays clean:

```
# .gitignore on main
/public
```

**Put non-generated deploy files in `static/`.** Because `clean` removes everything
`keep_files` doesn't match, a `CNAME`, `.nojekyll`, or `robots.txt` living only on
the deploy branch would be deleted and then dropped from the branch by the next
commit. Files in `static/` are copied into the output on every build, dotfiles
included, so they reproduce themselves. That is also the better home for them:
deploy config ends up version-controlled on `main` instead of stranded on a branch
nothing reproduces. Naming them in `keep_files` works too, but then nothing in your
source tree records that they exist.

## Plain server (rsync)

```sh
italic build
rsync -avz --delete public/ user@server:/var/www/example.com/
```

`--delete` keeps the server in sync with removals; run `italic clean` first if
you've changed permalinks and want a guaranteed-fresh build.

## Staging with drafts

A staging environment can include drafts:

```sh
italic build --drafts
```

See [Drafts](drafts.md).

## See also

- [CLI reference](../reference/cli.md)
- [Permalinks](permalinks.md) — `base_path` and URL filters
