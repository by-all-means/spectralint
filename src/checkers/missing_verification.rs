use regex::Regex;
use std::sync::LazyLock;

use crate::config::MissingVerificationConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::types::ParsedFile;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct MissingVerificationChecker {
    scope: ScopeFilter,
    min_action_verbs: usize,
}

impl MissingVerificationChecker {
    pub(crate) fn new(config: &MissingVerificationConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            min_action_verbs: config.min_action_verbs,
        }
    }
}

/// Section titles that indicate informational/descriptive content, not procedural steps.
/// These sections use action verbs in explanatory context ("the migration updated X")
/// rather than instructing the reader to perform actions.
static INFORMATIONAL_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:overview|architecture|design|pattern|migration|history|background|how\s+it\s+works|data\s+flow|key\s+(?:concepts|differences|components|classes|technologies)|compatibility|important\s+(?:patterns|notes)|endpoint|api|command|convention|standard|style|permission|pitfall|troubleshoot|faq|reference|structure|schema|model|summary|table\s+of\s+contents|contents|next\s+steps|auto[- ]?approved|authorized|capabilities|features|tools|getting\s+started|quick\s+start)\b").unwrap()
});

/// Action verbs that indicate procedural steps (not style guidance).
static ACTION_VERB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:run|execute|create|build|deploy|install|configure|implement|set\s+up|start|stop|restart|migrate|compile|generate|delete|remove|update|upgrade)\b").unwrap()
});

/// Verification signals — any of these in a section means verification is present.
static VERIFICATION_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:verify|validate|test|assert|expect|confirm|check|ensure)\b").unwrap()
});

static VERIFICATION_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:expected\s+output|should\s+(?:see|return|output|produce|display|show)|success\s+criteria|looks?\s+like)").unwrap()
});

/// Command patterns in code blocks that indicate test/verify steps.
static TEST_COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:cargo\s+test|npm\s+test|pytest|go\s+test|jest|mocha|rspec|make\s+test)\b")
        .unwrap()
});

impl Checker for MissingVerificationChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "missing-verification",
            description: "Flags action sections without verification or success criteria",
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

            // Skip files with < 2 sections
            if file.sections.len() < 2 {
                continue;
            }

            check_sections(file, self.min_action_verbs, &mut result);
        }

        result
    }
}

/// List markers to strip before checking for backtick-leading content.
static CMD_REF_LIST_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[-*+]|\d+\.)\s*").unwrap());

