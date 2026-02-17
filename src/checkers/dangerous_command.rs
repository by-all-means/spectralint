use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::code_block_lines;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct DangerousCommandChecker {
    scope: ScopeFilter,
}

impl DangerousCommandChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static IF_EXISTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bIF\s+EXISTS\b").unwrap());

static DANGEROUS_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"\brm\s+.*-[^\s]*r[^\s]*f", "rm -rf"),
        (r"\bgit\s+push\s+.*--force", "git push --force"),
        (r"\bgit\s+push\s+-f\b", "git push -f"),
        (r"\bgit\s+reset\s+--hard", "git reset --hard"),
        (r"\bgit\s+clean\s+-[^\s]*f", "git clean -f"),
        // Match DROP TABLE/DATABASE - note: filtering of "IF EXISTS" happens in the checker
        (r"(?i)\bDROP\s+(?:TABLE|DATABASE)\b", "DROP TABLE/DATABASE"),
        (r"(?i)\bTRUNCATE\s+TABLE\b", "TRUNCATE TABLE"),
        (r"--no-verify\b", "--no-verify"),
    ]
    .iter()
    .map(|(p, label)| (Regex::new(p).unwrap(), *label))
    .collect()
});

impl Checker for DangerousCommandChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in code_block_lines(&file.raw_lines) {
                for (pat, label) in DANGEROUS_PATTERNS.iter() {
                    if pat.is_match(line) {
                        // Special case: DROP TABLE/DATABASE with IF EXISTS is defensive and safe
                        if label.contains("DROP") && IF_EXISTS.is_match(line) {
                            continue;
                        }

                        emit!(
                            result,
                            file.path,
                            i + 1,
                            Severity::Warning,
                            Category::DangerousCommand,
                            suggest: "Add a confirmation step or restrict when this command may be used",
                            "Dangerous command in code block: {}",
                            label
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
        DangerousCommandChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_rm_rf_in_code_block() {
        let result = run_check(&["```", "rm -rf /", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert!(result.diagnostics[0].message.contains("rm -rf"));
    }

    #[test]
    fn test_rm_rf_outside_code_block_ignored() {
        let result = run_check(&["Never run rm -rf /"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_git_push_force() {
        let result = run_check(&["```", "git push --force", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_git_push_f() {
        let result = run_check(&["```", "git push -f", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_git_reset_hard() {
        let result = run_check(&["```", "git reset --hard HEAD~1", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_git_clean_f() {
        let result = run_check(&["```", "git clean -fd", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_drop_table() {
        let result = run_check(&["```sql", "DROP TABLE users;", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_truncate_table() {
        let result = run_check(&["```sql", "TRUNCATE TABLE logs;", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_no_verify() {
        let result = run_check(&["```", "git commit --no-verify", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_drop_table_if_exists_skipped() {
        let result = run_check(&["```sql", "DROP TABLE IF EXISTS old_table;", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "DROP TABLE IF EXISTS is idempotent and should not be flagged"
        );
    }

    #[test]
    fn test_drop_database_if_exists_skipped() {
        let result = run_check(&["```sql", "DROP DATABASE IF EXISTS test_db;", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "DROP DATABASE IF EXISTS is idempotent and should not be flagged"
        );
    }

    #[test]
    fn test_safe_command_no_diagnostic() {
        let result = run_check(&["```", "git push origin main", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_code_blocks_no_diagnostic() {
        let result = run_check(&["# Commands", "Run git push to deploy."]);
        assert!(result.diagnostics.is_empty());
    }
}
