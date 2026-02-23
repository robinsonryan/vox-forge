//! CLI argument parsing.

use clap::Parser;

/// Vox Forge — Desktop application.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    /// Enable verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}
