use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{is_heading, is_template_ref, is_within_project, ScopeFilter};
use super::Checker;

pub(crate) struct HardcodedFileStructureChecker {
    scope: ScopeFilter,
}

impl HardcodedFileStructureChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Source file extensions (non-.md — dead-reference handles .md files).
const SOURCE_EXTENSIONS: &[&str] = &[
    "ts",
    "tsx",
    "js",
    "jsx",
    "mjs",
    "cjs",
    "py",
    "rs",
    "go",
    "java",
    "kt",
    "rb",
    "php",
    "swift",
    "c",
    "cpp",
    "h",
    "hpp",
    "cs",
    "zig",
    "lua",
    "sh",
    "bash",
    "zsh",
    "yaml",
    "yml",
    "toml",
    "json",
    "xml",
    "html",
    "css",
    "scss",
    "sass",
    "less",
    "sql",
    "graphql",
    "proto",
    "tf",
    "dockerfile",
    "vue",
    "svelte",
    "astro",
    "ex",
    "exs",
    "erl",
    "hs",
    "ml",
    "mli",
    "r",
    "jl",
    "dart",
    "scala",
    "clj",
    "cljs",
    "elm",
    "nim",
];

/// Backtick-delimited path with directory component: `src/auth/handler.ts`
/// Requires at least one `/` to avoid FPs on bare filenames like `app.py`.
static BACKTICK_PATH: LazyLock<Regex> = LazyLock::new(|| {
    let exts = SOURCE_EXTENSIONS.join("|");
    Regex::new(&format!(r"`([^`\s]*/[^`\s]*\.(?:{exts}))`")).unwrap()
});

/// Bare path with directory component: src/auth/handler.ts
static BARE_PATH: LazyLock<Regex> = LazyLock::new(|| {
    let exts = SOURCE_EXTENSIONS.join("|");
    // Requires at least one `/` (directory component) to reduce FP
    Regex::new(&format!(
        r"(?:^|[\s,;(])([a-zA-Z0-9_.][a-zA-Z0-9_./\-]*?/[a-zA-Z0-9_.\-]+\.(?:{exts}))(?:[\s,;):]|$)"
    ))
    .unwrap()
});

/// Creation verb context — the file is being created, not referenced as dependency.
static CREATION_VERB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:create[sd]?|writ(?:e[sd]?|ing|ten)|generat(?:e[sd]?|ing)|sav(?:e[sd]?|ing)|outputs?|touch|mkdir|adds?)\b").unwrap()
});

/// Example/illustrative context.
static EXAMPLE_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:example|e\.g\.|such as|for instance|for example|like)\b").unwrap()
});

/// Paths that indicate the instruction file is a generic task template
/// (e.g., BMad tasks, plans, reports) where paths are illustrative.
static TEMPLATE_FILE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:/tasks/|/plans/|/reports/|/templates/|/prompts/|/commands/.*(?:tasks|bmad|agents|moai))").unwrap()
});

/// Returns true if the file path indicates a generic task template where
/// source paths are illustrative examples, not references to actual files.
fn is_template_instruction_file(file_path: &Path) -> bool {
    let path_str = file_path.to_string_lossy();
    TEMPLATE_FILE_PATTERN.is_match(&path_str)
}

fn should_skip_line(line: &str) -> bool {
    is_heading(line)
        || CREATION_VERB.is_match(line)
        || EXAMPLE_CONTEXT.is_match(line)
        || (line.contains("](") && line.contains('['))
}

