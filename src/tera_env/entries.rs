//! `entries` — turn a map into a key-sorted array, registered on both envs.
//!
//! Tera has no built-in for iterating a map in a defined order (its `sort`
//! filter only takes arrays). `map | entries` emits an array of
//! `{key, value}` objects sorted by `key`, so templates can walk a
//! `taxonomy(...)` map or a `group_by` result deterministically:
//!
//! ```html
//! {% for entry in tags | entries(sort="desc") %}
//!   {{ entry.key }}: {{ entry.value | length }}
//! {% endfor %}
//! ```
//!
//! `sort` is `"asc"` (default) or `"desc"`; any other value is an author error.

use std::collections::BTreeMap;
use tera::{Error, Kwargs, Map, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera) {
    env.register_filter(
        "entries",
        |map: &Map, kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            let descending = sort_is_descending(&kwargs)?;

            // Collect and sort by key so output is deterministic regardless of
            // how the map was built.
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            if descending {
                keys.reverse();
            }

            let entries: Vec<Value> = keys
                .into_iter()
                .map(|k| {
                    let entry: BTreeMap<&str, Value> =
                        [("key", k.as_value()), ("value", map[k].clone())]
                            .into_iter()
                            .collect();
                    Value::from(entry)
                })
                .collect();

            Ok(Value::from(entries))
        },
    );
}

/// `false` for `"asc"` (the default when omitted), `true` for `"desc"`. Any
/// other value is an author error, mirroring how the URL/`doc` adapters treat
/// malformed arguments rather than silently guessing.
fn sort_is_descending(kwargs: &Kwargs) -> TeraResult<bool> {
    match kwargs.get::<&str>("sort")? {
        None | Some("asc") => Ok(false),
        Some("desc") => Ok(true),
        Some(_) => Err(Error::message(
            "entries filter: `sort` must be \"asc\" or \"desc\"",
        )),
    }
}
