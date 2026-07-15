# Italic

_Publish your digital garden to the Atmosphere_

Publish your blog, markdown notes, or [Obsidian Vault](https://obsidian.md) to [Bluesky](https://bsky.app) and the web. Think in public, build your audience, own your data.

- **Blog-aware**: custom taxonomies, theming, archives, rss and more
- **Obsidian-compatible**: Wikilinks, backlinks, block references and support for Obsidian-flavored Markdown.
- **ATProto-enabled**: publish to [Bluesky](https://bsky.app) and [ATProto](https://atproto.com/), with support for bsky and [standard.site](https://standard.site/) records.

## Why Italic?

- **Fast**: Written in Rust with an embarrassingly parallel build pipeline. Generate thousands of pages in milliseconds.
- **Flexible**: make your corner of the web your own with deep theming, powerful macros, custom taxonomies, configurable collections
- **Free**: open source

## Feature list

Italic comes with everything you need to publish a blog, personal wiki, documentation site, or project site.

- **Content**
  - Callouts
  - Macros (shortcodes): define custom macros you can use in your Markdown for YouTube embeds, UI widgets, and use them in your .
- **Digital gardens and wikis**
  - Obsidian-flavored Markdown: supports loads of [Obsidian Markdown extensions](https://obsidian.md/help/syntax). 
  - Wikilinks: fuzzy link matching using the same algorithm as Obsidian
  - Backlinks: see everything that links back to a page.
  - Block references: deep link to headings and blocks with Obsidian-style purple links.
  - Hashtags: lifted up to tag taxonomy
  - Related pages: Surface related posts with a customizable algorithm
  - Obsidian Vaults: seamlessly transform your Vault into a website.
- **Blogging**
  - Draft posts
  - Tags
  - Archives: paginated archival posts
  - RSS
  - Publish multiple blogs on the same site (custom collections)
- **Content-driven websites**
  - Custom taxonomies: organize posts by tag, category, artist, label, phase of the moon — no problem.
  - Custom collections: glob-match files to define custom page collections you can use in templates
  - Page trees: organize pages into a tree for menus
- **Theming**
  - [Tera templates](https://keats.github.io/tera/): blazingly fast Jinja-like templates in Rust with support for template functions, filters, macros, template extension, and more.
  - Lots of built-in custom filters and functions
- **SEO**
  - Social cards: automatically add metadata for Twitter Cards, [Facebook Open Graph](https://ogp.me/), [schema.org](https://schema.org/) and more.
  - [sitemap.xml](https://www.sitemaps.org/protocol.html) support
- **ATProto integration**
  - Bluesky microblogging
  - standard.site integration

## Install

```sh
cargo install italic
```

This puts `italic` on your `PATH` (typically `~/.cargo/bin/italic`).

## Quick start

```sh
italic new my-site
cd my-site
echo '# Hello, world' > content/index.md
italic serve
```

Congrats! You have a website at <http://localhost:3000>.

To dress it up, grab a starter theme:

```sh
git clone --depth 1 https://github.com/gordonbrander/italic_themes.git themes/
```

```yaml
# config.yaml
theme: "themes/obsidian"
```

Then `italic build` outputs plain static files to `public/`, ready for any
host. The [quickstart](docs/getting-started/quickstart.md) covers all of this
in more detail.

## Documentation

Full documentation lives in [docs/](docs/index.md):

- **[Quickstart](docs/getting-started/quickstart.md)** — zero to website in four commands
- **[Tutorial](docs/getting-started/tutorial.md)** — publish your Obsidian vault, with backlinks, tags, and feeds
- **Concepts** — [project layout](docs/concepts/project-layout.md), [content model](docs/concepts/content-model.md), [the build pipeline](docs/concepts/build-pipeline.md)
- **Guides** — [wikilinks](docs/guides/wikilinks.md), [related pages](docs/guides/related.md), [collections](docs/guides/collections.md), [taxonomies](docs/guides/taxonomies.md), [templates](docs/guides/templates.md), [archives & feeds](docs/guides/archives.md), [themes](docs/guides/themes.md), [deployment](docs/guides/deployment.md), [migration](docs/guides/migration.md), and more
- **Reference** — [CLI](docs/reference/cli.md), [configuration](docs/reference/config.md), [frontmatter](docs/reference/frontmatter.md), [templates](docs/reference/templates.md)

## License

AGPL — see [LICENSE-AGPL](LICENSE-AGPL).
