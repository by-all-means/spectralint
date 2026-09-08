use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{is_template_ref, is_within_project};
use super::Checker;

/// Lines where the file is being created/written/deleted, not a dependency.
static ACTION_VERB_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:create|write|generate|save|output|delete|remove|adding)\b").unwrap()
});

/// "a file called X.md" / "a file named X.md" — file is being created.
static FILE_CALLED_NAMED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\b(?:called|named)\s+["'`]?[\w./-]+\.md"#).unwrap());

/// Example/illustrative context — not a real dependency.
static EXAMPLE_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:examples|example|e\.g\.(?:\s|,)|such as|for instance|for example)")
        .unwrap()
});

/// Arrow mappings (→, ->, =>, ~>) indicate routing tables, not dependencies.
fn has_arrow_mapping(line: &str) -> bool {
    line.contains('→') || line.contains("->") || line.contains("=>") || line.contains("~>")
}

/// Exclusion context — files listed as things NOT to touch.
static EXCLUSION_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:out[- ]of[- ]scope|do not modify|do not edit|do not touch|don't modify|don't edit|don't touch|excluded?|ignore)\b").unwrap()
});

/// Convention/documentation list context — listing file formats, not dependencies.
static CONVENTION_LIST_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:recognizes?\s+(?:these|the(?:se)?\s+)?files?|supported\s+files?|file\s+(?:formats?|types?|naming|names?)|naming\s+conventions?|instruction\s+files?|in\s+priority\s+order)\b").unwrap()
});

/// Backtick-delimited directory path (e.g. `` `src/templates/` ``).
static DIR_CONTEXT_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+/)`").unwrap());

/// Naming convention lines — the filename is prescribed, not a dependency.
static CONVENTION_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:kebab[- ]?case|camel[- ]?case|snake[- ]?case|pascal[- ]?case|naming\s*:)|\b(?:every|each)\b.*\b(?:must|should)\b|\b(?:the\s+)?corresponding\b|^#+\s+\S+\.md\s+(?:requirements|format|structure|template|conventions?|guidelines?|standards?)")
        .unwrap()
});

static PLACEHOLDER_FILENAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:FILE\.(?:md|zh\.md)|filename\.md|ref\d+\.md|[\w-]+-n\.md|.*\.\.\..*)$")
        .unwrap()
});

/// Check if any line within a window around `line_idx` (0-based) matches `pattern`.
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

/// Check if a preceding line establishes a directory context (e.g.
/// `` `src/templates/` ``) and the file reference resolves relative to it.
fn resolves_via_dir_context(
    lines: &[String],
    line_idx: usize,
    ref_path: &str,
    canonical_root: Option<&Path>,
    project_root: &Path,
) -> bool {
    let start = line_idx.saturating_sub(3);
    let end = line_idx.min(lines.len());
    start <= end
        && lines[start..end].iter().any(|line| {
            DIR_CONTEXT_PATH.captures_iter(line).any(|caps| {
                let resolved = project_root.join(&caps[1]).join(ref_path);
                resolved.exists() && is_within_project(&resolved, canonical_root, project_root)
            })
        })
}

/// Lines that talk about a file the agent creates or checks for at run time.
static RUNTIME_FILE_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:doesn't exist|does not exist|if (?:the |this |these |those )?(?:files? )?(?:\S+ )?(?:already )?exists?|will (?:be )?(?:creat|generat|writ|produc)|is (?:created|generated|written|produced)|(?:create|generate|write)s? (?:a |the )?(?:new )?(?:file )?`)",
    )
    .unwrap()
});

/// Directories that root a tool's own files: a skill under `.claude/skills/x/`
/// that says `rules/api.md` means `.claude/rules/api.md`.
const TOOL_DIRS: &[&str] = &[
    ".claude",
    ".cursor",
    ".github",
    ".agents",
    ".junie",
    ".kiro",
    ".roo",
    ".devin",
    ".windsurf",
];

