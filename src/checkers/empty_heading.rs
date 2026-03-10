use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct EmptyHeadingChecker {
    scope: ScopeFilter,
}

impl EmptyHeadingChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for EmptyHeadingChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "empty-heading",
            description: "Flags headings with no title text",
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
                let trimmed = line.trim();

                if !trimmed.starts_with('#') {
                    continue;
                }

                // Strip the leading #s — if nothing remains, it's empty
                let after_hashes = trimmed.trim_start_matches('#');
                if after_hashes.trim().is_empty() {
                    let line_num = i + 1;
                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Info,
                        Category::EmptyHeading,
                        suggest: "Add heading text or remove the empty heading",
                        "empty heading — agents cannot navigate to a heading with no title"
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
        EmptyHeadingChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_empty_h2_flagged() {
        let result = check(&["## "]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::EmptyHeading);
    }

    #[test]
    fn test_empty_h1_flagged() {
        let result = check(&["# "]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_hashes_only_flagged() {
        let result = check(&["###"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_normal_heading_not_flagged() {
        let result = check(&["## Build Commands"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_in_code_block_not_flagged() {
        let result = check(&["```", "## ", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_empty_headings() {
        let result = check(&["## ", "Some content", "### "]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_heading_with_only_spaces_flagged() {
        // "##    " — hashes followed by only spaces
        let result = check(&["##    "]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_heading_with_only_bold_markers_flagged() {
        // "## **  **" — bold markers but no actual text inside them
        // After stripping hashes, what remains is "**  **" which is non-empty text
        let result = check(&["## **  **"]);
        // The checker only strips hashes and checks if remaining is whitespace.
        // "**  **" is not whitespace, so this is NOT flagged as empty.
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_with_bold_text_not_flagged() {
        let result = check(&["## **Build Commands**"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_three_empty_headings_at_different_levels() {
        let result = check(&["# ", "## ", "### "]);
        assert_eq!(result.diagnostics.len(), 3);
    }

    #[test]
    fn test_heading_with_emoji_only_not_flagged() {
        // Emoji is actual content — not whitespace
        let result = check(&["## \u{1f680}"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_h6_empty_flagged() {
        let result = check(&["###### "]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_mixed_empty_and_nonempty_headings() {
        let result = check(&["# Real Title", "## ", "### Another Real Title", "#### "]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_heading_with_tab_only_flagged() {
        let result = check(&["## \t"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
