//! Local fallback formatting (regex-based cleanup).
//!
//! Used when the cloud LLM API is unavailable. Provides basic
//! filler removal, space normalization, capitalization, and punctuation.

use regex::Regex;
use std::sync::LazyLock;

/// Regex-based local fallback formatting.
///
/// Applies basic cleanup when the cloud API is unavailable:
/// 1. Remove common filler words
/// 2. Collapse multiple spaces
/// 3. Capitalize first character
/// 4. Ensure terminal punctuation
pub fn format_fallback(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut result = text.to_string();

    // 1. Remove filler words at word boundaries (case-insensitive)
    result = remove_fillers(&result);

    // 2. Collapse multiple spaces
    result = collapse_spaces(&result);

    // 3. Trim
    result = result.trim().to_string();

    // 4. Capitalize first character
    result = capitalize_first(&result);

    // 5. Add period if no terminal punctuation
    result = ensure_terminal_punctuation(&result);

    result
}

static FILLER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(uh huh|um|uh|you know|so yeah|i mean|kind of|sort of)\b")
        .expect("valid regex")
});

static MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" {2,}").expect("valid regex"));

fn remove_fillers(text: &str) -> String {
    FILLER_REGEX.replace_all(text, "").to_string()
}

fn collapse_spaces(text: &str) -> String {
    MULTI_SPACE.replace_all(text, " ").to_string()
}

fn capitalize_first(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let upper: String = c.to_uppercase().collect();
            format!("{upper}{}", chars.as_str())
        }
    }
}

fn ensure_terminal_punctuation(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let trimmed = text.trim_end();
    if let Some(last) = trimmed.chars().last()
        && ".!?".contains(last)
    {
        return trimmed.to_string();
    }
    format!("{trimmed}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_fallback integration tests ──────────────────────────

    #[test]
    fn empty_string_returns_empty() {
        assert_eq!(format_fallback(""), "");
    }

    #[test]
    fn simple_text_gets_capitalized_and_punctuated() {
        assert_eq!(format_fallback("hello world"), "Hello world.");
    }

    #[test]
    fn already_punctuated_text_not_double_punctuated() {
        assert_eq!(format_fallback("hello world."), "Hello world.");
    }

    #[test]
    fn question_mark_preserved() {
        assert_eq!(format_fallback("is this working?"), "Is this working?");
    }

    #[test]
    fn exclamation_mark_preserved() {
        assert_eq!(format_fallback("this is great!"), "This is great!");
    }

    #[test]
    fn already_capitalized_text_unchanged() {
        assert_eq!(format_fallback("Hello world"), "Hello world.");
    }

    // ── filler removal tests ───────────────────────────────────────

    #[test]
    fn removes_um() {
        assert_eq!(format_fallback("um hello there"), "Hello there.");
    }

    #[test]
    fn removes_uh() {
        assert_eq!(format_fallback("uh I think so"), "I think so.");
    }

    #[test]
    fn removes_you_know() {
        assert_eq!(
            format_fallback("it was you know pretty good"),
            "It was pretty good."
        );
    }

    #[test]
    fn removes_so_yeah() {
        assert_eq!(format_fallback("so yeah that happened"), "That happened.");
    }

    #[test]
    fn removes_i_mean() {
        assert_eq!(format_fallback("i mean it works fine"), "It works fine.");
    }

    #[test]
    fn removes_kind_of() {
        assert_eq!(
            format_fallback("it was kind of interesting"),
            "It was interesting."
        );
    }

    #[test]
    fn removes_sort_of() {
        assert_eq!(format_fallback("it sort of works"), "It works.");
    }

    #[test]
    fn removes_multiple_fillers() {
        assert_eq!(
            format_fallback("um so yeah I uh think it works"),
            "I think it works."
        );
    }

    #[test]
    fn filler_removal_case_insensitive() {
        assert_eq!(format_fallback("UM hello UH there"), "Hello there.");
    }

    // ── space collapsing tests ─────────────────────────────────────

    #[test]
    fn collapses_multiple_spaces() {
        assert_eq!(format_fallback("hello    world"), "Hello world.");
    }

    #[test]
    fn collapses_spaces_from_filler_removal() {
        // After removing "um", there may be extra spaces
        assert_eq!(format_fallback("hello um world"), "Hello world.");
    }

    // ── capitalization tests ───────────────────────────────────────

    #[test]
    fn capitalizes_lowercase_start() {
        assert_eq!(format_fallback("test"), "Test.");
    }

    #[test]
    fn handles_single_character() {
        assert_eq!(format_fallback("a"), "A.");
    }

    // ── edge cases ─────────────────────────────────────────────────

    #[test]
    fn whitespace_only_returns_empty() {
        // After trim, empty string
        assert_eq!(format_fallback("   "), "");
    }

    #[test]
    fn text_with_trailing_spaces() {
        assert_eq!(format_fallback("hello   "), "Hello.");
    }

    #[test]
    fn text_with_leading_spaces() {
        assert_eq!(format_fallback("   hello"), "Hello.");
    }

    // ── unit tests for internal functions ───────────────────────────

    #[test]
    fn capitalize_first_empty() {
        assert_eq!(capitalize_first(""), "");
    }

    #[test]
    fn capitalize_first_already_upper() {
        assert_eq!(capitalize_first("Hello"), "Hello");
    }

    #[test]
    fn capitalize_first_lower() {
        assert_eq!(capitalize_first("hello"), "Hello");
    }

    #[test]
    fn ensure_terminal_punctuation_empty() {
        assert_eq!(ensure_terminal_punctuation(""), "");
    }

    #[test]
    fn ensure_terminal_punctuation_adds_period() {
        assert_eq!(ensure_terminal_punctuation("hello"), "hello.");
    }

    #[test]
    fn ensure_terminal_punctuation_preserves_question() {
        assert_eq!(ensure_terminal_punctuation("hello?"), "hello?");
    }

    #[test]
    fn ensure_terminal_punctuation_preserves_exclamation() {
        assert_eq!(ensure_terminal_punctuation("hello!"), "hello!");
    }

    #[test]
    fn ensure_terminal_punctuation_preserves_period() {
        assert_eq!(ensure_terminal_punctuation("hello."), "hello.");
    }

    #[test]
    fn collapse_spaces_no_change_needed() {
        assert_eq!(collapse_spaces("hello world"), "hello world");
    }

    #[test]
    fn collapse_spaces_multiple() {
        assert_eq!(collapse_spaces("a   b    c"), "a b c");
    }

    #[test]
    fn remove_fillers_no_fillers() {
        assert_eq!(remove_fillers("hello world"), "hello world");
    }

    #[test]
    fn remove_fillers_removes_uh_huh() {
        assert_eq!(remove_fillers("uh huh that works"), " that works");
    }
}