/// Paths the Agent Skills spec resolves relative to the skill directory. A bare
/// file name in a skill is usually an artifact the skill writes (`00-scope.md`),
/// so only the spec's own directories are checked.
/// Directories whose contents are produced by a run, never committed.
fn under_generated_dir(path: &str) -> bool {
    path.split('/').any(|seg| {
        matches!(
            seg,
            "results"
                | "output"
                | "outputs"
                | "generated"
                | "tmp"
                | "temp"
                | "dist"
                | "build"
                | "target"
                | "node_modules"
                | ".cache"
        )
    })
}

fn is_skill_local_ref(path: &str) -> bool {
    let path = path.trim_start_matches("./");
    path.split_once('/')
        .is_some_and(|(first, _)| matches!(first, "references" | "scripts" | "assets"))
}

/// The nearest ancestor holding a `SKILL.md`: paths in a skill's `references/`
/// documents are written relative to the skill, not to the document.
fn enclosing_skill_dir(source_file: &Path) -> Option<PathBuf> {
    source_file
        .ancestors()
        .skip(1)
        .take(6)
        .find(|a| a.join("SKILL.md").is_file())
        .map(Path::to_path_buf)
}

fn enclosing_tool_dir(source_file: &Path) -> Option<&Path> {
    source_file.ancestors().skip(1).find(|a| {
        a.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| TOOL_DIRS.contains(&n))
    })
}

pub(crate) struct DeadReferenceChecker;

