use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::Checker;

/// Lines containing these verbs indicate the referenced file is being
/// created, written, or deleted as part of a workflow — not a dependency
/// that should already exist on disk.
static ACTION_VERB_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:create|write|generate|save|output|delete|remove|adding)\b").unwrap()
});

/// "a file called X.md" or "a file named X.md" — the file is being created.
static FILE_CALLED_NAMED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\b(?:called|named)\s+["'`]?[\w./-]+\.md"#).unwrap());

/// Lines that indicate the reference is an example, not a real dependency.
static EXAMPLE_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:examples|example|e\.g\.(?:\s|,)|such as|for instance|for example)")
        .unwrap()
});

/// Lines containing arrow mappings (→, ->, =>) are routing/mapping tables,
/// not actual file dependencies. e.g. "src/commands/init.ts → cli/init.md"
static ARROW_MAPPING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"→|->|=>").unwrap());

/// Lines in exclusion/out-of-scope sections — files listed as things NOT to
/// touch, not dependencies.
static EXCLUSION_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:out[- ]of[- ]scope|do not modify|do not edit|do not touch|don't modify|don't edit|don't touch|excluded?|ignore)\b").unwrap()
});

/// Naming convention / documentation list context — the reference is listing
/// file formats or supported filenames, not actual dependencies.
/// e.g., "Zed recognizes these files (in priority order):"
static CONVENTION_LIST_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:recognizes?\s+(?:these|the(?:se)?\s+)?files?|supported\s+files?|file\s+(?:formats?|types?|naming|names?)|naming\s+conventions?|instruction\s+files?|in\s+priority\s+order)\b").unwrap()
});

