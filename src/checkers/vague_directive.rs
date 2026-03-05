use regex::{Regex, RegexBuilder};
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::{is_directive_line, NON_DIRECTIVE_CONTEXT};
use crate::types::{Category, CheckResult, Severity};

use super::utils::{ScopeFilter, REGEX_SIZE_LIMIT};
use super::Checker;

/// Hedging phrases enabled by `strict = true` (opt-in only).
static STRICT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:",
        r"when possible",
        r"|if possible",
        r"|when needed",
        r"|as needed",
        r"|when necessary",
        r"|consider",
        r"|ideally",
        r"|should be fine",
        r"|as much as possible",
        r")\b",
    ))
    .unwrap()
});

pub(crate) struct VagueDirectiveChecker {
    strict: bool,
    extra_patterns: Vec<Regex>,
    scope: ScopeFilter,
}

impl VagueDirectiveChecker {
    pub(crate) fn new(strict: bool, extra_patterns: &[String], scope_patterns: &[String]) -> Self {
        let extra_patterns = extra_patterns
            .iter()
            .filter_map(
                |p| match RegexBuilder::new(p).size_limit(REGEX_SIZE_LIMIT).build() {
                    Ok(r) => Some(r),
                    Err(e) => {
                        eprintln!("Warning: invalid vague_directive extra_pattern \"{p}\": {e}");
                        None
                    }
                },
            )
            .collect();
        Self {
            strict,
            extra_patterns,
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for VagueDirectiveChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        let has_additional = self.strict || !self.extra_patterns.is_empty();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }
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

            if has_additional {
                for (i, line) in file.non_code_lines() {
                    if !is_directive_line(line) {
                        continue;
                    }

                    if line.starts_with('#') {
                        continue;
                    }

                    if NON_DIRECTIVE_CONTEXT.is_match(line) {
                        continue;
                    }

                    let found = if self.strict {
                        STRICT_PATTERN.find(line)
                    } else {
                        None
                    }
                    .or_else(|| self.extra_patterns.iter().find_map(|p| p.find(line)));

                    if let Some(m) = found {
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
            in_code_block: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(false, &[], &[]);
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
            in_code_block: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(
            false,
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
            in_code_block: vec![],
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
            in_code_block: vec![],
        };

        let ctx = CheckerContext {
            files: vec![in_scope, out_of_scope],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(false, &[], &["CLAUDE.md".to_string()]);
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
            in_code_block: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        // Scope set to CLAUDE.md — file should be checked
        let checker = VagueDirectiveChecker::new(false, &[], &["CLAUDE.md".to_string()]);
        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "In-scope file should still produce diagnostics when scope is set"
        );
    }

    #[test]
    fn test_strict_mode_flags_additional_patterns() {
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
                "Do this when possible.".to_string(),
                "Run tests when needed.".to_string(),
                "Scale as needed.".to_string(),
                "Restart when necessary.".to_string(),
                "Consider using caching.".to_string(),
            ],
            in_code_block: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(true, &[], &[]);
        let result = checker.check(&ctx);

        assert_eq!(
            result.diagnostics.len(),
            5,
            "Strict mode should flag all 5 additional patterns"
        );
    }

    #[test]
    fn test_non_strict_does_not_flag_strict_patterns() {
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
                "Do this when possible.".to_string(),
                "Consider using caching.".to_string(),
            ],
            in_code_block: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(false, &[], &[]);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "Non-strict mode should not flag strict-only patterns"
        );
    }

    #[test]
    fn test_strict_mode_plus_extra_patterns() {
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
                "Do this when possible.".to_string(),
                "This is probably fine.".to_string(),
            ],
            in_code_block: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = VagueDirectiveChecker::new(true, &[r"(?i)\bprobably\b".to_string()], &[]);
        let result = checker.check(&ctx);

        assert_eq!(
            result.diagnostics.len(),
            2,
            "Strict mode + extra patterns should flag both"
        );
    }
}
