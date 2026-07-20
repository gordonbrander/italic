//! General-purpose text-shaping filters, registered on both envs.
//!
//! - `truncate_words` — `text | truncate_words(length=N)`. Truncates at the
//!   last whitespace that fits, appending `…` when truncation happens. Default
//!   length is 250. Complements `striptags`, which strips HTML, and Tera's
//!   built-in `truncate`, which is not word-aware.
//! - `reading_time` — `page.content | reading_time`. Estimated reading time
//!   as `"N min read"`, from a simple character count of the tag-stripped text.

use crate::html;
use tera::{Kwargs, State, Tera, TeraResult};

/// Reading speed for `reading_time`: ≈200 words/minute × 5 chars/word.
const CHARS_PER_MINUTE: usize = 1000;

pub fn register(env: &mut Tera) {
    env.register_filter(
        "truncate_words",
        |text: &str, kwargs: Kwargs, _: &State| -> TeraResult<String> {
            let length = kwargs.get::<usize>("length")?.unwrap_or(250);
            Ok(html::truncate_words(text, length))
        },
    );

    env.register_filter(
        "reading_time",
        |text: &str, _: Kwargs, _: &State| -> TeraResult<String> {
            let chars = html::strip_tags(text).chars().count();
            let minutes = chars.div_ceil(CHARS_PER_MINUTE).max(1);
            Ok(format!("{} min read", minutes))
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(input: &str) -> String {
        let mut tera = Tera::default();
        register(&mut tera);
        let mut ctx = tera::Context::new();
        ctx.insert("text", &input);
        tera.render_str("{{ text | reading_time }}", &ctx, false)
            .unwrap()
    }

    #[test]
    fn reading_time_is_at_least_one_minute() {
        assert_eq!(render(""), "1 min read");
        assert_eq!(render("Short."), "1 min read");
    }

    #[test]
    fn reading_time_strips_tags_before_counting() {
        // 500 chars of text wrapped in enough tags to cross 1000 raw chars:
        // tags must not count toward the estimate.
        let text = format!(
            "{}{}{}",
            "<p>".repeat(300),
            "a".repeat(500),
            "</p>".repeat(300)
        );
        assert!(text.len() > 1000);
        assert_eq!(render(&text), "1 min read");
    }

    #[test]
    fn reading_time_rounds_up() {
        assert_eq!(render(&"a".repeat(1000)), "1 min read");
        assert_eq!(render(&"a".repeat(1001)), "2 min read");
        assert_eq!(render(&"a".repeat(3500)), "4 min read");
    }
}
