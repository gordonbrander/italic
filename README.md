# Italic

_Publish your digital garden to the Atmosphere_

Transform your Markdown notes or [Obsidian Vault](https://obsidian.md) into blogs, wikis, and websites that you can publish to [Bluesky](https://bsky.app) and the web. Think in public, build your audience, own your data.

- **Blog-aware**: custom taxonomies, theming, archives, rss and more
- **Obsidian-compatible**: Wikilinks, backlinks, block references and support for Obsidian-flavored Markdown.
- **ATProto-enabled**: publish to [Bluesky](https://bsky.app) and [ATProto](https://atproto.com/), with support for bsky and [standard.site](https://standard.site/) records.

## Features

- **Markup**
  - [Github-flavored Markdown](docs/guides/authoring.md)
  - [Multiple formats](docs/guides/authoring.md): publish content from Markdown, HTML, and YAML files
  - [Math (LaTeX/KaTeX)](docs/guides/authoring.md): inline/display math for technical gardens
  - [Components (shortcodes)](docs/guides/components.md): define custom components you can use in your Markdown for YouTube embeds, UI widgets, and more.
  - [Code fences with syntax highlighting](docs/guides/authoring.md)
  - [Callouts](docs/guides/authoring.md): info, warnings, etc.
- **Digital gardens**
  - [Wikilinks](docs/guides/wikilinks.md): fuzzy link matching using the same algorithm as Obsidian
  - [Backlinks](docs/guides/wikilinks.md): see everything that links back to a page.
  - [Block references](docs/guides/wikilinks.md): deep link to headings and blocks with Obsidian-style purple links.
  - [Hashtags](docs/guides/taxonomies.md): lifted up to tag taxonomy
  - [Obsidian Markdown extensions](docs/guides/authoring.md): supports lots of long-tail [Obsidian Markdown features](https://obsidian.md/help/syntax).
  - [Obsidian Vaults](docs/guides/migration.md): seamlessly transform your Vault into a website.
- **Blogs**
  - [Draft posts](docs/guides/drafts.md)
  - [Tags](docs/guides/taxonomies.md)
  - [Archives](docs/guides/archives.md): paginated archival posts
  - [RSS](docs/guides/archives.md): syndicate your posts, including multiple custom feeds
  - [Publish multiple blogs on the same site](docs/guides/collections.md) (custom collections)
- **Content websites**
  - [Custom permalinks](docs/guides/permalinks.md): customizable per-collection and per-page
  - [Redirects](docs/guides/redirects.md)
  - [Custom taxonomies](docs/guides/taxonomies.md): organize posts by tag, category, artist, label, phase of the moon — no problem.
  - [Custom collections](docs/guides/collections.md): glob-match files to define custom page collections you can use in templates
  - [Related pages](docs/guides/related.md): Surface related posts with a customizable algorithm
  - [Page trees](docs/guides/templates.md): organize pages into a tree for menus
  - [Collection defaults](docs/guides/collections.md): add custom metadata to groups of pages
  - [Data files](docs/guides/data.md): bulk-add custom template data via the `data/` folder
- **Theming**
  - [Themes](docs/guides/themes.md)
  - [Theme overrides](docs/guides/themes.md)
  - [Tera templates](docs/guides/templates.md): blazingly fast Jinja-like templates in Rust with support for template functions, filters, components, template extension, and more.
  - [Lots of built-in custom filters and functions](docs/reference/templates.md)
- **SEO**
  - [Social cards](docs/guides/metadata.md): automatically add metadata for Twitter Cards, [Facebook Open Graph](https://ogp.me/), [schema.org](https://schema.org/) and more.
  - [sitemap.xml](docs/guides/archives.md) support
- **Development**
  - [Built-in local dev server](docs/reference/cli.md)
  - [Super-fast hot reload](docs/reference/cli.md)
- **ATProto integration**
  - [PDS integration](docs/guides/publishing-atproto.md): publish and check your website's sync status
  - [Bluesky microblogging](docs/guides/publishing-atproto.md)
  - [standard.site records](docs/guides/publishing-atproto.md)

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
