use anyhow::Result;
use clap::{Parser, Subcommand};
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "italic", about = "A zero-config static site generator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build the site
    Build {
        /// Include draft documents (`draft: true`) in the output
        #[arg(long)]
        drafts: bool,
    },
    /// Watch source dirs and rebuild on change
    Watch,
    /// Serve the built site locally with live reload
    Serve {
        /// Port to bind
        #[arg(long, default_value_t = 3000)]
        port: u16,
        /// Host to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
    },
    /// Publish to and inspect your ATProto PDS
    Atproto {
        #[command(subcommand)]
        command: AtprotoCommand,
    },
    /// Scaffold a starter site at the given path. The path must not already exist.
    New {
        /// Output directory for the scaffolded site
        path: PathBuf,
    },
    /// Copy the configured theme's starter content into this project's content dir
    Scaffold,
    /// Remove the output directory
    Clean,
}

#[derive(Subcommand)]
enum AtprotoCommand {
    /// Publish standard.site document records to your PDS
    Publish {
        /// Build records and show what would change, but make no network calls
        #[arg(long)]
        dry_run: bool,
        /// Skip the confirmation prompt before creating new Bluesky posts
        #[arg(long)]
        yes: bool,
    },
    /// Check the site's expected ATProto records exist on your PDS
    Status,
    /// Resolve a handle (e.g. alice.bsky.social) to its DID, for ITALIC_ATPROTO_DID
    Did {
        /// The handle to resolve
        handle: String,
    },
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load .env if present; ignore if absent
    let cli = Cli::parse();
    match cli.command {
        Command::Build { drafts } => italic::build(drafts),
        Command::Atproto { command } => match command {
            AtprotoCommand::Publish { dry_run, yes } => {
                italic::atproto_publish(italic::atproto::Options { dry_run, yes })
            }
            AtprotoCommand::Status => italic::atproto_status(),
            AtprotoCommand::Did { handle } => italic::atproto_did(&handle),
        },
        Command::Watch => italic::watch(),
        Command::Serve { port, host } => italic::serve(host, port),
        Command::New { path } => italic::new(&path),
        Command::Scaffold => italic::scaffold(),
        Command::Clean => italic::clean(),
    }
}
