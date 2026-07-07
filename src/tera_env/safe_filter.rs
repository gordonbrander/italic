//! [`SafeFilter`]: a generic adapter that turns a plain closure into a *safe*
//! Tera filter.
//!
//! Tera's blanket `Filter` impl for closures hard-codes `is_safe() == false`, so
//! a closure that emits trusted markup (`<meta>`, `<link>`, a URL) would be
//! HTML-autoescaped in `.html`/`.xml` templates. Wrapping the closure in
//! `SafeFilter` overrides `is_safe()` to `true` without a bespoke struct per
//! filter. (Filters that need to *fail* on bad input still want their own struct —
//! this adapter is for infallible, string-producing filters.)

use std::collections::HashMap;
use tera::{Filter, Value};

/// Wraps an infallible `Fn(&Value, &HashMap<String, Value>) -> String` as a Tera
/// filter whose output is marked safe (not autoescaped). The piped value is the
/// first argument; the filter's keyword args are the second.
pub(crate) struct SafeFilter<F>(pub(crate) F);

impl<F> Filter for SafeFilter<F>
where
    F: Fn(&Value, &HashMap<String, Value>) -> String + Send + Sync,
{
    fn filter(&self, value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        Ok(Value::String((self.0)(value, args)))
    }

    fn is_safe(&self) -> bool {
        true
    }
}
