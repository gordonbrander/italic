# Components (shortcodes)

Components are italic's shortcodes: reusable snippets — video embeds,
responsive images, callouts — that you call from Markdown. They're plain
[Tera components](https://keats.github.io/tera/), so there's no separate
shortcode language to learn.

## Writing a component

Define a component in any file under `templates/` (a `templates/components/`
folder is a nice convention, but not required — components register globally
from wherever they're defined):

```html
<!-- templates/components/youtube.html -->
{% component youtube.embed(id: string) %}
<iframe src="https://www.youtube.com/embed/{{ id }}" allowfullscreen></iframe>
{% endcomponent youtube.embed %}
```

The name (`youtube.embed`) is declared in the component itself and can use
dots to namespace. Parameter types are optional; parameters with a default
(`variant: string = "primary"`) infer theirs. A component name must be unique
across the site's templates — defining it twice fails the build. (A site
template *file* still overrides a theme file of the same path, so a site can
replace a theme's component by shadowing the file that defines it.)

## Calling it from content

Call it from any Markdown body — no imports needed:

```markdown
Here's the talk:

{{<youtube.embed id="dQw4w9WgXcQ" />}}
```

The component expands *before* the Markdown render, so it can emit any HTML.
The same call works in layout templates. For argument values other than string
literals, wrap the value in braces: `{{<gallery images={page.data.images} />}}`.

Components can also wrap a body, which they receive as `body`:

```jinja
{% component callout(kind: string = "note") %}
<aside class="callout callout-{{ kind }}">{{ body }}</aside>
{% endcomponent callout %}
```

```markdown
{% <callout kind="warning"> %}
Mind the gap.
{% </callout> %}
```

## Content templates: the bigger picture

Component expansion works because italic runs a full Tera render on every
document body before rendering Markdown. That means documents can also use
partials, conditionals, loops, and the page's own data:

```markdown
---
tags: ["movies", "sci-fi", "review"]
---
This post is tagged:
{% for tag in page.data.tags %} #{{ tag }}{% endfor %}
```

**The content phase has limits.** The page index doesn't exist yet while
bodies render, so inside a document you can use `page`, `site`, `data`,
components, and the both-phase filters (`markdown`, `truncate_words`, the URL
filters, `entries`, `dirtree`, `filter_in_dir`, `omit_docs`, `dir()`) — but not
the functions and filters that read other pages (`collection()`, `all()`,
`taxonomy()`, `doc()`, `backlinks`, `related`). Those belong in layouts. See
[the build pipeline](../concepts/build-pipeline.md#consequences-worth-knowing).

## See also

- [Template reference](../reference/templates.md) — phase availability per filter
- [Templates guide](templates.md)