impl Checker for DeadReferenceChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "dead-reference",
            description: "Flags references and @imports to files that don't exist",
            default_severity: Severity::Error,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for (file_idx, file) in ctx.files.iter().enumerate() {
            if ctx.historical_indices.contains(&file_idx) {
                continue;
            }

            for file_ref in &file.file_refs {
                let is_import = file_ref.ref_kind == crate::parser::types::RefKind::Import;
                if is_import && !file.kind.supports_imports() {
                    continue;
                }
                // A skill is portable: only its own `references/`, `scripts/`, and
                // `assets/` files (or a bare file name) can be checked here; paths
                // into the installing project cannot.
                if file.kind.is_skill() && !is_skill_local_ref(&file_ref.path) {
                    continue;
                }
                if is_template_ref(&file_ref.path) {
                    continue;
                }
                if under_generated_dir(&file_ref.path) {
                    continue;
                }

                let basename = file_ref.path.rsplit('/').next().unwrap_or(&file_ref.path);
                if PLACEHOLDER_FILENAME.is_match(basename) {
                    continue;
                }

                if file_ref
                    .source_file
                    .components()
                    .any(|c| c.as_os_str() == "templates")
                {
                    continue;
                }

                // Resolve relative to source dir first, then project root.
                // After resolving, verify the path stays within the project root
                // to prevent `../../etc/passwd` style traversals from silently passing.
                let source_dir = file_ref.source_file.parent().unwrap_or(&ctx.project_root);
                let mut bases = vec![source_dir, ctx.project_root.as_path()];
                if let Some(tool_dir) = enclosing_tool_dir(&file_ref.source_file) {
                    bases.push(tool_dir);
                    // A nested `.claude/` in a monorepo belongs to the package around it.
                    if let Some(package_root) = tool_dir.parent() {
                        bases.push(package_root);
                    }
                }
                let skill_dir = file
                    .kind
                    .is_skill()
                    .then(|| enclosing_skill_dir(&file_ref.source_file))
                    .flatten();
                if let Some(skill_dir) = skill_dir.as_deref() {
                    bases.push(skill_dir);
                }
                if bases.iter().any(|base| {
                    let resolved = base.join(&file_ref.path);
                    resolved.exists()
                        && is_within_project(
                            &resolved,
                            ctx.canonical_root.as_deref(),
                            &ctx.project_root,
                        )
                }) {
                    continue;
                }

                // Bare filenames that exist somewhere in the tree are convention refs.
                // Imports resolve from the file itself, so this does not apply to them.
                if !is_import
                    && !file_ref.path.contains('/')
                    && ctx.filename_index.contains(&file_ref.path)
                {
                    continue;
                }

                let line_idx = file_ref.line.saturating_sub(1);

                if let Some(line_content) = file.raw_lines.get(line_idx) {
                    if ACTION_VERB_LINE.is_match(line_content)
                        || RUNTIME_FILE_CONTEXT.is_match(line_content)
                        || FILE_CALLED_NAMED.is_match(line_content)
                        || EXAMPLE_CONTEXT.is_match(line_content)
                        || has_arrow_mapping(line_content)
                        || CONVENTION_LINE.is_match(line_content)
                    {
                        continue;
                    }

                    // Skip references inside quoted strings (illustrative examples).
                    if let Some(pos) = line_content.find(&file_ref.path) {
                        let before = &line_content[..pos];
                        if before.chars().filter(|&c| c == '"').count() % 2 == 1 {
                            continue;
                        }

                        // Skip backtick-delimited markdown link syntax examples
                        // like `[Agent Panel](./agent-panel.md)`, but not plain
                        // backtick-quoted file refs like `agent.md`.
                        let after = &line_content[pos + file_ref.path.len()..];
                        if before.contains('`') && after.contains('`') {
                            let backticks_before = before.chars().filter(|&c| c == '`').count();
                            if backticks_before % 2 == 1 && before.contains("](") {
                                continue;
                            }
                        }
                    }
                }

                if has_nearby_match(&file.raw_lines, line_idx, 3, 3, &EXAMPLE_CONTEXT) {
                    continue;
                }

                if has_nearby_match(&file.raw_lines, line_idx, 5, 2, &EXCLUSION_CONTEXT) {
                    continue;
                }

                if has_nearby_match(&file.raw_lines, line_idx, 10, 3, &CONVENTION_LIST_CONTEXT) {
                    continue;
                }

                if resolves_via_dir_context(
                    &file.raw_lines,
                    line_idx,
                    &file_ref.path,
                    ctx.canonical_root.as_deref(),
                    &ctx.project_root,
                ) {
                    continue;
                }

                if is_import {
                    // `@scope/pkg` in prose is parsed as an import too, but a failed
                    // import of a package name is a no-op, so it only rates info.
                    let looks_like_package =
                        !file_ref.path.contains('.') && file_ref.path.matches('/').count() == 1;
                    let severity = if looks_like_package {
                        Severity::Info
                    } else {
                        Severity::Warning
                    };
                    emit!(
                        result,
                        Arc::new(file_ref.source_file.clone()),
                        file_ref.line,
                        severity,
                        Category::DeadReference,
                        suggest: "Create the file, or wrap the token in backticks if it is not an import",
                        "Import target does not exist: @{}",
                        file_ref.path
                    );
                    continue;
                }
                emit!(
                    result,
                    Arc::new(file_ref.source_file.clone()),
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "agent_definitions/followup_drafter.md".to_string(),
                line: 56,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),

            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "commands/[command].md".to_string(),
                    line: 10,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
                FileRef {
                    path: "agent_definitions/*.md".to_string(),
                    line: 20,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),

            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("docs/AGENTS.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "scout.md".to_string(),
                line: 5,
                source_file: root.join("docs/AGENTS.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("changelog.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "nonexistent.md".to_string(),
                line: 5,
                source_file: root.join("changelog.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let mut historical = HashSet::new();
        historical.insert(0); // mark first file as historical

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: historical,
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "agent_definitions/scout.md".to_string(),
                line: 10,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_parent_relative_reference() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create root/sibling.md
        fs::write(root.join("sibling.md"), "# Sibling").unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();

        // docs/AGENTS.md references "../sibling.md"
        let parsed = ParsedFile {
            path: Arc::new(root.join("docs/AGENTS.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "../sibling.md".to_string(),
                line: 5,
                source_file: root.join("docs/AGENTS.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("docs/AGENTS.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "../nonexistent.md".to_string(),
                line: 5,
                source_file: root.join("docs/AGENTS.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "templates/{name}.md".to_string(),
                line: 10,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: ".agent/skills/<skill-name>/SKILL.md".to_string(),
                line: 10,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "~/.claude/CLAUDE.md".to_string(),
                line: 4,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "path/to/agent.md".to_string(),
                line: 62,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "@playbook-name.md".to_string(),
                line: 10,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "SKILL.md".to_string(),
                line: 31,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let canonical_root = root.canonicalize().ok();
        let filename_index = crate::engine::cross_ref::build_filename_index(root);
        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root,
            filename_index,
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "NONEXISTENT.md".to_string(),
                line: 5,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "agents/scout.md".to_string(),
                line: 10,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "webkit-changes.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["Write up a summary in a file called webkit-changes.md".to_string()],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "webkit-changes.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["delete the webkit-changes.md file".to_string()],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "/Users/drew/code/basic-memory/CHANGELOG.md".to_string(),
                line: 135,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "cli-tool/components/agents/$ARGUMENTS.md".to_string(),
                    line: 27,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
                FileRef {
                    path: "$MD_OUT=reports/junit-nl-suite.md".to_string(),
                    line: 46,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "FILE.md".to_string(),
                    line: 238,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
                FileRef {
                    path: "FILE.zh.md".to_string(),
                    line: 238,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
                FileRef {
                    path: "filename.md".to_string(),
                    line: 33,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
                FileRef {
                    path: "ref1.md".to_string(),
                    line: 104,
                    source_file: root.join("CLAUDE.md"),
                    ..Default::default()
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("templates/claude-code/CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "memory/rules.md".to_string(),
                line: 203,
                source_file: root.join("templates/claude-code/CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "drafts/srs-sso-authentication-2024-01-15.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "**Examples**: `drafts/srs-sso-authentication-2024-01-15.md`".to_string(),
            ],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "fix-parser-edge-case.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "Use lowercase words separated by hyphens (e.g., `fix-parser-edge-case.md`)"
                    .to_string(),
            ],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("AGENTS.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: "./agent-panel.md".to_string(),
                    line: 1,
                    source_file: root.join("AGENTS.md"),
                    ..Default::default()
                },
                FileRef {
                    path: "../telemetry.md".to_string(),
                    line: 2,
                    source_file: root.join("AGENTS.md"),
                    ..Default::default()
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "- Relative links: `[Agent Panel](./agent-panel.md)`".to_string(),
                "- Parent directory: `[Telemetry](../telemetry.md)`".to_string(),
            ],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("AGENTS.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![
                FileRef {
                    path: ".github/copilot-instructions.md".to_string(),
                    line: 6,
                    source_file: root.join("AGENTS.md"),
                    ..Default::default()
                },
                FileRef {
                    path: "AGENT.md".to_string(),
                    line: 7,
                    source_file: root.join("AGENTS.md"),
                    ..Default::default()
                },
            ],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines,
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "optimize-images.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["- **Naming**: Use kebab-case: `optimize-images.md`".to_string()],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "base/skill-content.md".to_string(),
                line: 3,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "".to_string(),
                "2. **Templates** - Edit in `src/templates/`:".to_string(),
                "   - `base/skill-content.md` - Common content".to_string(),
            ],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "base/nonexistent.md".to_string(),
                line: 3,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "".to_string(),
                "Edit in `src/templates/`:".to_string(),
                "   - `base/nonexistent.md`".to_string(),
            ],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
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
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "config/setup.md".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["Load config/setup.md for configuration details.".to_string()],
            in_code_block: vec![],
            ..Default::default()
        };

        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Non-procedural missing reference should still be flagged"
        );
    }

    /// Build a context where `CLAUDE.md` references a bare `ref_path` on line 1
    /// with the given `raw_line` as the sole line content. The referenced file
    /// does NOT exist on disk, so only line-level skip heuristics prevent a
    /// diagnostic.
    fn bare_ref_ctx(root: &Path, ref_path: &str, raw_line: &str) -> CheckerContext {
        let parsed = ParsedFile {
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: ref_path.to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![raw_line.to_string()],
            in_code_block: vec![],
            ..Default::default()
        };
        CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
        }
    }

    #[test]
    fn test_path_traversal_outside_project_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Reference uses ../../.. traversal to reach outside the project.
        // Even though /etc/passwd exists on the system, the checker must
        // treat it as a dead reference because it escapes the project root.
        let parsed = ParsedFile {
            path: Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![FileRef {
                path: "../../../etc/passwd".to_string(),
                line: 1,
                source_file: root.join("CLAUDE.md"),
                ..Default::default()
            }],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["See ../../../etc/passwd for details".to_string()],
            in_code_block: vec![],
            ..Default::default()
        };

        let canonical_root = root.canonicalize().ok();
        let ctx = CheckerContext {
            files: vec![parsed],
            project_root: root.to_path_buf(),
            canonical_root,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
            settings_files: vec![],
        };

        let checker = DeadReferenceChecker;
        let result = checker.check(&ctx);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Path traversal escaping project root should be flagged as dead reference"
        );
        assert_eq!(result.diagnostics[0].category, Category::DeadReference);
    }

    #[test]
    fn test_convention_description_heading_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = bare_ref_ctx(dir.path(), "SKILL.md", "### SKILL.md Requirements");
        let result = DeadReferenceChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Convention description headings should be skipped"
        );
    }

    #[test]
    fn test_convention_every_must_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = bare_ref_ctx(
            dir.path(),
            "SKILL.md",
            "Every `SKILL.md` must include YAML frontmatter:",
        );
        let result = DeadReferenceChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Prescriptive convention lines (every X must) should be skipped"
        );
    }

    #[test]
    fn test_corresponding_reference_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = bare_ref_ctx(
            dir.path(),
            "SKILL.md",
            "you SHOULD read the corresponding `SKILL.md` to ensure compliance",
        );
        let result = DeadReferenceChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Generic 'corresponding' references should be skipped"
        );
    }

    // --- FP/FN regression tests ---

    #[test]
    fn test_reference_in_html_comment_still_checked() {
        // HTML comments are NOT filtered by the parser's non_code_lines
        // iterator, so file refs inside them ARE extracted. However, the
        // `-->` closing tag contains `->`, which triggers the arrow-mapping
        // heuristic (has_arrow_mapping), causing the reference to be
        // skipped. This is a known quirk: HTML comment closing syntax
        // collides with the `->` arrow pattern.
        //
        // Net effect: dead references inside HTML comments are silently
        // skipped due to the `-->` / `->` overlap. This test documents
        // the current behavior.
        let dir = tempfile::tempdir().unwrap();
        let ctx = bare_ref_ctx(dir.path(), "missing.md", "<!-- See missing.md -->");
        let result = DeadReferenceChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Reference in HTML comment is skipped (has_arrow_mapping matches `-->` as `->`)"
        );
    }

    #[test]
    fn test_tilde_arrow_mapping_skipped() {
        // Lines with ~> arrow mappings indicate routing tables, not real
        // file dependencies. The reference should not be flagged.
        let dir = tempfile::tempdir().unwrap();
        let ctx = bare_ref_ctx(dir.path(), "output.md", "Input ~> Transform ~> output.md");
        let result = DeadReferenceChecker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "File references on arrow-mapping lines (~>) should be skipped"
        );
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::checkers::utils::test_helpers::file_ctx_at;
    use crate::parser::types::{FileRef, RefKind};

    fn with_import(
        rel: &str,
        target: &str,
    ) -> (tempfile::TempDir, crate::engine::cross_ref::CheckerContext) {
        let (dir, mut ctx) = file_ctx_at(rel, &[&format!("Read @{target} before editing.")]);
        let source = ctx.files[0].path.to_path_buf();
        ctx.files[0].file_refs.push(FileRef {
            path: target.to_string(),
            line: 1,
            source_file: source,
            ref_kind: RefKind::Import,
        });
        (dir, ctx)
    }

    #[test]
    fn missing_import_in_claude_md_is_a_warning() {
        let (_dir, ctx) = with_import("CLAUDE.md", "docs/missing.md");
        let result = DeadReferenceChecker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(
            result.diagnostics[0].message,
            "Import target does not exist: @docs/missing.md"
        );
    }

    #[test]
    fn existing_import_target_is_fine() {
        let (dir, ctx) = with_import("CLAUDE.md", "docs/present.md");
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/present.md"), "# ok").unwrap();
        assert!(DeadReferenceChecker.check(&ctx).diagnostics.is_empty());
    }

    #[test]
    fn imports_are_ignored_in_kinds_that_do_not_read_them() {
        for rel in [
            ".claude/agents/reviewer.md",
            "AGENTS.md",
            ".cursor/rules/style.mdc",
        ] {
            let (_dir, ctx) = with_import(rel, "docs/missing.md");
            assert!(
                DeadReferenceChecker.check(&ctx).diagnostics.is_empty(),
                "{rel}"
            );
        }
        let (_dir, ctx) = with_import(".claude/rules/db.md", "docs/missing.md");
        assert_eq!(DeadReferenceChecker.check(&ctx).diagnostics.len(), 1);
    }

    #[test]
    fn package_scopes_get_the_backtick_hint() {
        let (_dir, ctx) = with_import("CLAUDE.md", "scope/pkg");
        let result = DeadReferenceChecker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .suggestion
            .as_deref()
            .unwrap()
            .contains("backticks"));
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn plain_mentions_stay_errors() {
        let (_dir, mut ctx) = file_ctx_at("CLAUDE.md", &["See docs/missing.md for details."]);
        let source = ctx.files[0].path.to_path_buf();
        ctx.files[0].file_refs.push(FileRef {
            path: "docs/missing.md".to_string(),
            line: 1,
            source_file: source,
            ref_kind: RefKind::Mention,
        });
        let result = DeadReferenceChecker.check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
    }
}

