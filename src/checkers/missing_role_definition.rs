use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{count_directive_lines, is_instruction_file, ScopeFilter};
use super::Checker;

/// Local threshold for this checker — files need 15+ directive lines to warrant
/// a role definition. Higher than the shared MIN_DIRECTIVE_LINES (5) because
/// small-to-medium files legitimately omit role definitions.
const ROLE_MIN_DIRECTIVE_LINES: usize = 15;

/// Higher threshold for sectionless files — flat bullet lists without headers
/// are intentionally terse and shouldn't be penalized for missing "You are...".
const ROLE_MIN_DIRECTIVE_LINES_SECTIONLESS: usize = 25;

/// Files with this many sections are structurally self-documenting —
/// the section headings themselves communicate purpose, so a "You are..."
/// preamble adds no value.
const MIN_SECTIONS_IMPLICIT_ROLE: usize = 3;

pub(crate) struct MissingRoleDefinitionChecker {
    scope: ScopeFilter,
}

impl MissingRoleDefinitionChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Patterns that establish a role identity.
static ROLE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        ^\s*(?:[-*]\s+)?(?:
            you\s+are\b
            | act\s+as\b
            | your\s+role\s+is\b
            | you\s+(?:serve|function|operate)\s+as\b
            | your\s+(?:name|identity)\s+is\b
            | you\s*\([^)]+\)\s+(?:are|have|can|should|must|will)\b   # You (Claude) are...
        )
        | \w+\s+is\s+(?:(?:an?|the)\s+)?(?:agent|assistant|bot|orchestrator|coordinator|helper)\b  # Alfred is the Orchestrator
        | this\s+(?:file|document|guide)\s+(?:provides|contains|defines|establishes|governs)\s+(?:guidance|instructions|rules|guidelines|context)\b
        | this\s+(?:file|document|guide)\s+governs\b
        | (?:instructions|guidelines|guidance|rules)\s+for\s+(?:claude|(?:the\s+)?(?:ai\s+)?(?:agent|assistant|model|llm)s?)\b
        ",
    )
    .unwrap()
});

/// Section titles that indicate a role/identity section.
/// Full-title matches require exact match; keyword matches use `\b` boundaries
/// so multi-word titles like "Claude AI Assistant Guidelines" are caught.
static ROLE_SECTION_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        # Full-title exact matches
        ^(?:
            role | identity | persona | purpose
            | who\s+you\s+are
            | system\s+(?:role|identity)
            | mission | directives? | authorized
            | about\s+(?:this\s+)?(?:agent|assistant|bot)
            | (?:instructions|guidelines|guidance)\s+for\s+\w+
            | project\s+overview
        )$
        # Keyword-contains matches (word boundaries)
        | \b(?:guidelines|conventions|overview|contributing)\b
        ",
    )
    .unwrap()
});

/// Path segments that indicate task/command/skill files (not top-level agent instructions).
static SKIP_PATH_SEGMENTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|/)(?:commands|skills|tasks)/").unwrap());

