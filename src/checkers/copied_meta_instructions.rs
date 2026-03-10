use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

/// Patterns that indicate leftover AI boilerplate / meta-instructions.
static META_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // "You are a helpful assistant" variants
        Regex::new(r"(?i)^you\s+are\s+a\s+helpful\s+(?:assistant|AI|chatbot|language\s+model)")
            .unwrap(),
        // "As an AI language model" disclaimers
        Regex::new(r"(?i)^as\s+an?\s+(?:AI|artificial\s+intelligence)\s+(?:language\s+)?model")
            .unwrap(),
        // "I'm an AI" / "I am an AI"
        Regex::new(r"(?i)^I[''']?m\s+an?\s+AI\b").unwrap(),
        Regex::new(r"(?i)^I\s+am\s+an?\s+AI\b").unwrap(),
        // "I cannot" / "I'm not able to" disclaimers
        Regex::new(r"(?i)^I\s+(?:cannot|can[''']?t|am\s+not\s+able\s+to)\s+(?:browse|access|search)\s+the\s+(?:internet|web)").unwrap(),
        // "My training data" / "my knowledge cutoff"
        Regex::new(r"(?i)\bmy\s+(?:training\s+data|knowledge\s+cutoff|training\s+cutoff)").unwrap(),
    ]
});

pub(crate) struct CopiedMetaInstructionsChecker {
    scope: ScopeFilter,
}

impl CopiedMetaInstructionsChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for CopiedMetaInstructionsChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "copied-meta-instructions",
            description: "Flags AI boilerplate",
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

            for (i, line) in non_code_lines_masked(&file.raw_lines, &file.in_code_block) {
                let line_num = i + 1;

                let trimmed = line.trim();
                if trimmed.is_empty() || is_heading(trimmed) {
                    continue;
                }

                // Strip leading list markers
                let text = trimmed
                    .trim_start_matches(['-', '*', '+'])
                    .trim_start()
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .trim_start_matches('.')
                    .trim_start();

                for pattern in META_PATTERNS.iter() {
                    if pattern.is_match(text) {
                        emit!(
                            result,
                            file.path,
                            line_num,
                            Severity::Info,
                            Category::CopiedMetaInstructions,
                            suggest: "Remove AI boilerplate — instruction files should contain project-specific directives",
                            "copied meta-instruction — this looks like AI-generated boilerplate, not a project directive"
                        );
                        break; // one per line
                    }
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

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        CopiedMetaInstructionsChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_helpful_assistant_flagged() {
        let result = check(&["You are a helpful assistant that writes clean code."]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].category,
            Category::CopiedMetaInstructions
        );
    }

    #[test]
    fn test_helpful_ai_flagged() {
        let result = check(&["You are a helpful AI that helps developers."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_as_an_ai_flagged() {
        let result = check(&["As an AI language model, I cannot browse the internet."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_im_an_ai_flagged() {
        let result = check(&["I'm an AI and I follow instructions."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_knowledge_cutoff_flagged() {
        let result = check(&["My training data only goes up to 2024."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_cannot_browse_flagged() {
        let result = check(&["I cannot browse the internet for live data."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_normal_instruction_not_flagged() {
        let result = check(&["Always run cargo test before committing."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_you_are_role_not_flagged() {
        // "You are a senior engineer" is a valid role definition, not boilerplate
        let result = check(&["You are a senior Rust engineer working on spectralint."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "You are a helpful assistant", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_in_list_item_flagged() {
        let result = check(&["- You are a helpful assistant that helps with tasks."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    // --- FP/FN regression tests ---

    #[test]
    fn test_boilerplate_in_code_block_not_flagged() {
        // Boilerplate text inside a fenced code block should not be flagged,
        // since it may be a quoted example or configuration snippet.
        let result = check(&["```", "You are a helpful assistant", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Boilerplate inside a code block should not be flagged"
        );
    }

    #[test]
    #[ignore = "FN: markdown emphasis (**bold**) inside boilerplate phrases breaks regex matching — checker does not strip inline formatting before pattern matching"]
    fn test_boilerplate_with_emphasis_still_flagged() {
        // Markdown emphasis around a word inside a known boilerplate phrase
        // should still be detected. Currently the regex does not account for
        // inline formatting like **bold** or *italic*, causing a false negative.
        let result = check(&["You are a **helpful** AI assistant"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Boilerplate with markdown emphasis should still be flagged"
        );
    }
}