#[cfg(test)]
mod runtime_context_tests {
    use super::*;

    #[test]
    fn lines_about_files_that_exist_at_run_time_are_skipped() {
        for line in [
            "Previous local docs if they exist (`notes-a.md`, `notes-b.md`)",
            "If `feature-inventory.md` doesn't exist, refuse and stop.",
            "The run writes `report.md` next to the sources.",
        ] {
            assert!(RUNTIME_FILE_CONTEXT.is_match(line), "{line}");
        }
        assert!(!RUNTIME_FILE_CONTEXT.is_match("Read `docs/guide.md` before starting."));
    }
}

#[cfg(test)]
mod skill_scope_tests {
    use super::*;
    use crate::checkers::utils::test_helpers::file_ctx_at;
    use crate::parser::types::{FileRef, RefKind};

    fn skill_with_ref(
        target: &str,
    ) -> (tempfile::TempDir, crate::engine::cross_ref::CheckerContext) {
        let (dir, mut ctx) = file_ctx_at(
            ".claude/skills/deploy/SKILL.md",
            &[&format!("See {target} for details.")],
        );
        let source = ctx.files[0].path.to_path_buf();
        ctx.files[0].file_refs.push(FileRef {
            path: target.to_string(),
            line: 1,
            source_file: source,
            ref_kind: RefKind::Mention,
        });
        (dir, ctx)
    }

