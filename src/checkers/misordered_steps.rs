use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

pub struct MisorderedStepsChecker {
    scope: ScopeFilter,
}

impl MisorderedStepsChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static STEP_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|\b)step\s*(\d{1,3})\b").unwrap());

impl Checker for MisorderedStepsChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut prev_step: Option<(usize, u32)> = None; // (line, number)

            for (idx, line) in file.non_code_lines() {
                let line_num = idx + 1;

                if is_heading(line) {
                    prev_step = None;
                    continue;
                }

                if let Some(caps) = STEP_PATTERN.captures(line) {
                    if let Ok(num) = caps[1].parse::<u32>() {
                        if let Some((prev_line, prev_num)) = prev_step {
                            // Step 1 resets a sequence (new enumeration)
                            if num != 1 && num < prev_num {
                                emit!(
                                    result,
                                    file.path,
                                    line_num,
                                    Severity::Warning,
                                    Category::MisorderedSteps,
                                    suggest: "Reorder steps to be sequential — agents execute steps in document order",
                                    "Step {} appears after step {} (line {})",
                                    num,
                                    prev_num,
                                    prev_line
                                );
                            }
                        }
                        prev_step = Some((line_num, num));
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
        MisorderedStepsChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_misordered_steps_detected() {
        let result = run_check(&[
            "# Setup",
            "Step 1: Install deps",
            "Step 3: Run tests",
            "Step 2: Configure",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Step 2"));
        assert!(result.diagnostics[0].message.contains("after step 3"));
    }

    #[test]
    fn test_ordered_steps_no_flag() {
        let result = run_check(&[
            "# Setup",
            "Step 1: Install",
            "Step 2: Configure",
            "Step 3: Run",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_steps_reset_at_new_section() {
        let result = run_check(&[
            "# Build",
            "Step 1: First",
            "Step 5: Last",
            "# Deploy",
            "Step 1: First deploy step",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_ignored() {
        let result = run_check(&[
            "# Steps",
            "Step 1: Do this",
            "```",
            "Step 5: in code",
            "Step 2: in code",
            "```",
            "Step 2: Continue",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_step_1_resets_sequence() {
        let result = run_check(&[
            "# Guide",
            "Step 1: First thing",
            "Step 5: Big jump",
            "Step 1: Start over",
        ]);
        // Step 1 after Step 5 should not flag (resets sequence)
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_steps_no_flag() {
        let result = run_check(&["# Build", "Run cargo build", "Run cargo test"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_case_insensitive() {
        let result = run_check(&[
            "# Setup",
            "STEP 1: Install",
            "step 3: Run tests",
            "Step 2: Configure",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Step 2"));
    }

    #[test]
    fn test_forward_gap_allowed() {
        let result = run_check(&["# Guide", "Step 1: Start", "Step 5: Middle", "Step 10: End"]);
        assert!(
            result.diagnostics.is_empty(),
            "Forward gaps should be allowed"
        );
    }

    #[test]
    fn test_multiple_misordered_steps() {
        let result = run_check(&[
            "# Setup",
            "Step 1: First",
            "Step 5: Fifth",
            "Step 3: Third",
            "Step 2: Second",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            2,
            "Both Step 3 and Step 2 should flag"
        );
    }

    #[test]
    fn test_step_without_space() {
        let result = run_check(&["# Guide", "Step1: First", "Step3: Third", "Step2: Second"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Step without space should still match"
        );
    }

    #[test]
    fn test_step_number_over_999_no_match() {
        let result = run_check(&[
            "# Guide",
            "Step 1: First",
            "Step 1000: Big number",
            "Step 2: Second",
        ]);
        // Step 1000 doesn't match the regex, so Step 2 after Step 1 is fine
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_blank_lines_between_steps() {
        let result = run_check(&[
            "# Guide",
            "Step 1: First",
            "",
            "",
            "Step 3: Third",
            "",
            "Step 2: Second",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
