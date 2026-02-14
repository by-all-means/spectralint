use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::Checker;

pub struct DeadReferenceChecker;

impl Checker for DeadReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for (file_idx, file) in ctx.files.iter().enumerate() {
            if ctx.historical_indices.contains(&file_idx) {
                continue;
            }

            for file_ref in &file.file_refs {
                // Skip template/glob patterns like commands/[command].md or *.md
                if file_ref.path.contains(['*', '[', '{']) {
                    continue;
                }

                // Try relative to source file's directory first, then project root
                let source_dir = file_ref.source_file.parent().unwrap_or(&ctx.project_root);
                let resolved_local = source_dir.join(&file_ref.path);
                let resolved_root = ctx.project_root.join(&file_ref.path);
                if !resolved_local.exists() && !resolved_root.exists() {
                    emit!(
                        result,
                        file_ref.source_file,
                        file_ref.line,
                        Severity::Error,
                        Category::DeadReference,
                        suggest: "Remove this reference or create the missing file",
                        "\"{}\" does not exist",
                        file_ref.path
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
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn test_dead_reference_detected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "agent_definitions/followup_drafter.md".to_string(),
                line: 56,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),

            historical_indices: HashSet::new(),
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(result.diagnostics[0].category, Category::DeadReference);
    }

    #[test]
    fn test_glob_pattern_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "commands/[command].md".to_string(),
                    line: 10,
                    source_file: root.join("CLAUDE.md"),
                },
                FileRef {
                    path: "agent_definitions/*.md".to_string(),
                    line: 20,
                    source_file: root.join("CLAUDE.md"),
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),

            historical_indices: HashSet::new(),
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Glob/template patterns should not be flagged"
        );
    }

    #[test]
    fn test_relative_to_source_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create docs/scout.md
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/scout.md"), "# Scout").unwrap();

        // docs/AGENTS.md references "scout.md" — should resolve to docs/scout.md
        let parsed = ParsedFile {
            path: root.join("docs/AGENTS.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "scout.md".to_string(),
                line: 5,
                source_file: root.join("docs/AGENTS.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Reference relative to source file should resolve"
        );
    }

    #[test]
    fn test_historical_file_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("changelog.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "nonexistent.md".to_string(),
                line: 5,
                source_file: root.join("changelog.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let mut historical = HashSet::new();
        historical.insert(0); // mark first file as historical

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: historical,
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Historical files should not produce dead-reference diagnostics"
        );
    }

    #[test]
    fn test_existing_reference_ok() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("agent_definitions")).unwrap();
        fs::write(root.join("agent_definitions/scout.md"), "# Scout").unwrap();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "agent_definitions/scout.md".to_string(),
                line: 10,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 0);
    }

    // ── Item 4: Dead reference edge cases ────────────────────────────────

    #[test]
    fn test_parent_relative_reference() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create root/sibling.md
        fs::write(root.join("sibling.md"), "# Sibling").unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();

        // docs/AGENTS.md references "../sibling.md"
        let parsed = ParsedFile {
            path: root.join("docs/AGENTS.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "../sibling.md".to_string(),
                line: 5,
                source_file: root.join("docs/AGENTS.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "../sibling.md should resolve relative to source file"
        );
    }

    #[test]
    fn test_broken_parent_relative_reference() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("docs")).unwrap();

        let parsed = ParsedFile {
            path: root.join("docs/AGENTS.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "../nonexistent.md".to_string(),
                line: 5,
                source_file: root.join("docs/AGENTS.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Broken ../ref should be flagged"
        );
    }

    #[test]
    fn test_curly_brace_pattern_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "templates/{name}.md".to_string(),
                line: 10,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Curly brace template patterns should be skipped"
        );
    }
}
