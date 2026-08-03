# Installing Italic

Italic is a single Rust binary. It is distributed **only** through crates.io
(or a git checkout) — there are no release tarballs, no Homebrew formula, no
npm or pip wrapper. Don't send anyone to a downloads page; there isn't one.

## Is it already installed?

Check before running anything else — this skill can be installed before or
after the binary:

```sh
command -v italic
```

A path (typically `~/.cargo/bin/italic`) means it's installed. Nothing means
it isn't — install it before offering to build a site.

There is **no `--version` flag**. To see which version is installed:

```sh
cargo install --list | grep italic     # → italic v0.3.2:
```

## Install

```sh
cargo install italic
```

This compiles from source and takes a few minutes the first time. It needs
Rust **1.95 or newer** (Italic is edition 2024). If `cargo` is present but too
old, `rustup update` fixes it; the failure otherwise shows up as a rustc error
about `edition2024`, not as an Italic error.

## If `cargo` is missing

Install Rust first, from <https://rustup.rs>:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

`-y` skips the interactive prompt (rustup's installer blocks waiting for input
otherwise). The `source` line matters: rustup adds `~/.cargo/bin` to the shell
profile, but the **current** shell won't see it until the profile is re-read,
and a non-interactive shell may never read it at all.

## `italic: command not found` right after a successful install

`~/.cargo/bin` isn't on `PATH`. Confirm the binary is there
(`ls ~/.cargo/bin/italic`), then either `source "$HOME/.cargo/env"` for this
shell or add the directory to `PATH` in the shell profile. Invoking it by full
path (`~/.cargo/bin/italic build`) always works.

## Upgrading

```sh
cargo install italic            # upgrades if crates.io has a newer version
cargo install italic --force    # reinstall the same version
```

Plain `cargo install italic` is a no-op when the newest version is already
installed — it prints `` package `italic vX.Y.Z` is already installed, use
--force to overwrite `` and exits **zero**. That is success, not an error.

## Installing from source

```sh
cargo install --git https://github.com/gordonbrander/italic   # latest main
cargo install --path .                                        # local checkout
```

## Uninstalling

```sh
cargo uninstall italic
```

## In CI

Same command — `cargo install italic` as a build step, on any image with a
Rust toolchain. Cache `~/.cargo` to avoid recompiling every run. Worked
examples for GitHub Actions, Netlify, and friends are in
[Deployment](deployment.md).
