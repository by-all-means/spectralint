use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

use crate::config::MissingEssentialSectionsConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{is_instruction_file, is_reasoning_prompt, ScopeFilter, COMMAND_NAMES};
use super::Checker;

pub(crate) struct MissingEssentialSectionsChecker {
    scope: ScopeFilter,
    min_lines: usize,
}

impl MissingEssentialSectionsChecker {
    pub(crate) fn new(config: &MissingEssentialSectionsConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            min_lines: config.min_lines,
        }
    }
}

/// Command patterns commonly found in code blocks.
static CODE_BLOCK_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)\b(?:{COMMAND_NAMES})\b")).unwrap());

/// Section headings that indicate build/test/setup content.
static SECTION_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:commands?|build|test(?:ing|s)?|setup|getting\s+started|installation|development|usage|quick\s*start)\b").unwrap()
});

/// Inline backtick commands.
static INLINE_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"`[^`]*\b(?:{COMMAND_NAMES})\b[^`]*`")).unwrap());

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
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "missing-essential-sections",
            description: "Flags files lacking build/test commands or setup sections",
            default_severity: Severity::Info,
            strict_only: true,
        }
    }

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

            // Skip reference/context files that don't contain imperative instructions.
            // Files like activity logs, curated lists, and context dumps don't need
            // build/test commands because they aren't telling the agent what to do.
            if !is_instruction_file(&file.raw_lines, &file.in_code_block) {
                continue;
            }

            // Skip reasoning/workflow agent prompts that have no code blocks, no
            // file references, and no shell command mentions — these are pure prose
            // instructions (e.g., FlowKit agent system prompts) and don't need
            // build/test commands.
            if is_reasoning_prompt(file) {
                continue;
            }

            // Signal 1: Code blocks containing command patterns
            let has_code_block_command = file
                .code_block_lines()
                .any(|(_, line)| CODE_BLOCK_COMMAND.is_match(line));

            // Signal 2: Section headings matching command/build/test patterns
            let has_command_section = file
                .sections
                .iter()
                .any(|s| SECTION_HEADING.is_match(&s.title));

            // Signal 3: Inline backtick commands
            let has_inline_command = file
                .non_code_lines()
                .any(|(_, line)| INLINE_COMMAND.is_match(line));

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
    use std::sync::Arc;

    fn run_check(lines: &[&str]) -> CheckResult {
        run_check_with_sections(lines, vec![])
    }

    fn run_check_with_sections(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        let config = MissingEssentialSectionsConfig {
            enabled: true,
            min_lines: 3,
            scope: Vec::new(),
            severity: None,
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
        // Include a code block (with non-command content) so the file is not
        // classified as a reasoning prompt — it has code context but no
        // build/test commands.
        let result = run_check(&[
            "# Guide",
            "",
            "Always follow best practices.",
            "",
            "You must ensure code quality.",
            "",
            "Never skip unit tests.",
            "",
            "```",
            "// example snippet",
            "```",
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
        let (_dir, ctx) = single_file_ctx_with_sections(&["# Short", "Just one thing."], vec![]);
        let config = MissingEssentialSectionsConfig {
            enabled: true,
            min_lines: 10,
            scope: Vec::new(),
            severity: None,
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
            path: Arc::new(root.join(".claude/commands/review.md")),
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
            in_code_block: vec![],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let config = MissingEssentialSectionsConfig {
            enabled: true,
            min_lines: 3,
            scope: Vec::new(),
            severity: None,
        };
        let result = MissingEssentialSectionsChecker::new(&config).check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Files in specialized directories (commands, agents, skills) should be skipped"
        );
    }

    #[test]
    fn test_reference_file_without_imperatives_skipped() {
        let result = run_check(&[
            "# Company Overview",
            "",
            "TechStart Inc is a B2B SaaS company.",
            "",
            "## Financial Snapshot",
            "",
            "- ARR: $2.4M",
            "- Burn Rate: $500K/month",
            "- Runway: 20 months",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Reference/context files without imperative instructions should be skipped"
        );
    }

    #[test]
    fn test_instruction_file_without_commands_flags() {
        // Include a code block so the file is recognized as code-related (not
        // a reasoning prompt), but the code block has no build/test commands.
        let result = run_check(&[
            "# Guidelines",
            "",
            "Always use TypeScript for new code.",
            "Never commit directly to main.",
            "Ensure all PRs have tests.",
            "",
            "Follow the coding standards.",
            "",
            "```typescript",
            "const x = 1;",
            "```",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Instruction files with imperatives but no build commands should be flagged"
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

    #[test]
    fn test_reasoning_agent_prompt_skipped() {
        // A pure prose reasoning/workflow agent prompt: headings and
        // imperative instructions but zero code blocks, zero file
        // references, and zero shell-command mentions. Should NOT be
        // flagged because it is not a coding agent configuration.
        let result = run_check(&[
            "# Research Agent",
            "",
            "You are a research analysis agent.",
            "",
            "## Core Responsibilities",
            "",
            "Always verify claims against primary sources.",
            "Never accept unverified assertions.",
            "Ensure conclusions follow from evidence.",
            "",
            "## Workflow",
            "",
            "1. Gather relevant information from the provided context.",
            "2. Analyze the data for patterns and inconsistencies.",
            "3. Synthesize findings into a structured report.",
            "",
            "## Output Format",
            "",
            "Use clear headings and bullet points.",
            "Avoid jargon when plain language suffices.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Reasoning agent prompts (pure prose, no code/commands/file refs) should be skipped"
        );
    }
}
