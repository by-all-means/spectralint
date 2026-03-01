use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

/// Opaque link text patterns.
static OPAQUE_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[(?:click\s+here|here|this\s+link|link|this\s+page|this)\]\(").unwrap()
});

pub(crate) struct ClickHereLinkChecker {
    scope: ScopeFilter,
}

impl ClickHereLinkChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for ClickHereLinkChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in non_code_lines_masked(&file.raw_lines, &file.in_code_block) {
                let line_num = i + 1;

                for m in OPAQUE_LINK.find_iter(line) {
                    // Extract the link text for the message
                    let text = &m.as_str()[1..m.as_str().len() - 2]; // strip [ and ](
                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Info,
                        Category::ClickHereLink,
                        suggest: "Use descriptive link text that explains where the link goes",
                        "opaque link text \"{text}\" — agents can't follow URLs, descriptive text is their only context"
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

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        ClickHereLinkChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_click_here_flagged() {
        let result = check(&["See [click here](https://example.com) for docs"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::ClickHereLink);
    }

    #[test]
    fn test_here_flagged() {
        let result = check(&["Documentation is [here](https://docs.example.com)"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_this_link_flagged() {
        let result = check(&["Follow [this link](https://example.com)"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_this_flagged() {
        let result = check(&["See [this](https://example.com) for more"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_descriptive_link_not_flagged() {
        let result = check(&["See the [API documentation](https://docs.example.com)"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "[click here](url)", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_case_insensitive() {
        let result = check(&["[Click Here](https://example.com)"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_here_in_longer_text_not_flagged() {
        // "here" as part of a longer descriptive text should not flag
        let result = check(&["[documented here in detail](https://example.com)"]);
        assert_eq!(result.diagnostics.len(), 0);
    }
}
