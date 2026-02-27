use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::{is_directive_line, non_code_lines};
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

pub struct GenericInstructionChecker {
    scope: ScopeFilter,
}

impl GenericInstructionChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static GENERIC_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\bfollow\s+best\s+practices?\b",
        r"(?i)\bwrite\s+clean\s+code\b",
        r"(?i)\bwrite\s+(?:good|quality|readable|maintainable)\s+code\b",
        r"(?i)\bthink\s+(?:step[- ]by[- ]step|carefully)\b",
        r"(?i)\bbe\s+(?:helpful|accurate|thorough)\s+and\s+(?:helpful|accurate|thorough)\b",
        r"(?i)\buse\s+(?:common\s+sense|good\s+(?:judgment|judgement))\b",
        r"(?i)\bpay\s+(?:close\s+)?attention\s+to\s+detail\b",
        r"(?i)\bensure\s+(?:code\s+)?quality\b",
        r"(?i)\bproduce\s+high[- ]quality\b",
        r"(?i)\bstrive\s+for\s+excellence\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

/// Check if the matched phrase is followed by elaboration (`:` or ` —` or ` -`).
fn has_elaboration_after(line: &str, match_end: usize) -> bool {
    let rest = line[match_end..].trim_start();
    rest.starts_with(':')
        || rest.starts_with("—")
        || rest.starts_with("- ")
        || rest.starts_with("– ")
}

impl Checker for GenericInstructionChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in non_code_lines(&file.raw_lines) {
                let line_num = idx + 1;

                if is_heading(line) || !is_directive_line(line) {
                    continue;
                }

                for pattern in GENERIC_PATTERNS.iter() {
                    if let Some(m) = pattern.find(line) {
                        // Skip if elaboration follows
                        if has_elaboration_after(line, m.end()) {
                            continue;
                        }

                        emit!(
                            result,
                            file.path,
                            line_num,
                            Severity::Info,
                            Category::GenericInstruction,
                            suggest: "Remove generic instructions — the model already knows this. Replace with specific, actionable rules.",
                            "Generic instruction: \"{}\"",
                            m.as_str()
                        );
                        break; // One flag per line
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
        GenericInstructionChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_follow_best_practices() {
        let result = run_check(&["- Follow best practices"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .message
            .contains("Follow best practices"));
    }

    #[test]
    fn test_write_clean_code() {
        let result = run_check(&["- Write clean code"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_think_step_by_step() {
        let result = run_check(&["- Think step by step"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_elaboration_with_colon_no_flag() {
        let result = run_check(&["- Follow best practices: use snake_case for variables"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_heading_no_flag() {
        let result = run_check(&["## Follow Best Practices"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_no_flag() {
        let result = run_check(&["```", "Write clean code", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_specific_action_no_flag() {
        let result = run_check(&["- Always run tests before committing"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_elaboration_with_dash_no_flag() {
        let result = run_check(&["- Follow best practices — use consistent naming"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_ensure_quality() {
        let result = run_check(&["- Ensure code quality"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_clean_file_no_flag() {
        let result = run_check(&[
            "# Build",
            "- Run `cargo test` before committing",
            "- Use snake_case for all variable names",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_write_good_code() {
        let result = run_check(&["- Write good code"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_write_maintainable_code() {
        let result = run_check(&["- Write maintainable code"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_use_common_sense() {
        let result = run_check(&["- Use common sense"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_use_good_judgment() {
        let result = run_check(&["- Use good judgment"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_pay_attention_to_detail() {
        let result = run_check(&["- Pay attention to detail"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_produce_high_quality() {
        let result = run_check(&["- Produce high-quality output"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_strive_for_excellence() {
        let result = run_check(&["- Strive for excellence"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_be_helpful_and_accurate() {
        let result = run_check(&["- Be helpful and accurate"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_two_patterns_one_line_single_flag() {
        let result = run_check(&["- Follow best practices and write clean code"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Two generic patterns on one line should produce only one diagnostic"
        );
    }

    #[test]
    fn test_elaboration_with_hyphen_space_no_flag() {
        let result = run_check(&["- Follow best practices - use consistent naming conventions"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_blockquote_no_flag() {
        let result = run_check(&["> Follow best practices"]);
        assert!(
            result.diagnostics.is_empty(),
            "Generic instruction in blockquote should not flag"
        );
    }
}