/// Returns true if >60% of content lines in a section start with a backtick
/// after stripping list markers. Indicates a command-reference section.
fn is_command_reference(section_lines: &[String], mask: &[bool], offset: usize) -> bool {
    let mut content_lines: usize = 0;
    let mut backtick_lines: usize = 0;

    for (i, line) in section_lines.iter().enumerate() {
        if *mask.get(offset + i).unwrap_or(&false) {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        content_lines += 1;
        let stripped = CMD_REF_LIST_MARKER.replace(trimmed, "");
        if stripped.starts_with('`') {
            backtick_lines += 1;
        }
    }

    // >60% backtick-leading lines with at least 3 content lines
    content_lines >= 3 && backtick_lines * 5 > content_lines * 3
}

fn check_sections(file: &ParsedFile, min_action_verbs: usize, result: &mut CheckResult) {
    for section in &file.sections {
        let start = section.line.saturating_sub(1);
        let end = section.end_line.min(file.raw_lines.len());

        if end <= start {
            continue;
        }

        let section_lines = &file.raw_lines[start..end];

        if section_lines.len() < 3 {
            continue;
        }

        if INFORMATIONAL_TITLE.is_match(&section.title) {
            continue;
        }

        if is_command_reference(section_lines, &file.in_code_block, start) {
            continue;
        }

        let mut action_count = 0;
        let mut has_verification = false;

        for (i, line) in section_lines.iter().enumerate() {
            let in_code = *file.in_code_block.get(start + i).unwrap_or(&false);

            if in_code {
                if TEST_COMMAND.is_match(line) {
                    has_verification = true;
                }
                continue;
            }

            if ACTION_VERB.is_match(line) {
                action_count += 1;
            }

            if VERIFICATION_SIGNAL.is_match(line) || VERIFICATION_PHRASE.is_match(line) {
                has_verification = true;
            }
        }

        if action_count >= min_action_verbs && !has_verification {
            emit!(
                result,
                file.path,
                section.line,
                Severity::Info,
                Category::MissingVerification,
                suggest: "Add verification steps: expected output, test commands, or success criteria",
                "Section \"{}\" has {} action directives but no verification or success criteria",
                section.title,
                action_count
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx_with_sections;
    use crate::parser::types::Section;

    fn run_check_with_sections(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        let config = MissingVerificationConfig {
            enabled: true,
            min_action_verbs: 4,
            scope: Vec::new(),
            severity: None,
        };
        MissingVerificationChecker::new(&config).check(&ctx)
    }

    #[test]
    fn test_section_with_actions_no_verification_flags() {
        let result = run_check_with_sections(
            &[
                "# Setup",
                "",
                "Run the build command.",
                "Execute the migration script.",
                "Install the dependencies.",
                "Create the config file.",
                "",
                "# Other",
                "",
                "Info here.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Setup".to_string(),
                    line: 1,
                    end_line: 7,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 8,
                    end_line: 10,
                },
            ],
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert!(result.diagnostics[0].message.contains("Setup"));
    }

    #[test]
    fn test_section_with_verification_passes() {
        let result = run_check_with_sections(
            &[
                "# Setup",
                "",
                "Run the build command.",
                "Execute the migration.",
                "Install the packages.",
                "Create the database.",
                "Verify the output is correct.",
                "",
                "# Other",
                "",
                "Info here.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Setup".to_string(),
                    line: 1,
                    end_line: 8,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 9,
                    end_line: 11,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section with verify keyword should pass"
        );
    }

    #[test]
    fn test_section_with_test_command_passes() {
        let result = run_check_with_sections(
            &[
                "# Setup",
                "",
                "Run the build command.",
                "Execute the migration.",
                "Install the packages.",
                "Create the config.",
                "",
                "```",
                "cargo test",
                "```",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Setup".to_string(),
                    line: 1,
                    end_line: 11,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 12,
                    end_line: 14,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section with test command in code block should pass"
        );
    }

    #[test]
    fn test_single_action_verb_below_threshold() {
        let result = run_check_with_sections(
            &[
                "# Setup",
                "",
                "Run the build command.",
                "Then wait for it.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Setup".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 8,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section with < 2 action verbs should not flag"
        );
    }

    #[test]
    fn test_single_section_file_skipped() {
        let result = run_check_with_sections(
            &[
                "# Only Section",
                "",
                "Run the build.",
                "Execute the deploy.",
            ],
            vec![Section {
                level: 1,
                title: "Only Section".to_string(),
                line: 1,
                end_line: 4,
            }],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Files with < 2 sections should be skipped"
        );
    }

    #[test]
    fn test_informational_section_skipped() {
        // "Architecture Overview" is descriptive, not procedural
        let result = run_check_with_sections(
            &[
                "# Architecture Overview",
                "",
                "The system creates connections and deploys workers.",
                "State is updated through the pipeline.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Architecture Overview".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 8,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Informational sections (Architecture, Design, etc.) should be skipped"
        );
    }

    #[test]
    fn test_design_pattern_section_skipped() {
        let result = run_check_with_sections(
            &[
                "# Elmish MVU Pattern",
                "",
                "The framework creates state and updates it.",
                "Components build views from the model.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Elmish MVU Pattern".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 8,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Pattern/design sections should be skipped"
        );
    }

    #[test]
    fn test_migration_section_skipped() {
        let result = run_check_with_sections(
            &[
                "# React Context Migration (2025)",
                "",
                "The project removed Redux and created new context providers.",
                "State updates now go through setState calls.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "React Context Migration (2025)".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 8,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Migration/history sections should be skipped"
        );
    }

    #[test]
    fn test_procedural_section_still_flags() {
        // "Setup" is procedural, should still flag
        let result = run_check_with_sections(
            &[
                "# Setup",
                "",
                "Run the build command.",
                "Execute the migration script.",
                "Install the dependencies.",
                "Create the output directory.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Setup".to_string(),
                    line: 1,
                    end_line: 7,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 8,
                    end_line: 10,
                },
            ],
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Procedural sections should still flag"
        );
    }

    #[test]
    fn test_expected_output_phrase_passes() {
        let result = run_check_with_sections(
            &[
                "# Deploy",
                "",
                "Run the deploy script.",
                "Execute the rollback plan.",
                "Install the monitoring agent.",
                "Configure the load balancer.",
                "You should see a success message.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Deploy".to_string(),
                    line: 1,
                    end_line: 8,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 9,
                    end_line: 11,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "\"should see\" phrase should count as verification"
        );
    }

    #[test]
    fn test_command_reference_section_skipped() {
        let result = run_check_with_sections(
            &[
                "# CLI Commands",
                "",
                "- `cargo build` — compile the project",
                "- `cargo test` — run all tests",
                "- `cargo run` — execute the binary",
                "- `cargo install` — install globally",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "CLI Commands".to_string(),
                    line: 1,
                    end_line: 7,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 8,
                    end_line: 10,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Command-reference sections (>60% backtick-leading) should be skipped"
        );
    }

    #[test]
    fn test_summary_section_skipped() {
        let result = run_check_with_sections(
            &[
                "# Summary",
                "",
                "Run the build pipeline.",
                "Execute the deployment.",
                "Install the monitoring.",
                "Configure the dashboard.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Summary".to_string(),
                    line: 1,
                    end_line: 7,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 8,
                    end_line: 10,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Summary sections should be skipped as informational"
        );
    }

    #[test]
    fn test_section_with_verification_at_end() {
        // Verification keyword at the very end of the section should still count
        let result = run_check_with_sections(
            &[
                "# Deploy",
                "",
                "Run the build command.",
                "Execute the migration script.",
                "Install the dependencies.",
                "Create the output directory.",
                "Finally, verify the deployment succeeded.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Deploy".to_string(),
                    line: 1,
                    end_line: 8,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 9,
                    end_line: 11,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section with verify keyword at end should pass"
        );
    }

    #[test]
    fn test_section_with_no_steps() {
        // A section with no action verbs at all should not flag
        let result = run_check_with_sections(
            &[
                "# Notes",
                "",
                "This section has no actionable steps.",
                "Just some information here.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Notes".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 8,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section with no action verbs should not flag"
        );
    }

    #[test]
    fn test_section_with_single_step() {
        // A section with only one action verb (below threshold of 4) should not flag
        let result = run_check_with_sections(
            &[
                "# Quick Fix",
                "",
                "Run the repair script.",
                "",
                "# Other",
                "",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Quick Fix".to_string(),
                    line: 1,
                    end_line: 4,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 5,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Section with a single action verb should not flag (below threshold)"
        );
    }
}
