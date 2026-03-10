use regex::RegexBuilder;

use crate::config::CustomPattern;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::REGEX_SIZE_LIMIT;
use super::Checker;

pub(crate) struct CustomPatternChecker {
    patterns: Vec<CompiledPattern>,
}

struct CompiledPattern {
    name: String,
    regex: regex::Regex,
    severity: Severity,
    message: String,
}

impl CustomPatternChecker {
    pub(crate) fn new(configs: &[CustomPattern]) -> Self {
        let patterns = configs
            .iter()
            .filter_map(|c| {
                match RegexBuilder::new(&c.pattern)
                    .size_limit(REGEX_SIZE_LIMIT)
                    .build()
                {
                    Ok(regex) => Some(CompiledPattern {
                        name: c.name.clone(),
                        regex,
                        severity: c.severity,
                        message: c.message.clone(),
                    }),
                    Err(e) => {
                        tracing::warn!("Invalid regex pattern for custom rule '{}': {e}", c.name);
                        None
                    }
                }
            })
            .collect();
        Self { patterns }
    }
}

impl Checker for CustomPatternChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "custom",
            description: "User-defined regex patterns from config",
            default_severity: Severity::Warning,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            for (i, line) in file.non_code_lines() {
                for pattern in &self.patterns {
                    if pattern.regex.is_match(line) {
                        emit!(
                            result,
                            file.path,
                            i + 1,
                            pattern.severity,
                            Category::CustomPattern(pattern.name.as_str().into()),
                            "{}",
                            pattern.message
                        );
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

    fn run_check(lines: &[&str], patterns: &[CustomPattern]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        CustomPatternChecker::new(patterns).check(&ctx)
    }

    #[test]
    fn test_custom_pattern_detection() {
        let result = run_check(
            &["# Instructions", "TODO: fix this later", "This is fine"],
            &[CustomPattern {
                name: "todo-comment".to_string(),
                pattern: r"(?i)\bTODO\b".to_string(),
                severity: Severity::Warning,
                message: "TODO comment found".to_string(),
            }],
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].line, 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(
            result.diagnostics[0].category,
            Category::CustomPattern("todo-comment".into())
        );
    }

    #[test]
    fn test_custom_pattern_skips_code_blocks() {
        let result = run_check(
            &["```", "TODO: this is in code", "```"],
            &[CustomPattern {
                name: "todo".to_string(),
                pattern: r"(?i)\bTODO\b".to_string(),
                severity: Severity::Warning,
                message: "TODO found".to_string(),
            }],
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_invalid_regex_handled_gracefully() {
        let checker = CustomPatternChecker::new(&[CustomPattern {
            name: "bad".to_string(),
            pattern: r"[invalid".to_string(),
            severity: Severity::Warning,
            message: "bad pattern".to_string(),
        }]);
        assert!(checker.patterns.is_empty());
    }

    #[test]
    fn test_multiple_patterns_same_line() {
        let result = run_check(
            &["# Instructions", "TODO: FIXME: handle this"],
            &[
                CustomPattern {
                    name: "todo".to_string(),
                    pattern: r"(?i)\bTODO\b".to_string(),
                    severity: Severity::Warning,
                    message: "TODO found".to_string(),
                },
                CustomPattern {
                    name: "fixme".to_string(),
                    pattern: r"(?i)\bFIXME\b".to_string(),
                    severity: Severity::Error,
                    message: "FIXME found".to_string(),
                },
            ],
        );
        assert_eq!(
            result.diagnostics.len(),
            2,
            "Both patterns should match the same line, producing 2 diagnostics"
        );
        assert_eq!(result.diagnostics[0].line, result.diagnostics[1].line);
    }
}
