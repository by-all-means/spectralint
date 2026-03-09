use std::sync::Arc;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::Checker;

pub(crate) struct MissingStandardFileChecker;

/// Check whether the directory looks like a real project root.
///
/// We look for common project-root signals:
/// - `.git` directory
/// - Package manager / build manifests (Cargo.toml, package.json, go.mod, etc.)
///
/// If none of these exist the directory is likely a temp dir, subdirectory, or
/// bare folder — and we should not flag it for missing CLAUDE.md.
fn looks_like_project_root(root: &std::path::Path) -> bool {
    const PROJECT_SIGNALS: &[&str] = &[
        ".git",
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "Gemfile",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "CMakeLists.txt",
        "Makefile",
        "flake.nix",
        "deno.json",
        "composer.json",
        "mix.exs",
        "Project.toml",
        ".hg",
    ];

    PROJECT_SIGNALS.iter().any(|s| root.join(s).exists())
}

impl Checker for MissingStandardFileChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "missing-standard-file",
            description: "Flags projects missing common instruction files",
            default_severity: Severity::Info,
            strict_only: true,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        if ctx.files.is_empty() {
            return result;
        }

        let has_claude_md = ctx.files.iter().any(|f| {
            f.path
                .strip_prefix(&ctx.project_root)
                .ok()
                .is_some_and(|p| p.to_string_lossy().eq_ignore_ascii_case("CLAUDE.md"))
        });
        let has_settings = ctx.project_root.join(".claude/settings.json").exists();

        if !has_claude_md && looks_like_project_root(&ctx.project_root) {
            emit!(
                result,
                Arc::new(ctx.project_root.join("CLAUDE.md")),
                0,
                Severity::Info,
                Category::MissingStandardFile,
                suggest: "Create a CLAUDE.md as the primary instruction file for AI agents",
                "Project has instruction files but no CLAUDE.md"
            );
        }

        let has_claude_dir = ctx.project_root.join(".claude").is_dir();

        if has_claude_md && has_claude_dir && !has_settings {
            emit!(
                result,
                Arc::new(ctx.project_root.join(".claude/settings.json")),
                0,
                Severity::Info,
                Category::MissingStandardFile,
                suggest: "Consider adding .claude/settings.json for tool permissions and project settings",
                "Project has CLAUDE.md but no .claude/settings.json"
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::ParsedFile;
    use std::collections::HashSet;

    fn make_file(root: &std::path::Path, name: &str) -> ParsedFile {
        ParsedFile {
            path: Arc::new(root.join(name)),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["# Test".to_string()],
            in_code_block: vec![false],
        }
    }

    #[test]
    fn test_missing_claude_md() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Create a .git directory so the checker recognises this as a project root
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let files = vec![make_file(root, "AGENTS.md")];
        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingStandardFileChecker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("no CLAUDE.md"));
    }

    #[test]
    fn test_bare_directory_not_flagged() {
        // A directory with no .git, no manifest files — not a project root.
        // The checker should NOT emit a missing-CLAUDE.md diagnostic here.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![make_file(root, "AGENTS.md")];
        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingStandardFileChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Bare directory without project root signals should not be flagged"
        );
    }

    #[test]
    fn test_has_claude_md_no_claude_dir_no_diagnostic() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![make_file(root, "CLAUDE.md")];
        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingStandardFileChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "No .claude/ dir means no suggestion for settings.json"
        );
    }

    #[test]
    fn test_has_claude_dir_but_no_settings() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        let files = vec![make_file(root, "CLAUDE.md")];
        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingStandardFileChecker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .message
            .contains(".claude/settings.json"));
    }

    #[test]
    fn test_empty_files_no_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let ctx = CheckerContext {
            files: vec![],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingStandardFileChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "No files means no diagnostics"
        );
    }

    #[test]
    fn test_case_insensitive_claude_md() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // lowercase claude.md should still count as CLAUDE.md
        let files = vec![make_file(root, "claude.md")];
        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingStandardFileChecker.check(&ctx);
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("no CLAUDE.md")),
            "claude.md (lowercase) should count as CLAUDE.md"
        );
    }

    #[test]
    fn test_has_claude_md_and_settings() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        std::fs::write(root.join(".claude/settings.json"), "{}").unwrap();
        let files = vec![make_file(root, "CLAUDE.md")];
        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingStandardFileChecker.check(&ctx);
        assert!(result.diagnostics.is_empty());
    }
}