    #[test]
    fn skills_are_checked_only_for_their_own_files() {
        assert!(is_skill_local_ref("references/guide.md"));
        assert!(is_skill_local_ref("./scripts/run.md"));
        assert!(!is_skill_local_ref("checklist.md"));
        assert!(!is_skill_local_ref(".claude/product-marketing.md"));
        assert!(!is_skill_local_ref("skills/evaluate/SKILL.md"));

        let (_dir, ctx) = skill_with_ref("references/missing.md");
        assert_eq!(DeadReferenceChecker.check(&ctx).diagnostics.len(), 1);
        let (_dir, ctx) = skill_with_ref(".claude/product-marketing.md");
        assert!(DeadReferenceChecker.check(&ctx).diagnostics.is_empty());
    }
}

#[cfg(test)]
mod skill_resource_tests {
    use super::*;
    use crate::checkers::utils::test_helpers::file_ctx_at;
    use crate::file_kind::FileKind;
    use crate::parser::types::{FileRef, RefKind};

    #[test]
    fn skill_resources_resolve_against_the_skill_directory() {
        let (dir, mut ctx) = file_ctx_at(
            ".claude/skills/design/references/logo.md",
            &["See references/prompts.md and docs/brand.md."],
        );
        let skill = dir.path().join(".claude/skills/design");
        std::fs::create_dir_all(skill.join("references")).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: design\n---").unwrap();
        std::fs::write(skill.join("references/prompts.md"), "# prompts").unwrap();
        ctx.files[0].kind = FileKind::SkillResource;
        let source = ctx.files[0].path.to_path_buf();
        for target in [
            "references/prompts.md",
            "references/missing.md",
            "docs/brand.md",
        ] {
            ctx.files[0].file_refs.push(FileRef {
                path: target.to_string(),
                line: 1,
                source_file: source.clone(),
                ref_kind: RefKind::Mention,
            });
        }
        let messages: Vec<String> = DeadReferenceChecker
            .check(&ctx)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect();
        // The existing sibling resolves via the skill directory, the project
        // path is out of scope for a portable skill, and only the genuinely
        // missing reference remains.
        assert_eq!(messages, vec!["\"references/missing.md\" does not exist"]);
    }
}

