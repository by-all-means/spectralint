use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{inside_inline_code, is_heading, ScopeFilter};
use super::Checker;

/// Double-negation patterns that create logical ambiguity.
static DOUBLE_NEGATION: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // "never don't" / "never doesn't"
        Regex::new(r"(?i)\bnever\s+don[''\u{2019}]?t\b").unwrap(),
        // "not fail to" / "not forget to"
        Regex::new(r"(?i)\bnot\s+(?:fail|forget|neglect|omit)\s+to\b").unwrap(),
        // "do not avoid" / "do not refuse" / "don't avoid"
        Regex::new(
            r"(?i)\b(?:do\s+not|don[''\u{2019}]?t)\s+(?:avoid|refuse|skip|ignore|neglect)\b",
        )
        .unwrap(),
        // "never avoid" / "never skip" / "never ignore"
        Regex::new(r"(?i)\bnever\s+(?:avoid|refuse|skip|ignore|neglect)\b").unwrap(),
        // "not unnecessary" / "not unimportant"
        Regex::new(r"(?i)\bnot\s+un(?:necessary|important|needed|required)\b").unwrap(),
    ]
});

pub(crate) struct DoubleNegationChecker {
    scope: ScopeFilter,
}

impl DoubleNegationChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for DoubleNegationChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in non_code_lines_masked(&file.raw_lines, &file.in_code_block) {
                let line_num = i + 1;

                // Skip headings — handled by imperative-heading
                if is_heading(line) {
                    continue;
                }

                for pattern in DOUBLE_NEGATION.iter() {
                    if let Some(m) = pattern.find(line) {
                        if inside_inline_code(line, m.start()) {
                            continue;
                        }

                        let matched = m.as_str();
                        emit!(
                            result,
                            file.path,
                            line_num,
                            Severity::Info,
                            Category::DoubleNegation,
                            suggest: "Rephrase as a positive directive for clarity",
                            "double negation \"{matched}\" — agents may misinterpret the intended meaning"
                        );
                        break; // One diagnostic per line
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

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        DoubleNegationChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_never_dont_flagged() {
        let result = check(&["Never don't validate user input"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::DoubleNegation);
    }

    #[test]
    fn test_not_fail_to_flagged() {
        let result = check(&["Do not fail to run tests before committing"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_dont_avoid_flagged() {
        let result = check(&["Don't avoid error handling"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_never_skip_flagged() {
        let result = check(&["Never skip the verification step"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_not_unnecessary_flagged() {
        let result = check(&["This step is not unnecessary"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_not_forget_to_flagged() {
        let result = check(&["Do not forget to update the changelog"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_simple_negation_not_flagged() {
        let result = check(&["Do not use eval()"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_simple_never_not_flagged() {
        let result = check(&["Never commit secrets to the repo"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "Never don't do this", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_code_not_flagged() {
        let result = check(&["Use `never don't` as a test case"]);
        assert_eq!(result.diagnostics.len(), 0);
    }
}
