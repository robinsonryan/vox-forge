//! Shared error types for the application.

use thiserror::Error;

/// Application-level error type.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience type alias for Results using our Error.
pub type Result<T> = std::result::Result<T, Error>;
