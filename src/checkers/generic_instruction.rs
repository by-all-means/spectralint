use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{has_elaboration_after, is_heading, ScopeFilter};
use super::Checker;

pub(crate) struct GenericInstructionChecker {
    scope: ScopeFilter,
}

impl GenericInstructionChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static GENERIC_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:",
        r"follow\s+best\s+practices?",
        r"|write\s+clean\s+code",
        r"|write\s+(?:good|quality|readable|maintainable)\s+code",
        r"|think\s+(?:step[- ]by[- ]step|carefully)",
        r"|be\s+(?:helpful|accurate|thorough)\s+and\s+(?:helpful|accurate|thorough)",
        r"|use\s+(?:common\s+sense|good\s+(?:judgment|judgement))",
        r"|pay\s+(?:close\s+)?attention\s+to\s+detail",
        r"|ensure\s+(?:code\s+)?quality",
        r"|produce\s+high[- ]quality",
        r"|strive\s+for\s+excellence",
        r")\b",
    ))
    .unwrap()
});

impl Checker for GenericInstructionChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                let line_num = idx + 1;

                if is_heading(line) || !is_directive_line(line) {
                    continue;
                }

                if let Some(m) = GENERIC_PATTERN.find(line) {
                    if !has_elaboration_after(line, m.end()) {
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
