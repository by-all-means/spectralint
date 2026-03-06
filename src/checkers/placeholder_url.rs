use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

pub(crate) struct PlaceholderUrlChecker {
    scope: ScopeFilter,
}

impl PlaceholderUrlChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static PLACEHOLDER_URLS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"https?://(?:",
        // Well-known placeholder domains
        r"(?:example\.com|example\.org|placeholder\.com|your-domain\.com|localhost:\d+|127\.0\.0\.1)",
        r"|",
        // Placeholder API subdomains
        r"(?:api\.example|your-api|my-api|test-api)\.",
        r"|",
        // Template URLs with {placeholders}
        r"\S*\{[^}]+\}\S*",
        r")",
    ))
    .unwrap()
});

const REAL_DOMAINS: &[&str] = &[
    "github.com/",
    "gitlab.com/",
    "bitbucket.org/",
    "npmjs.com/",
    "pypi.org/",
    "crates.io/",
    "hub.docker.com/",
];

/// Returns true if this is a template URL on a well-known real domain
/// (e.g., `https://github.com/org/repo/releases/{VERSION}/...`).
fn is_real_domain_template(url: &str) -> bool {
    // Only filter template URLs (those containing {placeholders})
    if !url.contains('{') {
        return false;
    }
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    REAL_DOMAINS
        .iter()
        .any(|domain| after_scheme.starts_with(domain))
}

impl Checker for PlaceholderUrlChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                if is_heading(line) || !is_directive_line(line) {
                    continue;
                }

                if let Some(m) = PLACEHOLDER_URLS.find(line) {
                    // Skip template URLs on well-known real domains
                    if is_real_domain_template(m.as_str()) {
                        continue;
                    }
                    emit!(
                        result,
                        file.path,
                        idx + 1,
                        Severity::Info,
                        Category::PlaceholderUrl,
                        suggest: "Replace placeholder URL with the actual endpoint or remove it",
                        "Placeholder URL: {}",
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
        PlaceholderUrlChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_example_com_flags() {
        let result = run_check(&["Send requests to https://example.com/api"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("example.com"));
    }

    #[test]
    fn test_api_example_flags() {
        let result = run_check(&["Use https://api.example.com/v2"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_your_api_flags() {
        let result = run_check(&["POST to https://your-api.company.com"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_real_url_no_flag() {
        let result = run_check(&["See https://github.com/owner/repo"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_in_code_block_no_flag() {
        let result = run_check(&["```bash", "curl https://example.com/api", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_template_url_flags() {
        let result = run_check(&["Endpoint: https://api.{env}.company.com/v2"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_localhost_flags() {
        let result = run_check(&["Health check at http://localhost:3000/health"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_loopback_flags() {
        let result = run_check(&["Connect to http://127.0.0.1:8080"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_heading_no_flag() {
        let result = run_check(&["## https://example.com setup"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_example_org_flags() {
        let result = run_check(&["Visit https://example.org for info"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_your_domain_flags() {
        let result = run_check(&["Deploy to https://your-domain.com"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_template_url_on_real_domain_no_flag() {
        let result = run_check(&[
            "Download from https://github.com/org/repo/releases/download/{VERSION}/app.zip",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Template URLs on well-known real domains should not flag"
        );
    }

    #[test]
    fn test_blockquote_no_flag() {
        let result = run_check(&["> See https://example.com/api for details"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_table_row_no_flag() {
        let result = run_check(&["| https://example.com | placeholder |"]);
        assert!(result.diagnostics.is_empty());
    }
}
