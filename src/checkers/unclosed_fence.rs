use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct UnclosedFenceChecker {
    scope: ScopeFilter,
}

impl UnclosedFenceChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for UnclosedFenceChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "unclosed-fence",
            description: "Flags code fences that are never closed",
            default_severity: Severity::Error,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut in_code_block = false;
            let mut fence_open_line: usize = 0;

            for (idx, line) in file.raw_lines.iter().enumerate() {
                if line.trim_start().starts_with("```") {
                    in_code_block = !in_code_block;
                    if in_code_block {
                        fence_open_line = idx + 1;
                    }
                }
            }

            if in_code_block {
                let lang = file.raw_lines[fence_open_line - 1]
                    .trim_start()
                    .strip_prefix("```")
                    .unwrap_or("")
                    .trim();
                let tag_info = if lang.is_empty() {
                    String::new()
                } else {
                    format!(" ({lang})")
                };
                emit!(
                    result,
                    file.path,
                    fence_open_line,
                    Severity::Error,
                    Category::UnclosedFence,
                    suggest: "Add a closing ``` — everything after this fence is treated as code",
                    "Code fence{} opened here is never closed",
                    tag_info
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
        UnclosedFenceChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_unclosed_at_eof() {
        let result = run_check(&["# Heading", "```python", "x = 1", "y = 2"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].line, 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert!(result.diagnostics[0].message.contains("(python)"));
    }

    #[test]
    fn test_properly_closed() {
        let result = run_check(&["```bash", "echo hello", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_blocks_last_unclosed() {
        let result = run_check(&[
            "```bash",
            "echo hello",
            "```",
            "Some text",
            "```json",
            r#"{"key": "value"}"#,
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].line, 5);
        assert!(result.diagnostics[0].message.contains("(json)"));
    }

    #[test]
    fn test_empty_file() {
        let result = run_check(&[]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_closing_fence_only() {
        // A lone ``` is treated as an opening fence (odd toggle) — if EOF after it,
        // it's "unclosed". But since it has no content, it's still flagged.
        let result = run_check(&["```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_inline_backticks_no_confusion() {
        let result = run_check(&["```python", "x = `backtick`", "y = ``double``", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_unclosed_fence_no_lang_tag() {
        let result = run_check(&["# Test", "```", "some code"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(!result.diagnostics[0].message.contains("("));
    }
}
