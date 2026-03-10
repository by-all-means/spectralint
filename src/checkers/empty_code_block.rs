use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct EmptyCodeBlockChecker {
    scope: ScopeFilter,
}

impl EmptyCodeBlockChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for EmptyCodeBlockChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "empty-code-block",
            description: "Flags code blocks with no content",
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

            let lines = &file.raw_lines;
            let mut i = 0;
            while i < lines.len() {
                let trimmed = lines[i].trim_start();
                if trimmed.starts_with("```") {
                    let open_line = i + 1; // 1-indexed
                    let open_idx = i;

                    // Find the closing fence
                    i += 1;
                    let mut has_content = false;
                    while i < lines.len() {
                        let t = lines[i].trim_start();
                        if t.starts_with("```") {
                            break;
                        }
                        if !lines[i].trim().is_empty() {
                            has_content = true;
                        }
                        i += 1;
                    }

                    // If we found a closing fence and no content between
                    if i < lines.len() && !has_content {
                        // Check it's truly empty (not just the open fence at EOF)
                        let close_idx = i;
                        if close_idx > open_idx {
                            emit!(
                                result,
                                file.path,
                                open_line,
                                Severity::Info,
                                Category::EmptyCodeBlock,
                                suggest: "Add content or remove the empty code block",
                                "empty code block — agents expect a command or example here"
                            );
                        }
                    }
                }
                i += 1;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        EmptyCodeBlockChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_empty_code_block_flagged() {
        let result = check(&["```", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::EmptyCodeBlock);
    }

    #[test]
    fn test_empty_code_block_with_language_tag() {
        let result = check(&["```bash", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_whitespace_only_code_block_flagged() {
        let result = check(&["```", "  ", "", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_code_block_with_content_not_flagged() {
        let result = check(&["```bash", "cargo test", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_empty_blocks() {
        let result = check(&["```", "```", "", "```python", "```"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_unclosed_fence_not_flagged() {
        // Unclosed fence is handled by unclosed-fence checker
        let result = check(&["```", "some content"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_with_only_tabs_and_spaces_flagged() {
        let result = check(&["```", "\t", "   \t  ", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_code_block_with_blank_lines_only_flagged() {
        let result = check(&["```", "", "", "", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_language_tag_with_options_but_no_content() {
        // Some code blocks use extended info strings like ```rust,no_run
        let result = check(&["```rust,no_run", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_nested_code_block_in_blockquote() {
        // Code fences inside blockquotes (> ```) are still fences
        // The checker works on raw lines, so indented fences still match
        let result = check(&["> ```bash", "> ```"]);
        // The checker trims leading whitespace before checking for ```,
        // but blockquote markers ("> ") are not whitespace — these don't match
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_consecutive_empty_code_blocks() {
        // Two empty code blocks immediately adjacent
        let result = check(&["```", "```", "```python", "```"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_code_block_with_single_space_line_flagged() {
        let result = check(&["```", " ", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_empty_then_nonempty_code_blocks() {
        let result = check(&["```", "```", "", "```bash", "echo hello", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "only the first empty block flags"
        );
    }

    #[test]
    fn test_indented_fence_empty_block() {
        // Fences can be indented up to 3 spaces in markdown
        let result = check(&["   ```", "   ```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
