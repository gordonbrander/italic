//! `dirtree` — fold an array of docs into a nested directory tree, registered
//! on both envs.
//!
//! Tera can iterate a flat `collection(...)` array, but there's no built-in way
//! to render docs as a hierarchy (sitemap, archive index, file-browser nav).
//! `docs | dirtree` groups docs by their `output_path` (the published URL
//! layout) and returns the content root's children as an array of nodes. Every
//! directory's `children` is the same kind of array, so one recursive component
//! walks the whole tree:
//!
//! ```html
//! {% component tree(nodes) %}
//! <ul>
//!   {% for n in nodes %}
//!     {% if n.kind == "dir" %}
//!       <li>{{ n.name }}{{<tree nodes={n.children} />}}</li>
//!     {% else %}
//!       <li><a href="{{ n.doc.id_path | link }}">{{ n.doc.title }}</a></li>
//!     {% endif %}
//!   {% endfor %}
//! </ul>
//! {% endcomponent tree %}
//!
//! {{<tree nodes={docs | dirtree} />}}
//! ```
//!
//! Each node is one of:
//! - `{kind: "dir",  name, path, children: [node...]}`
//! - `{kind: "file", name, path, doc: {...}}`
//!
//! where `name` is the single path segment at this level, `path` is the
//! accumulated output path from the root, and `doc` is the untouched doc value.
//! Children are sorted by `name` ascending (dirs and files interleaved).

use std::collections::BTreeMap;
use tera::{Error, Kwargs, State, Tera, TeraResult, Value};

pub fn register(env: &mut Tera) {
    env.register_filter(
        "dirtree",
        |docs: &[Value], _: Kwargs, _: &State| -> TeraResult<Value> { build_tree(docs) },
    );
}

/// Intermediate tree node. A `BTreeMap` keyed by path segment keeps siblings
/// sorted by name (ascending) and merges docs that share a parent directory.
enum Node {
    Dir(BTreeMap<String, Node>),
    File(Value),
}

/// Fold the doc values into a tree and emit the content root's children as an
/// array value. Kept free of Tera filter plumbing so it can be unit-tested
/// directly.
fn build_tree(docs: &[Value]) -> TeraResult<Value> {
    let mut root: BTreeMap<String, Node> = BTreeMap::new();

    'docs: for doc in docs {
        let output_path = doc
            .get_from_path("output_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Error::message("dirtree filter: every doc must have a string `output_path`")
            })?;

        // `PathBuf` serializes with `/` separators on this project's platforms;
        // splitting the string keeps us in the URL layout the user asked for.
        let segments: Vec<&str> = output_path.split('/').filter(|s| !s.is_empty()).collect();
        let Some((file_name, dirs)) = segments.split_last() else {
            // Empty / all-separator output path: nothing to place in the tree.
            continue;
        };

        // Descend (creating dirs as needed) to the doc's parent directory.
        let mut cursor = &mut root;
        for dir in dirs {
            let entry = cursor
                .entry((*dir).to_string())
                .or_insert_with(|| Node::Dir(BTreeMap::new()));
            match entry {
                Node::Dir(children) => cursor = children,
                // A file already claimed this name. Output paths are expected to
                // be unique, so this is degenerate; leave the file in place and
                // skip the colliding doc rather than clobbering it.
                Node::File(_) => continue 'docs,
            }
        }

        // Insert the doc as a leaf, unless a dir already owns the name.
        cursor
            .entry((*file_name).to_string())
            .or_insert_with(|| Node::File(doc.clone()));
    }

    Ok(to_value(&root, ""))
}

/// Recursively convert the intermediate tree into an array value of node
/// objects, threading the accumulated output-path `prefix` so each node carries
/// its full `path`.
fn to_value(nodes: &BTreeMap<String, Node>, prefix: &str) -> Value {
    let array: Vec<Value> = nodes
        .iter()
        .map(|(name, node)| {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };
            let mut obj: BTreeMap<&str, Value> = BTreeMap::new();
            obj.insert("name", Value::from(name.clone()));
            obj.insert("path", Value::from(path.clone()));
            match node {
                Node::Dir(children) => {
                    obj.insert("kind", Value::from("dir"));
                    obj.insert("children", to_value(children, &path));
                }
                Node::File(doc) => {
                    obj.insert("kind", Value::from("file"));
                    obj.insert("doc", doc.clone());
                }
            }
            Value::from(obj)
        })
        .collect();
    Value::from(array)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A doc value with just the fields `dirtree` reads/echoes, serialized to a
    /// map value the same shape a real `Doc` produces.
    fn doc(output_path: &str, title: &str) -> Value {
        let map: BTreeMap<&str, &str> = [("output_path", output_path), ("title", title)]
            .into_iter()
            .collect();
        Value::from_serializable(&map)
    }

    /// `node.<field>` as a string, for assertions.
    fn s<'a>(node: &'a Value, field: &'a str) -> &'a str {
        node.get_from_path(field).and_then(Value::as_str).unwrap()
    }

    fn children(node: &Value) -> &[Value] {
        node.get_from_path("children")
            .and_then(Value::as_array)
            .unwrap()
    }

    #[test]
    fn flat_files_sorted_ascending() {
        let tree = build_tree(&[doc("index.html", "Home"), doc("about.html", "About")]).unwrap();
        let top = tree.as_array().unwrap();
        assert_eq!(top.len(), 2);
        // Sorted by name: `about.html` before `index.html`.
        assert_eq!(s(&top[0], "kind"), "file");
        assert_eq!(s(&top[0], "name"), "about.html");
        assert_eq!(s(&top[0], "path"), "about.html");
        assert_eq!(s(&top[0], "doc.title"), "About");
        assert_eq!(s(&top[1], "name"), "index.html");
    }

    #[test]
    fn nests_dirs_and_accumulates_path() {
        let tree = build_tree(&[
            doc("posts/b/index.html", "B"),
            doc("posts/a/index.html", "A"),
            doc("about.html", "About"),
        ])
        .unwrap();

        let top = tree.as_array().unwrap();
        // Top level: `about.html` (file) before `posts` (dir), sorted by name.
        assert_eq!(s(&top[0], "kind"), "file");
        assert_eq!(s(&top[0], "name"), "about.html");
        assert_eq!(s(&top[1], "kind"), "dir");
        assert_eq!(s(&top[1], "name"), "posts");
        assert_eq!(s(&top[1], "path"), "posts");

        // `posts` has child dirs `a` and `b`, sorted ascending, paths accumulated.
        let posts = children(&top[1]);
        assert_eq!(posts.len(), 2);
        assert_eq!(s(&posts[0], "name"), "a");
        assert_eq!(s(&posts[0], "path"), "posts/a");
        assert_eq!(s(&posts[1], "name"), "b");
        assert_eq!(s(&posts[1], "path"), "posts/b");

        // Leaf file under `posts/a` carries its doc and full path.
        let a_children = children(&posts[0]);
        assert_eq!(s(&a_children[0], "kind"), "file");
        assert_eq!(s(&a_children[0], "path"), "posts/a/index.html");
        assert_eq!(s(&a_children[0], "doc.title"), "A");
    }

    #[test]
    fn non_array_input_is_an_error() {
        let mut env = Tera::default();
        register(&mut env);
        let mut ctx = tera::Context::new();
        ctx.insert("x", "not an array");
        assert!(env.render_str("{{ x | dirtree }}", &ctx, false).is_err());
    }

    #[test]
    fn missing_output_path_is_an_error() {
        let no_path = Value::from_serializable(&BTreeMap::from([("title", "No path")]));
        assert!(build_tree(&[no_path]).is_err());
    }
}
