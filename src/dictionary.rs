//! Personal dictionary management.
//!
//! Provides functions for adding, removing, importing, and exporting
//! custom terms. Terms are stored in the application configuration and
//! can be formatted for injection into LLM prompts so the model respects
//! domain-specific vocabulary.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};

/// Add a term to the personal dictionary.
///
/// Returns an error if the term is empty or already exists.
pub fn add_term(config: &mut Config, term: &str) -> Result<()> {
    let term = term.trim().to_string();
    if term.is_empty() {
        return Err(Error::Dictionary("Term cannot be empty".to_string()));
    }
    if config.dictionary.custom_terms.contains(&term) {
        return Err(Error::Dictionary(format!("Term '{term}' already exists")));
    }
    config.dictionary.custom_terms.push(term);
    Ok(())
}

/// Remove a term from the personal dictionary.
///
/// Returns an error if the term is not found.
pub fn remove_term(config: &mut Config, term: &str) -> Result<()> {
    let term = term.trim();
    let pos = config
        .dictionary
        .custom_terms
        .iter()
        .position(|t| t == term)
        .ok_or_else(|| Error::Dictionary(format!("Term '{term}' not found")))?;
    config.dictionary.custom_terms.remove(pos);
    Ok(())
}

/// List all terms in the personal dictionary.
pub fn list_terms(config: &Config) -> &[String] {
    &config.dictionary.custom_terms
}

/// Format terms for injection into an LLM prompt.
///
/// Returns `"(none configured)"` when the dictionary is empty, otherwise
/// returns a comma-separated list of all terms.
pub fn format_terms_for_prompt(config: &Config) -> String {
    if config.dictionary.custom_terms.is_empty() {
        "(none configured)".to_string()
    } else {
        config.dictionary.custom_terms.join(", ")
    }
}

/// Import terms from a newline-delimited text file.
///
/// Blank lines and duplicates are skipped. Returns the number of new
/// terms that were added.
pub fn import_from_file(config: &mut Config, path: &Path) -> Result<usize> {
    let content = std::fs::read_to_string(path)?;
    let mut count = 0;
    for line in content.lines() {
        let term = line.trim().to_string();
        if !term.is_empty() && !config.dictionary.custom_terms.contains(&term) {
            config.dictionary.custom_terms.push(term);
            count += 1;
        }
    }
    Ok(count)
}

/// Export terms to a newline-delimited text file.
pub fn export_to_file(config: &Config, path: &Path) -> Result<()> {
    let content = config.dictionary.custom_terms.join("\n");
    std::fs::write(path, content)?;
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_config() -> Config {
        Config::default()
    }

    // ── add_term ─────────────────────────────────────────────────────

    #[test]
    fn add_term_normal() {
        let mut config = empty_config();
        add_term(&mut config, "Kubernetes").expect("should succeed");
        assert_eq!(config.dictionary.custom_terms, vec!["Kubernetes"]);
    }

    #[test]
    fn add_term_trims_whitespace() {
        let mut config = empty_config();
        add_term(&mut config, "  gRPC  ").expect("should succeed");
        assert_eq!(config.dictionary.custom_terms, vec!["gRPC"]);
    }

    #[test]
    fn add_term_duplicate_errors() {
        let mut config = empty_config();
        add_term(&mut config, "Rust").expect("first add");
        let err = add_term(&mut config, "Rust").expect_err("duplicate");
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn add_term_empty_errors() {
        let mut config = empty_config();
        let err = add_term(&mut config, "").expect_err("empty term");
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn add_term_whitespace_only_errors() {
        let mut config = empty_config();
        let err = add_term(&mut config, "   ").expect_err("whitespace only");
        assert!(err.to_string().contains("cannot be empty"));
    }

    // ── remove_term ──────────────────────────────────────────────────

    #[test]
    fn remove_term_normal() {
        let mut config = empty_config();
        config.dictionary.custom_terms = vec!["Alpha".to_string(), "Beta".to_string()];
        remove_term(&mut config, "Alpha").expect("should succeed");
        assert_eq!(config.dictionary.custom_terms, vec!["Beta"]);
    }

    #[test]
    fn remove_term_not_found_errors() {
        let mut config = empty_config();
        let err = remove_term(&mut config, "missing").expect_err("not found");
        assert!(err.to_string().contains("not found"));
    }

    // ── list_terms ───────────────────────────────────────────────────

    #[test]
    fn list_terms_empty() {
        let config = empty_config();
        assert!(list_terms(&config).is_empty());
    }

    #[test]
    fn list_terms_returns_all() {
        let mut config = empty_config();
        config.dictionary.custom_terms = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        assert_eq!(list_terms(&config), &["A", "B", "C"]);
    }

    // ── format_terms_for_prompt ──────────────────────────────────────

    #[test]
    fn format_terms_empty() {
        let config = empty_config();
        assert_eq!(format_terms_for_prompt(&config), "(none configured)");
    }

    #[test]
    fn format_terms_with_entries() {
        let mut config = empty_config();
        config.dictionary.custom_terms = vec![
            "Kubernetes".to_string(),
            "gRPC".to_string(),
            "NATS".to_string(),
        ];
        assert_eq!(format_terms_for_prompt(&config), "Kubernetes, gRPC, NATS");
    }

    // ── import / export roundtrip ────────────────────────────────────

    #[test]
    fn import_export_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("terms.txt");

        // Export some terms
        let mut config = empty_config();
        config.dictionary.custom_terms = vec![
            "Anthropic".to_string(),
            "LangChain".to_string(),
            "tokio".to_string(),
        ];
        export_to_file(&config, &path).expect("export");

        // Import into a fresh config
        let mut fresh = empty_config();
        let count = import_from_file(&mut fresh, &path).expect("import");
        assert_eq!(count, 3);
        assert_eq!(
            fresh.dictionary.custom_terms,
            config.dictionary.custom_terms
        );
    }

    #[test]
    fn import_skips_blank_lines_and_duplicates() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("terms.txt");
        std::fs::write(&path, "Alpha\n\nBeta\nAlpha\n  \nGamma\n").expect("write");

        let mut config = empty_config();
        let count = import_from_file(&mut config, &path).expect("import");
        assert_eq!(count, 3);
        assert_eq!(
            config.dictionary.custom_terms,
            vec!["Alpha", "Beta", "Gamma"]
        );
    }

    #[test]
    fn import_from_nonexistent_file_errors() {
        let mut config = empty_config();
        let result = import_from_file(&mut config, Path::new("/tmp/voxforge-no-such-file.txt"));
        assert!(result.is_err());
    }
}
