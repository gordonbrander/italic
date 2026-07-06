//! The `publish` verb: build the index (no HTML written), then sync records to
//! the user's ATProto PDS. The networked, stateful half lives in
//! [`crate::publish`]; this is the thin CLI seam that reuses the build pipeline.

use crate::publish::{self, Options};
use anyhow::{Context, Result};

/// Run a publish. Drafts are never published (we build with `include_drafts =
/// false`), so they stay out of the PDS just as they stay out of `italic build`.
pub fn run(options: Options) -> Result<()> {
    let (config, site_data, index) =
        crate::build::build_index(false).context("building site before publish")?;
    publish::run(&config, &site_data, &index, options)
}
