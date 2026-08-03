//! Guards the agent skill under `.agents/skills/italic/`: every routed file
//! exists, every reference file is routed from SKILL.md, relative links and
//! anchors resolve, paths printed from `src/` stay valid, and the error
//! strings quoted in troubleshooting.md stay verbatim with the binary.
//!
//! The `.agents/` tree is excluded from the published crate, so every test
//! no-ops when it is absent.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn skill_dir() -> PathBuf {
    repo_root().join(".agents/skills/italic")
}

fn references_dir() -> PathBuf {
    skill_dir().join("references")
}

/// All `.md` files under `dir`, recursively.
fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Markdown source with fenced code blocks and inline code spans removed, so
/// example links and `#` comments inside code don't get parsed as real ones.
fn strip_code(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_fence = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push('\n');
            continue;
        }
        // Drop inline code spans.
        let mut in_span = false;
        for c in line.chars() {
            if c == '`' {
                in_span = !in_span;
            } else if !in_span {
                out.push(c);
            }
        }
        out.push('\n');
    }
    out
}

/// Every `](target)` link target in (code-stripped) markdown.
fn link_targets(stripped: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = stripped.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b']' && bytes[i + 1] == b'(' {
            if let Some(end) = stripped[i + 2..].find(')') {
                let raw = &stripped[i + 2..i + 2 + end];
                // Drop an optional link title: `](path "title")`.
                let target = raw.split_whitespace().next().unwrap_or("");
                if !target.is_empty() {
                    out.push(target.to_string());
                }
                i += 2 + end;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// GitHub-style heading anchor slugs for a markdown file, with `-N` suffixes
/// for duplicates.
fn heading_slugs(source: &str) -> BTreeSet<String> {
    fn slugify(heading: &str) -> String {
        heading
            .trim()
            .chars()
            .filter_map(|c| {
                if c.is_alphanumeric() {
                    Some(c.to_lowercase().next().unwrap_or(c))
                } else if c == ' ' {
                    Some('-')
                } else if c == '-' || c == '_' {
                    Some(c)
                } else {
                    None
                }
            })
            .collect()
    }
    let mut seen: Vec<String> = Vec::new();
    let mut out = BTreeSet::new();
    let mut in_fence = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || !trimmed.starts_with('#') {
            continue;
        }
        let text = trimmed.trim_start_matches('#').trim();
        let base = slugify(&text.replace('`', ""));
        let count = seen.iter().filter(|s| **s == base).count();
        seen.push(base.clone());
        if count == 0 {
            out.insert(base);
        } else {
            out.insert(format!("{base}-{count}"));
        }
    }
    out
}

fn is_external(target: &str) -> bool {
    target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with('#')
}

/// Check every relative link (and `#anchor` into a `.md` target) in `file`.
fn check_links(file: &Path, failures: &mut Vec<String>) {
    let source = fs::read_to_string(file).unwrap();
    let stripped = strip_code(&source);
    let base = file.parent().unwrap();
    for target in link_targets(&stripped) {
        if is_external(&target) {
            continue;
        }
        let (path_part, anchor) = match target.split_once('#') {
            Some((p, a)) => (p, Some(a.to_string())),
            None => (target.as_str(), None),
        };
        if path_part.is_empty() {
            continue;
        }
        let resolved = base.join(path_part);
        if !resolved.exists() {
            failures.push(format!(
                "{}: broken link `{}` (resolved: {})",
                file.display(),
                target,
                resolved.display()
            ));
            continue;
        }
        if let Some(anchor) = anchor {
            if resolved.extension().is_some_and(|e| e == "md") {
                let target_source = fs::read_to_string(&resolved).unwrap();
                if !heading_slugs(&target_source).contains(&anchor) {
                    failures.push(format!(
                        "{}: link `{}` — no heading `#{}` in {}",
                        file.display(),
                        target,
                        anchor,
                        resolved.display()
                    ));
                }
            }
        }
    }
}

#[test]
fn skill_links_resolve() {
    if !skill_dir().exists() {
        return;
    }
    let mut failures = Vec::new();
    for file in md_files(&skill_dir()) {
        check_links(&file, &mut failures);
    }
    for name in ["README.md", "AGENTS.md"] {
        let file = repo_root().join(name);
        if file.exists() {
            check_links(&file, &mut failures);
        }
    }
    assert!(failures.is_empty(), "broken links:\n{}", failures.join("\n"));
}

/// SKILL.md's routing table is the complete directory listing: remote agents
/// using the raw-URL fallback cannot `ls`, so every site-builder reference
/// file must be named in SKILL.md, and every named path must exist.
#[test]
fn router_is_complete() {
    if !skill_dir().exists() {
        return;
    }
    let skill = fs::read_to_string(skill_dir().join("SKILL.md")).unwrap();

    // Every `references/...md` token in SKILL.md.
    let mut routed = BTreeSet::new();
    let mut rest = skill.as_str();
    while let Some(start) = rest.find("references/") {
        let tail = &rest[start..];
        let token: String = tail
            .chars()
            .take_while(|c| !c.is_whitespace() && !"()`|,".contains(*c))
            .collect();
        if token.ends_with(".md") {
            routed.insert(token.clone());
        }
        rest = &rest[start + "references/".len()..];
    }

    let mut failures = Vec::new();
    for path in &routed {
        if !skill_dir().join(path).exists() {
            failures.push(format!("SKILL.md routes to `{path}`, which does not exist"));
        }
    }
    for file in md_files(&references_dir()) {
        let rel = file.strip_prefix(&skill_dir()).unwrap();
        let rel = rel.to_str().unwrap();
        if rel.starts_with("references/dev/") {
            continue; // covered by SKILL.md's blanket "skip dev/" note
        }
        if !routed.contains(rel) {
            failures.push(format!(
                "`{rel}` is not routed from SKILL.md — remote agents can't discover it"
            ));
        }
    }
    assert!(failures.is_empty(), "router gaps:\n{}", failures.join("\n"));
}

#[test]
fn fallback_url_matches_layout() {
    if !skill_dir().exists() {
        return;
    }
    let skill = fs::read_to_string(skill_dir().join("SKILL.md")).unwrap();
    let expected =
        "https://raw.githubusercontent.com/gordonbrander/italic/main/.agents/skills/italic/references/";
    assert!(
        skill.contains(expected),
        "SKILL.md's raw fallback URL must be exactly `{expected}`"
    );
}

/// Any `.agents/skills/italic/references/...` path mentioned in `src/` (for
/// example the URL `atproto status` prints at runtime) must exist on disk.
#[test]
fn src_doc_paths_exist() {
    if !skill_dir().exists() {
        return;
    }
    let needle = ".agents/skills/italic/references/";
    let mut failures = Vec::new();
    for file in rs_files(&repo_root().join("src")) {
        let source = fs::read_to_string(&file).unwrap();
        let mut rest = source.as_str();
        while let Some(start) = rest.find(needle) {
            let tail = &rest[start..];
            let token: String = tail
                .chars()
                .take_while(|c| !c.is_whitespace() && !"\")`>,]".contains(*c))
                .collect();
            let path = repo_root().join(token.trim_end_matches(['.', ':']));
            if !path.exists() {
                failures.push(format!("{}: stale doc path `{token}`", file.display()));
            }
            rest = &rest[start + needle.len()..];
        }
    }
    assert!(failures.is_empty(), "stale src paths:\n{}", failures.join("\n"));
}

fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The pre-flattening bucket paths must not resurface anywhere.
#[test]
fn no_stale_bucket_paths() {
    if !skill_dir().exists() {
        return;
    }
    let stale = [
        "references/guides/",
        "references/reference/",
        "references/concepts/",
        "references/getting-started/",
        "references/index.md",
    ];
    let mut files = md_files(&skill_dir());
    files.extend(rs_files(&repo_root().join("src")));
    for name in ["README.md", "AGENTS.md"] {
        let f = repo_root().join(name);
        if f.exists() {
            files.push(f);
        }
    }
    let mut failures = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).unwrap();
        for s in stale {
            if source.contains(s) {
                failures.push(format!("{}: mentions stale path `{s}`", file.display()));
            }
        }
    }
    assert!(failures.is_empty(), "stale paths:\n{}", failures.join("\n"));
}

