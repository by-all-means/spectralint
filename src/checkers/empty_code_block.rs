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
}
