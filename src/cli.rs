//! CLI argument parsing.
//!
//! Uses clap derive to define the full command structure for `voxforge`.

use clap::{Parser, Subcommand};

/// Vox Forge -- Cross-platform voice dictation.
#[derive(Parser, Debug)]
#[command(name = "voxforge", version, about = "Cross-platform voice dictation")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Increase logging verbosity (-v = debug, -vv = trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Top-level commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the daemon (headless, listens for hotkey)
    Daemon {
        /// Run in the background (detached)
        #[arg(long)]
        background: bool,
    },

    /// Start with system tray (default desktop mode)
    Tray,

    /// Open settings UI
    Settings {
        /// Jump to a specific settings tab
        #[arg(long)]
        tab: Option<String>,
    },

    /// Send toggle signal to running daemon
    Toggle,

    /// Send cancel signal to running daemon
    Cancel,

    /// Stop the daemon
    Stop,

    /// Show daemon status
    Status,

    /// Recalibrate microphone silence threshold on the running daemon
    Recalibrate,

    /// Tell the running daemon to reload its configuration from disk
    ReloadConfig,

    /// Record, transcribe, format, and print to stdout (one-shot)
    Dictate {
        /// Formatting mode (auto, code, email, chat, prose)
        #[arg(long, default_value = "auto")]
        mode: String,

        /// Recording timeout in seconds
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Dictionary management
    Dict {
        #[command(subcommand)]
        action: DictAction,
    },

    /// Log a correction (maps original -> corrected)
    Correct {
        /// The incorrect transcription
        original: String,

        /// The correct text
        corrected: String,
    },

    /// Correction history
    Corrections {
        #[command(subcommand)]
        action: CorrectionAction,
    },

    /// Model management
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },

    /// List audio input devices
    Devices,

    /// Run diagnostics
    Test {
        #[command(subcommand)]
        action: TestAction,
    },

    /// Provider management
    Provider {
        #[command(subcommand)]
        action: ProviderAction,
    },

    /// API key management
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
}

// ─── Config sub-commands ─────────────────────────────────────────────

/// Configuration sub-commands.
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show the current configuration
    Show,

    /// Open the config file in $EDITOR
    Edit,

    /// Print the config file path
    Path,

    /// Set a configuration value (dot-separated key)
    Set {
        /// Configuration key (e.g., `audio.silence_threshold_db`)
        key: String,

        /// Value to set
        value: String,
    },

    /// Create default config if none exists
    Init,
}

// ─── Dictionary sub-commands ─────────────────────────────────────────

/// Dictionary sub-commands.
#[derive(Subcommand, Debug)]
pub enum DictAction {
    /// List all custom dictionary terms
    List,

    /// Add a term to the custom dictionary
    Add {
        /// The term to add
        term: String,
    },

    /// Remove a term from the custom dictionary
    Remove {
        /// The term to remove
        term: String,
    },
}

// ─── Correction sub-commands ─────────────────────────────────────────

/// Correction history sub-commands.
#[derive(Subcommand, Debug)]
pub enum CorrectionAction {
    /// List recent corrections
    List,

    /// Clear all corrections
    Clear,
}

// ─── Model sub-commands ──────────────────────────────────────────────

/// Model management sub-commands.
#[derive(Subcommand, Debug)]
pub enum ModelAction {
    /// Download a Whisper model
    Download {
        /// Model name (tiny, base, small, medium, large)
        model: String,
    },

    /// List available / downloaded models
    List,

    /// Show info about a specific model
    Info {
        /// Model name
        model: String,
    },
}

// ─── Test sub-commands ───────────────────────────────────────────────

/// Diagnostic test sub-commands.
#[derive(Subcommand, Debug)]
pub enum TestAction {
    /// Test microphone input
    Mic,

    /// Test hotkey registration
    Hotkey,

    /// Test text output (typing simulation)
    Type,

    /// Test active window / context detection
    Context,

    /// Test formatting with a sample phrase
    Format {
        /// Optional sample text to format
        #[arg(default_value = "hello world this is a test")]
        text: String,
    },
}

// ─── Provider sub-commands ───────────────────────────────────────────

/// Provider management sub-commands.
#[derive(Subcommand, Debug)]
pub enum ProviderAction {
    /// List available providers
    List,

    /// Set the active STT (speech-to-text) provider
    SetStt {
        /// Provider name (`whisper_local`, `openai_whisper`)
        provider: String,
    },

    /// Set the active LLM (formatting) provider
    SetLlm {
        /// Provider name (anthropic, openai)
        provider: String,
    },

    /// Test provider connectivity
    Test {
        /// Provider name to test (or "all")
        #[arg(default_value = "all")]
        provider: String,
    },
}

// ─── Auth sub-commands ───────────────────────────────────────────────

/// API key management sub-commands.
#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Set an API key for a provider
    Set {
        /// Provider name (anthropic, openai)
        provider: String,

        /// API key value (omit to read from stdin)
        #[arg(long)]
        key: Option<String>,
    },

    /// Verify an API key works
    Verify {
        /// Provider name (anthropic, openai)
        provider: String,
    },

    /// Clear an API key
    Clear {
        /// Provider name (anthropic, openai)
        provider: String,
    },
}
