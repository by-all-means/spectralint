use regex::Regex;
use std::sync::LazyLock;

use crate::config::MissingVerificationConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::types::ParsedFile;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct MissingVerificationChecker {
    scope: ScopeFilter,
    min_action_verbs: usize,
}

impl MissingVerificationChecker {
    pub fn new(config: &MissingVerificationConfig) -> Self {
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
    Regex::new(r"(?i)\b(?:overview|architecture|design|pattern|migration|history|background|how\s+it\s+works|data\s+flow|key\s+(?:concepts|differences|components|classes|technologies)|compatibility|important\s+(?:patterns|notes))\b").unwrap()
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

fn check_sections(file: &ParsedFile, min_action_verbs: usize, result: &mut CheckResult) {
    for section in &file.sections {
        // Get lines for this section
        let start = section.line.saturating_sub(1);
        let end = section.end_line.min(file.raw_lines.len());

        if end <= start {
            continue;
        }

        let section_lines = &file.raw_lines[start..end];

        // Skip sections < 3 lines
        if section_lines.len() < 3 {
            continue;
        }

        // Skip informational/descriptive sections — they use action verbs in
        // explanatory context, not as procedural instructions.
        if INFORMATIONAL_TITLE.is_match(&section.title) {
            continue;
        }

        // Count action verbs in non-code lines
        let mut action_count = 0;
        let mut has_verification = false;
        let mut in_code_block = false;

        for line in section_lines {
            if line.trim().starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }

            if in_code_block {
                // Check for test commands in code blocks
                if TEST_COMMAND.is_match(line) {
                    has_verification = true;
                }
                continue;
            }

            // Count action verbs
            if ACTION_VERB.is_match(line) {
                action_count += 1;
            }

            // Check for verification signals
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
            min_action_verbs: 2,
            scope: Vec::new(),
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
                    end_line: 6,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 7,
                    end_line: 9,
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
                    end_line: 6,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 7,
                    end_line: 9,
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
                    end_line: 9,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 10,
                    end_line: 12,
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
                    end_line: 6,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 7,
                    end_line: 9,
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
                    end_line: 6,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 7,
                    end_line: 9,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "\"should see\" phrase should count as verification"
        );
    }
}
