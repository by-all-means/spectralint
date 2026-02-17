use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct PlaceholderTextChecker {
    scope: ScopeFilter,
}

impl PlaceholderTextChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static PLACEHOLDER_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\[TODO\]",
        r"(?i)\[TBD\]",
        r"(?i)\[FIXME\]",
        r"(?i)\[insert .+?\]",
        r"(?i)\betc\.?(?:\s|$)",
        r"(?i)\band so on\b",
        r"\.{3,}\s*$",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

/// Matches a proper enumeration before "etc." — 2+ comma-separated items.
/// Uses `[^,]+` instead of `\w+` to handle multi-word items ("code style")
/// and special characters (`@gala-chain/api`, `Optional[...]`).
static ENUMERATION_BEFORE_ETC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:[^,]+,\s*){2,}.*\betc\.?").unwrap());

/// Matches "or"-separated enumerations: "X or Y or Z etc"
static OR_ENUMERATION_BEFORE_ETC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:\w+\s+or\s+){2,}\w+\s+etc\.?").unwrap());

/// Returns true if this match is an "etc." that follows a proper enumeration.
/// `prev_line` is used to detect enumerations that wrap across lines.
fn is_etc_after_enumeration(line: &str, matched: &str, prev_line: Option<&str>) -> bool {
    let trimmed = matched.trim();
    if !trimmed
        .get(..3)
        .is_some_and(|s| s.eq_ignore_ascii_case("etc"))
    {
        return false;
    }
    if ENUMERATION_BEFORE_ETC.is_match(line) || OR_ENUMERATION_BEFORE_ETC.is_match(line) {
        return true;
    }
    // Handle wrapped lines: "analysis or review or\n      debugging etc"
    if let Some(prev) = prev_line {
        let combined = format!("{} {}", prev.trim(), line.trim());
        if ENUMERATION_BEFORE_ETC.is_match(&combined)
            || OR_ENUMERATION_BEFORE_ETC.is_match(&combined)
        {
            return true;
        }
    }
    false
}

impl Checker for PlaceholderTextChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in non_code_lines(&file.raw_lines) {
                let prev_line = i
                    .checked_sub(1)
                    .and_then(|idx| file.raw_lines.get(idx))
                    .map(|s| s.as_str());
                for pat in PLACEHOLDER_PATTERNS.iter() {
                    if let Some(m) = pat.find(line) {
                        if is_etc_after_enumeration(line, m.as_str(), prev_line) {
                            continue;
                        }
                        emit!(
                            result,
                            file.path,
                            i + 1,
                            Severity::Warning,
                            Category::PlaceholderText,
                            suggest: "Replace placeholder with actual content",
                            "Placeholder text found: \"{}\"",
                            m.as_str().trim()
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
        PlaceholderTextChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_todo_detected() {
        let result = run_check(&["# Title", "[TODO] implement this"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_tbd_detected() {
        let result = run_check(&["[TBD] needs review"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_fixme_detected() {
        let result = run_check(&["[FIXME] broken"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_insert_detected() {
        let result = run_check(&["[insert your name here]"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_etc_after_short_list_detected() {
        // Only 1 item before etc. — not a proper enumeration
        let result = run_check(&["tools, etc."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_etc_after_proper_enumeration_skipped() {
        // 2+ items before etc. — proper enumeration, should not flag
        let result = run_check(&["Use tools like grep, find, sed, etc."]);
        assert!(
            result.diagnostics.is_empty(),
            "etc. after a proper enumeration (2+ items) should not flag"
        );
    }

    #[test]
    fn test_etc_after_multi_word_items_skipped() {
        // Multi-word items: "code style, testing style, etc"
        let result = run_check(&["Use existing code style, testing style, etc"]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after multi-word enumeration should not flag"
        );
    }

    #[test]
    fn test_etc_after_special_char_items_skipped() {
        // Package names with special chars
        let result = run_check(&["Import from `@gala-chain/api`, `@gala-chain/client`, etc."]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after enumeration with special chars should not flag"
        );
    }

    #[test]
    fn test_etc_after_bracket_items_skipped() {
        // Items with brackets: Optional[...], Union[...], etc.
        let result = run_check(&[
            "Use `|` instead of `Union[...]`, `type | None` instead of `Optional[...]`, etc.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after enumeration with brackets should not flag"
        );
    }

    #[test]
    fn test_etc_after_or_enumeration_skipped() {
        let result = run_check(&["perform some analysis or review or debugging etc"]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after or-separated enumeration (2+ items) should not flag"
        );
    }

    #[test]
    fn test_etc_after_wrapped_or_enumeration_skipped() {
        let result = run_check(&[
            "    - Launch another instance to perform some analysis or review or",
            "      debugging etc",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after or-enumeration wrapped across lines should not flag"
        );
    }

    #[test]
    fn test_etc_without_enumeration_detected() {
        let result = run_check(&["configure everything etc."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_and_so_on_detected() {
        let result = run_check(&["Use grep, find, and so on"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_ellipsis_detected() {
        let result = run_check(&["Do something..."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_clean_file_no_diagnostics() {
        let result = run_check(&["# Title", "Run the tests."]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_in_code_block_skipped() {
        let result = run_check(&["```", "[TODO] implement", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_placeholders() {
        let result = run_check(&["[TODO] first", "[TBD] second"]);
        assert_eq!(result.diagnostics.len(), 2);
    }
}
