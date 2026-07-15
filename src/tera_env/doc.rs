//! Tera adapter for single-doc lookup. `doc(id_path="posts/hello.md")` returns
//! the full doc with that `id_path` from the frozen [`DocIndex`] shared across
//! the template phase. An unknown `id_path` returns none (no error), so a
//! template can guard with `{% if doc(id_path=...) %}` rather than failing the
//! build — mirroring `collection`'s lenient handling of an unknown name.

use crate::doc_index::DocIndex;
use std::path::Path;
use std::sync::Arc;
use tera::{Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "doc",
        move |kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            // A missing or non-string `id_path` is an author error (unlike an
            // unknown-but-well-formed path, which yields none).
            let id_path = kwargs.must_get::<&str>("id_path")?;
            match index.doc(Path::new(id_path)) {
                Some(doc) => Value::try_from_serializable(doc),
                None => Ok(Value::none()),
            }
        },
    );
}
