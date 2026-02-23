//! Vox Forge — Desktop application.
//!
//! Entry point and wiring layer. Parses CLI args, loads config,
//! initializes logging, and starts the application.

#![warn(clippy::pedantic)]

mod cli;
mod config;
mod error;

use anyhow::Result;
use clap::Parser;
use tracing::info;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // Initialize tracing
    let log_level = match cli.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .init();

    // Load configuration
    let config = config::Config::load()?;

    info!("{} starting...", config.app_name);

    // TODO: Initialize providers, start application

    info!("{} initialized successfully", config.app_name);

    Ok(())
}
