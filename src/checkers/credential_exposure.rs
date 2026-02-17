use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct CredentialExposureChecker {
    scope: ScopeFilter,
}

impl CredentialExposureChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static CREDENTIAL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r#"(?i)(?:password|secret|token|api[_-]?key)\s*[:=]\s*["'][^"']{8,}["']"#,
        r"\b(?:sk|pk)[-_](?:live|test)[-_][A-Za-z0-9]{20,}",
        r"\bAKIA[A-Z0-9]{16}\b",
        r"\bghp_[A-Za-z0-9]{36}\b",
        r"\bxox[bpas]-[A-Za-z0-9\-]{10,}",
        r"\beyJ[A-Za-z0-9_\-]{20,}\.[A-Za-z0-9_\-]{20,}",
        r"Bearer\s+[A-Za-z0-9_\-.]{20,}",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

/// Matches placeholder/example values that aren't real credentials.
static PLACEHOLDER_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:your[_-]|placeholder|changeme|change[_-]me|EXAMPLE|xxx|\.\.\.)"#).unwrap()
});

impl Checker for CredentialExposureChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Scan ALL lines including code blocks — secrets often live there
            for (i, line) in file.raw_lines.iter().enumerate() {
                for pat in CREDENTIAL_PATTERNS.iter() {
                    if let Some(m) = pat.find(line) {
                        let matched = m.as_str();

                        // Skip placeholder/example values (your-api-key, xxx, etc.)
                        if PLACEHOLDER_VALUE.is_match(matched) {
                            continue;
                        }

                        let display = match matched.char_indices().nth(30) {
                            Some((i, _)) => format!("{}...", &matched[..i]),
                            None => matched.to_string(),
                        };
                        emit!(
                            result,
                            file.path,
                            i + 1,
                            Severity::Error,
                            Category::CredentialExposure,
                            suggest: "Remove the credential and use an environment variable reference instead",
                            "Possible hardcoded credential: \"{}\"",
                            display
                        );
                        break;
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

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        CredentialExposureChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_api_key_detected() {
        let result = run_check(&["api_key = \"sk-live-abc123def456ghi789jkl012mno\""]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn test_aws_key_detected() {
        let result = run_check(&["Use AKIAIOSFODNN7REALKEY for access"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_github_token_detected() {
        let result = run_check(&["ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_env_var_reference_no_diagnostic() {
        let result = run_check(&["Use $API_KEY env var"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_short_password_no_diagnostic() {
        let result = run_check(&["password = \"short\""]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_bearer_token_detected() {
        let result = run_check(&["Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_inside_code_block_still_detected() {
        let result = run_check(&[
            "```",
            "token = \"sk-live-abc123def456ghi789jkl012mno\"",
            "```",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_slack_token_detected() {
        let result = run_check(&["xoxb-123456789012-abcdefghij"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_clean_file() {
        let result = run_check(&["# Config", "Set API_KEY as an environment variable."]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_placeholder_your_api_key_skipped() {
        let result = run_check(&["apiKey: 'your-api-key-here'"]);
        assert!(
            result.diagnostics.is_empty(),
            "Placeholder 'your-api-key' should not be flagged"
        );
    }

    #[test]
    fn test_placeholder_your_access_token_skipped() {
        let result = run_check(&[r#"TOKEN="your-access-token""#]);
        assert!(
            result.diagnostics.is_empty(),
            "Placeholder 'your-access-token' should not be flagged"
        );
    }

    #[test]
    fn test_placeholder_truncated_with_dots_skipped() {
        let result = run_check(&[r#"API_KEY = "AIzaSy...""#]);
        assert!(
            result.diagnostics.is_empty(),
            "Truncated value with ... should not be flagged"
        );
    }

    #[test]
    fn test_placeholder_changeme_skipped() {
        let result = run_check(&[r#"password = "changeme-please-replace""#]);
        assert!(
            result.diagnostics.is_empty(),
            "changeme placeholder should not be flagged"
        );
    }

    #[test]
    fn test_placeholder_example_skipped() {
        let result = run_check(&["Use AKIAIOSFODNN7EXAMPLE for access"]);
        assert!(
            result.diagnostics.is_empty(),
            "AWS example key with EXAMPLE should not be flagged"
        );
    }

    #[test]
    fn test_real_credential_still_detected() {
        let result = run_check(&[r#"api_key = "sk-proj-abc123def456ghi789jkl012mno""#]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Real-looking credentials should still be flagged"
        );
    }
}
