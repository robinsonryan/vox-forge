//! Correction logging for few-shot learning.
//!
//! Stores dictation entries and user corrections in JSONL format.
//! Corrections can be formatted for prompt injection so the LLM learns
//! from past mistakes without re-training.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A single correction log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionEntry {
    /// ISO-8601 timestamp of the dictation.
    pub ts: String,
    /// Raw transcript from the STT provider.
    pub raw: String,
    /// Formatted output produced by the LLM.
    pub formatted: String,
    /// User-supplied correction, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction: Option<String>,
    /// Name of the active application at dictation time.
    #[serde(default)]
    pub app: String,
}

/// Manages a JSONL-based correction log file.
pub struct CorrectionLog {
    path: PathBuf,
}

impl CorrectionLog {
    /// Create a new `CorrectionLog` that reads/writes at `path`.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Log a dictation (raw transcript and formatted output).
    pub fn log_dictation(&self, raw: &str, formatted: &str, app: &str) -> Result<()> {
        let entry = CorrectionEntry {
            ts: Utc::now().to_rfc3339(),
            raw: raw.to_string(),
            formatted: formatted.to_string(),
            correction: None,
            app: app.to_string(),
        };
        self.append_entry(&entry)
    }

    /// Attach a correction to the most recent entry whose `formatted` field
    /// matches `original`. The entire log is rewritten because JSONL does not
    /// support in-place edits.
    pub fn add_correction(&self, original: &str, corrected: &str) -> Result<()> {
        let mut entries = self.read_all()?;

        let found = entries.iter_mut().rev().find(|e| e.formatted == original);

        match found {
            Some(entry) => {
                entry.correction = Some(corrected.to_string());
                self.write_all(&entries)?;
                Ok(())
            }
            None => Err(Error::Corrections(format!(
                "No matching dictation found for: '{original}'"
            ))),
        }
    }

    /// Return the most recent corrections (entries that have a `correction` set),
    /// up to `max` entries.
    pub fn recent_corrections(&self, max: usize) -> Result<Vec<CorrectionEntry>> {
        let entries = self.read_all()?;
        let corrections: Vec<CorrectionEntry> = entries
            .into_iter()
            .filter(|e| e.correction.is_some())
            .collect();

        let start = corrections.len().saturating_sub(max);
        Ok(corrections[start..].to_vec())
    }

    /// Format recent corrections for injection into an LLM prompt.
    ///
    /// Returns an empty string when there are no corrections.
    pub fn format_for_prompt(&self, max: usize) -> Result<String> {
        let corrections = self.recent_corrections(max)?;
        if corrections.is_empty() {
            return Ok(String::new());
        }

        let mut lines = Vec::new();
        lines.push("Learn from these past corrections:".to_string());
        for entry in &corrections {
            if let Some(ref correction) = entry.correction {
                lines.push(format!(
                    "- You output: \"{}\" -> User wanted: \"{}\"",
                    entry.formatted, correction
                ));
            }
        }
        Ok(lines.join("\n"))
    }

    /// Return the `count` most recent entries (with or without corrections).
    pub fn list_recent(&self, count: usize) -> Result<Vec<CorrectionEntry>> {
        let entries = self.read_all()?;
        let start = entries.len().saturating_sub(count);
        Ok(entries[start..].to_vec())
    }

    /// Delete the log file, removing all entries.
    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────────