impl Checker for MissingRoleDefinitionChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "missing-role-definition",
            description: "Flags files without a role definition",
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

            let rel_path = file
                .path
                .strip_prefix(&ctx.project_root)
                .unwrap_or(&file.path);
            let rel_str = rel_path.to_string_lossy();
            if SKIP_PATH_SEGMENTS.is_match(&rel_str) {
                continue;
            }

            // Skip deeply nested files (depth > 2 components from project root).
            // e.g. "sub/CLAUDE.md" (2 components) passes, "sub/deep/CLAUDE.md" (3) is skipped.
            if rel_path.components().count() > 2 {
                continue;
            }

            let min_directives = if file.sections.is_empty() {
                ROLE_MIN_DIRECTIVE_LINES_SECTIONLESS
            } else {
                ROLE_MIN_DIRECTIVE_LINES
            };
            if count_directive_lines(&file.raw_lines, &file.in_code_block) < min_directives {
                continue;
            }

            // Skip reference/context files without imperative instructions.
            if !is_instruction_file(&file.raw_lines, &file.in_code_block) {
                continue;
            }

            // Files with 3+ sections are structurally self-documenting.
            if file.sections.len() >= MIN_SECTIONS_IMPLICIT_ROLE {
                continue;
            }

            let has_role_pattern = file
                .non_code_lines()
                .any(|(_, line)| ROLE_PATTERN.is_match(line));

            if has_role_pattern
                || file
                    .sections
                    .iter()
                    .any(|s| ROLE_SECTION_TITLE.is_match(&s.title))
            {
                continue;
            }

            emit!(
                result,
                file.path,
                1,
                Severity::Info,
                Category::MissingRoleDefinition,
                suggest: "Add a role definition like \"You are a...\" or a ## Role section",
                "No role definition found. Files with {}+ directive lines benefit from \
                 an explicit identity (\"You are...\", \"Act as...\", or a Role section).",
                min_directives
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::{single_file_ctx, single_file_ctx_with_sections};
    use crate::parser::types::Section;

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        MissingRoleDefinitionChecker::new(&[]).check(&ctx)
    }

    fn run_check_with_sections(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        MissingRoleDefinitionChecker::new(&[]).check(&ctx)
    }

    fn enough_directives() -> Vec<&'static str> {
        vec![
            "Always run tests.",
            "Never skip CI.",
            "Use structured logging.",
            "Follow the style guide.",
            "Run linting before commit.",
            "Keep functions small.",
            "Ensure proper error handling.",
            "Check return values.",
            "Avoid global state.",
            "Must validate all inputs.",
            "Write documentation for public APIs.",
            "Use meaningful variable names.",
            "Prefer composition over inheritance.",
            "Handle edge cases explicitly.",
            "Review your own code before submitting.",
            "Always check for null pointers.",
            "Never use hardcoded values.",
            "Use configuration files.",
            "Follow naming conventions.",
            "Run security checks.",
            "Keep code DRY.",
            "Ensure thread safety.",
            "Check permissions before access.",
            "Avoid deep nesting.",
            "Must handle errors gracefully.",
        ]
    }

    #[test]
    fn test_missing_role_flags() {
        let result = run_check(&enough_directives());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(
            result.diagnostics[0].category,
            Category::MissingRoleDefinition
        );
    }

    #[test]
    fn test_you_are_pattern_passes() {
        let mut lines = vec!["You are a senior Rust developer."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_act_as_pattern_passes() {
        let mut lines = vec!["Act as a code reviewer."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_your_role_is_pattern_passes() {
        let mut lines = vec!["Your role is to assist with development."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_role_section_title_passes() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![Section {
                level: 2,
                title: "Role".to_string(),
                line: 1,
                end_line: 6,
            }],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_identity_section_title_passes() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![Section {
                level: 2,
                title: "Identity".to_string(),
                line: 1,
                end_line: 6,
            }],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_too_few_directives_skipped() {
        let result = run_check(&[
            "Always run tests.",
            "Never skip CI.",
            "Use structured logging.",
            "Follow the style guide.",
            "Run linting before commit.",
            "Keep functions small.",
            "Ensure proper error handling.",
            "Check return values.",
            "Avoid global state.",
            "Must validate all inputs.",
            "Write documentation for public APIs.",
            "Use meaningful variable names.",
            "Prefer composition over inheritance.",
            "Handle edge cases explicitly.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Files with < 15 directive lines should be skipped"
        );
    }

    #[test]
    fn test_role_in_code_block_not_counted() {
        let mut lines = vec!["```", "You are a developer.", "```"];
        lines.extend(enough_directives());
        let result = run_check(&lines);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Role pattern in code block should not count"
        );
    }

    #[test]
    fn test_deeply_nested_file_skipped() {
        use crate::parser::types::ParsedFile;
        use std::collections::HashSet;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let lines = enough_directives();
        let file = ParsedFile {
            path: std::sync::Arc::new(root.join("sub/deep/CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines.iter().map(|s| s.to_string()).collect(),
            in_code_block: vec![],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let result = MissingRoleDefinitionChecker::new(&[]).check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Deeply nested files (depth > 2) should be skipped"
        );
    }

    #[test]
    fn test_parenthetical_you_pattern_passes() {
        let mut lines = vec!["You (Claude) are a senior developer."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "Parenthetical 'You (Claude) are' should count as role definition"
        );
    }

    #[test]
    fn test_third_person_name_assignment_passes() {
        let mut lines = vec!["Alfred is the orchestrator for all tasks."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "Third-person 'X is the orchestrator' should count as role definition"
        );
    }

    #[test]
    fn test_mission_section_title_passes() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![Section {
                level: 2,
                title: "Mission".to_string(),
                line: 1,
                end_line: 10,
            }],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section titled 'Mission' should count as role definition"
        );
    }

    #[test]
    fn test_this_file_provides_guidance_passes() {
        let mut lines =
            vec!["This file provides guidance to Claude Code on how to work with this project."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "'This file provides guidance...' should count as implicit role definition"
        );
    }

    #[test]
    fn test_this_document_contains_instructions_passes() {
        let mut lines = vec!["This document contains instructions for working with the codebase."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "'This document contains instructions...' should count as implicit role definition"
        );
    }

    #[test]
    fn test_guidelines_for_the_agent_passes() {
        let mut lines = vec!["Guidelines for the agent when reviewing pull requests."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "'Guidelines for the agent...' should count as implicit role definition"
        );
    }

    #[test]
    fn test_purpose_section_passes() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![Section {
                level: 2,
                title: "Purpose".to_string(),
                line: 1,
                end_line: 10,
            }],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section titled 'Purpose' should count as role definition"
        );
    }

    #[test]
    fn test_governs_pattern_passes() {
        let mut lines = vec!["This file governs Claude's behavior when working on this project."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "'This file governs...' should count as implicit role definition"
        );
    }

    #[test]
    fn test_guidelines_section_title_passes() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![Section {
                level: 1,
                title: "Repository Guidelines".to_string(),
                line: 1,
                end_line: 10,
            }],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section titled 'Repository Guidelines' should count as role definition"
        );
    }

    #[test]
    fn test_overview_section_title_passes() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![Section {
                level: 2,
                title: "Project Overview".to_string(),
                line: 1,
                end_line: 10,
            }],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section titled 'Project Overview' should count as role definition"
        );
    }

    #[test]
    fn test_multi_word_guidelines_title_passes() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![Section {
                level: 1,
                title: "Claude AI Assistant Guidelines".to_string(),
                line: 1,
                end_line: 10,
            }],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Multi-word title containing 'Guidelines' should count as role definition"
        );
    }

    #[test]
    fn test_this_guide_provides_passes() {
        let mut lines = vec!["This guide provides instructions for working in this repository."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "'This guide provides instructions...' should count as implicit role definition"
        );
    }

    #[test]
    fn test_guidelines_for_ai_agents_passes() {
        let mut lines = vec!["Guidelines for AI agents working in this repo."];
        lines.extend_from_slice(&enough_directives());
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "'Guidelines for AI agents' should count as implicit role definition"
        );
    }

    #[test]
    fn test_sectionless_file_needs_25_directives() {
        // 18 directive lines in a sectionless file should NOT flag
        let lines: Vec<&str> = vec![
            "Always run tests.",
            "Never skip CI.",
            "Use structured logging.",
            "Follow the style guide.",
            "Run linting before commit.",
            "Keep functions small.",
            "Ensure proper error handling.",
            "Check return values.",
            "Avoid global state.",
            "Must validate all inputs.",
            "Write documentation for public APIs.",
            "Use meaningful variable names.",
            "Prefer composition over inheritance.",
            "Handle edge cases explicitly.",
            "Review your own code before submitting.",
            "Always write tests first.",
            "Never merge without review.",
            "Use feature branches.",
        ];
        let result = run_check(&lines);
        assert!(
            result.diagnostics.is_empty(),
            "Sectionless files with < 25 directive lines should not flag"
        );
    }

    #[test]
    fn test_well_structured_file_skipped() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![
                Section {
                    level: 2,
                    title: "Development Workflow".to_string(),
                    line: 1,
                    end_line: 8,
                },
                Section {
                    level: 2,
                    title: "Testing".to_string(),
                    line: 9,
                    end_line: 16,
                },
                Section {
                    level: 2,
                    title: "Code Style".to_string(),
                    line: 17,
                    end_line: 24,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Files with 3+ sections are structurally self-documenting and should be skipped"
        );
    }

    #[test]
    fn test_two_section_file_still_flags() {
        let lines = enough_directives();
        let result = run_check_with_sections(
            &lines,
            vec![
                Section {
                    level: 2,
                    title: "Development Workflow".to_string(),
                    line: 1,
                    end_line: 12,
                },
                Section {
                    level: 2,
                    title: "Testing".to_string(),
                    line: 13,
                    end_line: 24,
                },
            ],
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Files with only 2 sections should still be checked for role definition"
        );
    }

    #[test]
    fn test_reference_file_without_imperatives_skipped() {
        let result = run_check(&[
            "# Company Overview",
            "",
            "TechStart Inc is a B2B SaaS company.",
            "- ARR: $2.4M",
            "- Burn Rate: $500K/month",
            "- Runway: 20 months",
            "- Revenue per Employee: $48K",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Reference files without imperative instructions should be skipped"
        );
    }
}
