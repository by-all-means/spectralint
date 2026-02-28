use crate::config::ExcessiveNestingConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct ExcessiveNestingChecker {
    max_depth: usize,
    scope: ScopeFilter,
}

impl ExcessiveNestingChecker {
    pub fn new(config: &ExcessiveNestingConfig) -> Self {
        Self {
            max_depth: config.max_depth,
            scope: ScopeFilter::new(&config.scope),
        }
    }
}

/// Detect list item indent level. Returns `Some(depth)` for list items, `None` for non-list lines.
/// Depth 1 = top-level list item, depth 2 = one level nested, etc.
fn list_depth(line: &str) -> Option<usize> {
    // Must be a list item (starts with optional indent + bullet/number)
    let trimmed = line.trim_start();
    let is_list = trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || (trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) && trimmed.contains(". "));

    if !is_list {
        return None;
    }

    let leading_spaces = line.len() - trimmed.len();

    // Convert spaces to depth. Most common: 2 or 4 spaces per level.
    // We use 2-space increments as the minimum unit.
    let depth = (leading_spaces / 2) + 1;
    Some(depth)
}

impl Checker for ExcessiveNestingChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                if let Some(depth) = list_depth(line) {
                    if depth > self.max_depth {
                        emit!(
                            result,
                            file.path,
                            idx + 1,
                            Severity::Info,
                            Category::ExcessiveNesting,
                            suggest: "Flatten this list or extract nested items into their own section",
                            "List item nested {} levels deep (max: {})",
                            depth,
                            self.max_depth
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

    fn run_check_with_depth(lines: &[&str], max_depth: usize) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        let config = ExcessiveNestingConfig {
            enabled: true,
            max_depth,
            scope: Vec::new(),
        };
        ExcessiveNestingChecker::new(&config).check(&ctx)
    }

    fn run_check(lines: &[&str]) -> CheckResult {
        run_check_with_depth(lines, 4)
    }

    #[test]
    fn test_shallow_nesting_no_flag() {
        let result = run_check(&[
            "- Level 1",
            "  - Level 2",
            "    - Level 3",
            "      - Level 4",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_deep_nesting_flagged() {
        let result = run_check(&[
            "- Level 1",
            "  - Level 2",
            "    - Level 3",
            "      - Level 4",
            "        - Level 5",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("5 levels"));
    }

    #[test]
    fn test_very_deep_nesting() {
        let result = run_check(&[
            "- Level 1",
            "  - Level 2",
            "    - Level 3",
            "      - Level 4",
            "        - Level 5",
            "          - Level 6",
        ]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_code_block_excluded() {
        let result = run_check(&[
            "```",
            "        - Deep indent in code",
            "          - Even deeper",
            "```",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_numbered_list_detected() {
        let result = run_check(&[
            "1. Level 1",
            "  1. Level 2",
            "    1. Level 3",
            "      1. Level 4",
            "        1. Level 5",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_custom_threshold() {
        let result = run_check_with_depth(&["- Level 1", "  - Level 2", "    - Level 3"], 2);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_non_list_deep_indent_no_flag() {
        let result = run_check(&["        Just some deeply indented text"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_mixed_bullets() {
        let result = run_check(&[
            "- Level 1",
            "  * Level 2",
            "    + Level 3",
            "      - Level 4",
            "        * Level 5",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
