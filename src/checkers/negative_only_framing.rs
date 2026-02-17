use regex::Regex;
use std::sync::LazyLock;

use crate::config::NegativeOnlyFramingConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::{is_directive_line, non_code_lines};
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct NegativeOnlyFramingChecker {
    scope: ScopeFilter,
    threshold: f64,
    min_negative_count: usize,
}

impl NegativeOnlyFramingChecker {
    pub fn new(config: &NegativeOnlyFramingConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            threshold: config.threshold,
            min_negative_count: config.min_negative_count,
        }
    }
}

/// Positive directive patterns.
static POSITIVE_PATTERNS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:always|must|should|use|run|follow|prefer|ensure|make\s+sure)\b").unwrap()
});

/// Negative directive patterns.
static NEGATIVE_PATTERNS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:never|do\s+not|don'?t|avoid|must\s+not|prohibited|forbidden)\b").unwrap()
});

impl Checker for NegativeOnlyFramingChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut positive_count = 0;
            let mut negative_count = 0;

            for (_, line) in non_code_lines(&file.raw_lines) {
                if !is_directive_line(line) {
                    continue;
                }

                if POSITIVE_PATTERNS.is_match(line) {
                    positive_count += 1;
                }
                if NEGATIVE_PATTERNS.is_match(line) {
                    negative_count += 1;
                }
            }

            if negative_count < self.min_negative_count {
                continue;
            }

            let total = positive_count + negative_count;
            if total == 0 {
                continue;
            }

            let ratio = negative_count as f64 / total as f64;
            if ratio >= self.threshold {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::NegativeOnlyFraming,
                    suggest: "Add positive directives (Always/Use/Run/Follow) so agents have a clear path forward",
                    "{:.0}% of directives are negative ({} negative vs {} positive). \
                     Agents need positive guidance, not just restrictions.",
                    ratio * 100.0,
                    negative_count,
                    positive_count
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
        run_check_with_config(lines, 0.75, 5)
    }

    fn run_check_with_config(
        lines: &[&str],
        threshold: f64,
        min_negative_count: usize,
    ) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        let config = NegativeOnlyFramingConfig {
            enabled: true,
            threshold,
            min_negative_count,
            scope: Vec::new(),
        };
        NegativeOnlyFramingChecker::new(&config).check(&ctx)
    }

    #[test]
    fn test_mostly_negative_flags() {
        let result = run_check(&[
            "Never modify production directly.",
            "Do not skip tests.",
            "Avoid hardcoding values.",
            "Don't commit secrets.",
            "Never bypass CI.",
            "Must not deploy on Fridays.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(
            result.diagnostics[0].category,
            Category::NegativeOnlyFraming
        );
    }

    #[test]
    fn test_balanced_no_flag() {
        let result = run_check(&[
            "Always run tests.",
            "Use structured logging.",
            "Never skip CI.",
            "Do not hardcode secrets.",
            "Follow the style guide.",
            "Avoid global state.",
            "Must not bypass review.",
            "Run linting before commit.",
            "Never deploy on Fridays.",
            "Don't modify production.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Balanced positive/negative should not flag"
        );
    }

    #[test]
    fn test_few_negatives_below_threshold() {
        let result = run_check(&["Never skip tests.", "Don't hardcode values."]);
        assert!(
            result.diagnostics.is_empty(),
            "Files with < 5 negatives should not flag"
        );
    }

    #[test]
    fn test_all_positive_no_flag() {
        let result = run_check(&[
            "Always run tests.",
            "Use structured logging.",
            "Follow the style guide.",
            "Run linting before commit.",
            "Ensure code compiles.",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_lines_skipped() {
        let result = run_check(&[
            "```",
            "Never do this.",
            "Do not do that.",
            "Avoid everything.",
            "Don't try.",
            "Never ever.",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Lines in code blocks should be skipped"
        );
    }

    #[test]
    fn test_blockquote_lines_skipped() {
        let result = run_check(&[
            "> Never do this.",
            "> Do not do that.",
            "> Avoid everything.",
            "> Don't try.",
            "> Never ever.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Lines in blockquotes should be skipped"
        );
    }

    #[test]
    fn test_custom_threshold() {
        // 3 negative, 1 positive = 75% negative
        let result = run_check_with_config(
            &[
                "Never skip tests.",
                "Do not hardcode.",
                "Avoid globals.",
                "Always lint.",
            ],
            0.70,
            3,
        );
        assert_eq!(result.diagnostics.len(), 1);
    }
}
