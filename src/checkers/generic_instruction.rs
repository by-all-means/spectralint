use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{has_elaboration_after, is_heading, is_reasoning_prompt, ScopeFilter};
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
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "generic-instruction",
            description: "Flags meaningless instructions the model already knows",
            default_severity: Severity::Info,
            strict_only: true,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Skip reasoning/workflow agent prompts (no code blocks, no file
            // references). These are domain-specific prose files where
            // spectralint lacks context to judge instruction value.
            if is_reasoning_prompt(file) {
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

    /// Run the checker on the given lines, prepending a code block so the file
    /// is not classified as a reasoning prompt (which would skip the check).
    fn run_check(lines: &[&str]) -> CheckResult {
        let mut all_lines: Vec<&str> = vec!["```bash", "echo hello", "```"];
        all_lines.extend_from_slice(lines);
        let (_dir, ctx) = single_file_ctx(&all_lines);
        GenericInstructionChecker::new(&[]).check(&ctx)
    }

    /// Run the checker on raw lines without any code block prefix.
    fn run_check_raw(lines: &[&str]) -> CheckResult {
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

    #[test]
    fn test_reasoning_agent_prompt_not_flagged() {
        // A pure prose reasoning/workflow agent prompt: no code blocks,
        // no file references. spectralint lacks context to judge whether
        // instructions are generic in this domain, so it should skip the file.
        let result = run_check_raw(&[
            "# Legal Document Review Agent",
            "",
            "You are a legal document review specialist.",
            "",
            "## Core Principles",
            "",
            "- Think step by step when analyzing contracts",
            "- Follow best practices for due diligence",
            "- Be helpful and accurate in your assessments",
            "- Pay attention to detail in every clause",
            "",
            "## Workflow",
            "",
            "- Review each section of the contract carefully",
            "- Identify potential risks and liabilities",
            "- Produce high-quality summaries for stakeholders",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Reasoning agent prompt (no code blocks, no file refs) should not be flagged"
        );
    }

    #[test]
    fn test_file_with_code_block_still_flagged() {
        // A file with code blocks is not a reasoning prompt, so generic
        // instructions should still be flagged.
        let result = run_check_raw(&[
            "# Build Guide",
            "",
            "```bash",
            "cargo build",
            "```",
            "",
            "- Follow best practices",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "File with code blocks should still flag generic instructions"
        );
    }
}
