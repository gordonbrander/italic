//! `markdown` filter — renders its string input through comrak. Registered on
//! both envs so `{% filter markdown %}…{% endfilter %}` (and `value | markdown`)
//! work in Markdown bodies and HTML/XML templates alike. Returns a safe value so
//! its HTML output is not autoescaped in `.html`/`.xml` templates.

use comrak::plugins::syntect::SyntectAdapter;
use std::sync::Arc;
use tera::{Error, Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera, options: comrak::Options<'static>, syntect: Arc<SyntectAdapter>) {
    env.register_filter(
        "markdown",
        move |input: &str, _: Kwargs, _: &State| -> TeraResult<Value> {
            let arena = comrak::Arena::new();
            let root = comrak::parse_document(&arena, input, &options);
            let mut plugins = comrak::options::Plugins::default();
            plugins.render.codefence_syntax_highlighter = Some(syntect.as_ref());
            let mut out = String::new();
            comrak::format_html_with_plugins(root, &options, &mut out, &plugins).map_err(|e| {
                Error::message(format!("markdown filter: comrak render failed: {e}"))
            })?;
            Ok(Value::safe_string(&out))
        },
    );
}
