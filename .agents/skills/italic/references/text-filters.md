# Text filters

All three work in **both phases**.

## `truncate_words` — word-aware truncation

Truncates at the last whitespace that fits, appending `…` when it cuts.
Unlike Tera's built-in `truncate`, it never splits a word. Pair with
`striptags` to summarize HTML.

| Kwarg | Default | Meaning |
|-------|---------|---------|
| `length` | `250` | Max length before truncation. |

```jinja
{{ page.content | striptags | truncate_words(length=140) }}
```

## `reading_time` — estimated reading time

Estimates from a character count of the tag-stripped text (~1000 characters
per minute); returns a string like `4 min read`, minimum `1 min read`.

```jinja
{{ page.content | reading_time }}
```

## `markdown` — render Markdown to HTML

Renders a Markdown string with the same renderer as document bodies
(GitHub-flavored, syntax-highlighted fences). Output is marked safe.
Wikilinks and `#hashtags` are **not** processed by this filter.

```jinja
{{ page.data.blurb | markdown }}

{% filter markdown %}
Some *Markdown*, a [link](https://example.com), and a `code` span.
{% endfilter %}
```
