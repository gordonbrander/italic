use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "knead", about = "A zero-config static site generator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build the site
    Build,
    /// Watch source dirs and rebuild on change
    Watch,
    /// Scaffold a starter site at the given path. The path must not already exist.
    New {
        /// Output directory for the scaffolded site
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build => knead::build(),
        Command::Watch => knead::watch(),
        Command::New { path } => knead::new(&path),
    }
}
