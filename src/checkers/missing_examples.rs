use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct MissingExamplesChecker {
    scope: ScopeFilter,
}

impl MissingExamplesChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Format-specifying language in imperative/instructional framing.
/// Must be a command to the agent, not a description of existing behavior.
/// Requires imperative verbs or "must/should" modals before the format name.
static FORMAT_SPEC: LazyLock<Regex> = LazyLock::new(|| {
    let fmt = r"(?:JSON|YAML|XML|CSV|markdown|HTML|TOML)";
    Regex::new(&format!(
        r"(?ix)
        (?:always\s+|must\s+|should\s+)?format\s+(?:as|in|the\s+output\s+as)\s+{fmt}
        | output\s+must\s+be\s+(?:valid\s+)?{fmt}
        | (?:must\s+|should\s+|always\s+)?respond\s+with\s+{fmt}
        | (?:must\s+|should\s+|always\s+)return\s+(?:a\s+)?(?:JSON|YAML|XML|CSV)\b
        | (?:must\s+be|should\s+be)\s+structured\s+as\s+(?:JSON|YAML|XML)
    "
    ))
    .unwrap()
});

/// Signals that an example is present inline.
static EXAMPLE_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:e\.g\.|for\s+example|here\s+is\s+(?:an?\s+)?example|such\s+as|like\s+this|sample\s+(?:output|response|input))\b").unwrap()
});

/// Section titles that indicate an example section.
static EXAMPLE_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:examples?|sample)\b").unwrap());

impl Checker for MissingExamplesChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "missing-examples",
            description: "Flags format specs without accompanying code examples",
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

            let has_example_section = file
                .sections
                .iter()
                .any(|s| EXAMPLE_TITLE.is_match(&s.title));

            if has_example_section {
                continue;
            }

            for section in &file.sections {
                let start = section.line.saturating_sub(1);
                let end = section.end_line.min(file.raw_lines.len());
                if end <= start {
                    continue;
                }

                let mut format_line = 0;
                let mut has_code_block = false;
                let mut has_example_signal = false;

                for idx in start..end {
                    if file.is_code(idx) {
                        has_code_block = true;
                        continue;
                    }

                    let line = &file.raw_lines[idx];

                    if format_line == 0 && FORMAT_SPEC.is_match(line) {
                        format_line = idx + 1;
                    }

                    if EXAMPLE_SIGNAL.is_match(line) {
                        has_example_signal = true;
                    }
                }

                if format_line > 0 && !has_code_block && !has_example_signal {
                    emit!(
                        result,
                        file.path,
                        format_line,
                        Severity::Info,
                        Category::MissingExamples,
                        suggest: "Add a code block with a concrete example of the expected format",
                        "Section \"{}\" specifies an output format but provides no example. \
                         Agents comply better when they can see the target shape.",
                        section.title
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
    use crate::checkers::utils::test_helpers::single_file_ctx_with_sections;
    use crate::parser::types::Section;

    fn run_check(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        MissingExamplesChecker::new(&[]).check(&ctx)
    }

    fn two_sections(end1: usize, end2: usize) -> Vec<Section> {
        vec![
            Section {
                level: 1,
                title: "Output".to_string(),
                line: 1,
                end_line: end1,
            },
            Section {
                level: 1,
                title: "Other".to_string(),
                line: end1 + 1,
                end_line: end2,
            },
        ]
    }

    #[test]
    fn test_format_spec_without_example_flags() {
        let result = run_check(
            &[
                "# Output",
                "Format as JSON when returning results.",
                "# Other",
                "Info.",
            ],
            two_sections(2, 4),
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::MissingExamples);
        assert!(result.diagnostics[0].message.contains("Output"));
    }

    #[test]
    fn test_format_spec_with_code_block_passes() {
        let result = run_check(
            &[
                "# Output",
                "Format as JSON when returning results.",
                "```json",
                "{\"key\": \"value\"}",
                "```",
                "# Other",
                "Info.",
            ],
            two_sections(5, 7),
        );
        assert!(
            result.diagnostics.is_empty(),
            "Format spec with code block should pass"
        );
    }

    #[test]
    fn test_format_spec_with_example_signal_passes() {
        let result = run_check(
            &[
                "# Output",
                "Format as JSON when returning results.",
                "For example, use {\"status\": \"ok\"}.",
                "# Other",
                "Info.",
            ],
            two_sections(3, 5),
        );
        assert!(
            result.diagnostics.is_empty(),
            "Format spec with example signal should pass"
        );
    }

    #[test]
    fn test_example_section_sibling_passes() {
        let result = run_check(
            &[
                "# Output",
                "Format as JSON when returning results.",
                "# Examples",
                "Some examples here.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Output".to_string(),
                    line: 1,
                    end_line: 2,
                },
                Section {
                    level: 1,
                    title: "Examples".to_string(),
                    line: 3,
                    end_line: 4,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Sibling Example section should suppress the warning"
        );
    }

    #[test]
    fn test_no_format_spec_no_flag() {
        let result = run_check(
            &["# Rules", "Always run tests.", "# Other", "Info."],
            two_sections(2, 4),
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_respond_with_yaml_flags() {
        let result = run_check(
            &[
                "# Config",
                "Respond with YAML for configuration values.",
                "# Other",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Config".to_string(),
                    line: 1,
                    end_line: 2,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 3,
                    end_line: 4,
                },
            ],
        );
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_descriptive_format_mention_no_flag() {
        let result = run_check(
            &[
                "# Backend Patterns",
                "Controllers validate input, call services, return JSON responses.",
                "# Other",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Backend Patterns".to_string(),
                    line: 1,
                    end_line: 2,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 3,
                    end_line: 4,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Descriptive mentions of formats (not instructions) should not flag"
        );
    }

    #[test]
    fn test_output_must_be_flags() {
        let result = run_check(
            &[
                "# Response",
                "The output must be valid JSON.",
                "# Other",
                "Info.",
            ],
            vec![
                Section {
                    level: 1,
                    title: "Response".to_string(),
                    line: 1,
                    end_line: 2,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 3,
                    end_line: 4,
                },
            ],
        );
        assert_eq!(result.diagnostics.len(), 1);
    }
}
