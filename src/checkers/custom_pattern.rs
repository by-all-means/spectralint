use regex::Regex;

use crate::config::CustomPattern;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines;
use crate::types::{Category, CheckResult, Severity};

use super::Checker;

pub struct CustomPatternChecker {
    patterns: Vec<CompiledPattern>,
}

struct CompiledPattern {
    name: String,
    regex: Regex,
    severity: Severity,
    message: String,
}

impl CustomPatternChecker {
    pub fn new(configs: &[CustomPattern]) -> Self {
        let patterns = configs
            .iter()
            .filter_map(|c| match Regex::new(&c.pattern) {
                Ok(regex) => Some(CompiledPattern {
                    name: c.name.clone(),
                    regex,
                    severity: c.severity,
                    message: c.message.clone(),
                }),
                Err(e) => {
                    eprintln!(
                        "Warning: invalid regex pattern for custom rule '{}': {e}",
                        c.name
                    );
                    None
                }
            })
            .collect();
        Self { patterns }
    }
}

impl Checker for CustomPatternChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            for (i, line) in non_code_lines(&file.raw_lines) {
                for pattern in &self.patterns {
                    if pattern.regex.is_match(line) {
                        emit!(
                            result,
                            file.path,
                            i + 1,
                            pattern.severity,
                            Category::CustomPattern(pattern.name.clone()),
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
    use crate::config::CustomPattern;
    use crate::parser::types::ParsedFile;
    use std::collections::HashSet;

    #[test]
    fn test_custom_pattern_detection() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "# Instructions".to_string(),
                "TODO: fix this later".to_string(),
                "This is fine".to_string(),
            ],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),

            historical_indices: HashSet::new(),
        };

        let checker = CustomPatternChecker::new(&[CustomPattern {
            name: "todo-comment".to_string(),
            pattern: r"(?i)\bTODO\b".to_string(),
            severity: Severity::Warning,
            message: "TODO comment found".to_string(),
        }]);

        let result = checker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].line, 2);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(
            result.diagnostics[0].category,
            Category::CustomPattern("todo-comment".to_string())
        );
    }

    #[test]
    fn test_custom_pattern_skips_code_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "```".to_string(),
                "TODO: this is in code".to_string(),
                "```".to_string(),
            ],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),

            historical_indices: HashSet::new(),
        };

        let checker = CustomPatternChecker::new(&[CustomPattern {
            name: "todo".to_string(),
            pattern: r"(?i)\bTODO\b".to_string(),
            severity: Severity::Warning,
            message: "TODO found".to_string(),
        }]);

        let result = checker.check(&ctx);
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

    // ── Item 10: Multiple custom patterns matching same line ─────────────

    #[test]
    fn test_multiple_patterns_same_line() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "# Instructions".to_string(),
                "TODO: FIXME: handle this".to_string(),
            ],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = CustomPatternChecker::new(&[
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
        ]);

        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            2,
            "Both patterns should match the same line, producing 2 diagnostics"
        );
        assert_eq!(result.diagnostics[0].line, result.diagnostics[1].line);
    }
}
