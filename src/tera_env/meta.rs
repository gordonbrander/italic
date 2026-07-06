//! Built-in `<head>` metadata filters (template phase only). These spare every
//! theme from hand-rolling the same SEO / social-card / feed-discovery boilerplate
//! out of `page`/`site`, and from re-deriving the absolute-URL prefixing that
//! `permalink`/`absolute_url` already encode.
//!
//! All are registered as **filters** (not functions) so they can be *safe*
//! (`is_safe() == true`): they emit raw `<meta>`/`<link>`/`<script>` markup, which
//! Tera's autoescape in `.html`/`.xml` templates would otherwise mangle. This
//! mirrors the `markdown` and `url` filters (see `src/tera_env/url.rs`). Each
//! filter pipes its primary subject and takes the rest as kwargs:
//!
//! ```html
//! <head>
//!   {{ page | metadata(site=site) }}            {# umbrella one-liner #}
//! </head>
//! ```
//!
//! or composed:
//!
//! ```html
//! {{ page | meta_description(site=site) }}
//! {{ page | meta_keywords }}
//! {{ page | canonical_link }}
//! {{ page | standard_link }}
//! {{ page | open_graph(site=site, type="article") }}
//! {{ page | twitter_card(site=site) }}
//! {{ page | json_ld(site=site) }}
//! {{ site | feed_links }}
//! ```
//!
//! `site_url`/`base_path`/feed names are captured at registration time (the filter
//! API only hands a filter its piped value + kwargs, never the render context), so
//! absolute URLs and the feed list don't need to be threaded through templates.
//! When `site_url` is `None`, URLs fall back to root-relative — same graceful
//! degradation as the `url` filters.

use super::safe_filter::SafeFilter;
use crate::html;
use crate::permalink;
use serde_json::{Map, Value as Json};
use std::collections::HashMap;
use std::path::Path;
use tera::{Tera, Value};

/// `<meta name="generator">` content — the engine and its version, from Cargo.
const GENERATOR: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

/// Register every metadata filter on the template env, capturing the site origin,
/// base path, and configured feed names for absolute-URL/feed composition.
///
/// Each filter pipes its primary subject (`page`, or `site` for `feed_links`) and
/// reads `site=`/`type=` kwargs via [`site_arg`]/[`type_arg`]; the rendering lives
/// in the `render_*` functions below. [`SafeFilter`] is the only adapter needed —
/// it makes each closure a *safe* filter so its markup isn't autoescaped.
pub fn register(
    env: &mut Tera,
    site_url: Option<String>,
    base_path: String,
    feed_names: Vec<String>,
) {
    let cfg = MetaCfg {
        site_url,
        base_path,
        feed_names,
    };

    let c = cfg.clone();
    env.register_filter(
        "metadata",
        SafeFilter(move |page: &Value, args: &Kwargs| {
            render_metadata(page, site_arg(args), type_arg(args), &c)
        }),
    );

    env.register_filter(
        "meta_description",
        SafeFilter(|page: &Value, args: &Kwargs| join(render_description(page, site_arg(args)))),
    );

    env.register_filter(
        "meta_keywords",
        SafeFilter(|page: &Value, _args: &Kwargs| join(render_keywords(page))),
    );

    let c = cfg.clone();
    env.register_filter(
        "canonical_link",
        SafeFilter(move |page: &Value, _args: &Kwargs| join(render_canonical(page, &c))),
    );

    env.register_filter(
        "standard_link",
        SafeFilter(|page: &Value, _args: &Kwargs| join(render_standard_link(page))),
    );

    let c = cfg.clone();
    env.register_filter(
        "open_graph",
        SafeFilter(move |page: &Value, args: &Kwargs| {
            join(render_open_graph(page, site_arg(args), type_arg(args), &c))
        }),
    );

    let c = cfg.clone();
    env.register_filter(
        "twitter_card",
        SafeFilter(move |page: &Value, args: &Kwargs| {
            join(render_twitter_card(page, site_arg(args), &c))
        }),
    );

    let c = cfg.clone();
    env.register_filter(
        "json_ld",
        SafeFilter(move |page: &Value, args: &Kwargs| {
            render_json_ld(page, site_arg(args), type_arg(args), &c)
        }),
    );

    env.register_filter(
        "system_meta",
        SafeFilter(|_page: &Value, _args: &Kwargs| join(render_system_meta())),
    );

    // `feed_links` pipes `site` (not `page`); it's the last user of `cfg`.
    env.register_filter(
        "feed_links",
        SafeFilter(move |site: &Value, _args: &Kwargs| render_feed_links(site, &cfg)),
    );
}