/// Try to resolve a path to check if it exists on disk.
/// 1. Source-relative  2. Root-relative  3. Basename in tree index (O(1) lookup)
fn path_exists(
    ref_path: &str,
    source_dir: &Path,
    canonical_root: Option<&Path>,
    project_root: &Path,
    filename_index: &HashSet<String>,
) -> bool {
    // 1. Source-relative
    let source_resolved = source_dir.join(ref_path);
    if source_resolved.exists() && is_within_project(&source_resolved, canonical_root, project_root)
    {
        return true;
    }

    // 2. Root-relative
    let root_resolved = project_root.join(ref_path);
    if root_resolved.exists() && is_within_project(&root_resolved, canonical_root, project_root) {
        return true;
    }

    // 3. Basename exists somewhere in tree (catches sub-package relative paths)
    let basename = ref_path.rsplit('/').next().unwrap_or(ref_path);
    filename_index.contains(basename)
}

impl Checker for HardcodedFileStructureChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "hardcoded-file-structure",
            description: "Flags references to non-.md source files that don't exist",
            default_severity: Severity::Info,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Skip generic task templates where paths are illustrative
            if is_template_instruction_file(&file.path) {
                continue;
            }

            let source_dir = file.path.parent().unwrap_or(&ctx.project_root);

            for (idx, line) in file.non_code_lines() {
                let line_num = idx + 1;

                if should_skip_line(line) {
                    continue;
                }

                // Check backtick-delimited paths
                for caps in BACKTICK_PATH.captures_iter(line) {
                    let ref_path = &caps[1];
                    check_path(
                        ref_path,
                        source_dir,
                        ctx.canonical_root.as_deref(),
                        &ctx.project_root,
                        &ctx.filename_index,
                        &file.path,
                        line_num,
                        &mut result,
                    );
                }

                // Check bare paths (only if not already caught by backtick)
                for caps in BARE_PATH.captures_iter(line) {
                    let ref_path = &caps[1];
                    // Skip if this path was already inside backticks on this line
                    if line.contains(&format!("`{ref_path}`")) {
                        continue;
                    }
                    // Skip "TypeScript/Node.js" style tech-name slashes (all components uppercase)
                    if ref_path
                        .split('/')
                        .all(|seg| seg.starts_with(|c: char| c.is_ascii_uppercase()))
                    {
                        continue;
                    }
                    check_path(
                        ref_path,
                        source_dir,
                        ctx.canonical_root.as_deref(),
                        &ctx.project_root,
                        &ctx.filename_index,
                        &file.path,
                        line_num,
                        &mut result,
                    );
                }
            }
        }

        result
    }
}

