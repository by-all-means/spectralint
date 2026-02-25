use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct LargeCodeBlockChecker {
    scope: ScopeFilter,
    max_lines: usize,
}

impl LargeCodeBlockChecker {
    pub fn new(config: &crate::config::LargeCodeBlockConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            max_lines: config.max_lines,
        }
    }
}

impl Checker for LargeCodeBlockChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut in_code_block = false;
            let mut block_start_line: usize = 0;
            let mut block_line_count: usize = 0;

            let maybe_emit = |result: &mut CheckResult, count: usize, start: usize| {
                if count > self.max_lines {
                    emit!(
                        result,
                        file.path,
                        start,
                        Severity::Info,
                        Category::LargeCodeBlock,
                        suggest: "Extract this code block into a separate file and reference it",
                        "Code block is {} lines (threshold: {})",
                        count,
                        self.max_lines
                    );
                }
            };

            for (idx, line) in file.raw_lines.iter().enumerate() {
                if line.trim_start().starts_with("```") {
                    if in_code_block {
                        maybe_emit(&mut result, block_line_count, block_start_line);
                        in_code_block = false;
                    } else {
                        in_code_block = true;
                        block_start_line = idx + 1; // 1-indexed
                        block_line_count = 0;
                    }
                } else if in_code_block {
                    block_line_count += 1;
                }
            }

            // Handle unclosed code block at end of file
            if in_code_block {
                maybe_emit(&mut result, block_line_count, block_start_line);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;
    use crate::config::LargeCodeBlockConfig;

    fn checker_with_max(max_lines: usize) -> LargeCodeBlockChecker {
        LargeCodeBlockChecker::new(&LargeCodeBlockConfig {
            enabled: true,
            max_lines,
            scope: vec![],
        })
    }

    #[test]
    fn test_large_block_detected() {
        let mut lines: Vec<&str> = vec!["# Test", "```python"];
        lines.extend(vec!["line1"; 5]);
        lines.push("```");

        let (_dir, ctx) = single_file_ctx(&lines);
        let checker = checker_with_max(3);
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::LargeCodeBlock);
        assert!(result.diagnostics[0].message.contains("5 lines"));
        assert_eq!(result.diagnostics[0].line, 2); // line of opening fence
    }

    #[test]
    fn test_small_block_passes() {
        let lines = vec!["# Test", "```", "line1", "line2", "```"];

        let (_dir, ctx) = single_file_ctx(&lines);
        let checker = checker_with_max(5);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "Block with 2 lines should not trigger at threshold 5"
        );
    }

    #[test]
    fn test_exactly_at_threshold_passes() {
        let mut lines: Vec<&str> = vec!["```"];
        lines.extend(vec!["x"; 3]);
        lines.push("```");

        let (_dir, ctx) = single_file_ctx(&lines);
        let checker = checker_with_max(3);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "Block exactly at threshold should not trigger (> not >=)"
        );
    }

    #[test]
    fn test_multiple_blocks() {
        // Two code blocks, both over threshold
        let mut lines: Vec<&str> = vec!["# Header"];
        lines.push("```");
        lines.extend(vec!["a"; 5]);
        lines.push("```");
        lines.push("Some text");
        lines.push("```bash");
        lines.extend(vec!["b"; 4]);
        lines.push("```");

        let (_dir, ctx) = single_file_ctx(&lines);
        let checker = checker_with_max(3);
        let result = checker.check(&ctx);

        assert_eq!(
            result.diagnostics.len(),
            2,
            "Both oversized blocks should be flagged"
        );
    }

    #[test]
    fn test_nested_backticks_in_code_block() {
        // A code block that contains `` (not ```) inside shouldn't confuse the parser
        let lines = vec![
            "```markdown",
            "Some `inline code` here",
            "More ``double backtick`` here",
            "Still inside the block",
            "```",
        ];

        let (_dir, ctx) = single_file_ctx(&lines);
        let checker = checker_with_max(2);
        let result = checker.check(&ctx);

        assert_eq!(
            result.diagnostics.len(),
            1,
            "3 content lines exceeds threshold of 2"
        );
    }

    #[test]
    fn test_default_threshold_40() {
        let mut lines: Vec<&str> = vec!["```"];
        lines.extend(vec!["code"; 41]);
        lines.push("```");

        let (_dir, ctx) = single_file_ctx(&lines);
        let checker = LargeCodeBlockChecker::new(&LargeCodeBlockConfig::default());
        let result = checker.check(&ctx);

        assert_eq!(
            result.diagnostics.len(),
            1,
            "41 lines should exceed default threshold of 40"
        );
    }

    #[test]
    fn test_indented_fence() {
        // Fences with leading whitespace should still be detected
        let mut lines: Vec<&str> = vec!["  ```"];
        lines.extend(vec!["code"; 5]);
        lines.push("  ```");

        let (_dir, ctx) = single_file_ctx(&lines);
        let checker = checker_with_max(3);
        let result = checker.check(&ctx);

        assert_eq!(
            result.diagnostics.len(),
            1,
            "Indented fences should be recognized"
        );
    }
}
