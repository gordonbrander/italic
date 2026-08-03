# Contributing

Italic is AGPL-licensed Rust. Issues and pull requests are welcome at
[github.com/gordonbrander/italic](https://github.com/gordonbrander/italic).

## Getting set up

```sh
git clone https://github.com/gordonbrander/italic.git
cd italic
cargo build
cargo test
```

## Repo tour

```
src/
  main.rs          # CLI (clap) — the subcommands
  config.rs        # config.yaml parsing, defaults, theme merging
  doc.rs           # the Doc type; frontmatter uplift
  permalink.rs     # permalink patterns, pagination URLs
  build.rs         # the pipeline driver — start here
  build/           # one module per stage: read, classify, defaults,
                   #   markup (incl. wikilink resolution), archive,
                   #   template, write, static_copy; plus the standard.site
                   #   verification artifacts (well_known, standard_link)
  atproto.rs       # `italic atproto publish`: sync to an ATProto PDS (networked)
  publish/         # atproto client+auth, document records, status
  tera_env.rs      # Tera environment assembly
  tera_env/        # one module per custom function/filter
scaffold/          # site skeleton emitted by `italic new`
tests/
  build.rs         # fixture-driven integration tests
  fixtures/        # numbered end-to-end sites (01_skeleton … 10_backlinks)
  skill_docs.rs    # guards the skill: links, router completeness, error strings
.agents/skills/italic/
  SKILL.md         # the Italic agent skill; its table routes to every doc
  references/      # this documentation — one file per question
```

The build pipeline's stage order and data contracts are documented in the
module comment at the top of `src/build.rs`, and at user level in
[The build pipeline](../build-pipeline.md). How the markup stage layers
wikilinks, block references, and embeds on top of comrak is covered in
[Extending Comrak](markup.md).

## Tests

- Unit tests live alongside the code (`#[cfg(test)]` modules).
- Integration tests in `tests/build.rs` run each `tests/fixtures/NN_*` site
  through a full build and compare the output tree against the fixture's
  `expected/` directory. Adding a feature? Add or extend a fixture — they
  double as living examples for the documentation.
- `tests/skill_docs.rs` guards the docs: every reference file is routed from
  SKILL.md's table, every relative link resolves, and every error string
  quoted in `troubleshooting.md` still exists verbatim in `src/`. Renaming a
  doc file or rewording an error message fails this test until the docs and
  router agree.

Run everything with `cargo test`.

## Conventions

- Errors are loud *within known blocks*: unknown query/`related:`/`atproto:`
  keys and bad references fail the build with a pointer. (Unknown *top-level*
  config keys are silently ignored — a documented trade-off; see
  [Troubleshooting § Silent failures](../troubleshooting.md#silent-failures).)
  Match the loud spirit in new features.
- User-visible changes that touch behavior get an update to the relevant page
  under `.agents/skills/italic/references/` — and a routing-table row in
  SKILL.md if it's a new page.

See [AGENTS.md](../../../../../AGENTS.md) for more conventions.
