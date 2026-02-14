use regex::Regex;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::{is_directive_line, non_code_lines};
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct VagueDirectiveChecker {
    extra_patterns: Vec<Regex>,
    scope: ScopeFilter,
}

impl VagueDirectiveChecker {
    pub fn new(extra_patterns: &[String], scope_patterns: &[String]) -> Self {
        let extra_patterns = extra_patterns
            .iter()
            .filter_map(|p| match Regex::new(p) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("Warning: invalid vague_directive extra_pattern \"{p}\": {e}");
                    None
                }
            })
            .collect();
        Self {
            extra_patterns,
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for VagueDirectiveChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }
            // Built-in patterns (already parsed)
            for directive in &file.directives {
                emit!(
                    result,
                    file.path,
                    directive.line,
                    Severity::Info,
                    Category::VagueDirective,
                    suggest: "Replace with a specific, deterministic instruction",
                    "Non-deterministic directive found: \"{}\"",
                    directive.pattern_matched.trim()
                );
            }

            // Extra user-defined patterns
            if !self.extra_patterns.is_empty() {
                for (i, line) in non_code_lines(&file.raw_lines) {
                    if !is_directive_line(line) {
                        continue;
                    }

                    for pattern in &self.extra_patterns {
                        if let Some(m) = pattern.find(line) {
                            emit!(
                                result,
                                file.path,
                                i + 1,
                                Severity::Info,
                                Category::VagueDirective,
                                suggest: "Replace with a specific, deterministic instruction",
                                "Non-deterministic directive found: \"{}\"",
                                m.as_str().trim()
                            );
                            break;
                        }
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
    use crate::parser::types::{Directive, ParsedFile};
    use std::collections::HashSet;

    #[test]
    fn test_vague_directive_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("instructions.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![Directive {
                line: 5,
                pattern_matched: "Try to".to_string(),
            }],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(&[], &[]);
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::VagueDirective);
    }

    #[test]
    fn test_extra_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("instructions.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "You should maybe do this.".to_string(),
                "This is probably fine.".to_string(),
                "This line is clean.".to_string(),
            ],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(
            &[
                r"(?i)\bmaybe\b".to_string(),
                r"(?i)\bprobably\b".to_string(),
            ],
            &[],
        );
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 2);
        let messages: Vec<&str> = result
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            messages.iter().any(|m| m.contains("maybe")),
            "Expected 'maybe' in diagnostics"
        );
        assert!(
            messages.iter().any(|m| m.contains("probably")),
            "Expected 'probably' in diagnostics"
        );
        // Verify severity and category
        for d in &result.diagnostics {
            assert_eq!(d.severity, Severity::Info);
            assert_eq!(d.category, Category::VagueDirective);
        }
    }

    #[test]
    fn test_scope_limits_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let in_scope = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![Directive {
                line: 3,
                pattern_matched: "Try to".to_string(),
            }],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let out_of_scope = ParsedFile {
            path: root.join("reports/output.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![Directive {
                line: 5,
                pattern_matched: "Try to".to_string(),
            }],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![in_scope, out_of_scope],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(&[], &["CLAUDE.md".to_string()]);
        let result = checker.check(&ctx);

        assert_eq!(
            result.diagnostics.len(),
            1,
            "Only the in-scope file should produce diagnostics"
        );
        assert!(result.diagnostics[0]
            .file
            .to_string_lossy()
            .contains("CLAUDE.md"));
    }

    #[test]
    fn test_scope_includes_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![Directive {
                line: 3,
                pattern_matched: "Try to".to_string(),
            }],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        // Scope set to CLAUDE.md — file should be checked
        let checker = VagueDirectiveChecker::new(&[], &["CLAUDE.md".to_string()]);
        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "In-scope file should still produce diagnostics when scope is set"
        );
    }
}
