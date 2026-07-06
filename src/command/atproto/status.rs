//! The `atproto status` verb: load config + state, then read back the ATProto
//! records `italic atproto publish` wrote and confirm they still match. The
//! networked, read-only half lives in [`crate::atproto::status`]; this is the
//! thin CLI seam.
//!
//! Unlike `atproto publish`, `atproto status` does **not** build the site — it
//! only needs the `atproto:` config (for credentials/host) and the sidecar
//! state file, so it still works while your content is mid-edit.

use crate::atproto::status;
use crate::config::Config;
use anyhow::Result;
use std::path::Path;

pub fn run() -> Result<()> {
    let (config, _theme) = Config::load_with_theme(Path::new("config.yaml"))?;
    status::run(&config)
}
