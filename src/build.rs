//! The site build pipeline. Stages run in order, each consuming the index
//! the previous stage left behind. `║` marks the embarrassingly parallel
//! (Rayon) stages — each only *reads* the frozen classification and *writes*
//! its own doc(s); `classify` is the sequential barrier that produces that
//! immutable snapshot:
//!
//! 1. [`read`] — scan `content/` into a [`DocIndex`](crate::doc_index::DocIndex).
//! 2. [`markup`] ║ — render markdown bodies through Tera + comrak.
//! 3. [`classify`] — freeze collections + taxonomies from the source docs.
//! 4. [`archive`] ║ — run `archives/` over the frozen classification, append
//!    the emitted view pages (not re-classified).
//! 5. [`template`] ║ — apply each doc's Tera template.
//! 6. [`write`] — write rendered bodies to `output_dir`.
//! 7. [`static_copy`] — copy `static/` over the top.

pub mod archive;
pub mod classify;
pub mod markup;
pub mod read;
pub mod static_copy;
pub mod template;
pub mod write;

use crate::config::Config;
use crate::site_data::SiteData;
use anyhow::Result;
use std::path::Path;

pub fn run() -> Result<()> {
    let (config, site) = Config::load(Path::new("config.yaml"))?;
    let site_data = SiteData::load(&config, site)?;
    let mut index = read::run(&config)?;
    markup::run(&config, &site_data, &mut index)?;
    // Freeze classification from source docs, then share it (by `Arc`) with the
    // archives and template phases so both read the same immutable snapshot.
    let classification = classify::run(&config, &index);
    archive::run(&config, &site_data, &classification, &mut index)?;
    template::run(&config, &site_data, &classification, &mut index)?;
    write::run(&config, &index)?;
    static_copy::run(&config)?;
    Ok(())
}
