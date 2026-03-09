use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::types::ParsedFile;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct ContextWindowWasteChecker {
    scope: ScopeFilter,
}

impl ContextWindowWasteChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Decorative divider: 4+ repeated decoration characters (not `---` which is valid HR).
static DECORATIVE_DIVIDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^[>\s]*(?:",
        r"[=]{4,}",         // ====
        r"|[*]{4,}",        // ****
        r"|[~]{4,}",        // ~~~~
        r"|[+]{4,}",        // ++++
        r"|[#]{4,}",        // #### (without space = not a heading)
        r"|[\u{2550}]{4,}", // ════
        r"|[\u{2500}]{4,}", // ────
        r"|[\u{2501}]{4,}", // ━━━━
        r"|[\u{2015}]{4,}", // ――――
        r"|[-]{4,}",        // ---- (4+, longer than HR's ---)
        r")\s*$",
    ))
    .unwrap()
});

/// Decorative HTML comment: <!-- followed by mostly decoration chars -->
static DECORATIVE_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*<!--\s*[-=~*─━═]{3,}\s*-->").unwrap());

impl Checker for ContextWindowWasteChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "context-window-waste",
            description: "Flags decorative elements that waste context window tokens",
            default_severity: Severity::Info,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            check_blank_lines(file, &mut result);
            check_decorative_dividers(file, &mut result);
            check_decorative_comments(file, &mut result);
        }

        result
    }
}

fn check_blank_lines(file: &ParsedFile, result: &mut CheckResult) {
    let mut consecutive_blanks = 0;
    let mut run_start = 0;

    for (i, line) in file.non_code_lines() {
        if line.trim().is_empty() {
            if consecutive_blanks == 0 {
                run_start = i;
            }
            consecutive_blanks += 1;
        } else {
            if consecutive_blanks >= 3 {
                emit!(
                    result,
                    file.path,
                    run_start + 1,
                    Severity::Info,
                    Category::ContextWindowWaste,
                    suggest: "Reduce to a single blank line",
                    "{} consecutive blank lines waste context window tokens",
                    consecutive_blanks
                );
            }
            consecutive_blanks = 0;
        }
    }

    // Handle trailing blank lines
    if consecutive_blanks >= 3 {
        emit!(
            result,
            file.path,
            run_start + 1,
            Severity::Info,
            Category::ContextWindowWaste,
            suggest: "Reduce to a single blank line",
            "{} consecutive blank lines waste context window tokens",
            consecutive_blanks
        );
    }
}

fn check_decorative_dividers(file: &ParsedFile, result: &mut CheckResult) {
    for (idx, line) in file.non_code_lines() {
        let trimmed = line.trim();

        // Skip standard markdown HR (exactly "---" or "***" or "___")
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            continue;
        }

        if DECORATIVE_DIVIDER.is_match(trimmed) {
            emit!(
                result,
                file.path,
                idx + 1,
                Severity::Info,
                Category::ContextWindowWaste,
                suggest: "Remove decorative divider — use markdown headings for structure",
                "Decorative divider wastes context window tokens"
            );
        }
    }
}

