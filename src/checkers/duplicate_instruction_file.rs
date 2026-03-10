use std::collections::HashSet;

use rayon::prelude::*;

use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::parser::types::ParsedFile;
use crate::types::{Category, CheckResult, Diagnostic, RuleMeta, Severity};

use super::utils::{is_instruction_file, normalize_directive, ScopeFilter, MIN_DIRECTIVE_LINES};
use super::Checker;

/// Collect normalized directive lines for a file as a pre-computed HashSet.
fn collect_directive_set(file: &ParsedFile) -> HashSet<String> {
    file.non_code_lines()
        .filter(|(_, line)| is_directive_line(line) && !line.trim().is_empty())
        .map(|(_, line)| normalize_directive(line))
        .filter(|d| !d.is_empty())
        .collect()
}

pub(crate) struct DuplicateInstructionFileChecker {
    scope: ScopeFilter,
}

impl DuplicateInstructionFileChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Minimum overlap ratio to consider files as near-duplicates.
const OVERLAP_THRESHOLD: f64 = 0.7;

/// Check if file A references file B (parent/child relationship).
fn references_other(file: &ParsedFile, other: &ParsedFile) -> bool {
    let Some(other_name) = other.path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    file.file_refs.iter().any(|r| r.path.contains(other_name))
}

impl Checker for DuplicateInstructionFileChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "duplicate-instruction-file",
            description: "Flags near-duplicate instruction files",
            default_severity: Severity::Warning,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        if ctx.files.len() < 2 {
            return result;
        }

        // Pre-compute HashSets once per file (avoids re-hashing per pair).
        let file_directives: Vec<Option<HashSet<String>>> = ctx
            .files
            .iter()
            .map(|f| {
                if !self.scope.includes(&f.path, &ctx.project_root) {
                    return None;
                }
                if !is_instruction_file(&f.raw_lines, &f.in_code_block) {
                    return None;
                }
                let set = collect_directive_set(f);
                if set.len() < MIN_DIRECTIVE_LINES {
                    return None;
                }
                Some(set)
            })
            .collect();

        // Collect indices of files that have valid directive sets
        let valid_indices: Vec<usize> = file_directives
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| opt.as_ref().map(|_| i))
            .collect();

        // Generate all (i, j) pairs from valid indices where i < j
        let pairs: Vec<(usize, usize)> = valid_indices
            .iter()
            .enumerate()
            .flat_map(|(pos, &i)| valid_indices[pos + 1..].iter().map(move |&j| (i, j)))
            .collect();

        let pair_diagnostics: Vec<Diagnostic> = pairs
            .par_iter()
            .filter_map(|&(i, j)| {
                let set_a = file_directives[i].as_ref().unwrap();
                let set_b = file_directives[j].as_ref().unwrap();

                if references_other(&ctx.files[i], &ctx.files[j])
                    || references_other(&ctx.files[j], &ctx.files[i])
                {
                    return None;
                }

                let intersection = set_a.intersection(set_b).count();
                let min_size = set_a.len().min(set_b.len());

                if min_size == 0 {
                    return None;
                }

                let overlap = intersection as f64 / min_size as f64;

                if overlap >= OVERLAP_THRESHOLD {
                    let (emit_file, other_file) = if set_a.len() <= set_b.len() {
                        (&ctx.files[i], &ctx.files[j])
                    } else {
                        (&ctx.files[j], &ctx.files[i])
                    };

                    Some(Diagnostic {
                        file: emit_file.path.clone(),
                        line: 1,
                        column: None,
                        end_line: None,
                        end_column: None,
                        severity: Severity::Warning,
                        category: Category::DuplicateInstructionFile,
                        message: format!(
                            "{:.0}% overlap with {} \u{2014} these files may be near-duplicates",
                            overlap * 100.0,
                            other_file.path.display()
                        ),
                        suggestion: Some(
                            "Consolidate into one file or split responsibilities clearly"
                                .to_string(),
                        ),
                        fix: None,
                    })
                } else {
                    None
                }
            })
            .collect();

        result.diagnostics.extend(pair_diagnostics);

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
        let raw_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let in_code_block = crate::parser::build_code_block_mask(&raw_lines);
        ParsedFile {
            path: std::sync::Arc::new(root.join(name)),
            sections: vec![],
            tables: vec![],
            file_refs,
            directives: vec![],
            suppress_comments: vec![],
            raw_lines,
            in_code_block,
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
            canonical_root: None,
            filename_index: HashSet::new(),
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
            canonical_root: None,
            filename_index: HashSet::new(),
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
            canonical_root: None,
            filename_index: HashSet::new(),
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
            canonical_root: None,
            filename_index: HashSet::new(),
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
            canonical_root: None,
            filename_index: HashSet::new(),
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
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_same_name_different_dirs_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir_all(root.join("subA")).unwrap();
        std::fs::create_dir_all(root.join("subB")).unwrap();

        let lines = imperative_lines();

        let file_a = make_file(root, "subA/CLAUDE.md", &lines, vec![]);
        let file_b = make_file(root, "subB/CLAUDE.md", &lines, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Files with same name in different directories and identical content should flag"
        );
    }

    #[test]
    fn test_similar_but_not_matching_content_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Files with some overlap but well below the 70% threshold
        let lines_a = vec![
            "# Build Guidelines",
            "- Always run cargo build first",
            "- Never push broken builds",
            "- Use release mode for benchmarks",
            "- Ensure no compiler warnings",
            "- Must pass all CI checks",
            "- Run integration tests weekly",
        ];
        let lines_b = vec![
            "# Testing Guidelines",
            "- Always write property tests",
            "- Never mock database calls",
            "- Use snapshot testing for UI",
            "- Ensure edge cases covered",
            "- Must have regression tests",
            "- Run fuzz tests on parsers",
        ];

        let file_a = make_file(root, "CLAUDE.md", &lines_a, vec![]);
        let file_b = make_file(root, "AGENTS.md", &lines_b, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Files with similar structure but different directives should not flag"
        );
    }

    #[test]
    fn test_empty_files_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file_a = make_file(root, "CLAUDE.md", &[], vec![]);
        let file_b = make_file(root, "AGENTS.md", &[], vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Empty files should not flag as duplicates"
        );
    }

    #[test]
    fn test_partial_overlap_above_threshold_flags() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // 8 shared lines + 1 heading + 2 unique per file = 11 directives each
        // Intersection = 8, min_size = 11, overlap = 8/11 = 0.727 (> 0.7 threshold)
        let shared_lines = vec![
            "- Always run tests before committing",
            "- Never skip code review",
            "- Use descriptive variable names",
            "- Ensure all functions have docstrings",
            "- Avoid global state",
            "- Must follow naming conventions",
            "- Run linter before pushing",
            "- Always check for warnings",
        ];

        let mut lines_a: Vec<&str> = vec!["# Guidelines A"];
        lines_a.extend_from_slice(&shared_lines);
        lines_a.extend_from_slice(&[
            "- Always profile before optimizing",
            "- Never use raw SQL queries",
        ]);

        let mut lines_b: Vec<&str> = vec!["# Guidelines B"];
        lines_b.extend_from_slice(&shared_lines);
        lines_b.extend_from_slice(&[
            "- Always document API changes",
            "- Never expose internal errors",
        ]);

        let file_a = make_file(root, "CLAUDE.md", &lines_a, vec![]);
        let file_b = make_file(root, "AGENTS.md", &lines_b, vec![]);

        let ctx = CheckerContext {
            files: vec![file_a, file_b],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };

        let checker = DuplicateInstructionFileChecker::new(&[]);
        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Files with >70% overlap should flag"
        );
    }
}
