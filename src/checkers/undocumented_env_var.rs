use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{inside_inline_code, is_heading, ScopeFilter};
use super::Checker;

/// Env var reference patterns.
static ENV_VAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        \$\{?[A-Z_][A-Z0-9_]{2,}\}?   # $VAR or ${VAR} (3+ chars in name)
        | process\.env\.[A-Z_]\w+        # process.env.VAR
        | os\.environ\[                  # os.environ[
        | ENV\[                          # ENV[
    ",
    )
    .unwrap()
});

/// Context patterns that indicate the env var is being documented/explained.
static EXPLANATION_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:set|configure|export|defaults?|define|should\s+be|specify|contains?|holds?|stores?|points?\s+to|refers?\s+to)\b|[=:]").unwrap()
});

pub(crate) struct UndocumentedEnvVarChecker {
    scope: ScopeFilter,
}

impl UndocumentedEnvVarChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for UndocumentedEnvVarChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let lines = &file.raw_lines;
            let mask = &file.in_code_block;

            for (i, line) in non_code_lines_masked(lines, mask) {
                let line_num = i + 1;

                // Skip headings
                if is_heading(line) {
                    continue;
                }

                // Check same line + adjacent lines for explanation context (once per line)
                if EXPLANATION_CONTEXT.is_match(line) {
                    continue;
                }
                let has_adjacent_context = (i > 0
                    && !mask.get(i - 1).copied().unwrap_or(false)
                    && EXPLANATION_CONTEXT.is_match(&lines[i - 1]))
                    || (i + 1 < lines.len()
                        && !mask.get(i + 1).copied().unwrap_or(false)
                        && EXPLANATION_CONTEXT.is_match(&lines[i + 1]));
                if has_adjacent_context {
                    continue;
                }

                for m in ENV_VAR.find_iter(line) {
                    if inside_inline_code(line, m.start()) {
                        continue;
                    }

                    let var_ref = m.as_str();
                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Info,
                        Category::UndocumentedEnvVar,
                        suggest: "Add a brief explanation of what this variable does",
                        "env var reference `{var_ref}` without nearby explanation"
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
        UndocumentedEnvVarChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_undocumented_env_var_flagged() {
        let result = check(&["Make sure $DATABASE_URL is available"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::UndocumentedEnvVar);
    }

    #[test]
    fn test_documented_with_set() {
        let result = check(&["Set $DATABASE_URL to the connection string"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_documented_with_colon() {
        let result = check(&["$DATABASE_URL: the PostgreSQL connection string"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_documented_with_equals() {
        let result = check(&["$DATABASE_URL = postgres://localhost/mydb"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_documented_on_adjacent_line() {
        let result = check(&[
            "Set the following variable:",
            "Use $DATABASE_URL in your config",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_short_var_not_flagged() {
        // $AB is only 2 chars, below the 3-char minimum
        let result = check(&["Use $AB somewhere"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "export DATABASE_URL=postgres://localhost/db", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_not_flagged() {
        let result = check(&["## $DATABASE_URL"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_code_not_flagged() {
        let result = check(&["Use `$DATABASE_URL` in your env"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_braces_syntax_flagged() {
        let result = check(&["Provide ${API_KEY} before running"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_process_env_flagged() {
        let result = check(&["Access process.env.SECRET_TOKEN in your app"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_documented_with_export() {
        let result = check(&["export $DATABASE_URL before starting"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_documented_with_configure() {
        let result = check(&["Configure $API_KEY in your environment"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_documented_with_default() {
        let result = check(&["$API_KEY defaults to empty string"]);
        assert_eq!(result.diagnostics.len(), 0);
    }
}
