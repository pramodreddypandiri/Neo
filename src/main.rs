/// main.rs
///
/// Neo CLI entry point.
///
/// Parses command-line arguments and dispatches to the appropriate
/// command implementation in the cli/ module.
///
/// Usage:
///   neo init                          # full scan, generate neo.md
///   neo init --agent claude           # also write CLAUDE.md instructions
///   neo update src/auth.ts src/api.ts # incremental update for changed files
///   neo validate                      # check neo.md is in sync

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod types;
mod core;
mod parser;
mod ai;
mod agent;
mod cli;

pub use types::*;

/// Neo — AI-native codebase map for coding agents
#[derive(Parser)]
#[command(name = "neo")]
#[command(version = "0.1.0")]
#[command(about = "AI-native codebase map for coding agents")]
struct Cli {
    /// Project root directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan codebase and generate neo.md from scratch
    Init {
        /// Which agent instruction files to generate: claude, all
        #[arg(long, default_value = "all")]
        agent: String,
    },

    /// Update neo.md for specific changed files
    /// Example: neo update src/auth/token.ts src/api/user.ts
    Update {
        /// Relative file paths that changed
        #[arg(required = true)]
        files: Vec<String>,
    },

    /// Validate that neo.md is in sync with the codebase (for CI)
    Validate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Resolve project root to absolute path
    let project_root = cli.root.canonicalize().unwrap_or_else(|_| {
        std::env::current_dir().expect("Could not determine current directory")
    });

    match cli.command {
        Commands::Init { agent } => {
            cli::init::run(project_root, agent).await?;
        }
        Commands::Update { files } => {
            cli::update::run(project_root, files).await?;
        }
        Commands::Validate => {
            cli::validate::run(project_root)?;
        }
    }

    Ok(())
}
