//! Prompt template management for formatting modes.
//!
//! Embeds prompt templates at compile time and provides interpolation
//! and auto-mode resolution based on active window context.

use std::collections::HashMap;

use crate::config::AutoRulesConfig;

/// Formatting modes that determine which prompt template to use.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattingMode {
    /// General-purpose dictation cleanup.
    Standard,
    /// Optimized for code editors and terminals.
    Code,
    /// Professional email composition.
    Email,
    /// Casual messaging (Slack, Discord, etc.).
    Chat,
    /// No LLM formatting — pass through raw text.
    Raw,
}

// Embed templates at compile time
const PROMPT_STANDARD: &str = include_str!("../../prompts/format_standard.txt");
const PROMPT_CODE: &str = include_str!("../../prompts/format_code.txt");
const PROMPT_EMAIL: &str = include_str!("../../prompts/format_email.txt");
const PROMPT_CHAT: &str = include_str!("../../prompts/format_chat.txt");

/// Get the prompt template for a formatting mode.
///
/// Returns `None` for `Raw` mode since it skips LLM entirely.
pub fn get_template(mode: &FormattingMode) -> Option<&'static str> {
    match mode {
        FormattingMode::Standard => Some(PROMPT_STANDARD),
        FormattingMode::Code => Some(PROMPT_CODE),
        FormattingMode::Email => Some(PROMPT_EMAIL),
        FormattingMode::Chat => Some(PROMPT_CHAT),
        FormattingMode::Raw => None,
    }
}

/// Interpolate variables into a template.
///
/// Replaces `{key}` placeholders with corresponding values from `vars`.
pub fn interpolate(template: &str, vars: &HashMap<&str, &str>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}

/// Build the full system prompt for a given mode and context.
///
/// Returns `None` for `Raw` mode (no LLM formatting needed).
pub fn build_system_prompt(
    mode: &FormattingMode,
    app_name: &str,
    window_title: &str,
    custom_terms: &[String],
    recent_corrections: &str,
) -> Option<String> {
    let template = get_template(mode)?;

    let terms_str = if custom_terms.is_empty() {
        String::from("(none configured)")
    } else {
        custom_terms.join(", ")
    };

    let corrections_section = if recent_corrections.is_empty() {
        String::new()
    } else {
        format!("\nLearn from these past corrections:\n{recent_corrections}")
    };

    let mut vars = HashMap::new();
    vars.insert("app_name", app_name);
    vars.insert("window_title", window_title);
    let terms_ref = terms_str.as_str();
    let corrections_ref = corrections_section.as_str();
    vars.insert("custom_terms", terms_ref);
    vars.insert("recent_corrections", corrections_ref);

    Some(interpolate(template, &vars))
}

