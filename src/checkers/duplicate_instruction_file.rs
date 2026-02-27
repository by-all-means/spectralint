use std::collections::HashSet;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::types::ParsedFile;
use crate::parser::{is_directive_line, non_code_lines};
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_instruction_file, ScopeFilter, MIN_DIRECTIVE_LINES};
use super::Checker;

pub struct DuplicateInstructionFileChecker {
    scope: ScopeFilter,
}

impl DuplicateInstructionFileChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Minimum overlap ratio to consider files as near-duplicates.
const OVERLAP_THRESHOLD: f64 = 0.7;

/// Normalize a directive line for comparison: strip list markers, lowercase, collapse whitespace.
fn normalize_directive(line: &str) -> String {
    let trimmed = line.trim();
    let stripped = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| {
            // Strip numbered list markers like "1. ", "12. "
            let after_digits = trimmed.trim_start_matches(|c: char| c.is_ascii_digit());
            (after_digits.len() < trimmed.len())
                .then(|| after_digits.strip_prefix(". "))
                .flatten()
        })
        .unwrap_or(trimmed);

    stripped
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Collect normalized directive lines for a file.
fn collect_directives(raw_lines: &[String]) -> Vec<String> {
    non_code_lines(raw_lines)
        .filter(|(_, line)| is_directive_line(line) && !line.trim().is_empty())
        .map(|(_, line)| normalize_directive(line))
        .filter(|d| !d.is_empty())
        .collect()
}

/// Check if file A references file B (parent/child relationship).
fn references_other(file: &ParsedFile, other: &ParsedFile) -> bool {
    let other_name = other
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if other_name.is_empty() {
        return false;
    }
    file.file_refs.iter().any(|r| r.path.contains(other_name))
}

impl Checker for DuplicateInstructionFileChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        if ctx.files.len() < 2 {
            return result;
        }

        let file_directives: Vec<Option<Vec<String>>> = ctx
            .files
            .iter()
            .map(|f| {
                if !self.scope.includes(&f.path, &ctx.project_root) {
                    return None;
                }
                if !is_instruction_file(&f.raw_lines) {
                    return None;
                }
                let directives = collect_directives(&f.raw_lines);
                if directives.len() < MIN_DIRECTIVE_LINES {
                    return None;
                }
                Some(directives)
            })
            .collect();

        for (i, directives_a_opt) in file_directives.iter().enumerate() {
            let Some(ref directives_a) = directives_a_opt else {
                continue;
            };
            for (j, directives_b_opt) in file_directives.iter().enumerate().skip(i + 1) {
                let Some(ref directives_b) = directives_b_opt else {
                    continue;
                };

                if references_other(&ctx.files[i], &ctx.files[j])
                    || references_other(&ctx.files[j], &ctx.files[i])
                {
                    continue;
                }

                let set_a: HashSet<&str> = directives_a.iter().map(|s| s.as_str()).collect();
                let set_b: HashSet<&str> = directives_b.iter().map(|s| s.as_str()).collect();

                let intersection = set_a.intersection(&set_b).count();
                let min_size = set_a.len().min(set_b.len());

                if min_size == 0 {
                    continue;
                }

                let overlap = intersection as f64 / min_size as f64;

                if overlap >= OVERLAP_THRESHOLD {
                    let (emit_file, other_file) = if directives_a.len() <= directives_b.len() {
                        (&ctx.files[i], &ctx.files[j])
                    } else {
                        (&ctx.files[j], &ctx.files[i])
                    };

                    emit!(
                        result,
                        emit_file.path,
                        1,
                        Severity::Warning,
                        Category::DuplicateInstructionFile,
                        suggest: "Consolidate into one file or split responsibilities clearly",
                        "{:.0}% overlap with {} — these files may be near-duplicates",
                        overlap * 100.0,
                        other_file.path.display()
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
    use crate::parser::types::{FileRef, ParsedFile};

    fn make_file(
        root: &std::path::Path,
        name: &str,
        lines: &[&str],
        file_refs: Vec<FileRef>,
    ) -> ParsedFile {
        ParsedFile {
            path: root.join(name),
            sections: vec![],
            tables: vec![],
            file_refs,
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn imperative_lines() -> Vec<&'static str> {
        vec![
            "# Guidelines",
            "- Always run tests before committing",
            "- Never skip code review",
            "- Use descriptive variable names",
            "- Ensure all functions have docstrings",
            "- Avoid global state",
            "- Must follow naming conventions",
            "- Run linter before pushing",
        ]
    }

    #[test]
    fn test_high_overlap_flags() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let lines_a = imperative_lines();
        let lines_b = imperative_lines(); // Identical

        let file_a = make_file(root, "CLAUDE.md", &lines_a, vec![]);
        let file_b = make_file(root, "AGENTS.md", &lines_b, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_low_overlap_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let lines_a = vec![
            "# Build",
            "- Always run cargo build",
            "- Never skip compilation",
            "- Use release mode for production",
            "- Ensure no warnings",
            "- Must pass clippy",
        ];
        let lines_b = vec![
            "# Testing",
            "- Always write unit tests",
            "- Never commit without tests",
            "- Use mocking for external services",
            "- Ensure coverage above 80%",
            "- Must run integration tests",
        ];

        let file_a = make_file(root, "CLAUDE.md", &lines_a, vec![]);
        let file_b = make_file(root, "AGENTS.md", &lines_b, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_few_directives_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let lines = vec!["# Short", "- Always do X", "- Never do Y"];

        let file_a = make_file(root, "CLAUDE.md", &lines, vec![]);
        let file_b = make_file(root, "AGENTS.md", &lines, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_parent_child_reference_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let lines = imperative_lines();

        let file_a = make_file(
            root,
            "CLAUDE.md",
            &lines,
            vec![FileRef {
                path: "AGENTS.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
            }],
        );
        let file_b = make_file(root, "AGENTS.md", &lines, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_self_comparison_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let lines = imperative_lines();
        let file_a = make_file(root, "CLAUDE.md", &lines, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_three_files_only_matching_pair_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let lines_ab = imperative_lines();
        let lines_c = vec![
            "# Deployment",
            "- Always use staging first",
            "- Never deploy on Fridays",
            "- Use blue-green deployment",
            "- Ensure rollback plan exists",
            "- Must monitor after deploy",
        ];

        let file_a = make_file(root, "CLAUDE.md", &lines_ab, vec![]);
        let file_b = make_file(root, "AGENTS.md", &lines_ab, vec![]);
        let file_c = make_file(root, "DEPLOY.md", &lines_c, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b, file_c],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
