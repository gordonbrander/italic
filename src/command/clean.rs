//! `italic clean` — empty the output directory, preserving anything matched by
//! the site's `keep_files` globs.
//!
//! The directory itself is never removed, only its contents. That matters for the
//! deploy-branch-as-worktree pattern, where `output_dir` *is* a git worktree: the
//! default `keep_files` of `[".git"]` preserves the worktree's `.git`, and leaving
//! the directory in place keeps the worktree registered. `italic build` only writes
//! over the top and never removes, so `clean` + `build` is the way to get a tree
//! with no stale output from deleted docs.

use crate::config::Config;
use anyhow::{Context, Result};
use globset::{GlobSet, GlobSetBuilder};
use std::fs;
use std::path::Path;

pub fn run() -> Result<()> {
    let (config, _) = Config::load_with_theme(Path::new("config.yaml"))?;
    clean(&config)
}

/// Remove everything under `config.output_dir` that no `keep_files` glob matches.
/// A missing output dir is a no-op, so cleaning twice is harmless.
pub fn clean(config: &Config) -> Result<()> {
    if !config.output_dir.exists() {
        return Ok(());
    }
    let mut builder = GlobSetBuilder::new();
    for glob in &config.keep_files {
        builder.add(glob.clone());
    }
    let keep = builder
        .build()
        .context("building the `keep_files` glob set")?;

    let report = clean_dir(&config.output_dir, &config.output_dir, &keep)?;
    if report.kept > 0 {
        eprintln!(
            "cleaned {} ({} removed, {} kept)",
            config.output_dir.display(),
            report.removed,
            report.kept
        );
    } else {
        eprintln!(
            "cleaned {} ({} removed)",
            config.output_dir.display(),
            report.removed
        );
    }
    Ok(())
}

/// Tally of what [`clean_dir`] did, so [`clean`] can report it.
#[derive(Default)]
struct CleanReport {
    removed: usize,
    /// Entries a `keep_files` glob matched. Non-zero anywhere in a subtree means
    /// the enclosing directories must survive to hold them.
    kept: usize,
}

/// Recursively empty `dir`, keeping entries whose path relative to `base` matches
/// `keep`. A directory that matches is kept whole and never descended into; one
/// that doesn't is descended into and then removed if nothing inside survived.
fn clean_dir(dir: &Path, base: &Path, keep: &GlobSet) -> Result<CleanReport> {
    let mut report = CleanReport::default();
    let entries = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("reading an entry in {}", dir.display()))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(base)
            .expect("read_dir entries are always under the base we started from");
        if keep.is_match(rel) {
            report.kept += 1;
            continue;
        }
        // `file_type` does not follow symlinks, so a symlink to a directory is
        // unlinked rather than recursed into.
        let file_type = entry
            .file_type()
            .with_context(|| format!("reading the file type of {}", path.display()))?;
        if file_type.is_dir() {
            let inner = clean_dir(&path, base, keep)?;
            report.removed += inner.removed;
            report.kept += inner.kept;
            if inner.kept == 0 {
                fs::remove_dir(&path).with_context(|| format!("removing {}", path.display()))?;
                report.removed += 1;
            }
        } else {
            fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
            report.removed += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};
    use globset::GlobBuilder;
    use std::path::PathBuf;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    /// A config whose output dir is `out`, keeping `patterns`. An empty slice keeps
    /// nothing; `None` uses the real default (`[".git"]`).
    fn config(out: PathBuf, patterns: Option<&[&str]>) -> Config {
        let keep_files = match patterns {
            Some(patterns) => patterns
                .iter()
                .map(|p| GlobBuilder::new(p).literal_separator(true).build().unwrap())
                .collect(),
            None => Config::default().keep_files,
        };
        Config {
            output_dir: out,
            keep_files,
            ..Config::default()
        }
    }

    #[test]
    fn default_keeps_dot_git_and_removes_output() {
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join(".git"), "gitdir: ../.git/worktrees/public");
        write(&out.join("index.html"), "<h1>hi</h1>");
        clean(&config(out.clone(), None)).unwrap();
        assert!(out.join(".git").exists());
        assert!(!out.join("index.html").exists());
        cleanup(&base);
    }

    #[test]
    fn output_dir_itself_survives() {
        // The worktree-critical assertion: removing the dir would unregister it.
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join("index.html"), "x");
        clean(&config(out.clone(), None)).unwrap();
        assert!(out.exists(), "output_dir must remain, even when emptied");
        cleanup(&base);
    }

    #[test]
    fn missing_output_dir_is_a_noop() {
        let base = tempdir("clean");
        clean(&config(base.join("nope"), None)).unwrap();
        cleanup(&base);
    }

    #[test]
    fn emptied_subdirs_are_pruned() {
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join("notes/a.html"), "a");
        write(&out.join("notes/deep/b.html"), "b");
        clean(&config(out.clone(), None)).unwrap();
        assert!(!out.join("notes").exists());
        cleanup(&base);
    }

    #[test]
    fn dir_holding_a_kept_file_is_not_pruned() {
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join("notes/keep.txt"), "keep");
        write(&out.join("notes/drop.html"), "drop");
        clean(&config(out.clone(), Some(&["notes/keep.txt"]))).unwrap();
        assert!(out.join("notes/keep.txt").exists());
        assert!(!out.join("notes/drop.html").exists());
        assert!(out.join("notes").exists());
        cleanup(&base);
    }

    #[test]
    fn empty_keep_files_removes_everything_but_the_dir() {
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join(".git"), "gitdir: elsewhere");
        write(&out.join("notes/a.html"), "a");
        clean(&config(out.clone(), Some(&[]))).unwrap();
        assert!(!out.join(".git").exists());
        assert!(!out.join("notes").exists());
        assert!(out.exists());
        cleanup(&base);
    }

    #[test]
    fn literal_separator_scopes_the_default_to_the_top_level() {
        // `.git` must not match `notes/.git` — `**/.git` is how you ask for that.
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join(".git"), "top");
        write(&out.join("notes/.git"), "nested");
        clean(&config(out.clone(), None)).unwrap();
        assert!(out.join(".git").exists());
        assert!(!out.join("notes/.git").exists());
        assert!(!out.join("notes").exists());
        cleanup(&base);
    }

    #[test]
    fn double_star_matches_nested_subtrees() {
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join("media/img/a.png"), "a");
        write(&out.join("media/b.png"), "b");
        write(&out.join("index.html"), "drop");
        clean(&config(out.clone(), Some(&["media/**"]))).unwrap();
        assert!(out.join("media/img/a.png").exists());
        assert!(out.join("media/b.png").exists());
        assert!(!out.join("index.html").exists());
        cleanup(&base);
    }

    #[test]
    fn matching_dir_is_kept_whole_without_descending() {
        // `.git` matches the directory itself, so its contents survive even though
        // nothing inside would match on its own.
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join(".git/HEAD"), "ref: refs/heads/deploy");
        write(&out.join(".git/objects/ab/cdef"), "obj");
        clean(&config(out.clone(), None)).unwrap();
        assert!(out.join(".git/HEAD").exists());
        assert!(out.join(".git/objects/ab/cdef").exists());
        cleanup(&base);
    }

    #[test]
    fn cleaning_twice_is_harmless() {
        let base = tempdir("clean");
        let out = base.join("public");
        write(&out.join("index.html"), "x");
        let config = config(out.clone(), None);
        clean(&config).unwrap();
        clean(&config).unwrap();
        assert!(out.exists());
        cleanup(&base);
    }
}
