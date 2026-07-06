//! The `atproto status` verb: build the site index, then compare the expected
//! records against what the PDS holds. The networked, read-only half lives in
//! [`crate::atproto::status`]; this is the thin CLI seam.
//!
//! Like `atproto publish`, `atproto status` builds the [`DocIndex`]
//! (drafts excluded, no HTML written) — the current content defines what each
//! record *should contain*, and the PDS is the source of truth for what is
//! actually published; differing records report CHANGED. There is no local
//! state file.
//!
//! [`DocIndex`]: crate::doc_index::DocIndex

use crate::atproto::status;
use anyhow::Result;

pub fn run() -> Result<()> {
    let (config, site_data, index) = crate::build::build_index(false)?;
    status::run(&config, &site_data, &index)
}
