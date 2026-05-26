//! Watch mode: rebuild on every change to a source path or `config.yaml`.
//!
//! Per spec §2, v1 is full-rebuild only — `watch` is a debounced loop around
//! [`crate::build`]. Errors raised during a rebuild are logged and the watcher
//! keeps running; only watcher-setup errors propagate out of [`run`].

use crate::config::Config;
use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(150);

pub fn run() -> Result<()> {
    rebuild();

    let (config, _) = Config::load(Path::new("config.yaml"))?;
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(DEBOUNCE, tx)?;

    let dirs = [
        &config.content_dir,
        &config.templates_dir,
        &config.generators_dir,
        &config.data_dir,
        &config.static_dir,
    ];
    for dir in dirs {
        if dir.exists() {
            debouncer.watcher().watch(dir, RecursiveMode::Recursive)?;
        }
    }
    let config_path = Path::new("config.yaml");
    if config_path.exists() {
        debouncer
            .watcher()
            .watch(config_path, RecursiveMode::NonRecursive)?;
    }

    eprintln!("watching for changes…");

    for batch in rx {
        match batch {
            Ok(_) => rebuild(),
            Err(e) => eprintln!("watch error: {e}"),
        }
    }
    Ok(())
}

fn rebuild() {
    let start = Instant::now();
    match crate::build() {
        Ok(()) => eprintln!("rebuilt in {:?}", start.elapsed()),
        Err(e) => eprintln!("build failed: {e:#}"),
    }
}
