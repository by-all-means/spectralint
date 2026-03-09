use crate::config::TokenBudgetConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct TokenBudgetChecker {
    warn_tokens: usize,
    max_tokens: usize,
    scope: ScopeFilter,
}

impl TokenBudgetChecker {
    pub(crate) fn new(config: &TokenBudgetConfig) -> Self {
        Self {
            warn_tokens: config.warn_tokens,
            max_tokens: config.max_tokens,
            scope: ScopeFilter::new(&config.scope),
        }
    }
}

/// Estimate the token count for the given lines by joining them with newlines
/// and dividing total character count by 4 (~4 chars per token for English text).
fn estimate_tokens(raw_lines: &[String]) -> usize {
    let total_chars: usize = raw_lines
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>()
        .saturating_sub(1);
    total_chars / 4
}

impl Checker for TokenBudgetChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "token-budget",
            description: "Estimates token cost and flags context window overuse",
            default_severity: Severity::Info,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let estimated_tokens = estimate_tokens(&file.raw_lines);

            if estimated_tokens >= self.max_tokens {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Warning,
                    Category::TokenBudget,
                    suggest: "Split into focused sub-files, remove low-value content, or move verbose examples into separate reference files",
                    "File has ~{} estimated tokens (exceeds {} token budget). \
                     Large files consume excessive context window and degrade LLM performance.",
                    estimated_tokens,
                    self.max_tokens
                );
            } else if estimated_tokens >= self.warn_tokens {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::TokenBudget,
                    suggest: "Split into focused sub-files, remove low-value content, or move verbose examples into separate reference files",
                    "File has ~{} estimated tokens (approaching {} token budget). \
                     Consider splitting to stay within budget.",
                    estimated_tokens,
                    self.max_tokens
                );
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::ParsedFile;
    use std::collections::HashSet;

    fn run_check_with_chars(total_chars: usize) -> CheckResult {
        run_check_with_config(total_chars, 4000, 8000)
    }

    fn run_check_with_config(
        total_chars: usize,
        warn_tokens: usize,
        max_tokens: usize,
    ) -> CheckResult {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Build lines that produce exactly `total_chars` characters when joined
        // with newlines. We use a single line to make the math precise:
        // a single line of length N has total_chars = N (no newlines).
        let content = "x".repeat(total_chars);
        let lines = vec![content];

        let file = ParsedFile {
            path: std::sync::Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines,
            in_code_block: vec![false],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let config = TokenBudgetConfig {
            enabled: true,
            warn_tokens,
            max_tokens,
            scope: Vec::new(),
            severity: None,
        };
        TokenBudgetChecker::new(&config).check(&ctx)
    }

    #[test]
    fn test_small_file_no_diagnostic() {
        // 100 chars => 25 tokens, well below warn threshold of 4000
        let result = run_check_with_chars(100);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_below_warn_threshold_no_diagnostic() {
        // 15_999 chars => 3999 tokens, just below warn_tokens=4000
        let result = run_check_with_chars(15_999);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_at_warn_threshold_info() {
        // 16_000 chars => 4000 tokens, exactly at warn_tokens=4000
        let result = run_check_with_chars(16_000);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::TokenBudget);
    }

    #[test]
    fn test_between_thresholds_info() {
        // 20_000 chars => 5000 tokens, between warn (4000) and max (8000)
        let result = run_check_with_chars(20_000);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn test_at_max_threshold_warning() {
        // 32_000 chars => 8000 tokens, exactly at max_tokens=8000
        let result = run_check_with_chars(32_000);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[0].category, Category::TokenBudget);
    }

    #[test]
    fn test_above_max_threshold_warning() {
        // 40_000 chars => 10_000 tokens, above max_tokens=8000
        let result = run_check_with_chars(40_000);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_custom_thresholds() {
        // 400 chars => 100 tokens, with warn=50, max=200
        let result = run_check_with_config(400, 50, 200);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].severity,
            Severity::Info,
            "100 tokens is between warn=50 and max=200, so should be Info"
        );

        // 800 chars => 200 tokens, at max=200
        let result = run_check_with_config(800, 50, 200);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_empty_file_no_diagnostic() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = ParsedFile {
            path: std::sync::Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let config = TokenBudgetConfig::default();
        let result = TokenBudgetChecker::new(&config).check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multi_line_token_estimation() {
        // 5 lines of 100 chars each = 500 chars + 4 newlines = 504 chars => 126 tokens
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let lines: Vec<String> = (0..5).map(|_| "x".repeat(100)).collect();
        let file = ParsedFile {
            path: std::sync::Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines,
            in_code_block: vec![false; 5],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        // warn_tokens=100, max_tokens=200 => 126 tokens should trigger Info
        let config = TokenBudgetConfig {
            enabled: true,
            warn_tokens: 100,
            max_tokens: 200,
            scope: Vec::new(),
            severity: None,
        };
        let result = TokenBudgetChecker::new(&config).check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn test_message_contains_token_count() {
        let result = run_check_with_chars(32_000);
        let msg = &result.diagnostics[0].message;
        assert!(
            msg.contains("8000"),
            "Message should contain the estimated token count"
        );
    }

    #[test]
    fn test_suggestion_present() {
        let result = run_check_with_chars(32_000);
        assert!(
            result.diagnostics[0].suggestion.is_some(),
            "Diagnostic should include a suggestion"
        );
    }

    #[test]
    fn test_estimate_tokens_helper() {
        // Single line: 400 chars => 100 tokens
        assert_eq!(estimate_tokens(&["x".repeat(400)]), 100);

        // Two lines of 200 chars: 200 + 200 + 1 newline = 401 chars => 100 tokens
        assert_eq!(estimate_tokens(&["x".repeat(200), "x".repeat(200)]), 100);

        // Empty
        assert_eq!(estimate_tokens(&[] as &[String]), 0);

        // Single empty line
        assert_eq!(estimate_tokens(&[String::new()]), 0);
    }
}