/// Every fenced `text` line in troubleshooting.md must contain a fragment
/// that appears verbatim in `src/` — the anti-drift contract that keeps
/// grep-your-stderr working.
#[test]
fn troubleshooting_error_strings_are_verbatim() {
    if !skill_dir().exists() {
        return;
    }
    // Corpus: all of src/, with Rust string-continuation (`\` + newline +
    // indent) joined and whitespace collapsed, so wrapped string literals
    // match their runtime form.
    let mut corpus = String::new();
    for file in rs_files(&repo_root().join("src")) {
        let source = fs::read_to_string(&file).unwrap();
        let mut joined = String::with_capacity(source.len());
        for line in source.lines() {
            let t = line.trim_end();
            if let Some(stripped) = t.strip_suffix('\\') {
                joined.push_str(stripped);
            } else {
                joined.push_str(t.trim_start());
                joined.push(' ');
            }
        }
        corpus.push_str(&joined.split_whitespace().collect::<Vec<_>>().join(" "));
        corpus.push(' ');
    }

    let source = fs::read_to_string(references_troubleshooting()).unwrap();
    let mut failures = Vec::new();
    let mut in_text_fence = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_text_fence = trimmed == "```text";
            continue;
        }
        if !in_text_fence || trimmed.is_empty() {
            continue;
        }
        // Split around `<placeholder>` tokens (concrete in the docs,
        // interpolated in source), then look for any contiguous word window
        // of >= 16 chars that greps against the corpus — proof the wording
        // is live even when an adjacent word is a source-side interpolation.
        let hit = split_placeholders(trimmed).iter().any(|piece| {
            let words: Vec<&str> = piece.split_whitespace().collect();
            (0..words.len()).any(|i| {
                (i..words.len()).any(|j| {
                    let window = words[i..=j].join(" ");
                    window.len() >= 16 && corpus.contains(window.as_str())
                })
            })
        });
        if !hit {
            failures.push(format!(
                "troubleshooting.md quotes an error not found in src/: `{trimmed}`"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "error-string drift:\n{}",
        failures.join("\n")
    );
}

fn references_troubleshooting() -> PathBuf {
    references_dir().join("troubleshooting.md")
}

/// Split a doc line on `<placeholder>` tokens.
fn split_placeholders(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            // Consume through the matching `>` when it looks like a short
            // placeholder; otherwise keep the literal `<`.
            let ahead: String = chars.clone().take_while(|c| *c != '>').collect();
            if ahead.len() <= 20 && chars.clone().any(|c| c == '>') {
                for _ in 0..=ahead.len() {
                    chars.next();
                }
                out.push(std::mem::take(&mut current));
                continue;
            }
        }
        current.push(c);
    }
    out.push(current);
    out
}

/// Warn (never fail) when a reference file outgrows the size budget.
#[test]
fn size_budget_warnings() {
    if !skill_dir().exists() {
        return;
    }
    let exceptions = ["troubleshooting.md", "frontmatter.md", "dev/markup.md"];
    for file in md_files(&references_dir()) {
        let rel = file
            .strip_prefix(&references_dir())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        if exceptions.contains(&rel.as_str()) {
            continue;
        }
        let lines = fs::read_to_string(&file).unwrap().lines().count();
        if lines > 150 {
            eprintln!("size budget: references/{rel} is {lines} lines (target <= 150)");
        }
    }
}
