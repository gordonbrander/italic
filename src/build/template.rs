use crate::config::Config;
use crate::doc_index::DocIndex;
use crate::site_data::SiteData;
use crate::tera_env::build_template_env;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::sync::Arc;

pub fn run(
    config: &Config,
    site_data: &SiteData,
    classification: &Arc<DocIndex>,
    index: &mut DocIndex,
) -> Result<()> {
    // The env's `collection()`/`taxonomy()`/`backlinks` functions read from the
    // frozen classification (source docs only — archive pages are absent by
    // design). The mutable `index` below is rendered in place.
    let env = build_template_env(config, classification.clone())?;

    // Each doc renders independently against the frozen `env`/`site_data` and
    // writes only its own `content`. `Tera::render` takes `&self`, so the env is
    // shared across Rayon workers by reference (no per-thread clone). Reads of
    // other docs go through the env's snapshot, never the index being mutated.
    index.par_docs_mut().try_for_each(|doc| {
        let Some(template_name) = doc.template.clone() else {
            return Ok(());
        };

        let mut ctx = tera::Context::new();
        ctx.insert("page", &*doc);
        ctx.insert("site", &site_data.site);
        ctx.insert("data", &site_data.data);
        if let Some(pagination) = doc.data.get("pagination") {
            ctx.insert("pagination", pagination);
        }

        doc.content = env.render(&template_name, &ctx).with_context(|| {
            format!(
                "rendering template `{}` for {}",
                template_name,
                doc.id_path.display()
            )
        })?;
        Ok::<(), anyhow::Error>(())
    })
}