#[cfg(test)]
mod placeholder_tests {
    use super::*;

    #[test]
    fn numbered_placeholders_are_not_references() {
        assert!(PLACEHOLDER_FILENAME.is_match("thread-N.md"));
        assert!(PLACEHOLDER_FILENAME.is_match("step-n.md"));
        assert!(PLACEHOLDER_FILENAME.is_match("01-...md"));
        assert!(!PLACEHOLDER_FILENAME.is_match("plan.md"));
        assert!(!PLACEHOLDER_FILENAME.is_match("thread-1.md"));
    }
}

#[cfg(test)]
mod nested_tool_dir_tests {
    use super::*;
    use crate::checkers::utils::test_helpers::file_ctx_at;
    use crate::parser::types::{FileRef, RefKind};

    #[test]
    fn nested_claude_dir_resolves_against_its_package_root() {
        let (dir, mut ctx) = file_ctx_at(
            "compiler/.claude/agents/port.md",
            &["Follow docs/architecture.md for data modelling."],
        );
        std::fs::create_dir_all(dir.path().join("compiler/docs")).unwrap();
        std::fs::write(dir.path().join("compiler/docs/architecture.md"), "# arch").unwrap();
        let source = ctx.files[0].path.to_path_buf();
        ctx.files[0].file_refs.push(FileRef {
            path: "docs/architecture.md".to_string(),
            line: 1,
            source_file: source,
            ref_kind: RefKind::Mention,
        });
        assert!(DeadReferenceChecker.check(&ctx).diagnostics.is_empty());
    }
}

#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn runtime_shapes_are_skipped() {
        assert!(under_generated_dir("scripts/pr-status/results/index.md"));
        assert!(under_generated_dir("build/report.md"));
        assert!(!under_generated_dir("docs/results-summary.md"));
        assert!(RUNTIME_FILE_CONTEXT.is_match("- If data-model.md exists: extract entities"));
        assert!(RUNTIME_FILE_CONTEXT.is_match("If the file already exists, skip it."));
        assert!(!RUNTIME_FILE_CONTEXT.is_match("Read data-model.md before starting."));
        assert!(crate::checkers::utils::is_template_ref(
            "FEATURE_DIR/spec.md"
        ));
        assert!(!crate::checkers::utils::is_template_ref("docs/API.md"));
    }
}