fn check_decorative_comments(file: &ParsedFile, result: &mut CheckResult) {
    for (idx, line) in file.non_code_lines() {
        if DECORATIVE_COMMENT.is_match(line) {
            emit!(
                result,
                file.path,
                idx + 1,
                Severity::Info,
                Category::ContextWindowWaste,
                suggest: "Remove decorative comment",
                "Decorative HTML comment wastes context window tokens"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::{count_matching, single_file_ctx};

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        ContextWindowWasteChecker::new(&[]).check(&ctx)
    }

    // ── Blank lines ──

    #[test]
    fn test_three_consecutive_blanks_flagged() {
        let result = run_check(&["# Title", "", "", "", "Content"]);
        assert_eq!(count_matching(&result, "consecutive blank lines"), 1);
    }

    #[test]
    fn test_two_consecutive_blanks_no_flag() {
        let result = run_check(&["# Title", "", "", "Content"]);
        assert_eq!(count_matching(&result, "consecutive blank lines"), 0);
    }

    #[test]
    fn test_many_consecutive_blanks() {
        let result = run_check(&["# Title", "", "", "", "", "", "Content"]);
        assert_eq!(count_matching(&result, "consecutive blank lines"), 1);
        assert!(result.diagnostics[0].message.contains('5'));
    }

    #[test]
    fn test_blanks_in_code_block_no_flag() {
        let result = run_check(&["```", "", "", "", "", "```"]);
        assert_eq!(count_matching(&result, "consecutive blank lines"), 0);
    }

    #[test]
    fn test_trailing_blanks_flagged() {
        let result = run_check(&["Content", "", "", ""]);
        assert_eq!(count_matching(&result, "consecutive blank lines"), 1);
    }

    // ── Decorative dividers ──

    #[test]
    fn test_equals_divider_flagged() {
        let result = run_check(&["======"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 1);
    }

    #[test]
    fn test_stars_divider_flagged() {
        let result = run_check(&["******"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 1);
    }

    #[test]
    fn test_tildes_divider_flagged() {
        let result = run_check(&["~~~~~~"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 1);
    }

    #[test]
    fn test_long_dashes_flagged() {
        let result = run_check(&["--------"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 1);
    }

    #[test]
    fn test_unicode_box_drawing_flagged() {
        let result = run_check(&["════════════"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 1);
    }

    #[test]
    fn test_unicode_thin_line_flagged() {
        let result = run_check(&["────────────"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 1);
    }

    #[test]
    fn test_unicode_thick_line_flagged() {
        let result = run_check(&["━━━━━━━━━━━━"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 1);
    }

    #[test]
    fn test_standard_hr_not_flagged() {
        let result = run_check(&["---"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_standard_hr_stars_not_flagged() {
        let result = run_check(&["***"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_divider_in_code_block_not_flagged() {
        let result = run_check(&["```", "========", "```"]);
        assert_eq!(count_matching(&result, "Decorative divider"), 0);
    }

    // ── Decorative comments ──

    #[test]
    fn test_decorative_comment_dashes_flagged() {
        let result = run_check(&["<!-- ----- -->"]);
        assert_eq!(count_matching(&result, "Decorative HTML comment"), 1);
    }

    #[test]
    fn test_decorative_comment_equals_flagged() {
        let result = run_check(&["<!-- ===== -->"]);
        assert_eq!(count_matching(&result, "Decorative HTML comment"), 1);
    }

    #[test]
    fn test_decorative_comment_unicode_flagged() {
        let result = run_check(&["<!-- ──────── -->"]);
        assert_eq!(count_matching(&result, "Decorative HTML comment"), 1);
    }

    #[test]
    fn test_spectralint_comment_not_flagged() {
        let result = run_check(&["<!-- spectralint-disable dead-reference -->"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_real_comment_not_flagged() {
        let result = run_check(&["<!-- This is a real comment about the section -->"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_decorative_comment_in_code_block_not_flagged() {
        let result = run_check(&["```html", "<!-- ----- -->", "```"]);
        assert_eq!(count_matching(&result, "Decorative HTML comment"), 0);
    }

    // ── Mixed ──

    #[test]
    fn test_multiple_waste_types() {
        let result = run_check(&[
            "# Title",
            "",
            "",
            "",
            "========",
            "<!-- ----- -->",
            "Content",
        ]);
        assert!(result.diagnostics.len() >= 3);
    }

    #[test]
    fn test_clean_file_no_flags() {
        let result = run_check(&[
            "# Title",
            "",
            "## Section",
            "",
            "Some content here.",
            "",
            "---",
            "",
            "## Another Section",
        ]);
        assert!(result.diagnostics.is_empty());
    }
}
