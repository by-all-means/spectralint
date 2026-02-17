use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

/// Check whether byte offset `pos` falls inside an inline backtick span.
fn inside_inline_code(line: &str, pos: usize) -> bool {
    line[..pos].chars().filter(|&c| c == '`').count() % 2 == 1
}

pub struct StaleReferenceChecker {
    scope: ScopeFilter,
}

impl StaleReferenceChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// "deprecated in favor/lieu/preference" is a permanent statement, not time-sensitive.
static DEPRECATED_PERMANENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bdeprecated\s+in\s+(?:favor|lieu|preference)\b").unwrap());

static STALE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\b(?:before|after|until|since|as of)\s+(?:january|february|march|april|may|june|july|august|september|october|november|december)\s+20\d{2}",
        r"(?i)\b(?:before|after|until|since|as of)\s+20\d{2}",
        r"(?i)\b(?:before|after|until|since|as of)\s+\d{1,2}/\d{1,2}/\d{2,4}",
        r"(?i)\bif\b.*\byear\b.*\b20\d{2}\b",
        r"(?i)\bdeprecated\s+(?:in|since|after)\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

impl Checker for StaleReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in non_code_lines(&file.raw_lines) {
                // Skip "deprecated in favor/lieu/preference" — permanent statements
                if DEPRECATED_PERMANENT.is_match(line) {
                    continue;
                }

                for pat in STALE_PATTERNS.iter() {
                    if let Some(m) = pat.find(line) {
                        // Skip matches inside inline backtick code (e.g., `--since 2024-01-01`)
                        if inside_inline_code(line, m.start()) {
                            continue;
                        }
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
                        break;
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
        let result = run_check(&["Before 2024 this was different"]);
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
        let result = run_check(&["As of January 2026, the old endpoint is gone"]);
        assert_eq!(result.diagnostics.len(), 1);
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
