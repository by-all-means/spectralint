use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

pub struct OutdatedModelReferenceChecker {
    scope: ScopeFilter,
}

impl OutdatedModelReferenceChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static OUTDATED_MODELS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:",
        r"gpt[- ]?3\.5",
        r"|gpt[- ]?4[- ]?turbo",
        r"|claude[- ]?2",
        r"|claude[- ]?instant",
        r"|claude[- ]?3[- ]?(?:haiku|sonnet|opus)",
        r"|text-davinci",
        r"|code-davinci",
        r"|claude-(?:v1|1)",
        r")\b",
    ))
    .unwrap()
});

/// Lines containing these words (case-insensitive) are self-documenting
/// deprecation and should not be flagged.
static EXCLUSION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:histor(?:y|ical(?:ly)?)|changelog|deprecated)\b").unwrap()
});

impl Checker for OutdatedModelReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                if is_heading(line) || EXCLUSION_PATTERN.is_match(line) {
                    continue;
                }

                if let Some(m) = OUTDATED_MODELS.find(line) {
                    emit!(
                        result,
                        file.path,
                        idx + 1,
                        Severity::Info,
                        Category::OutdatedModelReference,
                        suggest: "Update to a current model name — outdated references may cause agent confusion or API errors",
                        "Outdated model reference: {}",
                        m.as_str()
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
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        OutdatedModelReferenceChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_gpt35_flags() {
        let result = run_check(&["Use GPT-3.5 for cheap tasks"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("GPT-3.5"));
    }

    #[test]
    fn test_claude3_sonnet_flags() {
        let result = run_check(&["Use claude-3-sonnet for summaries"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_current_model_no_flag() {
        let result = run_check(&["Use claude-sonnet-4-20250514"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_gpt4o_no_flag() {
        let result = run_check(&["Use gpt-4o for reasoning"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_in_code_block_no_flag() {
        let result = run_check(&["```", "model = 'gpt-3.5-turbo'", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_historical_context_no_flag() {
        let result = run_check(&["Historically we used GPT-3.5"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_heading_no_flag() {
        let result = run_check(&["## GPT-3.5 Migration"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_claude2_flags() {
        let result = run_check(&["We recommend claude-2 for this task"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_text_davinci_flags() {
        let result = run_check(&["Use text-davinci-003 as fallback"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_claude_instant_flags() {
        let result = run_check(&["Try claude-instant for speed"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_deprecated_keyword_no_flag() {
        let result = run_check(&["GPT-3.5 is deprecated, use gpt-4o instead"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_changelog_no_flag() {
        let result = run_check(&["changelog: migrated from claude-2 to claude-sonnet-4"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_gpt4_turbo_flags() {
        let result = run_check(&["Switch to gpt-4-turbo for faster responses"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_claude_v1_flags() {
        let result = run_check(&["claude-v1 was the first model"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
