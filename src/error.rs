//! Shared error types for the application.

use thiserror::Error;

/// Application-level error type.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("Transcription error: {0}")]
    Transcription(String),

    #[error("Formatting error: {0}")]
    Formatting(String),

    #[error("Output error: {0}")]
    Output(String),

    #[error("Hotkey error: {0}")]
    Hotkey(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Dictionary error: {0}")]
    Dictionary(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

/// Convenience type alias for Results using our Error.
pub type Result<T> = std::result::Result<T, Error>;
