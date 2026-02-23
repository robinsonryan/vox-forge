//! Application configuration.
//!
//! Loads from TOML config file with environment variable overrides.

use std::path::PathBuf;

use serde::Deserialize;

/// Application configuration loaded from TOML.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Application display name.
    #[serde(default = "default_app_name")]
    pub app_name: String,
}

fn default_app_name() -> String {
    env!("CARGO_PKG_NAME").to_string()
}

impl Config {
    /// Loads configuration from the default config file path.
    ///
    /// # Errors
    ///
    /// Returns error if the config file cannot be read or parsed.
    pub fn load() -> crate::error::Result<Self> {
        let config_path = Self::config_path();
        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            toml::from_str(&contents).map_err(|e| crate::error::Error::Config(e.to_string()))
        } else {
            Ok(Self::default())
        }
    }

    /// Returns the default config file path.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(env!("CARGO_PKG_NAME"))
            .join("config.toml")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_name: default_app_name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_app_name() {
        let config = Config::default();
        assert!(!config.app_name.is_empty());
    }
}
