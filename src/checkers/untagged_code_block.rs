use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct UntaggedCodeBlockChecker {
    scope: ScopeFilter,
}

impl UntaggedCodeBlockChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

const MIN_CONTENT_LINES: usize = 2;

impl Checker for UntaggedCodeBlockChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "untagged-code-block",
            description: "Flags code fences without a language tag",
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

            let mut in_code_block = false;
            let mut block_is_untagged = false;
            let mut block_open_line: usize = 0;
            let mut content_lines: usize = 0;

            let maybe_emit = |result: &mut CheckResult,
                              untagged: bool,
                              lines: usize,
                              open_line: usize| {
                if untagged && lines >= MIN_CONTENT_LINES {
                    emit!(
                        result,
                        file.path,
                        open_line,
                        Severity::Info,
                        Category::UntaggedCodeBlock,
                        suggest: "Add a language tag (e.g., ```bash, ```json) to help agents parse the block correctly",
                        "Code fence has no language tag"
                    );
                }
            };

            for (idx, line) in file.raw_lines.iter().enumerate() {
                let trimmed = line.trim_start();
                if let Some(after_fence) = trimmed.strip_prefix("```") {
                    if in_code_block {
                        maybe_emit(
                            &mut result,
                            block_is_untagged,
                            content_lines,
                            block_open_line,
                        );
                        in_code_block = false;
                    } else {
                        in_code_block = true;
                        block_open_line = idx + 1;
                        content_lines = 0;
                        block_is_untagged = after_fence.trim().is_empty();
                    }
                } else if in_code_block {
                    content_lines += 1;
                }
            }

            if in_code_block {
                maybe_emit(
                    &mut result,
                    block_is_untagged,
                    content_lines,
                    block_open_line,
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
        UntaggedCodeBlockChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_bare_fence_with_content() {
        let result = run_check(&["```", "line1", "line2", "line3", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].line, 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn test_tagged_fence_no_flag() {
        let result = run_check(&["```python", "x = 1", "y = 2", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_bare_fence_one_line_no_flag() {
        let result = run_check(&["```", "only one line", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_closing_fence_not_flagged() {
        // The closing ``` should not be treated as an untagged opening fence
        let result = run_check(&["```bash", "echo hello", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_untagged_blocks() {
        let result = run_check(&["```", "a", "b", "```", "text", "```", "c", "d", "e", "```"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_whitespace_after_backticks_flags() {
        let result = run_check(&["```   ", "line1", "line2", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_unclosed_untagged_with_content() {
        let result = run_check(&["```", "line1", "line2"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_exactly_two_content_lines_flags() {
        let result = run_check(&["```", "a", "b", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
