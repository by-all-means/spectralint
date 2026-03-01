use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{inside_inline_code, ScopeFilter};
use super::Checker;

/// Pattern to extract month + year from stale reference matches.
static MONTH_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(january|february|march|april|may|june|july|august|september|october|november|december)\s+(20\d{2})").unwrap()
});

/// Pattern to extract bare year from stale reference matches.
static BARE_YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(20\d{2})\b").unwrap());

fn month_to_number(month: &str) -> Option<u32> {
    match month.to_lowercase().as_str() {
        "january" => Some(1),
        "february" => Some(2),
        "march" => Some(3),
        "april" => Some(4),
        "may" => Some(5),
        "june" => Some(6),
        "july" => Some(7),
        "august" => Some(8),
        "september" => Some(9),
        "october" => Some(10),
        "november" => Some(11),
        "december" => Some(12),
        _ => None,
    }
}

/// Returns true if the referenced date is in the past (with 30-day grace period).
/// If no date can be parsed, returns true (flag anyway as before).
fn is_date_in_past(matched_text: &str) -> bool {
    // Current date: compile-time fallback, or use env var for testing
    let (now_year, now_month) = current_year_month();

    // Try month + year first
    if let Some(caps) = MONTH_YEAR.captures(matched_text) {
        if let (Some(month), Ok(year)) = (month_to_number(&caps[1]), caps[2].parse::<u32>()) {
            // Add 1 month grace period
            let grace_month = if month == 12 {
                (year + 1, 1)
            } else {
                (year, month + 1)
            };
            return (now_year, now_month) >= grace_month;
        }
    }

    // Try bare year
    if let Some(caps) = BARE_YEAR.captures(matched_text) {
        if let Ok(year) = caps[1].parse::<u32>() {
            // Flag if current year is past the referenced year
            return now_year > year;
        }
    }

    // Can't parse a date — flag anyway (legacy behavior)
    true
}

static CURRENT_DATE: LazyLock<(u32, u32)> = LazyLock::new(|| {
    if let Ok(val) = std::env::var("SPECTRALINT_CURRENT_DATE") {
        let parts: Vec<&str> = val.split('-').collect();
        if parts.len() >= 2 {
            if let (Ok(y), Ok(m)) = (parts[0].parse(), parts[1].parse()) {
                return (y, m);
            }
        }
    }
    // Default: 2026-03 (current date from system context)
    (2026, 3)
});

fn current_year_month() -> (u32, u32) {
    *CURRENT_DATE
}

pub(crate) struct StaleReferenceChecker {
    scope: ScopeFilter,
}

impl StaleReferenceChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// "deprecated in favor/lieu/preference" is a permanent statement, not time-sensitive.
static DEPRECATED_PERMANENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bdeprecated\s+in\s+(?:favor|lieu|preference)\b").unwrap());

static STALE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:",
        r"\b(?:before|after|until|since|as of)\s+(?:january|february|march|april|may|june|july|august|september|october|november|december)\s+20\d{2}",
        r"|\b(?:before|after|until|since|as of)\s+20\d{2}",
        r"|\b(?:before|after|until|since|as of)\s+\d{1,2}/\d{1,2}/\d{2,4}",
        r"|\bif\b.*\byear\b.*\b20\d{2}\b",
        r"|\bdeprecated\s+(?:in|since|after)\b",
        r")",
    ))
    .unwrap()
});

/// Time markers in descriptive/project-context prose are often legitimate
/// snapshots ("runway until...", "as of ..."), not stale instructions.
/// Require at least one directive/action signal before flagging.
static ACTION_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:use|switch|migrate|move|replace|run|execute|update|enable|disable|must|should|need(?:s)?\s+to|deprecated)\b",
    )
    .unwrap()
});

impl Checker for StaleReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in file.non_code_lines() {
                // Skip "deprecated in favor/lieu/preference" — permanent statements
                if DEPRECATED_PERMANENT.is_match(line) {
                    continue;
                }

                // Focus on actionable instructions; skip descriptive status prose.
                if !ACTION_CONTEXT.is_match(line) {
                    continue;
                }

                if let Some(m) = STALE_PATTERN.find(line) {
                    // Skip matches inside inline backtick code (e.g., `--since 2024-01-01`)
                    if !inside_inline_code(line, m.start()) && is_date_in_past(line) {
                        emit!(
                            result,
                            file.path,
                            i + 1,
                            Severity::Warning,
                            Category::StaleReference,
                            suggest: "Remove time-sensitive logic or replace with a permanent instruction",
                            "Time-sensitive reference found: \"{}\"",
                            m.as_str()
                        );
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        StaleReferenceChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_after_month_year_detected() {
        let result = run_check(&["After March 2025, use new API"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_before_year_detected() {
        let result = run_check(&["Before 2024, use the legacy endpoint"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_since_date_detected() {
        let result = run_check(&["Since 1/15/2024 we use the new format"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_if_year_detected() {
        let result = run_check(&["If the year is 2025, switch to v2"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_if_number_no_false_positive() {
        let result = run_check(&["if you need to handle 2048 connections"]);
        assert!(
            result.diagnostics.is_empty(),
            "Bare numbers in 2000-range should not trigger stale-reference"
        );
    }

    #[test]
    fn test_deprecated_since_detected() {
        let result = run_check(&["This feature is deprecated since v3"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_clean_instruction_no_diagnostic() {
        let result = run_check(&["Use the new API for all requests."]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_in_code_block_skipped() {
        let result = run_check(&["```", "After March 2025, use new API", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_as_of_detected() {
        let result = run_check(&["As of January 2026, use the v2 endpoint"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_descriptive_snapshot_skipped() {
        let result = run_check(&["Current runway: 20 months (until September 2025)"]);
        assert!(
            result.diagnostics.is_empty(),
            "Descriptive status snapshots should not trigger stale-reference"
        );
    }

    #[test]
    fn test_deprecated_in_detected() {
        let result = run_check(&["This was deprecated in the last release"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_deprecated_in_favor_not_detected() {
        let result = run_check(&["deprecated in favor of the new API"]);
        assert!(
            result.diagnostics.is_empty(),
            "\"deprecated in favor of\" is a permanent statement, should not flag"
        );
    }

    #[test]
    fn test_deprecated_in_lieu_not_detected() {
        let result = run_check(&["deprecated in lieu of the v2 endpoint"]);
        assert!(
            result.diagnostics.is_empty(),
            "\"deprecated in lieu of\" is a permanent statement, should not flag"
        );
    }

    #[test]
    fn test_deprecated_in_preference_not_detected() {
        let result = run_check(&["deprecated in preference to the new system"]);
        assert!(
            result.diagnostics.is_empty(),
            "\"deprecated in preference to\" is a permanent statement, should not flag"
        );
    }

    #[test]
    fn test_inline_code_skipped() {
        let result = run_check(&[
            "- Test JSON output: `node build/cli.js --dir . --since 2024-01-01 --json`",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Dates inside inline backtick code should not trigger stale-reference"
        );
    }

    #[test]
    fn test_deprecated_in_v3_still_detected() {
        let result = run_check(&["deprecated in v3.2"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "\"deprecated in v3.2\" is time-sensitive, should still flag"
        );
    }
}