/// A Tera filter's keyword arguments.
type Kwargs = HashMap<String, Value>;

/// Config captured once at registration and shared by the filter closures.
#[derive(Clone)]
struct MetaCfg {
    site_url: Option<String>,
    base_path: String,
    feed_names: Vec<String>,
}

/// The `site=` kwarg, or a `Null` placeholder so `field(site, …)` lookups just
/// miss (used by every filter that takes `site` as a kwarg rather than the pipe).
fn site_arg(args: &Kwargs) -> &Value {
    static NULL: Value = Value::Null;
    args.get("site").unwrap_or(&NULL)
}

/// The `type=` kwarg (`og:type` / JSON-LD `@type`), defaulting to `"article"`.
fn type_arg(args: &Kwargs) -> &str {
    args.get("type")
        .and_then(Value::as_str)
        .unwrap_or("article")
}

// ---------------------------------------------------------------------------
// Value readers
// ---------------------------------------------------------------------------

/// A trimmed, non-empty string field of `v`, or `None`.
fn field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// A trimmed, non-empty `page.data.<key>` string field.
fn data_field<'a>(page: &'a Value, key: &str) -> Option<&'a str> {
    page.get("data").and_then(|d| field(d, key))
}

/// Page title, falling back to the site title.
fn title<'a>(page: &'a Value, site: &'a Value) -> Option<&'a str> {
    field(page, "title").or_else(|| field(site, "title"))
}

/// Meta description: the page summary, else the site description.
fn description<'a>(page: &'a Value, site: &'a Value) -> Option<&'a str> {
    field(page, "summary").or_else(|| field(site, "description"))
}