/// Resolve formatting mode from config auto-rules and active window info.
///
/// If `default_mode` is `"auto"`, inspects the current app context
/// (name, window title, executable) against the configured rules.
/// Otherwise, uses the literal mode string from config.
pub fn resolve_mode(
    default_mode: &str,
    auto_rules: &AutoRulesConfig,
    app_name: &str,
    window_title: &str,
    executable: &str,
) -> FormattingMode {
    // If not auto, use the configured mode directly
    if default_mode != "auto" {
        return match default_mode {
            "code" => FormattingMode::Code,
            "email" => FormattingMode::Email,
            "chat" => FormattingMode::Chat,
            "raw" => FormattingMode::Raw,
            _ => FormattingMode::Standard,
        };
    }

    // Auto-mode: check rules against app context
    let targets = [
        app_name.to_lowercase(),
        window_title.to_lowercase(),
        executable.to_lowercase(),
    ];

    // Check code rules
    for pattern in &auto_rules.code {
        let lower = pattern.to_lowercase();
        if targets.iter().any(|t| t.contains(&lower)) {
            return FormattingMode::Code;
        }
    }

    // Check email rules
    for pattern in &auto_rules.email {
        let lower = pattern.to_lowercase();
        if targets.iter().any(|t| t.contains(&lower)) {
            return FormattingMode::Email;
        }
    }

    // Check chat rules
    for pattern in &auto_rules.chat {
        let lower = pattern.to_lowercase();
        if targets.iter().any(|t| t.contains(&lower)) {
            return FormattingMode::Chat;
        }
    }

    // Default fallback
    FormattingMode::Standard
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AutoRulesConfig;

    #[test]
    fn get_template_returns_some_for_standard() {
        let template = get_template(&FormattingMode::Standard);
        assert!(template.is_some());
        assert!(
            template
                .expect("template")
                .contains("voice dictation post-processor")
        );
    }

    #[test]
    fn get_template_returns_some_for_code() {
        let template = get_template(&FormattingMode::Code);
        assert!(template.is_some());
        assert!(template.expect("template").contains("software developer"));
    }

    #[test]
    fn get_template_returns_some_for_email() {
        let template = get_template(&FormattingMode::Email);
        assert!(template.is_some());
        assert!(template.expect("template").contains("email composition"));
    }

    #[test]
    fn get_template_returns_some_for_chat() {
        let template = get_template(&FormattingMode::Chat);
        assert!(template.is_some());
        assert!(template.expect("template").contains("casual messaging"));
    }

    #[test]
    fn get_template_returns_none_for_raw() {
        assert!(get_template(&FormattingMode::Raw).is_none());
    }

    #[test]
    fn interpolate_replaces_placeholders() {
        let template = "Hello {name}, welcome to {place}!";
        let mut vars = HashMap::new();
        vars.insert("name", "Alice");
        vars.insert("place", "Wonderland");
        let result = interpolate(template, &vars);
        assert_eq!(result, "Hello Alice, welcome to Wonderland!");
    }

    #[test]
    fn interpolate_handles_empty_vars() {
        let template = "No placeholders here.";
        let vars = HashMap::new();
        let result = interpolate(template, &vars);
        assert_eq!(result, "No placeholders here.");
    }

    #[test]
    fn interpolate_handles_missing_placeholder() {
        let template = "Hello {name}, your id is {id}.";
        let mut vars = HashMap::new();
        vars.insert("name", "Bob");
        // {id} not provided — should remain as-is
        let result = interpolate(template, &vars);
        assert_eq!(result, "Hello Bob, your id is {id}.");
    }

    #[test]
    fn interpolate_handles_repeated_placeholder() {
        let template = "{x} and {x} again";
        let mut vars = HashMap::new();
        vars.insert("x", "val");
        let result = interpolate(template, &vars);
        assert_eq!(result, "val and val again");
    }

    #[test]
    fn build_system_prompt_returns_none_for_raw() {
        let result = build_system_prompt(&FormattingMode::Raw, "test", "title", &[], "");
        assert!(result.is_none());
    }

    #[test]
    fn build_system_prompt_interpolates_app_name() {
        let result = build_system_prompt(&FormattingMode::Standard, "VS Code", "main.rs", &[], "");
        let prompt = result.expect("should produce prompt");
        assert!(prompt.contains("VS Code"));
        assert!(prompt.contains("main.rs"));
        assert!(prompt.contains("(none configured)"));
    }

    #[test]
    fn build_system_prompt_includes_custom_terms() {
        let terms = vec!["VoxForge".to_string(), "Anthropic".to_string()];
        let result = build_system_prompt(&FormattingMode::Standard, "app", "title", &terms, "");
        let prompt = result.expect("should produce prompt");
        assert!(prompt.contains("VoxForge, Anthropic"));
    }

    #[test]
    fn build_system_prompt_includes_corrections() {
        let result = build_system_prompt(
            &FormattingMode::Standard,
            "app",
            "title",
            &[],
            "user corrected 'teh' to 'the'",
        );
        let prompt = result.expect("should produce prompt");
        assert!(prompt.contains("Learn from these past corrections:"));
        assert!(prompt.contains("user corrected 'teh' to 'the'"));
    }

    #[test]
    fn build_system_prompt_omits_corrections_section_when_empty() {
        let result = build_system_prompt(&FormattingMode::Standard, "app", "title", &[], "");
        let prompt = result.expect("should produce prompt");
        assert!(!prompt.contains("Learn from these past corrections:"));
    }

    #[test]
    fn resolve_mode_explicit_code() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("code", &rules, "", "", "");
        assert_eq!(mode, FormattingMode::Code);
    }

    #[test]
    fn resolve_mode_explicit_email() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("email", &rules, "", "", "");
        assert_eq!(mode, FormattingMode::Email);
    }

    #[test]
    fn resolve_mode_explicit_chat() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("chat", &rules, "", "", "");
        assert_eq!(mode, FormattingMode::Chat);
    }

    #[test]
    fn resolve_mode_explicit_raw() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("raw", &rules, "", "", "");
        assert_eq!(mode, FormattingMode::Raw);
    }

    #[test]
    fn resolve_mode_explicit_standard() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("standard", &rules, "", "", "");
        assert_eq!(mode, FormattingMode::Standard);
    }

    #[test]
    fn resolve_mode_unknown_falls_back_to_standard() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("nonsense", &rules, "", "", "");
        assert_eq!(mode, FormattingMode::Standard);
    }

    #[test]
    fn resolve_mode_auto_detects_code_by_app_name() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("auto", &rules, "Cursor", "untitled", "cursor");
        assert_eq!(mode, FormattingMode::Code);
    }

    #[test]
    fn resolve_mode_auto_detects_code_by_window_title() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("auto", &rules, "Unknown", "vim - main.rs", "unknown");
        assert_eq!(mode, FormattingMode::Code);
    }

    #[test]
    fn resolve_mode_auto_detects_email() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("auto", &rules, "Thunderbird", "Inbox", "thunderbird");
        assert_eq!(mode, FormattingMode::Email);
    }

    #[test]
    fn resolve_mode_auto_detects_chat() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("auto", &rules, "Slack", "#general", "slack");
        assert_eq!(mode, FormattingMode::Chat);
    }

    #[test]
    fn resolve_mode_auto_falls_back_to_standard() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("auto", &rules, "LibreOffice", "Document", "libreoffice");
        assert_eq!(mode, FormattingMode::Standard);
    }

    #[test]
    fn resolve_mode_auto_case_insensitive() {
        let rules = AutoRulesConfig::default();
        // "CURSOR" should still match "cursor" in rules
        let mode = resolve_mode("auto", &rules, "CURSOR", "file.py", "CURSOR");
        assert_eq!(mode, FormattingMode::Code);
    }

    #[test]
    fn resolve_mode_auto_matches_executable() {
        let rules = AutoRulesConfig::default();
        let mode = resolve_mode("auto", &rules, "Unknown App", "Untitled", "discord");
        assert_eq!(mode, FormattingMode::Chat);
    }

    #[test]
    fn resolve_mode_code_takes_priority_over_email() {
        // If both code and email match, code wins (checked first)
        let rules = AutoRulesConfig {
            code: vec!["hybrid".to_string()],
            email: vec!["hybrid".to_string()],
            chat: vec![],
        };
        let mode = resolve_mode("auto", &rules, "hybrid", "title", "hybrid");
        assert_eq!(mode, FormattingMode::Code);
    }
}
