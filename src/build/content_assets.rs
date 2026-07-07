use crate::config::Config;
use crate::doc_index::DocIndex;
use anyhow::{Context, Result};
use std::fs;

/// Mirror every co-located media file ([`DocIndex::assets`]) from `content_dir`
/// into `output_dir`, preserving its `content/`-relative subpath
/// (`content/blog/x.png` → `<output>/blog/x.png`). This is what lets a note
/// reference an image sitting next to it.
///
/// Runs after [`write`](crate::build::write) and *after*
/// [`static_copy`](crate::build::static_copy), so a co-located file wins on a
/// path collision — the most-local layer overrides the more general ones
/// (colocated > site `static/` > theme `static/`).
pub fn run(config: &Config, index: &DocIndex) -> Result<()> {
    for rel in index.assets() {
        let src = config.content_dir.join(rel);
        let dest = config.output_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(&src, &dest)
            .with_context(|| format!("copying {} -> {}", src.display(), dest.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cleanup, tempdir};
    use std::path::Path;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn mirrors_assets_preserving_subpath() {
        let base = tempdir("content_assets");
        let content = base.join("content");
        let out = base.join("public");
        write(&content.join("blog/diagram.png"), "PNG");
        write(&content.join("logo.svg"), "SVG");
        let mut index = DocIndex::new();
        index.add_asset(std::path::PathBuf::from("blog/diagram.png"));
        index.add_asset(std::path::PathBuf::from("logo.svg"));
        let config = Config {
            content_dir: content,
            output_dir: out.clone(),
            ..Config::default()
        };
        run(&config, &index).unwrap();
        assert_eq!(
            fs::read_to_string(out.join("blog/diagram.png")).unwrap(),
            "PNG"
        );
        assert_eq!(fs::read_to_string(out.join("logo.svg")).unwrap(), "SVG");
        cleanup(&base);
    }

    #[test]
    fn colocated_overlays_static() {
        // static_copy runs first, so a file already sits at the dest path;
        // the co-located asset must overwrite it (colocated > static).
        let base = tempdir("content_assets");
        let content = base.join("content");
        let out = base.join("public");
        write(&content.join("blog/diagram.png"), "COLOCATED");
        // Simulate static_copy having already landed a file at the same path.
        write(&out.join("blog/diagram.png"), "STATIC");
        let mut index = DocIndex::new();
        index.add_asset(std::path::PathBuf::from("blog/diagram.png"));
        let config = Config {
            content_dir: content,
            output_dir: out.clone(),
            ..Config::default()
        };
        run(&config, &index).unwrap();
        assert_eq!(
            fs::read_to_string(out.join("blog/diagram.png")).unwrap(),
            "COLOCATED"
        );
        cleanup(&base);
    }

    #[test]
    fn no_assets_is_a_noop() {
        let base = tempdir("content_assets");
        let config = Config {
            content_dir: base.join("content"),
            output_dir: base.join("public"),
            ..Config::default()
        };
        run(&config, &DocIndex::new()).unwrap();
        cleanup(&base);
    }
}
