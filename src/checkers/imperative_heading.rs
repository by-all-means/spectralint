use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

/// Heading line pattern.
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+)$").unwrap());

/// Imperative verbs that shouldn't be in headings.
static IMPERATIVE_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:always|never|do\s+not|don[''\u{2019}]?t|must|avoid|ensure|make\s+sure|use|run|follow|check|verify|prefer|keep)\s",
    )
    .unwrap()
});

/// Headings that are legitimately imperative (how-to / getting-started style).
static EXCEPTION_PATTERNS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:how\s+to|getting\s+started|use\s+with|run\s+(?:the|a)\b|check\s+(?:the|a|for)\b|make\s+(?:a|the)\b|keep\s+(?:a|the)\b)").unwrap()
});

pub(crate) struct ImperativeHeadingChecker {
    scope: ScopeFilter,
}

impl ImperativeHeadingChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for ImperativeHeadingChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "imperative-heading",
            description: "Flags headings that contain instructions instead of topics",
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

            for (i, line) in non_code_lines_masked(&file.raw_lines, &file.in_code_block) {
                let line_num = i + 1;

                let Some(caps) = HEADING.captures(line) else {
                    continue;
                };

                let title = caps[2].trim();

                // Skip short headings (1-2 words are usually fine like "## Use")
                if title.split_whitespace().count() < 3 {
                    continue;
                }

                if !IMPERATIVE_START.is_match(title) {
                    continue;
                }

                // Allow legitimate imperative headings
                if EXCEPTION_PATTERNS.is_match(title) {
                    continue;
                }

                emit!(
                    result,
                    file.path,
                    line_num,
                    Severity::Info,
                    Category::ImperativeHeading,
                    suggest: "Use a noun/topic heading and put the directive in the body",
                    "imperative heading \"{title}\" — headings should be topics, not instructions"
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

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        ImperativeHeadingChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_imperative_heading_flagged() {
        let result = check(&["## Always Run Tests Before Committing"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::ImperativeHeading);
    }

    #[test]
    fn test_never_heading_flagged() {
        let result = check(&["## Never Use Global State"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_do_not_heading_flagged() {
        let result = check(&["### Do Not Commit Secrets"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_avoid_heading_flagged() {
        let result = check(&["## Avoid Using eval() In Production"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_noun_heading_not_flagged() {
        let result = check(&["## Testing Strategy"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_short_heading_not_flagged() {
        // Two-word headings like "## Use This" are too short to flag
        let result = check(&["## Always commit"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_how_to_heading_not_flagged() {
        let result = check(&["## How to Run the Tests"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_getting_started_not_flagged() {
        let result = check(&["## Getting Started with the API"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "## Always Run Tests Before Committing", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_ensure_heading_flagged() {
        let result = check(&["## Ensure All Tests Pass First"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_must_heading_flagged() {
        let result = check(&["## Must Follow the Style Guide"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_h1_heading_flagged() {
        let result = check(&["# Never Deploy Without Review"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
