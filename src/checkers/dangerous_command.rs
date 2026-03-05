use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct DangerousCommandChecker {
    scope: ScopeFilter,
}

impl DangerousCommandChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static IF_EXISTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bIF\s+EXISTS\b").unwrap());

const FORCE_WITH_LEASE: &str = "--force-with-lease";

/// Common build/artifact directories where `rm -rf <dir>` is safe routine cleanup.
/// Only matches relative paths (with optional `.?/` prefix) to avoid matching system directories.
static SAFE_RM_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\brm\s+(?:-[^\s]*\s+)*(?:\.?/)?(?:build|dist|out|target|node_modules|\.cache|__pycache__|\.next|\.nuxt|coverage|\.tox|\.mypy_cache|\.pytest_cache|\.venv|venv|vendor)\b").unwrap()
});

static DANGEROUS_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"\brm\s+.*?-[^\s]*r[^\s]*f", "rm -rf"),
        (r"\bgit\s+push\s+.*?--force", "git push --force"),
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

            let mut seen_in_block: HashSet<&'static str> = HashSet::new();
            let mut prev_idx: Option<usize> = None;

            for (i, line) in file.code_block_lines() {
                // Reset per-block dedup when there's a gap in line indices
                // (gap means we left one code block and entered another).
                if prev_idx.is_some_and(|prev| i > prev + 1) {
                    seen_in_block.clear();
                }
                prev_idx = Some(i);

                let trimmed = line.trim_start();
                // Skip shell comments (#) and SQL comments (--)
                if trimmed.starts_with('#') || trimmed.starts_with("--") {
                    continue;
                }

                for (pat, label) in DANGEROUS_PATTERNS.iter() {
                    if pat.is_match(line) {
                        // Special case: DROP TABLE/DATABASE with IF EXISTS is defensive and safe
                        if label.contains("DROP") && IF_EXISTS.is_match(line) {
                            continue;
                        }

                        // Skip SQL keywords inside string literals (test payloads like SQL injection tests)
                        if label.contains("DROP") || label.contains("TRUNCATE") {
                            if let Some(m) = pat.find(line) {
                                let before = &line[..m.start()];
                                let in_double = before.matches('"').count() % 2 == 1;
                                let in_single = before.matches('\'').count() % 2 == 1;
                                if in_double || in_single {
                                    continue;
                                }
                            }
                        }

                        // Special case: --force-with-lease is the safe variant
                        if *label == "git push --force" && line.contains(FORCE_WITH_LEASE) {
                            continue;
                        }

                        // Special case: rm -rf targeting known build/artifact dirs is routine cleanup
                        if *label == "rm -rf" && SAFE_RM_TARGET.is_match(line) {
                            continue;
                        }

                        // Prevent noisy repetition in long blocks/lists of the same command.
                        if !seen_in_block.insert(label) {
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
    fn test_git_push_force_with_lease_skipped() {
        let result = run_check(&["```", "git push --force-with-lease", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "--force-with-lease is the safe variant and should not be flagged"
        );
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

    #[test]
    fn test_shell_comment_skipped() {
        let result = run_check(&["```bash", "# rm -rf /tmp/old", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Shell comments should not be flagged"
        );
    }

    #[test]
    fn test_sql_comment_skipped() {
        let result = run_check(&["```sql", "-- DROP TABLE users;", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "SQL comments should not be flagged"
        );
    }

    #[test]
    fn test_duplicate_in_same_block_warns_once() {
        let result = run_check(&["```bash", "rm -rf /tmp/old", "rm -rf /tmp/new", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Same command twice in one block should only warn once"
        );
    }

    #[test]
    fn test_duplicate_across_blocks_warns_per_block() {
        let result = run_check(&[
            "```bash",
            "rm -rf /tmp/old",
            "```",
            "Some text in between",
            "```bash",
            "rm -rf /tmp/new",
            "```",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            2,
            "Same command in separate blocks should warn in each block"
        );
    }

    #[test]
    fn test_rm_rf_build_dir_skipped() {
        let result = run_check(&["```bash", "rm -rf build", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "rm -rf targeting a build artifact directory should not be flagged"
        );
    }

    #[test]
    fn test_rm_rf_node_modules_skipped() {
        let result = run_check(&["```bash", "rm -rf node_modules", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "rm -rf targeting node_modules should not be flagged"
        );
    }

    #[test]
    fn test_rm_rf_dist_with_path_skipped() {
        let result = run_check(&["```bash", "rm -rf ./dist", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "rm -rf targeting ./dist should not be flagged"
        );
    }

    #[test]
    fn test_rm_rf_system_path_still_flagged() {
        let result = run_check(&["```bash", "rm -rf /", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "rm -rf targeting / should still be flagged"
        );
    }

    #[test]
    fn test_rm_rf_unknown_dir_still_flagged() {
        let result = run_check(&["```bash", "rm -rf data", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "rm -rf targeting unknown directories should still be flagged"
        );
    }
}