/// Backtick-delimited directory path (ending with `/`).
/// Used to detect "directory context" in preceding lines, e.g.:
///   `src/templates/`:  — sub-items are relative to this directory
static DIR_CONTEXT_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+/)`").unwrap());

/// Lines describing naming conventions — the filename is an example, not a real reference.
/// e.g., "**Naming**: Use kebab-case: `optimize-images.md`"
static NAMING_CONVENTION_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:kebab[- ]?case|camel[- ]?case|snake[- ]?case|pascal[- ]?case|naming\s*:)")
        .unwrap()
});

/// Generic placeholder filenames that are clearly not real files.
static PLACEHOLDER_FILENAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:FILE\.(?:md|zh\.md)|filename\.md|ref\d+\.md)$").unwrap());

/// Directories to skip when searching for convention references.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "build",
    "dist",
    ".next",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
];

/// Check if a file with the given name exists anywhere under `root`,
/// skipping common non-source directories. Used to detect convention
/// references like `SKILL.md` that exist in many subdirectories.
fn file_exists_in_tree(root: &Path, filename: &str) -> bool {
    fn walk(dir: &Path, target: &str) -> bool {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return false,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if SKIP_DIRS.contains(&name_str.as_ref()) {
                    continue;
                }
                if walk(&entry.path(), target) {
                    return true;
                }
            } else if name_str == target {
                return true;
            }
        }
        false
    }
    walk(root, filename)
}

/// Check if any line within a window around `line_idx` (0-based) matches `pattern`.
/// Includes `before` lines before and `after` lines after `line_idx`.
fn has_nearby_match(
    lines: &[String],
    line_idx: usize,
    before: usize,
    after: usize,
    pattern: &Regex,
) -> bool {
    let start = line_idx.saturating_sub(before);
    let end = (line_idx + after).min(lines.len());
    start < end && lines[start..end].iter().any(|l| pattern.is_match(l))
}

/// Check if a preceding line establishes a directory context and the file
/// reference resolves relative to that directory.
/// e.g., line N says "Edit in `src/templates/`:" and line N+1 lists `base/foo.md`
/// → try resolving `src/templates/base/foo.md` from the project root.
fn resolves_via_dir_context(
    lines: &[String],
    line_idx: usize,
    ref_path: &str,
    project_root: &Path,
) -> bool {
    let start = line_idx.saturating_sub(3);
    for i in start..line_idx {
        if let Some(line) = lines.get(i) {
            for caps in DIR_CONTEXT_PATH.captures_iter(line) {
                let dir = &caps[1];
                let resolved = project_root.join(dir).join(ref_path);
                if resolved.exists() {
                    return true;
                }
            }
        }
    }
    false
}

pub struct DeadReferenceChecker;

impl Checker for DeadReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for (file_idx, file) in ctx.files.iter().enumerate() {
            if ctx.historical_indices.contains(&file_idx) {
                continue;
            }

            for file_ref in &file.file_refs {
                // Skip template/glob patterns like commands/[command].md, *.md,
                // <skill-name>/SKILL.md, or path/to/agent.md
                if file_ref.path.contains(['*', '[', '{', '<', '>']) {
                    continue;
                }

                // Skip home directory references (~/.claude/CLAUDE.md)
                if file_ref.path.starts_with('~') {
                    continue;
                }

                // Skip absolute paths (/Users/drew/code/basic-memory/CHANGELOG.md)
                if file_ref.path.starts_with('/') {
                    continue;
                }

                // Skip paths containing shell variables ($ARGUMENTS, $MD_OUT)
                if file_ref.path.contains('$') {
                    continue;
                }

                // Skip obvious template/example paths
                if file_ref.path.starts_with("path/to/")
                    || file_ref.path.starts_with("@")
                    || file_ref.path.starts_with("example/")
                {
                    continue;
                }

                // Skip generic placeholder filenames (FILE.md, filename.md, ref1.md)
                let basename = file_ref.path.rsplit('/').next().unwrap_or(&file_ref.path);
                if PLACEHOLDER_FILENAME.is_match(basename) {
                    continue;
                }

                // Skip files inside templates/ directories — paths are meant
                // to be resolved after the template is instantiated elsewhere.
                if file_ref
                    .source_file
                    .components()
                    .any(|c| c.as_os_str() == "templates")
                {
                    continue;
                }

                // Try relative to source file's directory first, then project root
                let source_dir = file_ref.source_file.parent().unwrap_or(&ctx.project_root);
                let resolved_local = source_dir.join(&file_ref.path);
                let resolved_root = ctx.project_root.join(&file_ref.path);
                if resolved_local.exists() || resolved_root.exists() {
                    continue;
                }

                // For bare filenames (no directory component), check if the file
                // exists anywhere in the project tree. If so, it's a convention
                // reference (e.g., `SKILL.md` mentioned generically when the file
                // exists in many subdirectories).
                if !file_ref.path.contains('/')
                    && file_exists_in_tree(&ctx.project_root, &file_ref.path)
                {
                    continue;
                }

                let line_idx = file_ref.line.saturating_sub(1);

                if let Some(line_content) = file.raw_lines.get(line_idx) {
                    // Skip procedural references — lines where the file is being
                    // created, written, or deleted as part of a workflow instruction.
                    // Also skip example/illustrative references and arrow mappings.
                    if ACTION_VERB_LINE.is_match(line_content)
                        || FILE_CALLED_NAMED.is_match(line_content)
                        || EXAMPLE_CONTEXT.is_match(line_content)
                        || ARROW_MAPPING.is_match(line_content)
                        || NAMING_CONVENTION_LINE.is_match(line_content)
                    {
                        continue;
                    }

                    // Skip references inside quoted strings — these are illustrative
                    // examples of what output should look like, not real dependencies.
                    if let Some(pos) = line_content.find(&file_ref.path) {
                        let before = &line_content[..pos];
                        if before.chars().filter(|&c| c == '"').count() % 2 == 1 {
                            continue;
                        }

                        // Skip references inside backtick-delimited inline code that
                        // demonstrate markdown link syntax, not real dependencies.
                        // e.g., `[Agent Panel](./agent-panel.md)`
                        // BUT: don't skip plain backtick-quoted file references like
                        // `agent.md` which are legitimate references in markdown.
                        let after = &line_content[pos + file_ref.path.len()..];
                        if before.contains('`') && after.contains('`') {
                            let backticks_before = before.chars().filter(|&c| c == '`').count();
                            if backticks_before % 2 == 1 && before.contains("](") {
                                continue;
                            }
                        }
                    }
                }

                // Check a ±3 line window for example context (headings like
                // "**Examples**:" often appear a line or two before the references).
                if has_nearby_match(&file.raw_lines, line_idx, 3, 3, &EXAMPLE_CONTEXT) {
                    continue;
                }

                // Check a window (-5/+2) for exclusion context (headings like
                // "Out-of-Scope", "Do Not Modify") — files listed as things to
                // avoid, not dependencies.
                if has_nearby_match(&file.raw_lines, line_idx, 5, 2, &EXCLUSION_CONTEXT) {
                    continue;
                }

                // Check a window (-10/+3) for naming convention / documentation
                // list context — e.g., "Zed recognizes these files".
                if has_nearby_match(&file.raw_lines, line_idx, 10, 3, &CONVENTION_LIST_CONTEXT) {
                    continue;
                }

                // Check if a preceding line establishes a directory context
                // and the reference resolves relative to that directory.
                // e.g., "Edit in `src/templates/`:" → `base/foo.md` resolves
                // to `src/templates/base/foo.md`.
                if resolves_via_dir_context(
                    &file.raw_lines,
                    line_idx,
                    &file_ref.path,
                    &ctx.project_root,
                ) {
                    continue;
                }

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

    #[test]
    fn test_angle_bracket_template_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: ".agent/skills/<skill-name>/SKILL.md".to_string(),
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
            "Angle bracket template paths should be skipped"
        );
    }

    #[test]
    fn test_home_directory_path_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "~/.claude/CLAUDE.md".to_string(),
                line: 4,
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
            "Home directory references should be skipped"
        );
    }

    #[test]
    fn test_example_path_to_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "path/to/agent.md".to_string(),
                line: 62,
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
            "path/to/ example paths should be skipped"
        );
    }

    #[test]
    fn test_at_prefix_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "@playbook-name.md".to_string(),
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
            "@ prefix paths should be skipped"
        );
    }

    #[test]
    fn test_convention_reference_bare_filename_in_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create SKILL.md in multiple subdirectories (convention pattern)
        fs::create_dir_all(root.join("skills/agent-a")).unwrap();
        fs::create_dir_all(root.join("skills/agent-b")).unwrap();
        fs::write(root.join("skills/agent-a/SKILL.md"), "# A").unwrap();
        fs::write(root.join("skills/agent-b/SKILL.md"), "# B").unwrap();

        // CLAUDE.md references bare "SKILL.md" (convention, not a root file)
        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "SKILL.md".to_string(),
                line: 31,
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
            "Bare filename that exists in subdirectories should be treated as convention reference"
        );
    }

    #[test]
    fn test_truly_dead_bare_filename_still_flags() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // No SKILL.md exists anywhere
        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "NONEXISTENT.md".to_string(),
                line: 5,
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
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Truly nonexistent bare filename should still be flagged"
        );
    }

    #[test]
    fn test_path_with_directory_still_flags_even_if_basename_exists() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create scout.md in a DIFFERENT directory
        fs::create_dir_all(root.join("other")).unwrap();
        fs::write(root.join("other/scout.md"), "# Scout").unwrap();

        // Reference uses explicit wrong path
        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "agents/scout.md".to_string(),
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
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Path with directory component should still be checked at exact path"
        );
    }

    #[test]
    fn test_procedural_create_reference_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "webkit-changes.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["Write up a summary in a file called webkit-changes.md".to_string()],
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
            "File reference in a creation instruction should be skipped"
        );
    }

    #[test]
    fn test_procedural_delete_reference_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "webkit-changes.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["delete the webkit-changes.md file".to_string()],
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
            "File reference in a deletion instruction should be skipped"
        );
    }

    #[test]
    fn test_absolute_path_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "/Users/drew/code/basic-memory/CHANGELOG.md".to_string(),
                line: 135,
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
            "Absolute paths should be skipped"
        );
    }

    #[test]
    fn test_shell_variable_path_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "cli-tool/components/agents/$ARGUMENTS.md".to_string(),
                    line: 27,
                    source_file: root.join("CLAUDE.md"),
                },
                FileRef {
                    path: "$MD_OUT=reports/junit-nl-suite.md".to_string(),
                    line: 46,
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
            "Paths with shell variables should be skipped"
        );
    }

    #[test]
    fn test_placeholder_filename_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "FILE.md".to_string(),
                    line: 238,
                    source_file: root.join("CLAUDE.md"),
                },
                FileRef {
                    path: "FILE.zh.md".to_string(),
                    line: 238,
                    source_file: root.join("CLAUDE.md"),
                },
                FileRef {
                    path: "filename.md".to_string(),
                    line: 33,
                    source_file: root.join("CLAUDE.md"),
                },
                FileRef {
                    path: "ref1.md".to_string(),
                    line: 104,
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
            "Generic placeholder filenames should be skipped"
        );
    }

    #[test]
    fn test_template_directory_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("templates/claude-code")).unwrap();
        fs::write(root.join("templates/claude-code/CLAUDE.md"), "# Template").unwrap();

        let parsed = ParsedFile {
            path: root.join("templates/claude-code/CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "memory/rules.md".to_string(),
                line: 203,
                source_file: root.join("templates/claude-code/CLAUDE.md"),
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
            "Files inside templates/ directories should be skipped"
        );
    }

    #[test]
    fn test_example_context_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "drafts/srs-sso-authentication-2024-01-15.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "**Examples**: `drafts/srs-sso-authentication-2024-01-15.md`".to_string(),
            ],
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
            "References on lines with 'example' context should be skipped"
        );
    }

    #[test]
    fn test_eg_context_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "fix-parser-edge-case.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "Use lowercase words separated by hyphens (e.g., `fix-parser-edge-case.md`)"
                    .to_string(),
            ],
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
            "References on lines with 'e.g.' context should be skipped"
        );
    }

    #[test]
    fn test_backtick_markdown_link_demo_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "./agent-panel.md".to_string(),
                    line: 1,
                    source_file: root.join("AGENTS.md"),
                },
                FileRef {
                    path: "../telemetry.md".to_string(),
                    line: 2,
                    source_file: root.join("AGENTS.md"),
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "- Relative links: `[Agent Panel](./agent-panel.md)`".to_string(),
                "- Parent directory: `[Telemetry](../telemetry.md)`".to_string(),
            ],
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
            "Markdown link demos inside backticks should be skipped"
        );
    }

    #[test]
    fn test_convention_list_context_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let mut raw_lines = vec![String::new(); 10];
        raw_lines[0] = "Zed recognizes these files (in priority order):".to_string();
        raw_lines[5] = "- `.github/copilot-instructions.md`".to_string();
        raw_lines[6] = "- `AGENT.md`".to_string();

        let parsed = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: ".github/copilot-instructions.md".to_string(),
                    line: 6,
                    source_file: root.join("AGENTS.md"),
                },
                FileRef {
                    path: "AGENT.md".to_string(),
                    line: 7,
                    source_file: root.join("AGENTS.md"),
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines,
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
            "File naming convention lists should be skipped"
        );
    }

    #[test]
    fn test_naming_convention_example_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "optimize-images.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["- **Naming**: Use kebab-case: `optimize-images.md`".to_string()],
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
            "Naming convention examples (kebab-case, etc.) should be skipped"
        );
    }

    #[test]
    fn test_dir_context_resolution_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create the file at the directory-context-relative path
        fs::create_dir_all(root.join("src/templates/base")).unwrap();
        fs::write(root.join("src/templates/base/skill-content.md"), "# Skill").unwrap();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "base/skill-content.md".to_string(),
                line: 3,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "".to_string(),
                "2. **Templates** - Edit in `src/templates/`:".to_string(),
                "   - `base/skill-content.md` - Common content".to_string(),
            ],
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
            "References that resolve via directory context from preceding line should be skipped"
        );
    }

    #[test]
    fn test_dir_context_no_match_still_flags() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Directory context exists but file does NOT exist there
        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "base/nonexistent.md".to_string(),
                line: 3,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "".to_string(),
                "Edit in `src/templates/`:".to_string(),
                "   - `base/nonexistent.md`".to_string(),
            ],
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
            "Dir context that doesn't resolve should still flag"
        );
    }

    #[test]
    fn test_non_procedural_reference_still_flags() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let parsed = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "config/setup.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["Load config/setup.md for configuration details.".to_string()],
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
            "Non-procedural missing reference should still be flagged"
        );
    }
}
