use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

pub struct StaleStyleRuleChecker {
    scope: ScopeFilter,
}

impl StaleStyleRuleChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Imperative verb + formatting rule (indentation, semicolons, quotes, trailing commas, etc.)
static STYLE_RULE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:use|always\s+use|prefer|enforce|require|set|configure)\s+",
        r"(?:",
        r"(?:2|4|8)\s+spaces?\s+(?:for\s+)?indent",
        r"|tabs?\s+(?:for\s+)?indent",
        r"|(?:single|double)\s+quotes?",
        r"|semicolons?(?:\s+at\s+(?:the\s+)?end)?",
        r"|trailing\s+commas?",
        r"|(?:K&R|Allman|1TBS|Stroustrup|GNU)\s+(?:brace\s+)?style",
        r"|(?:sorted|alphabetical)\s+imports?",
        r"|(?:import|require)\s+(?:sorting|ordering)",
        r")",
    ))
    .unwrap()
});

/// "max line length of 80" etc.
static LINE_LENGTH_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:max(?:imum)?\s+)?line\s+(?:length|width)\s+(?:of\s+|to\s+|at\s+|=\s*)?\d{2,3}\b",
    )
    .unwrap()
});

/// "use camelCase for variables" etc.
static NAMING_STYLE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:use|prefer|enforce|always\s+use)\s+",
        r"(?:camelCase|PascalCase|snake_case|kebab-case|SCREAMING_SNAKE_CASE|UPPER_CASE)",
        r"\s+for\s+",
    ))
    .unwrap()
});

/// CLI/shell context — the quote/style advice is about command-line usage, not code formatting.
static CLI_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:CLI|command[- ]line|shell|terminal|argument|flag)\b").unwrap()
});

fn should_skip(line: &str) -> bool {
    is_heading(line) || !is_directive_line(line) || line.contains('`') || CLI_CONTEXT.is_match(line)
}

impl Checker for StaleStyleRuleChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                let line_num = idx + 1;

                if should_skip(line) {
                    continue;
                }

                let matched = STYLE_RULE_PATTERN
                    .find(line)
                    .or_else(|| LINE_LENGTH_PATTERN.find(line))
                    .or_else(|| NAMING_STYLE_PATTERN.find(line));

                if let Some(m) = matched {
                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Info,
                        Category::StaleStyleRule,
                        suggest: "Remove formatter-enforceable style rules — configure your formatter instead.",
                        "Style rule wastes context tokens: \"{}\"",
                        m.as_str()
                    );
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
        StaleStyleRuleChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_use_2_spaces() {
        let result = run_check(&["- Use 2 spaces for indentation"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_prefer_single_quotes() {
        let result = run_check(&["- Prefer single quotes"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_always_use_semicolons() {
        let result = run_check(&["- Always use semicolons"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_trailing_commas() {
        let result = run_check(&["- Use trailing commas"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_line_length() {
        let result = run_check(&["- Max line length of 80"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_line_width_120() {
        let result = run_check(&["- Set line width to 120"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_naming_convention_camel_case() {
        let result = run_check(&["- Use camelCase for variables"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_naming_convention_snake_case() {
        let result = run_check(&["- Prefer snake_case for functions"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_sorted_imports() {
        let result = run_check(&["- Use sorted imports"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_brace_style() {
        let result = run_check(&["- Use K&R brace style"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    // ── FP exclusions ──

    #[test]
    fn test_heading_no_flag() {
        let result = run_check(&["## Use 2 spaces for indentation"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_no_flag() {
        let result = run_check(&["```", "Use 2 spaces for indentation", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_backtick_context_no_flag() {
        let result = run_check(&["- Use `prettier` to enforce single quotes"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_non_directive_no_flag() {
        let result = run_check(&["The team prefers 2 spaces for indentation."]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_specific_action_no_flag() {
        let result = run_check(&["- Use meaningful variable names"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_clean_file_no_flag() {
        let result = run_check(&[
            "# Build",
            "- Run `cargo test` before committing",
            "- Never push directly to main",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_tabs_for_indentation() {
        let result = run_check(&["- Use tabs for indentation"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_allman_brace_style() {
        let result = run_check(&["- Enforce Allman brace style"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_maximum_line_length() {
        let result = run_check(&["- Maximum line length of 100"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_cli_argument_quotes_no_flag() {
        let result = run_check(&["- Use single quotes for code snippets passed as CLI arguments"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_command_line_flag_no_flag() {
        let result = run_check(&["- Use double quotes for shell arguments"]);
        assert!(result.diagnostics.is_empty());
    }
}