#[allow(clippy::too_many_arguments)]
fn check_path(
    ref_path: &str,
    source_dir: &Path,
    canonical_root: Option<&Path>,
    project_root: &Path,
    filename_index: &HashSet<String>,
    file_path: &Path,
    line_num: usize,
    result: &mut CheckResult,
) {
    if is_template_ref(ref_path) {
        return;
    }

    if path_exists(
        ref_path,
        source_dir,
        canonical_root,
        project_root,
        filename_index,
    ) {
        return;
    }

    emit!(
        result,
        Arc::new(file_path.to_path_buf()),
        line_num,
        Severity::Info,
        Category::HardcodedFileStructure,
        suggest: "Verify the path exists or update to match the current project structure.",
        "Hardcoded source path not found: `{}`",
        ref_path
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        HardcodedFileStructureChecker::new(&[]).check(&ctx)
    }

    fn run_check_with_file(lines: &[&str], create_file: &str) -> CheckResult {
        let (dir, ctx) = single_file_ctx(lines);
        // Create the referenced file on disk
        let path = dir.path().join(create_file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "").unwrap();
        HardcodedFileStructureChecker::new(&[]).check(&ctx)
    }

    // ── Positive cases (file doesn't exist) ──

    #[test]
    fn test_backtick_path_missing() {
        let result = run_check(&["- Auth logic lives in `src/auth/handler.ts`"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .message
            .contains("src/auth/handler.ts"));
    }

    #[test]
    fn test_bare_path_missing() {
        let result = run_check(&["- The config is at src/config/db.py"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    // ── File exists (no flag) ──

    #[test]
    fn test_file_exists_no_flag() {
        let result = run_check_with_file(
            &["- Auth logic lives in `src/auth/handler.ts`"],
            "src/auth/handler.ts",
        );
        assert!(result.diagnostics.is_empty());
    }

    // ── FP exclusions ──

    #[test]
    fn test_creation_verb_no_flag() {
        let result = run_check(&["- Create a file at `src/auth/handler.ts`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_example_context_no_flag() {
        let result = run_check(&["- For example, `src/auth/handler.ts`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_heading_no_flag() {
        let result = run_check(&["## src/auth/handler.ts"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_no_flag() {
        let result = run_check(&["```", "src/auth/handler.ts", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_template_ref_no_flag() {
        let result = run_check(&["- Check `src/*/handler.ts`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_markdown_link_no_flag() {
        let result = run_check(&["- See [handler](src/auth/handler.ts)"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_md_extension_no_flag() {
        // .md files are dead-reference's job
        let result = run_check(&["- See `docs/setup.md`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_directory_component_no_flag() {
        // Bare filenames without a directory component should not be flagged
        let result = run_check(&["- Edit handler.ts"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_clean_file_no_flag() {
        let result = run_check(&[
            "# Build",
            "- Run `cargo test` before committing",
            "- Never push directly to main",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_paths_on_line() {
        let result = run_check(&["- Update `src/auth/login.ts` and `src/auth/logout.ts`"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_generate_verb_no_flag() {
        let result = run_check(&["- Generate `src/models/user.py`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_basename_exists_in_tree_no_flag() {
        // File exists at a different relative path (sub-package root scenario)
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Create the file at a deeper path
        let path = root.join("packages/next/src/cli/main.ts");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
        // Create the context with the file already present
        let raw_lines = vec!["- Entry point is `src/cli/main.ts`".to_string()];
        let in_code_block = crate::parser::build_code_block_mask(&raw_lines);
        let file = crate::parser::types::ParsedFile {
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines,
            in_code_block,
        };
        let canonical_root = root.canonicalize().ok();
        let filename_index = crate::engine::cross_ref::build_filename_index(root);
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root,
            filename_index,
            historical_indices: HashSet::new(),
        };
        let result = HardcodedFileStructureChecker::new(&[]).check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Should not flag when basename exists in tree"
        );
    }

    #[test]
    fn test_extension_list_no_flag() {
        let result = run_check(&["- Use TypeScript (`.ts/.tsx`) for components"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_placeholder_path_to_no_flag() {
        let result = run_check(&["- Single test: `tests/path/to/test_file.py`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_placeholder_xxx_no_flag() {
        let result = run_check(&["- Create step definitions (`src/steps/xxx/xxx.steps.ts`)"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_tech_name_slash_no_flag() {
        // "TypeScript/Node.js" is tech names with a slash, not a file path
        let result = run_check(&["- TypeScript/Node.js implementation"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_template_task_file_no_flag() {
        assert!(is_template_instruction_file(Path::new(
            ".claude/commands/BMad/tasks/apply-qa-fixes.md"
        )));
        assert!(is_template_instruction_file(Path::new(
            ".claude/plans/animated-installer.md"
        )));
        assert!(is_template_instruction_file(Path::new(
            ".claude/reports/test-audit.md"
        )));
        assert!(is_template_instruction_file(Path::new(
            ".claude/prompts/nl-unity-suite-t.md"
        )));
        assert!(is_template_instruction_file(Path::new(
            ".claude/commands/moai/1-plan.md"
        )));
        // Regular instruction files should NOT be skipped
        assert!(!is_template_instruction_file(Path::new("CLAUDE.md")));
        assert!(!is_template_instruction_file(Path::new("AGENTS.md")));
        assert!(!is_template_instruction_file(Path::new(
            ".claude/skills/my-skill/SKILL.md"
        )));
        assert!(!is_template_instruction_file(Path::new(
            ".claude/commands/deploy.md"
        )));
    }
}
