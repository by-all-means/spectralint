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

    #[test]
    fn test_invalid_regex_mixed_with_valid() {
        let checker = CustomPatternChecker::new(&[
            CustomPattern {
                name: "bad".to_string(),
                pattern: r"(unclosed".to_string(),
                severity: Severity::Warning,
                message: "bad pattern".to_string(),
            },
            CustomPattern {
                name: "good".to_string(),
                pattern: r"\bHACK\b".to_string(),
                severity: Severity::Warning,
                message: "HACK found".to_string(),
            },
        ]);
        assert_eq!(
            checker.patterns.len(),
            1,
            "Invalid regex should be skipped but valid patterns kept"
        );
        assert_eq!(checker.patterns[0].name, "good");
    }

    #[test]
    fn test_pattern_matching_across_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let raw_a: Vec<String> = vec!["# File A".to_string(), "HACK: quick workaround".to_string()];
        let raw_b: Vec<String> = vec![
            "# File B".to_string(),
            "This is clean".to_string(),
            "HACK: another one".to_string(),
        ];
        let mask_a = crate::parser::build_code_block_mask(&raw_a);
        let mask_b = crate::parser::build_code_block_mask(&raw_b);
        use crate::parser::types::ParsedFile;
        use std::collections::HashSet;
        let file_a = ParsedFile {
            path: std::sync::Arc::new(root.join("a.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: raw_a,
            in_code_block: mask_a,
        };
        let file_b = ParsedFile {
            path: std::sync::Arc::new(root.join("b.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: raw_b,
            in_code_block: mask_b,
        };
        let ctx = crate::engine::cross_ref::CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let patterns = vec![CustomPattern {
            name: "hack".to_string(),
            pattern: r"\bHACK\b".to_string(),
            severity: Severity::Warning,
            message: "HACK found".to_string(),
        }];
        let result = CustomPatternChecker::new(&patterns).check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            2,
            "Pattern should match in both files"
        );
    }

    #[test]
    fn test_case_sensitive_pattern() {
        // Pattern without (?i) should be case-sensitive
        let result = run_check(
            &["# Title", "todo: lowercase", "TODO: uppercase"],
            &[CustomPattern {
                name: "todo-exact".to_string(),
                pattern: r"\bTODO\b".to_string(),
                severity: Severity::Warning,
                message: "uppercase TODO found".to_string(),
            }],
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Case-sensitive pattern should only match exact case"
        );
        assert_eq!(result.diagnostics[0].line, 3);
    }

    #[test]
    fn test_pattern_in_fenced_code_block_skipped() {
        let result = run_check(
            &[
                "# Instructions",
                "```python",
                "# TODO: implement this function",
                "def foo(): pass",
                "```",
                "TODO: real task outside code block",
            ],
            &[CustomPattern {
                name: "todo".to_string(),
                pattern: r"\bTODO\b".to_string(),
                severity: Severity::Warning,
                message: "TODO found".to_string(),
            }],
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Pattern inside code block should be skipped, only prose match reported"
        );
        assert_eq!(result.diagnostics[0].line, 6);
    }

    #[test]
    fn test_no_patterns_no_diagnostics() {
        let result = run_check(&["# Title", "TODO: something"], &[]);
        assert!(
            result.diagnostics.is_empty(),
            "No patterns configured should produce no diagnostics"
        );
    }
}
