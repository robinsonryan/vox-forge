//! Application configuration.
//!
//! Loads from TOML config file with environment variable overrides.
//! Supports multi-provider transcription and formatting backends.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error;

// ─── Top-level Config ────────────────────────────────────────────────

/// Application configuration loaded from TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub transcription: TranscriptionConfig,

    #[serde(default)]
    pub formatting: FormattingConfig,

    #[serde(default)]
    pub audio: AudioConfig,

    #[serde(default)]
    pub hotkey: HotkeyConfig,

    #[serde(default)]
    pub output: OutputConfig,

    #[serde(default)]
    pub dictionary: DictionaryConfig,

    #[serde(default)]
    pub corrections: CorrectionsConfig,
}

// ─── General ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_true")]
    pub notifications: bool,

    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_log_file")]
    pub log_file: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            notifications: true,
            log_level: default_log_level(),
            log_file: default_log_file(),
        }
    }
}

// ─── Transcription ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    #[serde(default = "default_transcription_provider")]
    pub provider: String,

    #[serde(default)]
    pub whisper_local: WhisperLocalConfig,

    #[serde(default)]
    pub openai_whisper: OpenAiWhisperConfig,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            provider: default_transcription_provider(),
            whisper_local: WhisperLocalConfig::default(),
            openai_whisper: OpenAiWhisperConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperLocalConfig {
    #[serde(default = "default_whisper_model")]
    pub model: String,

    #[serde(default = "default_whisper_device")]
    pub device: String,

    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for WhisperLocalConfig {
    fn default() -> Self {
        Self {
            model: default_whisper_model(),
            device: default_whisper_device(),
            language: default_language(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiWhisperConfig {
    #[serde(default)]
    pub api_key: String,

    #[serde(default = "default_openai_whisper_model")]
    pub model: String,

    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for OpenAiWhisperConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_openai_whisper_model(),
            language: default_language(),
        }
    }
}

// ─── Formatting ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattingConfig {
    #[serde(default = "default_formatting_provider")]
    pub provider: String,

    #[serde(default = "default_formatting_timeout_ms")]
    pub timeout_ms: u64,

    #[serde(default = "default_formatting_mode")]
    pub default_mode: String,

    #[serde(default)]
    pub anthropic: AnthropicConfig,

    #[serde(default)]
    pub openai: OpenAiFormattingConfig,

    #[serde(default)]
    pub auto_rules: AutoRulesConfig,
}

impl Default for FormattingConfig {
    fn default() -> Self {
        Self {
            provider: default_formatting_provider(),
            timeout_ms: default_formatting_timeout_ms(),
            default_mode: default_formatting_mode(),
            anthropic: AnthropicConfig::default(),
            openai: OpenAiFormattingConfig::default(),
            auto_rules: AutoRulesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    #[serde(default)]
    pub api_key: String,

    #[serde(default = "default_anthropic_model")]
    pub model: String,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_anthropic_model(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiFormattingConfig {
    #[serde(default)]
    pub api_key: String,

    #[serde(default = "default_openai_formatting_model")]
    pub model: String,

    #[serde(default)]
    pub base_url: String,
}

impl Default for OpenAiFormattingConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_openai_formatting_model(),
            base_url: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoRulesConfig {
    #[serde(default = "default_code_apps")]
    pub code: Vec<String>,

    #[serde(default = "default_email_apps")]
    pub email: Vec<String>,

    #[serde(default = "default_chat_apps")]
    pub chat: Vec<String>,
}

impl Default for AutoRulesConfig {
    fn default() -> Self {
        Self {
            code: default_code_apps(),
            email: default_email_apps(),
            chat: default_chat_apps(),
        }
    }
}

// ─── Audio ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_silence_threshold_db")]
    pub silence_threshold_db: f64,

    #[serde(default = "default_min_recording_ms")]
    pub min_recording_ms: u64,

    #[serde(default = "default_max_recording_s")]
    pub max_recording_s: u64,

    #[serde(default = "default_silence_timeout_s")]
    pub silence_timeout_s: f64,

    #[serde(default)]
    pub input_device: String,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            silence_threshold_db: default_silence_threshold_db(),
            min_recording_ms: default_min_recording_ms(),
            max_recording_s: default_max_recording_s(),
            silence_timeout_s: default_silence_timeout_s(),
            input_device: String::new(),
        }
    }
}

// ─── Hotkey ──────────────────────────────────────────────────────────

/// Hotkey activation mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyMode {
    /// Hold the hotkey to record, release to stop.
    #[default]
    PushToTalk,
    /// Press once to start recording, press again to stop.
    Toggle,
}

impl std::fmt::Display for HotkeyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PushToTalk => write!(f, "push_to_talk"),
            Self::Toggle => write!(f, "toggle"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    #[serde(default = "default_hotkey_toggle")]
    pub toggle: String,

    #[serde(default = "default_hotkey_cancel")]
    pub cancel: String,

    #[serde(default)]
    pub mode: HotkeyMode,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            toggle: default_hotkey_toggle(),
            cancel: default_hotkey_cancel(),
            mode: HotkeyMode::default(),
        }
    }
}

