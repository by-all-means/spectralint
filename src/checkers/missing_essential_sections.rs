use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

use crate::config::MissingEssentialSectionsConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::{code_block_lines, non_code_lines};
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct MissingEssentialSectionsChecker {
    scope: ScopeFilter,
    min_lines: usize,
}

impl MissingEssentialSectionsChecker {
    pub fn new(config: &MissingEssentialSectionsConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            min_lines: config.min_lines,
        }
    }
}

/// Command patterns commonly found in code blocks.
static CODE_BLOCK_COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:cargo|npm|npx|yarn|pnpm|pytest|make|go\s+(?:build|test|run)|docker|pip|poetry|gradle|mvn|bundle|rake|mix|dotnet|cmake)\b").unwrap()
});

/// Section headings that indicate build/test/setup content.
static SECTION_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:commands?|build|test(?:ing|s)?|setup|getting\s+started|installation|development|usage|quick\s*start)\b").unwrap()
});

/// Inline backtick commands.
static INLINE_COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"`[^`]*\b(?:cargo|npm|npx|yarn|pnpm|pytest|make|go\s+(?:build|test|run)|docker|pip|poetry|gradle|mvn|bundle|rake|mix|dotnet|cmake)\b[^`]*`").unwrap()
});

/// Specialized subdirectories whose files serve specific purposes (commands,
/// agent definitions, skills, etc.) and are not expected to contain build/test
/// commands. Only top-level or general instruction files should be checked.
const SPECIALIZED_DIRS: &[&str] = &[
    "commands",
    "agents",
    "skills",
    "tasks",
    "prompts",
    "references",
    "researches",
];

/// Returns true if any component of the relative path is a specialized directory.
fn is_specialized_file(file_path: &Path, project_root: &Path) -> bool {
    let rel = file_path
        .parent()
        .and_then(|p| p.strip_prefix(project_root).ok())
        .unwrap_or(Path::new(""));
    rel.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        SPECIALIZED_DIRS.iter().any(|d| s.eq_ignore_ascii_case(d))
    })
}

impl Checker for MissingEssentialSectionsChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Skip short files
            if file.raw_lines.len() < self.min_lines {
                continue;
            }

            // Skip files in specialized subdirectories (commands, agents, skills, etc.)
            // — these serve specific purposes and don't need build/test commands
            if is_specialized_file(&file.path, &ctx.project_root) {
                continue;
            }

            // Signal 1: Code blocks containing command patterns
            let has_code_block_command = code_block_lines(&file.raw_lines)
                .any(|(_, line)| CODE_BLOCK_COMMAND.is_match(line));

            // Signal 2: Section headings matching command/build/test patterns
            let has_command_section = file
                .sections
                .iter()
                .any(|s| SECTION_HEADING.is_match(&s.title));

            // Signal 3: Inline backtick commands
            let has_inline_command =
                non_code_lines(&file.raw_lines).any(|(_, line)| INLINE_COMMAND.is_match(line));

            if has_code_block_command || has_command_section || has_inline_command {
                continue;
            }

            emit!(
                result,
                file.path,
                1,
                Severity::Info,
                Category::MissingEssentialSections,
                suggest: "Add a section with build/test commands so agents know how to verify their work",
                "No build/test commands or setup section found. Instruction files are most \
                 effective when they include concrete commands agents can run."
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx_with_sections;
    use crate::parser::types::{ParsedFile, Section};
    use std::collections::HashSet;

    fn run_check(lines: &[&str]) -> CheckResult {
        run_check_with_sections(lines, vec![])
    }

    fn run_check_with_sections(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        let config = MissingEssentialSectionsConfig {
            enabled: true,
            min_lines: 3,
            scope: Vec::new(),
        };
        MissingEssentialSectionsChecker::new(&config).check(&ctx)
    }

    #[test]
    fn test_file_with_code_block_command_passes() {
        let result = run_check(&[
            "# Setup",
            "",
            "Do the work.",
            "",
            "```",
            "cargo test",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "File with cargo command in code block should pass"
        );
    }

    #[test]
    fn test_file_with_section_heading_passes() {
        let result = run_check_with_sections(
            &[
                "# Project",
                "",
                "Some info.",
                "",
                "## Getting Started",
                "",
                "Follow the steps.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Project".to_string(),
                    line: 1,
                    end_line: 4,
                },
                Section {
                    level: 2,
                    title: "Getting Started".to_string(),
                    line: 5,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "File with Getting Started heading should pass"
        );
    }

    #[test]
    fn test_file_with_inline_command_passes() {
        let result = run_check(&[
            "# Guide",
            "",
            "Run `cargo test` to verify.",
            "",
            "More lines.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "File with inline cargo command should pass"
        );
    }

    #[test]
    fn test_file_without_commands_flags() {
        let result = run_check(&[
            "# Guide",
            "",
            "Be careful with the code.",
            "",
            "Follow best practices.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(
            result.diagnostics[0].category,
            Category::MissingEssentialSections
        );
    }

    #[test]
    fn test_short_file_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec!["# Short".to_string(), "Just one thing.".to_string()],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };
        let config = MissingEssentialSectionsConfig {
            enabled: true,
            min_lines: 10,
            scope: Vec::new(),
        };
        let result = MissingEssentialSectionsChecker::new(&config).check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Short files below min_lines should be skipped"
        );
    }

    #[test]
    fn test_specialized_directory_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // File inside .claude/commands/ — specialized, should be skipped
        let file = ParsedFile {
            path: root.join(".claude/commands/review.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![
                "# Review".to_string(),
                "".to_string(),
                "Review the code carefully.".to_string(),
                "".to_string(),
                "Check for issues.".to_string(),
            ],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };
        let config = MissingEssentialSectionsConfig {
            enabled: true,
            min_lines: 3,
            scope: Vec::new(),
        };
        let result = MissingEssentialSectionsChecker::new(&config).check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Files in specialized directories (commands, agents, skills) should be skipped"
        );
    }

    #[test]
    fn test_npm_command_in_code_block_passes() {
        let result = run_check(&["# Dev", "", "Info here.", "", "```", "npm run build", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_test_section_heading_passes() {
        let result = run_check_with_sections(
            &[
                "# Project",
                "",
                "Content.",
                "",
                "## Testing",
                "",
                "Details.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Project".to_string(),
                    line: 1,
                    end_line: 4,
                },
                Section {
                    level: 2,
                    title: "Testing".to_string(),
                    line: 5,
                    end_line: 7,
                },
            ],
        );
        assert!(result.diagnostics.is_empty());
    }
}
