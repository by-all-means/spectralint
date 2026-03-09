use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{count_directive_lines, ScopeFilter, MIN_DIRECTIVE_LINES};
use super::Checker;

pub(crate) struct UnboundedScopeChecker {
    scope: ScopeFilter,
}

impl UnboundedScopeChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Capability-granting patterns.
static CAPABILITY_GRANT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)\b(?:
            you\s+can\s+(?:modify|delete|remove|execute|create|update|write|access)\b
            | full\s+(?:write\s+)?access
            | unrestricted\s+access
            | unlimited\s+(?:access|permissions?)
            | freely\s+(?:modify|delete|create|execute)
            | modify\s+any(?:thing|\s+files?)
            | delete\s+any(?:thing|\s+files?)
            | execute\s+any(?:thing|\s+commands?)
        )",
    )
    .unwrap()
});

/// Refusal/boundary patterns that constrain capabilities.
static BOUNDARY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)\b(?:
            refuse | decline | reject
            | never\s+(?:modify|delete|remove|execute|create)
            | do\s+not\s+(?:modify|delete|remove|execute|create)
            | don'?t\s+(?:modify|delete|remove|execute|create)
            | out\s+of\s+scope
            | off[\s-]limits
            | ask\s+(?:for\s+)?confirm
            | must\s+not | prohibited | forbidden
            | restricted\s+to
            | only\s+(?:modify|access|touch)\b
        )",
    )
    .unwrap()
});

impl Checker for UnboundedScopeChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "unbounded-scope",
            description: "Detects capability grants without boundary constraints",
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

            if count_directive_lines(&file.raw_lines, &file.in_code_block) < MIN_DIRECTIVE_LINES {
                continue;
            }

            let mut has_capability = false;
            let mut has_boundary = false;

            for (_, line) in file.non_code_lines() {
                if CAPABILITY_GRANT.is_match(line) {
                    has_capability = true;
                }
                if BOUNDARY_PATTERN.is_match(line) {
                    has_boundary = true;
                }
                if has_capability && has_boundary {
                    break;
                }
            }

            if has_capability && !has_boundary {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::UnboundedScope,
                    suggest: "Add boundary constraints (\"never modify...\", \"out of scope\", \"ask for confirmation before...\")",
                    "File grants capabilities (modify/delete/execute/access) but defines no \
                     boundaries or refusal conditions. Unbounded agents are unpredictable."
                );
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
        UnboundedScopeChecker::new(&[]).check(&ctx)
    }

    fn base_directives() -> Vec<&'static str> {
        vec![
            "Always run tests.",
            "Follow the style guide.",
            "Use structured logging.",
            "Run linting before commit.",
            "Keep functions small.",
        ]
    }

    #[test]
    fn test_capability_without_boundary_flags() {
        let mut lines = base_directives();
        lines.push("You can modify any files in the project.");
        let result = run_check(&lines);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::UnboundedScope);
    }

    #[test]
    fn test_capability_with_boundary_passes() {
        let mut lines = base_directives();
        lines.push("You can modify any files in the project.");
        lines.push("Never modify files in the vendor/ directory.");
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_full_access_without_boundary_flags() {
        let mut lines = base_directives();
        lines.push("You have full write access to the codebase.");
        let result = run_check(&lines);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_unrestricted_with_confirmation_passes() {
        let mut lines = base_directives();
        lines.push("You have unrestricted access to all files.");
        lines.push("Ask for confirmation before destructive operations.");
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_capability_no_flag() {
        let result = run_check(&base_directives());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_too_few_directives_skipped() {
        let result = run_check(&["You can delete anything.", "Run tests."]);
        assert!(
            result.diagnostics.is_empty(),
            "Files with < 5 directive lines should be skipped"
        );
    }

    #[test]
    fn test_out_of_scope_boundary_passes() {
        let mut lines = base_directives();
        lines.push("You can execute any commands.");
        lines.push("Database operations are out of scope.");
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_capability_in_code_block_not_counted() {
        let mut lines = base_directives();
        lines.extend_from_slice(&["```", "You can modify any files.", "```"]);
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "Capability in code block should not trigger"
        );
    }

    #[test]
    fn test_delete_anything_flags() {
        let mut lines = base_directives();
        lines.push("Freely delete temporary files.");
        let result = run_check(&lines);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
