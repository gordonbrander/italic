//! The `pubstatus` verb: load config + state, then read back the ATProto records
//! `italic publish` wrote and confirm they still match. The networked, read-only
//! half lives in [`crate::publish::pubstatus`]; this is the thin CLI seam.
//!
//! Unlike `publish`, `pubstatus` does **not** build the site — it only needs the
//! `publish:` config (for credentials/host) and the sidecar state file, so it
//! still works while your content is mid-edit.

use crate::config::Config;
use crate::publish::pubstatus;
use anyhow::Result;
use std::path::Path;

pub fn run() -> Result<()> {
    let (config, _theme) = Config::load_with_theme(Path::new("config.yaml"))?;
    pubstatus::run(&config)
}
