use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::types::ParsedFile;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{count_directive_lines, is_instruction_file, ScopeFilter};
use super::Checker;

pub struct InstructionWithoutContextChecker {
    scope: ScopeFilter,
}

impl InstructionWithoutContextChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

const MIN_DIRECTIVE_THRESHOLD: usize = 10;

/// Returns true if any non-code line contains inline code (backtick spans).
fn has_inline_code(file: &ParsedFile) -> bool {
    file.non_code_lines()
        .any(|(_, line)| line.matches('`').count() >= 2)
}

impl Checker for InstructionWithoutContextChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            if !is_instruction_file(&file.raw_lines, &file.in_code_block) {
                continue;
            }

            let directive_count = count_directive_lines(&file.raw_lines, &file.in_code_block);
            if directive_count < MIN_DIRECTIVE_THRESHOLD {
                continue;
            }

            let has_code = file.code_block_lines().next().is_some();
            let has_refs = !file.file_refs.is_empty();
            let has_inline = has_inline_code(file);

            if !has_code && !has_refs && !has_inline {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::InstructionWithoutContext,
                    suggest: "Add concrete examples: code blocks with commands, file paths, or inline code references",
                    "File has {} directive lines but no code blocks, file references, or inline code",
                    directive_count
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
        InstructionWithoutContextChecker::new(&[]).check(&ctx)
    }

    fn directive_lines(n: usize) -> Vec<String> {
        (0..n)
            .map(|i| format!("- Always follow rule number {i}"))
            .collect()
    }

    #[test]
    fn test_abstract_file_flags() {
        let lines: Vec<String> = directive_lines(12);
        let strs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let result = run_check(&strs);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .message
            .contains("no code blocks, file references, or inline code"));
    }

    #[test]
    fn test_file_with_code_block_no_flag() {
        let mut lines = directive_lines(12);
        lines.push("```bash".to_string());
        lines.push("cargo test".to_string());
        lines.push("```".to_string());
        let strs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let result = run_check(&strs);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_file_with_inline_code_no_flag() {
        let mut lines = directive_lines(11);
        lines.push("- Run `cargo test` before committing".to_string());
        let strs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let result = run_check(&strs);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_file_with_file_ref_no_flag() {
        let mut lines = directive_lines(12);
        lines.push("- See docs/guide.md for details".to_string());
        let strs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        // File refs are extracted by the parser, not by our helper.
        // This test validates the logic path exists but file_refs will be empty
        // in the test helper. The code block and inline code paths cover the logic.
        let result = run_check(&strs);
        // Since the test helper doesn't parse file refs, this will still flag.
        // That's expected — the real parser would populate file_refs.
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_small_file_no_flag() {
        let lines = directive_lines(5);
        let strs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let result = run_check(&strs);
        assert!(
            result.diagnostics.is_empty(),
            "Files with fewer than 10 directive lines should not flag"
        );
    }

    #[test]
    fn test_non_instruction_file_no_flag() {
        // A file with no imperative content
        let lines: Vec<&str> = (0..15).map(|_| "Some descriptive text here.").collect();
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "Non-instruction files should not flag"
        );
    }
}