/// Tag display texts from `page.terms.tags` (value side of slug → text),
/// deterministically ordered (the source `BTreeMap` is sorted by slug).
fn tags(page: &Value) -> Vec<&str> {
    page.get("terms")
        .and_then(|t| t.get("tags"))
        .and_then(Value::as_object)
        .map(|m| m.values().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Keyword string: tag texts joined, else `page.data.keywords` (a string or a
/// list of strings).
fn keywords(page: &Value) -> Option<String> {
    let tags = tags(page);
    if !tags.is_empty() {
        return Some(tags.join(", "));
    }
    match page.get("data").and_then(|d| d.get("keywords")) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Array(items)) => {
            let parts: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        _ => None,
    }
}

/// Absolute URL for `path`: passed through verbatim when already absolute,
/// otherwise prefixed with `site_url + base_path` (root-relative when no
/// `site_url`). Mirrors `url::AbsoluteUrlFilter`.
fn abs_url(path: &str, cfg: &MetaCfg) -> String {
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("//") {
        return path.to_string();
    }
    format!(
        "{}{}/{}",
        cfg.site_url.as_deref().unwrap_or(""),
        cfg.base_path,
        path.trim_start_matches('/')
    )
}

/// The page's canonical absolute URL, derived from its `output_path` the same way
/// `url::PermalinkFilter` derives `permalink`.
fn page_url(page: &Value, cfg: &MetaCfg) -> Option<String> {
    let output_path = page.get("output_path").and_then(Value::as_str)?;
    let url = permalink::to_url(Path::new(output_path));
    Some(format!(
        "{}{}{}",
        cfg.site_url.as_deref().unwrap_or(""),
        cfg.base_path,
        url
    ))
}

/// The site's root absolute URL (`site_url + base_path + "/"`).
fn site_root(cfg: &MetaCfg) -> String {
    format!(
        "{}{}/",
        cfg.site_url.as_deref().unwrap_or(""),
        cfg.base_path
    )
}

/// Resolved social image: per-page `data.image`, else site-wide `site.image`,
/// made absolute, paired with `data.image_alt` when present.
fn image(page: &Value, site: &Value, cfg: &MetaCfg) -> Option<(String, Option<String>)> {
    let raw = data_field(page, "image").or_else(|| field(site, "image"))?;
    let alt = data_field(page, "image_alt").map(str::to_string);
    Some((abs_url(raw, cfg), alt))
}

// ---------------------------------------------------------------------------
// Tag emitters
// ---------------------------------------------------------------------------

fn meta_name(name: &str, content: &str) -> String {
    format!(
        "<meta name=\"{}\" content=\"{}\">",
        name,
        html::escape(content)
    )
}

fn meta_prop(prop: &str, content: &str) -> String {
    format!(
        "<meta property=\"{}\" content=\"{}\">",
        prop,
        html::escape(content)
    )
}

/// Join rendered lines with newlines, dropping the block entirely when empty.
fn join(lines: Vec<String>) -> String {
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Block renderers
// ---------------------------------------------------------------------------

/// Italic/system-controlled tags. Today just the generator tag; the home for
/// future engine-owned `<head>` metadata (pagination `rel=prev/next`, etc.).
fn render_system_meta() -> Vec<String> {
    vec![meta_name("generator", GENERATOR)]
}

fn render_description(page: &Value, site: &Value) -> Vec<String> {
    description(page, site)
        .map(|d| vec![meta_name("description", d)])
        .unwrap_or_default()
}

fn render_keywords(page: &Value) -> Vec<String> {
    keywords(page)
        .map(|k| vec![meta_name("keywords", &k)])
        .unwrap_or_default()
}

fn render_canonical(page: &Value, cfg: &MetaCfg) -> Vec<String> {
    page_url(page, cfg)
        .map(|url| {
            vec![format!(
                "<link rel=\"canonical\" href=\"{}\">",
                html::escape(&url)
            )]
        })
        .unwrap_or_default()
}

/// standard.site per-document ownership proof, from `page.data.atproto_uri`
/// (injected by the `standard_link` build pass once `italic publish` has
/// recorded document records — see `crate::build::standard_link`). The AT-URI is
/// already absolute; it must not go through [`abs_url`], which would mangle the
/// `at://` scheme.
fn render_standard_link(page: &Value) -> Vec<String> {
    data_field(page, "atproto_uri")
        .map(|uri| {
            vec![format!(
                "<link rel=\"site.standard.document\" href=\"{}\">",
                html::escape(uri)
            )]
        })
        .unwrap_or_default()
}

fn render_open_graph(page: &Value, site: &Value, og_type: &str, cfg: &MetaCfg) -> Vec<String> {
    let mut out = vec![meta_prop("og:type", og_type)];
    if let Some(t) = title(page, site) {
        out.push(meta_prop("og:title", t));
    }
    if let Some(d) = description(page, site) {
        out.push(meta_prop("og:description", d));
    }
    if let Some(url) = page_url(page, cfg) {
        out.push(meta_prop("og:url", &url));
    }
    if let Some(name) = field(site, "title") {
        out.push(meta_prop("og:site_name", name));
    }
    out.push(meta_prop(
        "og:locale",
        field(site, "locale").unwrap_or("en_US"),
    ));
    if let Some((url, alt)) = image(page, site, cfg) {
        out.push(meta_prop("og:image", &url));
        if let Some(alt) = alt {
            out.push(meta_prop("og:image:alt", &alt));
        }
    }
    if og_type == "article" {
        if let Some(date) = field(page, "date") {
            out.push(meta_prop("article:published_time", date));
        }
        if let Some(updated) = field(page, "updated") {
            out.push(meta_prop("article:modified_time", updated));
        }
        if let Some(author) = data_field(page, "author").or_else(|| field(site, "author")) {
            out.push(meta_prop("article:author", author));
        }
        for tag in tags(page) {
            out.push(meta_prop("article:tag", tag));
        }
    }
    out
}

fn render_twitter_card(page: &Value, site: &Value, cfg: &MetaCfg) -> Vec<String> {
    let img = image(page, site, cfg);
    let card = if img.is_some() {
        "summary_large_image"
    } else {
        "summary"
    };
    let mut out = vec![meta_name("twitter:card", card)];
    if let Some(handle) = field(site, "twitter") {
        out.push(meta_name("twitter:site", handle));
        out.push(meta_name("twitter:creator", handle));
    }
    if let Some(t) = title(page, site) {
        out.push(meta_name("twitter:title", t));
    }
    if let Some(d) = description(page, site) {
        out.push(meta_name("twitter:description", d));
    }
    if let Some((url, alt)) = img {
        out.push(meta_name("twitter:image", &url));
        if let Some(alt) = alt {
            out.push(meta_name("twitter:image:alt", &alt));
        }
    }
    out
}

/// JSON-LD: a `BlogPosting` for `type="article"`, else a `WebSite`. Built with
/// `serde_json` so quoting/escaping is always valid, then `<` is `<`-escaped
/// so a title containing `</script>` can't break out of the `<script>` element.
fn render_json_ld(page: &Value, site: &Value, og_type: &str, cfg: &MetaCfg) -> String {
    let Some(title) = title(page, site) else {
        return String::new();
    };
    let mut obj = Map::new();
    obj.insert("@context".into(), Json::from("https://schema.org"));
    if og_type == "article" {
        obj.insert("@type".into(), Json::from("BlogPosting"));
        obj.insert("headline".into(), Json::from(title));
        if let Some(d) = description(page, site) {
            obj.insert("description".into(), Json::from(d));
        }
        if let Some(url) = page_url(page, cfg) {
            obj.insert("url".into(), Json::from(url));
        }
        if let Some(date) = field(page, "date") {
            obj.insert("datePublished".into(), Json::from(date));
        }
        if let Some(updated) = field(page, "updated") {
            obj.insert("dateModified".into(), Json::from(updated));
        }
        if let Some(author) = data_field(page, "author").or_else(|| field(site, "author")) {
            let mut a = Map::new();
            a.insert("@type".into(), Json::from("Person"));
            a.insert("name".into(), Json::from(author));
            obj.insert("author".into(), Json::Object(a));
        }
        if let Some((url, _)) = image(page, site, cfg) {
            obj.insert("image".into(), Json::from(url));
        }
    } else {
        obj.insert("@type".into(), Json::from("WebSite"));
        obj.insert("name".into(), Json::from(title));
        if let Some(d) = description(page, site) {
            obj.insert("description".into(), Json::from(d));
        }
        obj.insert("url".into(), Json::from(site_root(cfg)));
    }
    let json = Json::Object(obj).to_string().replace('<', "\\u003c");
    format!("<script type=\"application/ld+json\">{}</script>", json)
}

fn render_feed_links(site: &Value, cfg: &MetaCfg) -> String {
    let base_title = field(site, "title").unwrap_or("Feed");
    let links: Vec<String> = cfg
        .feed_names
        .iter()
        .map(|name| {
            let title = if name == "all" {
                base_title.to_string()
            } else {
                format!("{} — {}", base_title, name)
            };
            let href = abs_url(&format!("/feed/{}.xml", name), cfg);
            format!(
                "<link rel=\"alternate\" type=\"application/rss+xml\" title=\"{}\" href=\"{}\">",
                html::escape(&title),
                html::escape(&href)
            )
        })
        .collect();
    join(links)
}

/// The umbrella: a complete, sensible `<head>` block in one call. Themes that want
/// finer control compose the individual filters instead.
fn render_metadata(page: &Value, site: &Value, og_type: &str, cfg: &MetaCfg) -> String {
    let mut blocks: Vec<String> = vec![
        "<meta charset=\"utf-8\">".to_string(),
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">".to_string(),
    ];
    blocks.extend(render_system_meta());
    if let Some(t) = page_title(page, site) {
        blocks.push(format!("<title>{}</title>", html::escape(&t)));
    }
    blocks.extend(render_description(page, site));
    blocks.extend(render_keywords(page));
    // Drafts (rendered locally via `serve`/`watch` or `--drafts`) should never be
    // indexed if they leak to a host.
    if page.get("draft").and_then(Value::as_bool) == Some(true) {
        blocks.push(meta_name("robots", "noindex"));
    }
    blocks.extend(render_canonical(page, cfg));
    blocks.extend(render_standard_link(page));
    blocks.extend(render_open_graph(page, site, og_type, cfg));
    blocks.extend(render_twitter_card(page, site, cfg));
    let json_ld = render_json_ld(page, site, og_type, cfg);
    if !json_ld.is_empty() {
        blocks.push(json_ld);
    }
    let feeds = render_feed_links(site, cfg);
    if !feeds.is_empty() {
        blocks.push(feeds);
    }
    join(blocks)
}

/// `<title>` text for the umbrella: `Page · Site` when both exist, else whichever
/// is present.
fn page_title(page: &Value, site: &Value) -> Option<String> {
    match (field(page, "title"), field(site, "title")) {
        (Some(p), Some(s)) => Some(format!("{} · {}", p, s)),
        (Some(p), None) => Some(p.to_string()),
        (None, Some(s)) => Some(s.to_string()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg() -> MetaCfg {
        MetaCfg {
            site_url: Some("https://example.com".to_string()),
            base_path: String::new(),
            feed_names: vec!["all".to_string()],
        }
    }

    fn page() -> Value {
        json!({
            "title": "Hello & <World>",
            "summary": "A \"short\" summary",
            "output_path": "posts/hello/index.html",
            "draft": false,
            "date": "2026-06-29T00:00:00Z",
            "updated": "2026-06-30T00:00:00Z",
            "terms": { "tags": { "rust": "Rust", "ssg": "SSG" } },
            "data": { "image": "/img/cover.png", "image_alt": "Cover", "author": "Ada" }
        })
    }

    fn site() -> Value {
        json!({
            "title": "My Site",
            "description": "Site desc",
            "author": "Site Author",
            "twitter": "@handle",
            "locale": "en_GB",
            "image": "/img/default.png"
        })
    }

    /// Render a `{{ subject | <filter>(... ) }}` expression through a registered
    /// Tera env. `args` is the raw kwarg string, e.g. `"site=site"`.
    fn render(filter: &str, subject: &str, args: &str) -> String {
        let mut tera = Tera::default();
        register(&mut tera, cfg().site_url, cfg().base_path, cfg().feed_names);
        let mut ctx = tera::Context::new();
        ctx.insert("page", &page());
        ctx.insert("site", &site());
        let expr = if args.is_empty() {
            format!("{{{{ {} | {} }}}}", subject, filter)
        } else {
            format!("{{{{ {} | {}({}) }}}}", subject, filter, args)
        };
        tera.render_str(&expr, &ctx).unwrap()
    }

    #[test]
    fn description_falls_back_to_site() {
        assert_eq!(
            render("meta_description", "page", "site=site"),
            r#"<meta name="description" content="A &quot;short&quot; summary">"#
        );
        let no_summary = json!({});
        let mut tera = Tera::default();
        register(&mut tera, None, String::new(), vec![]);
        let mut ctx = tera::Context::new();
        ctx.insert("page", &no_summary);
        ctx.insert("site", &site());
        assert_eq!(
            tera.render_str("{{ page | meta_description(site=site) }}", &ctx)
                .unwrap(),
            r#"<meta name="description" content="Site desc">"#
        );
    }

    #[test]
    fn keywords_from_tags() {
        assert_eq!(
            render("meta_keywords", "page", ""),
            r#"<meta name="keywords" content="Rust, SSG">"#
        );
    }

    #[test]
    fn canonical_is_absolute() {
        assert_eq!(
            render("canonical_link", "page", ""),
            r#"<link rel="canonical" href="https://example.com/posts/hello/">"#
        );
    }

    #[test]
    fn open_graph_has_og_and_article_tags() {
        let out = render("open_graph", "page", "site=site");
        assert!(out.contains(r#"<meta property="og:type" content="article">"#));
        assert!(out.contains(r#"<meta property="og:title" content="Hello &amp; &lt;World&gt;">"#));
        assert!(
            out.contains(r#"<meta property="og:url" content="https://example.com/posts/hello/">"#)
        );
        assert!(out.contains(r#"<meta property="og:site_name" content="My Site">"#));
        assert!(out.contains(r#"<meta property="og:locale" content="en_GB">"#));
        assert!(
            out.contains(
                r#"<meta property="og:image" content="https://example.com/img/cover.png">"#
            )
        );
        assert!(out.contains(r#"<meta property="og:image:alt" content="Cover">"#));
        assert!(out.contains(
            r#"<meta property="article:published_time" content="2026-06-29T00:00:00Z">"#
        ));
        assert!(
            out.contains(
                r#"<meta property="article:modified_time" content="2026-06-30T00:00:00Z">"#
            )
        );
        assert!(out.contains(r#"<meta property="article:author" content="Ada">"#));
        assert!(out.contains(r#"<meta property="article:tag" content="Rust">"#));
        assert!(out.contains(r#"<meta property="article:tag" content="SSG">"#));
    }

    #[test]
    fn open_graph_website_type_omits_article_tags() {
        let out = render("open_graph", "page", r#"site=site, type="website""#);
        assert!(out.contains(r#"<meta property="og:type" content="website">"#));
        assert!(!out.contains("article:"));
    }

    #[test]
    fn twitter_card_large_with_image() {
        let out = render("twitter_card", "page", "site=site");
        assert!(out.contains(r#"<meta name="twitter:card" content="summary_large_image">"#));
        assert!(out.contains(r#"<meta name="twitter:site" content="@handle">"#));
        assert!(out.contains(r#"<meta name="twitter:creator" content="@handle">"#));
        assert!(out.contains(
            r#"<meta name="twitter:image" content="https://example.com/img/cover.png">"#
        ));
    }

    #[test]
    fn twitter_card_downgrades_without_image() {
        let no_image = json!({ "title": "T", "summary": "S" });
        let mut tera = Tera::default();
        register(&mut tera, cfg().site_url, cfg().base_path, cfg().feed_names);
        let mut ctx = tera::Context::new();
        ctx.insert("page", &no_image);
        ctx.insert("site", &json!({ "title": "Site" }));
        let out = tera
            .render_str("{{ page | twitter_card(site=site) }}", &ctx)
            .unwrap();
        assert!(out.contains(r#"<meta name="twitter:card" content="summary">"#));
        assert!(!out.contains("twitter:image"));
    }

    #[test]
    fn root_relative_when_no_site_url() {
        let mut tera = Tera::default();
        register(&mut tera, None, String::new(), vec![]);
        let mut ctx = tera::Context::new();
        ctx.insert("page", &page());
        ctx.insert("site", &site());
        let out = tera
            .render_str("{{ page | canonical_link }}", &ctx)
            .unwrap();
        assert_eq!(out, r#"<link rel="canonical" href="/posts/hello/">"#);
    }

    #[test]
    fn draft_emits_noindex() {
        let draft = json!({ "title": "D", "summary": "S", "output_path": "d.html", "draft": true });
        let mut tera = Tera::default();
        register(&mut tera, cfg().site_url, cfg().base_path, cfg().feed_names);
        let mut ctx = tera::Context::new();
        ctx.insert("page", &draft);
        ctx.insert("site", &site());
        let out = tera
            .render_str("{{ page | metadata(site=site) }}", &ctx)
            .unwrap();
        assert!(out.contains(r#"<meta name="robots" content="noindex">"#));
        // The non-draft fixture page must not get noindex.
        assert!(!render("metadata", "page", "site=site").contains("noindex"));
    }

    /// A page carrying the AT-URI the `standard_link` build pass injects.
    fn page_with_atproto_uri() -> Value {
        let mut p = page();
        p["data"]["atproto_uri"] = json!("at://did:plc:abc/site.standard.document/xyz");
        p
    }

    #[test]
    fn standard_link_emits_proof_link() {
        let mut tera = Tera::default();
        register(&mut tera, cfg().site_url, cfg().base_path, cfg().feed_names);
        let mut ctx = tera::Context::new();
        ctx.insert("page", &page_with_atproto_uri());
        let out = tera.render_str("{{ page | standard_link }}", &ctx).unwrap();
        // Exact output: the `at://` slashes must survive unescaped.
        assert_eq!(
            out,
            r#"<link rel="site.standard.document" href="at://did:plc:abc/site.standard.document/xyz">"#
        );
    }

    #[test]
    fn standard_link_empty_when_absent() {
        assert_eq!(render("standard_link", "page", ""), "");
    }

    #[test]
    fn metadata_umbrella_includes_standard_link_after_canonical() {
        let mut tera = Tera::default();
        register(&mut tera, cfg().site_url, cfg().base_path, cfg().feed_names);
        let mut ctx = tera::Context::new();
        ctx.insert("page", &page_with_atproto_uri());
        ctx.insert("site", &site());
        let out = tera
            .render_str("{{ page | metadata(site=site) }}", &ctx)
            .unwrap();
        let canonical = out.find(r#"rel="canonical""#).expect("canonical link");
        let standard = out
            .find(r#"rel="site.standard.document""#)
            .expect("standard.site link");
        assert!(canonical < standard);
        // A page without the AT-URI emits no proof link.
        assert!(!render("metadata", "page", "site=site").contains("site.standard.document"));
    }

    /// Version-agnostic: assert against `env!`, never a hardcoded number.
    fn generator_tag() -> String {
        format!(
            r#"<meta name="generator" content="{} {}">"#,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        )
    }

    #[test]
    fn system_meta_has_generator_tag() {
        assert_eq!(render("system_meta", "page", ""), generator_tag());
    }

    #[test]
    fn metadata_umbrella_includes_generator() {
        assert!(render("metadata", "page", "site=site").contains(&generator_tag()));
    }

    #[test]
    fn json_ld_is_valid_json_and_script_safe() {
        let out = render("json_ld", "page", "site=site");
        let inner = out
            .strip_prefix(r#"<script type="application/ld+json">"#)
            .and_then(|s| s.strip_suffix("</script>"))
            .expect("script wrapper");
        // No literal `</script>` breakout, and the body round-trips as JSON.
        assert!(!inner.contains("</script>"));
        let parsed: Json = serde_json::from_str(inner).expect("valid JSON-LD");
        assert_eq!(parsed["@type"], json!("BlogPosting"));
        assert_eq!(parsed["headline"], json!("Hello & <World>"));
        assert_eq!(parsed["author"]["name"], json!("Ada"));
    }

    #[test]
    fn feed_links_one_per_configured_feed() {
        let mut tera = Tera::default();
        register(
            &mut tera,
            Some("https://example.com".to_string()),
            String::new(),
            vec!["all".to_string(), "posts".to_string()],
        );
        let mut ctx = tera::Context::new();
        ctx.insert("site", &site());
        let out = tera.render_str("{{ site | feed_links }}", &ctx).unwrap();
        assert_eq!(out.matches("<link").count(), 2);
        assert!(out.contains(r#"title="My Site" href="https://example.com/feed/all.xml""#));
        assert!(
            out.contains(r#"title="My Site — posts" href="https://example.com/feed/posts.xml""#)
        );
    }

    #[test]
    fn feed_links_empty_when_no_feeds() {
        let mut tera = Tera::default();
        register(&mut tera, None, String::new(), vec![]);
        let mut ctx = tera::Context::new();
        ctx.insert("site", &site());
        assert_eq!(
            tera.render_str("{{ site | feed_links }}", &ctx).unwrap(),
            ""
        );
    }

    #[test]
    fn safe_filter_is_not_autoescaped_in_html_templates() {
        let mut tera = Tera::default();
        register(&mut tera, cfg().site_url, cfg().base_path, cfg().feed_names);
        // A named `.html` template *is* autoescaped; `is_safe()` must keep the
        // markup raw.
        tera.add_raw_template("t.html", "{{ page | canonical_link }}")
            .unwrap();
        let mut ctx = tera::Context::new();
        ctx.insert("page", &page());
        ctx.insert("site", &site());
        let out = tera.render("t.html", &ctx).unwrap();
        assert!(out.starts_with("<link rel=\"canonical\""));
        assert!(!out.contains("&lt;link"));
    }
}