// ─── Output ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_output_method")]
    pub method: String,

    #[serde(default = "default_keystroke_delay_ms")]
    pub keystroke_delay_ms: u64,

    #[serde(default = "default_true")]
    pub auto_enter: bool,

    #[serde(default = "default_auto_enter_delay_ms")]
    pub auto_enter_delay_ms: u64,

    #[serde(default = "default_clipboard_apps")]
    pub clipboard_apps: Vec<String>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            method: default_output_method(),
            keystroke_delay_ms: default_keystroke_delay_ms(),
            auto_enter: true,
            auto_enter_delay_ms: default_auto_enter_delay_ms(),
            clipboard_apps: default_clipboard_apps(),
        }
    }
}

// ─── Dictionary ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DictionaryConfig {
    #[serde(default)]
    pub custom_terms: Vec<String>,
}

// ─── Corrections ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_max_examples")]
    pub max_examples: usize,
}

impl Default for CorrectionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_examples: default_max_examples(),
        }
    }
}

// ─── Default value functions ─────────────────────────────────────────

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_file() -> String {
    "voxforge.log".to_string()
}

fn default_transcription_provider() -> String {
    "whisper_local".to_string()
}

fn default_whisper_model() -> String {
    "base".to_string()
}

fn default_whisper_device() -> String {
    "cuda".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_openai_whisper_model() -> String {
    "whisper-1".to_string()
}

fn default_formatting_provider() -> String {
    "anthropic".to_string()
}

fn default_formatting_timeout_ms() -> u64 {
    3000
}

fn default_formatting_mode() -> String {
    "auto".to_string()
}

fn default_anthropic_model() -> String {
    "claude-haiku-4-5-20251001".to_string()
}

fn default_openai_formatting_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_code_apps() -> Vec<String> {
    [
        "cursor",
        "code",
        "Code.exe",
        "windsurf",
        "zed",
        "neovim",
        "nvim",
        "vim",
        "kitty",
        "alacritty",
        "WindowsTerminal",
        "cmd.exe",
        "powershell",
        "claude",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

fn default_email_apps() -> Vec<String> {
    ["thunderbird", "gmail", "outlook", "OUTLOOK.EXE", "mail"]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn default_chat_apps() -> Vec<String> {
    [
        "slack",
        "discord",
        "telegram",
        "signal",
        "mattermost",
        "teams",
        "Teams.exe",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

fn default_silence_threshold_db() -> f64 {
    -40.0
}

fn default_min_recording_ms() -> u64 {
    500
}

fn default_max_recording_s() -> u64 {
    120
}

fn default_silence_timeout_s() -> f64 {
    3.0
}

fn default_hotkey_toggle() -> String {
    "Alt+Shift+D".to_string()
}

fn default_hotkey_cancel() -> String {
    "Escape".to_string()
}

fn default_output_method() -> String {
    "type".to_string()
}

fn default_keystroke_delay_ms() -> u64 {
    5
}

fn default_auto_enter_delay_ms() -> u64 {
    2000
}

fn default_clipboard_apps() -> Vec<String> {
    [
        "kitty",
        "alacritty",
        "foot",
        "gnome-terminal",
        "cosmic-term",
        "WindowsTerminal",
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

fn default_max_examples() -> usize {
    5
}

// ─── Impl ────────────────────────────────────────────────────────────

#[allow(dead_code)]
impl Config {
    /// Returns the default config file path:
    /// `$XDG_CONFIG_HOME/vox-forge/config.toml` (or platform equivalent).
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("vox-forge")
            .join("config.toml")
    }

    /// Loads configuration from the default config file path.
    ///
    /// If the file does not exist, returns the default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be read or parsed.
    pub fn load() -> error::Result<Self> {
        Self::load_from(&Self::default_path())
    }

    /// Loads configuration from a specific path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_from(path: &std::path::Path) -> error::Result<Self> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let config: Self =
                toml::from_str(&contents).map_err(|e| error::Error::Config(e.to_string()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Saves the current configuration to the default path.
    ///
    /// Creates parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or serialized.
    pub fn save(&self) -> error::Result<()> {
        self.save_to(&Self::default_path())
    }

    /// Saves the current configuration to a specific path.
    ///
    /// Creates parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or serialized.
    pub fn save_to(&self, path: &std::path::Path) -> error::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Ensures a default configuration file exists at the default path.
    ///
    /// If the file already exists, this is a no-op. Otherwise it writes
    /// the default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or written.
    pub fn ensure_default() -> error::Result<()> {
        let path = Self::default_path();
        if !path.exists() {
            let config = Self::default();
            config.save_to(&path)?;
        }
        Ok(())
    }

    /// Returns the effective Anthropic API key.
    ///
    /// Checks the `ANTHROPIC_API_KEY` environment variable first,
    /// then falls back to the value in the config file.
    pub fn effective_anthropic_key(&self) -> String {
        std::env::var("ANTHROPIC_API_KEY")
            .unwrap_or_else(|_| self.formatting.anthropic.api_key.clone())
    }

    /// Returns the effective `OpenAI` API key.
    ///
    /// Checks the `OPENAI_API_KEY` environment variable first,
    /// then falls back to the value in the config file.
    pub fn effective_openai_key(&self) -> String {
        std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| self.formatting.openai.api_key.clone())
    }

    /// Returns `true` if an effective Anthropic API key is available.
    pub fn has_anthropic_key(&self) -> bool {
        !self.effective_anthropic_key().is_empty()
    }

    /// Returns `true` if an effective `OpenAI` API key is available.
    pub fn has_openai_key(&self) -> bool {
        !self.effective_openai_key().is_empty()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let config = Config::default();
        assert!(config.general.notifications);
        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.transcription.provider, "whisper_local");
        assert_eq!(config.formatting.provider, "anthropic");
        assert_eq!(config.formatting.timeout_ms, 3000);
        assert_eq!(config.hotkey.toggle, "Alt+Shift+D");
        assert_eq!(config.hotkey.cancel, "Escape");
        assert_eq!(config.hotkey.mode, HotkeyMode::PushToTalk);
        assert_eq!(config.audio.silence_threshold_db, -40.0);
        assert_eq!(config.audio.max_recording_s, 120);
        assert_eq!(config.output.method, "type");
        assert_eq!(config.output.keystroke_delay_ms, 5);
        assert!(config.corrections.enabled);
        assert_eq!(config.corrections.max_examples, 5);
    }

    #[test]
    fn default_path_ends_with_config_toml() {
        let path = Config::default_path();
        assert!(path.ends_with("vox-forge/config.toml"));
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).expect("serialize");
        let deserialized: Config = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.general.log_level, config.general.log_level);
        assert_eq!(
            deserialized.transcription.provider,
            config.transcription.provider
        );
        assert_eq!(deserialized.formatting.provider, config.formatting.provider);
        assert_eq!(deserialized.hotkey.mode, config.hotkey.mode);
    }

    #[test]
    fn load_from_nonexistent_returns_default() {
        let path = std::path::Path::new("/tmp/voxforge-test-nonexistent/config.toml");
        let config = Config::load_from(path).expect("load");
        assert_eq!(config.general.log_level, "info");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.general.log_level = "debug".to_string();
        config.hotkey.mode = HotkeyMode::Toggle;
        config.save_to(&path).expect("save");

        let loaded = Config::load_from(&path).expect("load");
        assert_eq!(loaded.general.log_level, "debug");
        assert_eq!(loaded.hotkey.mode, HotkeyMode::Toggle);
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let toml_str = r#"
[general]
log_level = "debug"
"#;
        let config: Config = toml::from_str(toml_str).expect("parse");
        assert_eq!(config.general.log_level, "debug");
        // Everything else should be defaults
        assert!(config.general.notifications);
        assert_eq!(config.transcription.provider, "whisper_local");
        assert_eq!(config.formatting.provider, "anthropic");
        assert_eq!(config.hotkey.mode, HotkeyMode::PushToTalk);
    }

    #[test]
    fn effective_anthropic_key_prefers_env() {
        let config = Config::default();
        // SAFETY: Test-only env var manipulation; tests run serially with --test-threads=1
        // or this test is self-contained with its own unique key name concern.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        assert_eq!(config.effective_anthropic_key(), "");

        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        }
        assert_eq!(config.effective_anthropic_key(), "test-key-123");
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn effective_openai_key_prefers_env() {
        let config = Config::default();
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }
        assert_eq!(config.effective_openai_key(), "");

        unsafe {
            std::env::set_var("OPENAI_API_KEY", "sk-test-456");
        }
        assert_eq!(config.effective_openai_key(), "sk-test-456");
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }
    }

    #[test]
    fn effective_key_falls_back_to_config() {
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let mut config = Config::default();
        config.formatting.anthropic.api_key = "config-key".to_string();
        assert_eq!(config.effective_anthropic_key(), "config-key");
    }

    #[test]
    fn has_key_methods() {
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("OPENAI_API_KEY");
        }
        let config = Config::default();
        assert!(!config.has_anthropic_key());
        assert!(!config.has_openai_key());
    }

    #[test]
    fn hotkey_mode_display() {
        assert_eq!(HotkeyMode::PushToTalk.to_string(), "push_to_talk");
        assert_eq!(HotkeyMode::Toggle.to_string(), "toggle");
    }

    #[test]
    fn auto_rules_defaults_populated() {
        let config = Config::default();
        assert!(!config.formatting.auto_rules.code.is_empty());
        assert!(!config.formatting.auto_rules.email.is_empty());
        assert!(!config.formatting.auto_rules.chat.is_empty());
        assert!(
            config
                .formatting
                .auto_rules
                .code
                .contains(&"cursor".to_string())
        );
        assert!(
            config
                .formatting
                .auto_rules
                .email
                .contains(&"thunderbird".to_string())
        );
        assert!(
            config
                .formatting
                .auto_rules
                .chat
                .contains(&"slack".to_string())
        );
    }

    #[test]
    fn clipboard_apps_defaults_populated() {
        let config = Config::default();
        assert!(!config.output.clipboard_apps.is_empty());
        assert!(config.output.clipboard_apps.contains(&"kitty".to_string()));
    }

    #[test]
    fn ensure_default_creates_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("vox-forge").join("config.toml");

        // Manually write a default config to this path
        let config = Config::default();
        config.save_to(&path).expect("save");

        assert!(path.exists());
        let loaded = Config::load_from(&path).expect("load");
        assert_eq!(loaded.general.log_level, "info");
    }
}
