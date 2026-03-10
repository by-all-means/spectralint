use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct CredentialExposureChecker {
    scope: ScopeFilter,
}

impl CredentialExposureChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static CREDENTIAL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?:",
        r#"(?i)(?:password|secret|token|api[_-]?key)\s*[:=]\s*["'][^"']{8,}["']"#,
        r"|\b(?:sk|pk)[-_](?:live|test)[-_][A-Za-z0-9]{20,}",
        r"|\bAKIA[A-Z0-9]{16}\b",
        r"|\bghp_[A-Za-z0-9]{36}\b",
        r"|\bxox[bpas]-[A-Za-z0-9\-]{10,}",
        r"|\beyJ[A-Za-z0-9_\-]{20,}\.[A-Za-z0-9_\-]{20,}",
        r"|Bearer\s+[A-Za-z0-9_\-.]{20,}",
        r")",
    ))
    .unwrap()
});

/// Matches placeholder/example values that aren't real credentials.
static PLACEHOLDER_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:your[_-]|placeholder|changeme|change[_-]me|EXAMPLE|xxx|\.\.\.)"#).unwrap()
});

/// Matches lines in test/example/fixture contexts where credentials are expected.
static TEST_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:test|example|sample|dummy|fake|mock|fixture|setUp|setup)").unwrap()
});

/// Credential values that are obviously test/dummy data.
static TEST_CREDENTIAL_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)^["']?(?:test|example|sample|dummy|fake|mock|password|secret|secure|changeme|abc|123)"#,
    )
    .unwrap()
});

impl Checker for CredentialExposureChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "credential-exposure",
            description: "Detects hardcoded secrets and API keys",
            default_severity: Severity::Error,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Scan ALL lines including code blocks — secrets often live there
            for (i, line) in file.raw_lines.iter().enumerate() {
                if let Some(m) = CREDENTIAL_PATTERN.find(line) {
                    let matched = m.as_str();

                    if PLACEHOLDER_VALUE.is_match(matched) {
                        continue;
                    }

                    if TEST_CONTEXT.is_match(line) {
                        continue;
                    }

                    // Extract value after the delimiter to check for fake credentials
                    if let Some(eq_pos) = matched.find(['=', ':']) {
                        let value_part = matched[eq_pos + 1..].trim_start();
                        if TEST_CREDENTIAL_VALUE.is_match(value_part) {
                            continue;
                        }
                    }

                    // Redact to avoid leaking the credential value in any output format.
                    // Show only the type prefix (e.g., "sk-***", "ghp_***", "AKIA***").
                    let display = if let Some(delim) = matched.find(['=', ':']) {
                        let key = matched[..delim].trim();
                        format!("{key}=***")
                    } else if let Some((byte_pos, _)) = matched.char_indices().nth(4) {
                        format!("{}***", &matched[..byte_pos])
                    } else {
                        "****".to_string()
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

    #[test]
    fn test_test_context_password_skipped() {
        let result = run_check(&[r#"password='testpass123' # setUp fixture"#]);
        assert!(
            result.diagnostics.is_empty(),
            "Credentials in test/setUp context should not be flagged"
        );
    }

    #[test]
    fn test_example_context_skipped() {
        let result = run_check(&[r#"# Example: token = "sk-live-abc123def456ghi789jkl012mno""#]);
        assert!(
            result.diagnostics.is_empty(),
            "Credentials in example context should not be flagged"
        );
    }

    #[test]
    fn test_test_credential_value_skipped() {
        let result = run_check(&[r#"password = "testpassword12345""#]);
        assert!(
            result.diagnostics.is_empty(),
            "Obviously fake credential values like 'testpassword' should not be flagged"
        );
    }

    #[test]
    fn test_real_credential_not_in_test_context_still_flagged() {
        let result = run_check(&[r#"secret = "a]k9#mP2$xQ7!nR4vL8wB""#]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Real-looking credentials outside test context should still be flagged"
        );
    }

    // --- FP/FN regression tests ---

    #[test]
    fn test_hashed_value_not_flagged() {
        // SHA256 hashes look like long hex strings but are not credentials.
        let result = run_check(&[
            "sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "SHA256 hash values should not be flagged as credentials"
        );
    }

    #[test]
    fn test_jwt_in_example_context() {
        // A JWT that would normally match the credential pattern, but appears
        // in an "Example" context line and should be suppressed.
        let result = run_check(&[
            "# Example: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4iLCJpYXQiOjE1MTYyMzkwMjJ9",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "JWT in an example context line should not be flagged"
        );
    }

    #[test]
    fn test_key_in_yaml_format() {
        // YAML-style key: value with quoted credential
        let result = run_check(&[r#"api_key: "rk-prod-7f3a9b2c4d5e6f7890abcdef""#]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "YAML-formatted credential should be detected"
        );
    }

    #[test]
    fn test_key_with_single_quotes() {
        let result = run_check(&["token = 'sk-live-abc123def456ghi789jkl012mno'"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Credential with single quotes should be detected"
        );
    }

    #[test]
    fn test_base64_jwt_token() {
        // A base64-encoded JWT-like token (eyJ prefix)
        let result = run_check(&[
            "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJodHRwczovL2F1dGguZXhhbXBsZS5jb20ifQ",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Base64-encoded JWT token should be detected"
        );
    }

    #[test]
    fn test_aws_access_key_pattern() {
        // Real AWS access key pattern: AKIA followed by 16 uppercase alphanumeric chars
        let result = run_check(&["AWS_ACCESS_KEY_ID=AKIAIOSFODNN7REALKEY"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "AWS access key pattern should be detected"
        );
    }

    #[test]
    fn test_short_value_not_flagged() {
        // Short values (< 8 chars) should not match the key=value pattern
        let result = run_check(&[r#"password = "abc""#]);
        assert!(
            result.diagnostics.is_empty(),
            "Short password value should not be flagged"
        );
    }

    #[test]
    fn test_short_token_value_not_flagged() {
        let result = run_check(&[r#"secret: "12345""#]);
        assert!(
            result.diagnostics.is_empty(),
            "Short secret value should not be flagged"
        );
    }
}
