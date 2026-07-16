//! General-purpose text-shaping filters, registered on both envs.
//!
//! - `truncate_words` — `text | truncate_words(length=N)`. Truncates at the
//!   last whitespace that fits, appending `…` when truncation happens. Default
//!   length is 250. Complements `striptags`, which strips HTML, and Tera's
//!   built-in `truncate`, which is not word-aware.

use crate::html;
use tera::{Kwargs, State, Tera, TeraResult};

pub fn register(env: &mut Tera) {
    env.register_filter(
        "truncate_words",
        |text: &str, kwargs: Kwargs, _: &State| -> TeraResult<String> {
            let length = kwargs.get::<usize>("length")?.unwrap_or(250);
            Ok(html::truncate_words(text, length))
        },
    );
}
