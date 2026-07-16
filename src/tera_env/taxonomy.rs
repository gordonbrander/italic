//! Tera adapter for taxonomies. `taxonomy(name="tags")` returns the whole
//! taxonomy as an object keyed by term (slug) → list of docs, so templates can
//! iterate `{% for term, docs in taxonomy(name="tags") %}` or index a single
//! term `taxonomy(name="tags")["rust"]`. An unknown name returns an empty
//! object. Only source docs are classified, so generated archive pages never
//! appear here.

use crate::doc::Doc;
use crate::doc_index::DocIndex;
use std::collections::BTreeMap;
use std::sync::Arc;
use tera::{Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera, index: Arc<DocIndex>) {
    env.register_function(
        "taxonomy",
        move |kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            let name = kwargs.must_get::<&str>("name")?;
            // Resolve each term's cached id list back to docs. Unknown taxonomy
            // name → empty map. A `BTreeMap` keeps term iteration sorted (Tera 2
            // maps preserve insertion order via the `preserve_order` feature).
            let mut out: BTreeMap<String, Vec<&Doc>> = BTreeMap::new();
            if let Some(taxonomy) = index.get_taxonomy(name) {
                for (term, ids) in taxonomy {
                    let docs: Vec<&Doc> = ids.iter().filter_map(|id| index.doc(id)).collect();
                    out.insert(term.clone(), docs);
                }
            }
            Value::try_from_serializable(&out)
        },
    );
}
