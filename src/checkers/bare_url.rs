use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{inside_inline_code, is_heading, ScopeFilter};
use super::Checker;

/// Bare URL pattern: http(s) URLs not inside markdown link syntax.
static BARE_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https?://[^\s)>\]]+").unwrap());

pub(crate) struct BareUrlChecker {
    scope: ScopeFilter,
}

impl BareUrlChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for BareUrlChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "bare-url",
            description: "Flags raw URLs not wrapped in markdown link syntax",
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

                // Skip headings
                if is_heading(line) {
                    continue;
                }

                for m in BARE_URL.find_iter(line) {
                    let start = m.start();

                    // Skip URLs inside inline code
                    if inside_inline_code(line, start) {
                        continue;
                    }

                    // Skip if preceded by ]( — it's already a markdown link target
                    if start >= 2 && &line[start - 2..start] == "](" {
                        continue;
                    }

                    // Skip if preceded by ( and the line has []( pattern — markdown link
                    if start >= 1 && &line[start - 1..start] == "(" && line[..start].contains("](")
                    {
                        continue;
                    }

                    // Skip angle-bracket URLs: <https://...>
                    if start >= 1 && line.as_bytes()[start - 1] == b'<' {
                        continue;
                    }

                    let url = m.as_str();
                    // Trim trailing punctuation that's likely not part of the URL
                    let url = url.trim_end_matches(['.', ',', ';', ')']);

                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Info,
                        Category::BareUrl,
                        suggest: "Wrap in markdown link: [descriptive text](url)",
                        "bare URL not wrapped in markdown link syntax: {url}"
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
        BareUrlChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_bare_url_flagged() {
        let result = check(&["Check out https://example.com for docs"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::BareUrl);
    }

    #[test]
    fn test_markdown_link_not_flagged() {
        let result = check(&["Check out [the docs](https://example.com) for info"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_code_not_flagged() {
        let result = check(&["Use `https://example.com/api` for the endpoint"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "curl https://example.com/api", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_not_flagged() {
        let result = check(&["## https://example.com"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_angle_bracket_url_not_flagged() {
        let result = check(&["Visit <https://example.com> for more"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_bare_urls() {
        let result = check(&["See https://foo.com and https://bar.com"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_url_with_trailing_period() {
        let result = check(&["Visit https://example.com."]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .message
            .contains("https://example.com"));
        // Trailing period should be trimmed from the URL in the message
        assert!(!result.diagnostics[0].message.ends_with('.'));
    }
}
