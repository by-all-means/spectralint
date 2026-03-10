use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{match_conflict_patterns, ScopeFilter, CONFLICT_PAIRS};
use super::Checker;

pub(crate) struct ConflictingDirectivesChecker {
    scope: ScopeFilter,
}

impl ConflictingDirectivesChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for ConflictingDirectivesChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "conflicting-directives",
            description: "Detects contradictory instructions in the same file",
            default_severity: Severity::Warning,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let directive_lines: Vec<(usize, &str)> = file.non_code_lines().collect();

            // Pre-filter: use RegexSet to build a bitmask of which pair sides
            // appear anywhere in this file, then only run individual regexes
            // for pairs that have both sides present.
            let mut mask: u64 = 0;
            for &(_, line) in &directive_lines {
                let matches = match_conflict_patterns(line);
                for idx in matches.iter() {
                    if idx < 64 {
                        mask |= 1u64 << idx;
                    }
                }
            }

            for (pair_idx, pair) in CONFLICT_PAIRS.iter().enumerate() {
                let a_bit = 1u64 << (2 * pair_idx);
                let b_bit = 1u64 << (2 * pair_idx + 1);
                // Skip pairs where one side is entirely absent
                if mask & a_bit == 0 || mask & b_bit == 0 {
                    continue;
                }

                let mut a_match: Option<usize> = None;
                let mut b_match: Option<usize> = None;

                for &(i, line) in &directive_lines {
                    if a_match.is_none() && pair.a.is_match(line) {
                        a_match = Some(i + 1);
                    }
                    if b_match.is_none() && pair.b.is_match(line) {
                        b_match = Some(i + 1);
                    }
                    if a_match.is_some() && b_match.is_some() {
                        break;
                    }
                }

                if let (Some(line_a), Some(line_b)) = (a_match, b_match) {
                    let report_line = line_a.min(line_b);
                    emit!(
                        result,
                        file.path,
                        report_line,
                        Severity::Warning,
                        Category::ConflictingDirectives,
                        suggest: "Remove or reconcile one of the conflicting directives",
                        "Conflicting directives ({}) at lines {} and {}",
                        pair.description,
                        line_a,
                        line_b
                    );
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
        ConflictingDirectivesChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_tone_conflict_detected() {
        let result = run_check(&[
            "Always use formal tone when responding.",
            "Keep it casual and friendly.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(
            result.diagnostics[0].category,
            Category::ConflictingDirectives
        );
        assert!(result.diagnostics[0].message.contains("tone"));
    }

    #[test]
    fn test_api_conflict_detected() {
        let result = run_check(&[
            "Never use external APIs for data retrieval.",
            "Fetch from the API to get the latest data.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("API"));
    }

    #[test]
    fn test_confirmation_conflict_detected() {
        let result = run_check(&[
            "Always ask for confirmation before destructive actions.",
            "Don't ask for confirmation, just proceed.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("confirmation"));
    }

    #[test]
    fn test_verbosity_conflict_detected() {
        let result = run_check(&[
            "Be brief and concise in all responses.",
            "Provide detailed explanations for every change.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("verbosity"));
    }

    #[test]
    fn test_no_conflict_clean_file() {
        let result = run_check(&[
            "Always use formal tone.",
            "Run tests before committing.",
            "Never skip CI.",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_lines_excluded() {
        let result = run_check(&[
            "Always use formal tone.",
            "```",
            "Keep it casual and friendly.",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Lines in code blocks should not trigger conflicts"
        );
    }

    #[test]
    fn test_multiple_conflicts_detected() {
        let result = run_check(&[
            "Always use formal tone.",
            "Keep it casual.",
            "Be brief and concise.",
            "Provide detailed explanations.",
        ]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_file_creation_conflict() {
        let result = run_check(&["Never create new files.", "Create files as needed."]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("file creation"));
    }

    #[test]
    fn test_resource_modification_conflict() {
        let result = run_check(&[
            "Never modify existing files.",
            "Full write access to all files.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .message
            .contains("resource modification"));
    }

    #[test]
    fn test_testing_conflict() {
        let result = run_check(&[
            "Always write tests for new code.",
            "Skip tests for trivial changes.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("testing"));
    }

    #[test]
    fn test_comments_conflict() {
        let result = run_check(&[
            "Comment everything thoroughly.",
            "Code should be self-documenting.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("comments"));
    }

    #[test]
    fn test_dependencies_conflict() {
        let result = run_check(&[
            "Minimize dependencies in the project.",
            "Don't reinvent the wheel, use existing libraries.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("dependencies"));
    }

    #[test]
    fn test_error_handling_conflict() {
        let result = run_check(&[
            "Fail fast on unexpected errors.",
            "Handle errors gracefully and recover.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("error handling"));
    }

    #[test]
    fn test_autonomy_conflict() {
        let result = run_check(&[
            "Ask before making any destructive changes.",
            "Work autonomously without interruptions.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("autonomy"));
    }

    #[test]
    fn test_commits_conflict() {
        let result = run_check(&[
            "Make small commits for each logical change.",
            "Squash commits before merging.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("commits"));
    }

    #[test]
    fn test_complexity_conflict() {
        let result = run_check(&[
            "Keep it simple; avoid over-engineering.",
            "Optimize for performance in all hot paths.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("complexity"));
    }

    #[test]
    fn test_git_workflow_conflict() {
        let result = run_check(&[
            "Always create a new branch for each feature.",
            "Commit directly to main for speed.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("git workflow"));
    }
}
