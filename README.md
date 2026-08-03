# Italic

_Publish your digital garden to the Atmosphere_

Transform your Markdown notes or [Obsidian Vault](https://obsidian.md) into blogs, wikis, and websites that you can publish to [Bluesky](https://bsky.app) and the web. Think in public, build your audience, own your data.

- **Blog-aware**: custom taxonomies, theming, archives, rss and more
- **Obsidian-compatible**: Wikilinks, backlinks, block references and support for Obsidian-flavored Markdown.
- **ATProto-enabled**: publish to [Bluesky](https://bsky.app) and [ATProto](https://atproto.com/), with support for bsky and [standard.site](https://standard.site/) records.

## Features

- **Markup**
  - [Github-flavored Markdown](.agents/skills/italic/references/guides/authoring.md)
  - [Multiple formats](.agents/skills/italic/references/guides/authoring.md): publish content from Markdown, HTML, and YAML files
  - [Math (LaTeX/KaTeX)](.agents/skills/italic/references/guides/authoring.md): inline/display math for technical gardens
  - [Components (shortcodes)](.agents/skills/italic/references/guides/components.md): define custom components you can use in your Markdown for YouTube embeds, UI widgets, and more.
  - [Code fences with syntax highlighting](.agents/skills/italic/references/guides/authoring.md)
  - [Callouts](.agents/skills/italic/references/guides/authoring.md): info, warnings, etc.
- **Digital gardens**
  - [Wikilinks](.agents/skills/italic/references/guides/wikilinks.md): fuzzy link matching using the same algorithm as Obsidian
  - [Backlinks](.agents/skills/italic/references/guides/wikilinks.md): see everything that links back to a page.
  - [Block references](.agents/skills/italic/references/guides/wikilinks.md): deep link to headings and blocks with Obsidian-style purple links.
  - [Hashtags](.agents/skills/italic/references/guides/taxonomies.md): lifted up to tag taxonomy
  - [Obsidian Markdown extensions](.agents/skills/italic/references/guides/authoring.md): supports lots of long-tail [Obsidian Markdown features](https://obsidian.md/help/syntax).
  - [Obsidian Vaults](.agents/skills/italic/references/guides/migration.md): seamlessly transform your Vault into a website.
- **Blogs**
  - [Draft posts](.agents/skills/italic/references/guides/drafts.md)
  - [Tags](.agents/skills/italic/references/guides/taxonomies.md)
  - [Archives](.agents/skills/italic/references/guides/archives.md): paginated archival posts
  - [RSS](.agents/skills/italic/references/guides/archives.md): syndicate your posts, including multiple custom feeds
  - [Publish multiple blogs on the same site](.agents/skills/italic/references/guides/collections.md) (custom collections)
- **Content websites**
  - [Custom permalinks](.agents/skills/italic/references/guides/permalinks.md): customizable per-collection and per-page
  - [Redirects](.agents/skills/italic/references/guides/redirects.md)
  - [Custom taxonomies](.agents/skills/italic/references/guides/taxonomies.md): organize posts by tag, category, artist, label, phase of the moon — no problem.
  - [Custom collections](.agents/skills/italic/references/guides/collections.md): glob-match files to define custom page collections you can use in templates
  - [Related pages](.agents/skills/italic/references/guides/related.md): Surface related posts with a customizable algorithm
  - [Page trees](.agents/skills/italic/references/guides/templates.md): organize pages into a tree for menus
  - [Collection defaults](.agents/skills/italic/references/guides/collections.md): add custom metadata to groups of pages
  - [Data files](.agents/skills/italic/references/guides/data.md): bulk-add custom template data via the `data/` folder
- **Theming**
  - [Themes](.agents/skills/italic/references/guides/themes.md)
  - [Theme overrides](.agents/skills/italic/references/guides/themes.md)
  - [Tera templates](.agents/skills/italic/references/guides/templates.md): blazingly fast Jinja-like templates in Rust with support for template functions, filters, components, template extension, and more.
  - [Lots of built-in custom filters and functions](.agents/skills/italic/references/reference/templates.md)
- **SEO**
  - [Social cards](.agents/skills/italic/references/guides/metadata.md): automatically add metadata for Twitter Cards, [Facebook Open Graph](https://ogp.me/), [schema.org](https://schema.org/) and more.
  - [sitemap.xml](.agents/skills/italic/references/guides/archives.md) support
- **Development**
  - [Built-in local dev server](.agents/skills/italic/references/reference/cli.md)
  - [Super-fast hot reload](.agents/skills/italic/references/reference/cli.md)
- **ATProto integration**
  - [PDS integration](.agents/skills/italic/references/guides/publishing-atproto.md): publish and check your website's sync status
  - [Bluesky microblogging](.agents/skills/italic/references/guides/publishing-atproto.md)
  - [standard.site records](.agents/skills/italic/references/guides/publishing-atproto.md)

## Install

```sh
cargo install italic
```

## Quick start

The fastest way to get started is to install the Italic skill and let your coding agent set up a site for you.

Claude Code:

```sh
/plugin marketplace add gordonbrander/italic
/plugin install italic@italic
```

Codex:

```sh
$skill-installer install https://github.com/gordonbrander/italic/tree/main/.agents/skills/italic
```

Or, do it by hand:

```sh
italic new my-site
cd my-site
echo '# Hello, world' > content/index.md
italic serve
```

Congrats! You have a website at <http://localhost:3000>.

`italic build` outputs plain static files to `public/`, ready for any
host. The [quickstart](.agents/skills/italic/references/getting-started/quickstart.md) covers all of this
in more detail.

## Documentation

Full documentation lives in
[.agents/skills/italic/references/](.agents/skills/italic/references/index.md),
where it doubles as the reference material for the Italic agent skill:

- **[Quickstart](.agents/skills/italic/references/getting-started/quickstart.md)** — zero to website in four commands
- **[Tutorial](.agents/skills/italic/references/getting-started/tutorial.md)** — publish your Obsidian vault, with backlinks, tags, and feeds
- **Concepts** — [project layout](.agents/skills/italic/references/concepts/project-layout.md), [content model](.agents/skills/italic/references/concepts/content-model.md), [the build pipeline](.agents/skills/italic/references/concepts/build-pipeline.md)
- **Guides** — [wikilinks](.agents/skills/italic/references/guides/wikilinks.md), [related pages](.agents/skills/italic/references/guides/related.md), [collections](.agents/skills/italic/references/guides/collections.md), [taxonomies](.agents/skills/italic/references/guides/taxonomies.md), [templates](.agents/skills/italic/references/guides/templates.md), [archives & feeds](.agents/skills/italic/references/guides/archives.md), [themes](.agents/skills/italic/references/guides/themes.md), [deployment](.agents/skills/italic/references/guides/deployment.md), [migration](.agents/skills/italic/references/guides/migration.md), and more
- **Reference** — [CLI](.agents/skills/italic/references/reference/cli.md), [configuration](.agents/skills/italic/references/reference/config.md), [frontmatter](.agents/skills/italic/references/reference/frontmatter.md), [templates](.agents/skills/italic/references/reference/templates.md)

## License

AGPL — see [LICENSE-AGPL](LICENSE-AGPL).
