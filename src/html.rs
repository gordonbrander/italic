/// Escape the five XML/HTML special characters so a string can be safely
/// embedded in attribute values or element text.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Strip HTML tags from `html`, collapse all whitespace runs (including
/// newlines) to a single space, and trim. Naive state machine — sufficient for
/// well-formed input like pulldown-cmark output, not a general HTML parser.
/// HTML entities pass through unchanged.
pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut prev_space = true;
    for c in html.chars() {
        if in_tag {
            if c == '>' {
                in_tag = false;
            }
            continue;
        }
        if c == '<' {
            // Treat tag boundaries as word separators so that `</p><p>` doesn't
            // glue adjacent block content together.
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
            in_tag = true;
            continue;
        }
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Truncate `text` to at most `max_chars` Unicode scalar values, breaking at
/// the last whitespace boundary that fits, and append `…` when truncation
/// happens (the ellipsis counts toward the budget). When `text` fits in
/// `max_chars`, returns it unchanged. When the prefix contains no whitespace,
/// hard-cuts at `max_chars - 1` and appends `…`.
pub fn truncate_words(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let budget = max_chars - 1;
    let prefix: String = text.chars().take(budget).collect();
    let cut = match prefix.rfind(char::is_whitespace) {
        Some(i) => prefix[..i].trim_end().to_string(),
        None => prefix,
    };
    format!("{}…", cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_all_five() {
        assert_eq!(escape("a & b < c > d \" e ' f"), "a &amp; b &lt; c &gt; d &quot; e &#39; f");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        assert_eq!(escape("hello world"), "hello world");
    }

    #[test]
    fn preserves_non_ascii() {
        assert_eq!(escape("café · 你好"), "café · 你好");
    }

    #[test]
    fn empty_string() {
        assert_eq!(escape(""), "");
    }

    #[test]
    fn strip_tags_removes_simple_tags() {
        assert_eq!(strip_tags("<p>hello <b>world</b></p>"), "hello world");
    }

    #[test]
    fn strip_tags_handles_attributes() {
        assert_eq!(
            strip_tags(r#"<a href="https://x.test" class="ext">click</a>"#),
            "click"
        );
    }

    #[test]
    fn strip_tags_collapses_whitespace() {
        assert_eq!(strip_tags("a\n\n  b   c"), "a b c");
    }

    #[test]
    fn strip_tags_trims_edges() {
        assert_eq!(strip_tags("  <p>hi</p>  "), "hi");
    }

    #[test]
    fn strip_tags_empty_input() {
        assert_eq!(strip_tags(""), "");
    }

    #[test]
    fn strip_tags_preserves_entities() {
        // Entities pass through unchanged — fine for a preview blurb.
        assert_eq!(
            strip_tags("<p>caf&eacute; &amp; tea</p>"),
            "caf&eacute; &amp; tea"
        );
    }

    #[test]
    fn strip_tags_nested() {
        assert_eq!(
            strip_tags("<div><p>one</p><p>two <em>three</em></p></div>"),
            "one two three"
        );
    }

    #[test]
    fn truncate_words_returns_unchanged_when_within_limit() {
        assert_eq!(truncate_words("hello world", 250), "hello world");
    }

    #[test]
    fn truncate_words_empty_input() {
        assert_eq!(truncate_words("", 250), "");
    }

    #[test]
    fn truncate_words_breaks_on_word_boundary() {
        // 26 chars total; cap at 15 → budget 14 → last space at 11 → "hello world" + ellipsis.
        let out = truncate_words("hello world foo bar baz", 15);
        assert_eq!(out, "hello world…");
        assert!(out.chars().count() <= 15);
    }

    #[test]
    fn truncate_words_hard_cuts_when_no_whitespace() {
        // No whitespace before the budget → hard cut to budget-1 then ellipsis.
        let out = truncate_words("abcdefghijklmnop", 8);
        assert_eq!(out, "abcdefg…");
        assert_eq!(out.chars().count(), 8);
    }

    #[test]
    fn truncate_words_counts_unicode_scalar_values() {
        // "café" is 4 chars (5 bytes). With limit 4 it should pass through.
        assert_eq!(truncate_words("café", 4), "café");
    }

    #[test]
    fn truncate_words_handles_multibyte_in_truncation() {
        // 6 chars: "héllo世界 abc" → 10 chars. Limit 7 → budget 6 → last space at 7?
        // Build a deterministic case: "你好 世界 abc" — chars: 你, 好, ' ', 世, 界, ' ', a, b, c (9).
        let out = truncate_words("你好 世界 abc", 7);
        // budget 6 → prefix "你好 世界 " → trim → "你好 世界" → "你好 世界…"
        assert_eq!(out, "你好 世界…");
    }

    #[test]
    fn truncate_words_zero_limit() {
        assert_eq!(truncate_words("hello", 0), "");
    }
}