    /// Read all entries from the JSONL file. Malformed lines are skipped
    /// with a warning.
    fn read_all(&self) -> Result<Vec<CorrectionEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = std::fs::File::open(&self.path)?;
        let reader = std::io::BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<CorrectionEntry>(trimmed) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    tracing::warn!("Skipping malformed correction entry: {e}");
                }
            }
        }

        Ok(entries)
    }

    /// Rewrite the entire log file with `entries`. Used when an in-place
    /// update (like adding a correction) is needed.
    fn write_all(&self, entries: &[CorrectionEntry]) -> Result<()> {
        Self::ensure_parent(&self.path)?;

        let mut file = std::fs::File::create(&self.path)?;
        for entry in entries {
            let json = serde_json::to_string(entry)?;
            writeln!(file, "{json}")?;
        }
        Ok(())
    }

    /// Append a single entry to the end of the log file.
    fn append_entry(&self, entry: &CorrectionEntry) -> Result<()> {
        Self::ensure_parent(&self.path)?;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        let json = serde_json::to_string(entry)?;
        writeln!(file, "{json}")?;
        Ok(())
    }

    /// Create parent directories if they don't exist.
    fn ensure_parent(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a `CorrectionLog` in a fresh temp directory.
    fn temp_log() -> (tempfile::TempDir, CorrectionLog) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("corrections.jsonl");
        let log = CorrectionLog::new(path);
        (dir, log)
    }

    // ── log_dictation ────────────────────────────────────────────────

    #[test]
    fn log_dictation_creates_entry() {
        let (_dir, log) = temp_log();
        log.log_dictation("hello world", "Hello, world.", "code")
            .expect("log");

        let entries = log.read_all().expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw, "hello world");
        assert_eq!(entries[0].formatted, "Hello, world.");
        assert_eq!(entries[0].app, "code");
        assert!(entries[0].correction.is_none());
        assert!(!entries[0].ts.is_empty());
    }

    #[test]
    fn log_dictation_appends() {
        let (_dir, log) = temp_log();
        log.log_dictation("first", "First.", "app1").expect("log 1");
        log.log_dictation("second", "Second.", "app2")
            .expect("log 2");

        let entries = log.read_all().expect("read");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].formatted, "First.");
        assert_eq!(entries[1].formatted, "Second.");
    }

    // ── add_correction ───────────────────────────────────────────────

    #[test]
    fn add_correction_updates_matching_entry() {
        let (_dir, log) = temp_log();
        log.log_dictation("raw", "Formatted output.", "app")
            .expect("log");
        log.add_correction("Formatted output.", "Corrected output.")
            .expect("correction");

        let entries = log.read_all().expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].correction.as_deref(), Some("Corrected output."));
    }

    #[test]
    fn add_correction_updates_most_recent_match() {
        let (_dir, log) = temp_log();
        log.log_dictation("raw1", "Same output.", "app")
            .expect("log 1");
        log.log_dictation("raw2", "Same output.", "app")
            .expect("log 2");
        log.add_correction("Same output.", "Fixed.")
            .expect("correction");

        let entries = log.read_all().expect("read");
        // First entry should NOT have the correction
        assert!(entries[0].correction.is_none());
        // Second (most recent) should
        assert_eq!(entries[1].correction.as_deref(), Some("Fixed."));
    }

    #[test]
    fn add_correction_no_match_errors() {
        let (_dir, log) = temp_log();
        log.log_dictation("raw", "Output A.", "app").expect("log");

        let err = log
            .add_correction("No such output.", "Fix.")
            .expect_err("no match");
        assert!(err.to_string().contains("No matching dictation"));
    }

    // ── recent_corrections ───────────────────────────────────────────

    #[test]
    fn recent_corrections_filters_only_corrected() {
        let (_dir, log) = temp_log();
        log.log_dictation("r1", "F1.", "app").expect("log");
        log.log_dictation("r2", "F2.", "app").expect("log");
        log.log_dictation("r3", "F3.", "app").expect("log");

        // Only correct F2
        log.add_correction("F2.", "Fixed F2.").expect("correct");

        let corrections = log.recent_corrections(10).expect("recent");
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].formatted, "F2.");
    }

    #[test]
    fn recent_corrections_respects_max() {
        let (_dir, log) = temp_log();
        for i in 0..5 {
            let formatted = format!("F{i}.");
            log.log_dictation("raw", &formatted, "app").expect("log");
            log.add_correction(&formatted, &format!("C{i}."))
                .expect("correct");
        }

        let corrections = log.recent_corrections(3).expect("recent");
        assert_eq!(corrections.len(), 3);
        // Should be the last 3
        assert_eq!(corrections[0].correction.as_deref(), Some("C2."));
        assert_eq!(corrections[1].correction.as_deref(), Some("C3."));
        assert_eq!(corrections[2].correction.as_deref(), Some("C4."));
    }

    #[test]
    fn recent_corrections_empty_log() {
        let (_dir, log) = temp_log();
        let corrections = log.recent_corrections(10).expect("recent");
        assert!(corrections.is_empty());
    }

    // ── format_for_prompt ────────────────────────────────────────────

    #[test]
    fn format_for_prompt_with_corrections() {
        let (_dir, log) = temp_log();
        log.log_dictation("raw", "Bad output.", "app").expect("log");
        log.add_correction("Bad output.", "Good output.")
            .expect("correct");

        let prompt = log.format_for_prompt(5).expect("format");
        assert!(prompt.contains("Learn from these past corrections:"));
        assert!(prompt.contains("Bad output."));
        assert!(prompt.contains("Good output."));
    }

    #[test]
    fn format_for_prompt_empty_returns_empty() {
        let (_dir, log) = temp_log();
        let prompt = log.format_for_prompt(5).expect("format");
        assert!(prompt.is_empty());
    }

    #[test]
    fn format_for_prompt_no_corrections_returns_empty() {
        let (_dir, log) = temp_log();
        log.log_dictation("raw", "Output.", "app").expect("log");

        let prompt = log.format_for_prompt(5).expect("format");
        assert!(prompt.is_empty());
    }

    // ── list_recent ──────────────────────────────────────────────────

    #[test]
    fn list_recent_returns_tail() {
        let (_dir, log) = temp_log();
        for i in 0..10 {
            log.log_dictation("raw", &format!("F{i}."), "app")
                .expect("log");
        }

        let recent = log.list_recent(3).expect("list");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].formatted, "F7.");
        assert_eq!(recent[1].formatted, "F8.");
        assert_eq!(recent[2].formatted, "F9.");
    }

    #[test]
    fn list_recent_more_than_available() {
        let (_dir, log) = temp_log();
        log.log_dictation("raw", "Only one.", "app").expect("log");

        let recent = log.list_recent(100).expect("list");
        assert_eq!(recent.len(), 1);
    }

    // ── clear ────────────────────────────────────────────────────────

    #[test]
    fn clear_removes_file() {
        let (_dir, log) = temp_log();
        log.log_dictation("raw", "Output.", "app").expect("log");
        assert!(log.path.exists());

        log.clear().expect("clear");
        assert!(!log.path.exists());

        // Reading after clear should return empty
        let entries = log.read_all().expect("read");
        assert!(entries.is_empty());
    }

    #[test]
    fn clear_on_nonexistent_is_ok() {
        let (_dir, log) = temp_log();
        // No file created yet
        log.clear().expect("clear should be idempotent");
    }

    // ── malformed lines ──────────────────────────────────────────────

    #[test]
    fn read_all_handles_malformed_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("corrections.jsonl");

        // Write a mix of valid and invalid lines
        let valid = serde_json::to_string(&CorrectionEntry {
            ts: "2025-01-01T00:00:00Z".to_string(),
            raw: "valid".to_string(),
            formatted: "Valid.".to_string(),
            correction: None,
            app: "test".to_string(),
        })
        .expect("serialize");

        let content = format!("{valid}\nthis is not json\n{valid}\n\n");
        std::fs::write(&path, content).expect("write");

        let log = CorrectionLog::new(path);
        let entries = log.read_all().expect("read");
        // Should have 2 valid entries, skipping the bad line
        assert_eq!(entries.len(), 2);
    }

    // ── JSONL roundtrip ──────────────────────────────────────────────

    #[test]
    fn jsonl_roundtrip() {
        let (_dir, log) = temp_log();

        log.log_dictation("hello world", "Hello, world!", "vscode")
            .expect("log 1");
        log.log_dictation("fix the bug", "Fix the bug.", "terminal")
            .expect("log 2");
        log.add_correction("Hello, world!", "Hello, World!")
            .expect("correct");

        let entries = log.read_all().expect("read");
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].raw, "hello world");
        assert_eq!(entries[0].formatted, "Hello, world!");
        assert_eq!(entries[0].correction.as_deref(), Some("Hello, World!"));
        assert_eq!(entries[0].app, "vscode");

        assert_eq!(entries[1].raw, "fix the bug");
        assert_eq!(entries[1].formatted, "Fix the bug.");
        assert!(entries[1].correction.is_none());
        assert_eq!(entries[1].app, "terminal");
    }
}
