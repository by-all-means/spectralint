use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{inside_inline_code, ScopeFilter};
use super::Checker;

static ATTRIBUTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // "<verb> with/by/using <AI tool>"
        Regex::new(r"(?i)\b(?:generated|created|built|made|written|authored)\s+(?:with|by|using)\s+(?:claude\s+code|claude|copilot|chatgpt|gpt-?\d|cursor|windsurf|aider|cody)\b").unwrap(),
        // "Co-Authored-By: <AI>"
        Regex::new(r"(?i)co-authored-by:\s*(?:claude|copilot|chatgpt|gpt-?\d|cursor|windsurf|aider|cody)\b").unwrap(),
        // Emoji robot/rocket + "Generated with" (common badge pattern)
        Regex::new(r"(?i)[\U0001F916\U0001F680]\s*generated\s+(?:with|by|using)\b").unwrap(),
    ]
});

/// Returns true if the matched range sits inside a quoted substring.
fn inside_quotes(line: &str, match_start: usize, match_end: usize) -> bool {
    ['"', '\u{201C}', '\u{201D}']
        .iter()
        .any(|&q| line[..match_start].contains(q) && line[match_end..].contains(q))
}

static PROHIBITION_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:do\s+not |don[\u{2019}']t |never |avoid |without )").unwrap()
});

fn is_prohibition_context(line: &str) -> bool {
    PROHIBITION_CONTEXT.is_match(line)
}

pub(crate) struct GeneratedAttributionChecker {
    scope: ScopeFilter,
}

impl GeneratedAttributionChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for GeneratedAttributionChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "generated-attribution",
            description: "Flags AI-tool attribution lines that waste context tokens",
            default_severity: Severity::Info,
            strict_only: false,
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
                if trimmed.is_empty() {
                    continue;
                }

                // Skip HTML comments — not visible to agents
                if trimmed.starts_with("<!--") {
                    continue;
                }

                // Skip lines that instruct against using attribution
                if is_prohibition_context(trimmed) {
                    continue;
                }

                for pattern in ATTRIBUTION_PATTERNS.iter() {
                    if let Some(m) = pattern.find(trimmed) {
                        let absolute_pos =
                            (trimmed.as_ptr() as usize - line.as_ptr() as usize) + m.start();
                        // Skip if the match is inside inline code
                        if inside_inline_code(line, absolute_pos) {
                            continue;
                        }
                        // Skip if the match is inside quotes (likely an example)
                        if inside_quotes(trimmed, m.start(), m.end()) {
                            continue;
                        }

                        emit!(
                            result,
                            file.path,
                            line_num,
                            Severity::Info,
                            Category::GeneratedAttribution,
                            suggest: "Remove AI-tool attribution — it wastes context tokens and provides no value to agents",
                            "AI-tool attribution line — this is noise in an instruction file"
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
        GeneratedAttributionChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_generated_with_claude_code() {
        let result = check(&["Generated with Claude Code"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].category,
            Category::GeneratedAttribution
        );
    }

    #[test]
    fn test_emoji_generated_with() {
        let result = check(&["\u{1F916} Generated with Claude Code"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_created_by_chatgpt() {
        let result = check(&["Created by ChatGPT"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_co_authored_by_claude() {
        let result = check(&["Co-Authored-By: Claude"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_built_with_cursor() {
        let result = check(&["Built with Cursor"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_made_with_copilot() {
        let result = check(&["Made with Copilot"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_written_using_aider() {
        let result = check(&["Written using Aider"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_generated_by_gpt4() {
        let result = check(&["Generated by GPT4"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_generated_by_gpt_4() {
        let result = check(&["Generated by GPT-4"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_case_insensitive() {
        let result = check(&["GENERATED WITH CLAUDE CODE"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_normal_instruction_not_flagged() {
        let result = check(&["Always run cargo test before committing."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "Generated with Claude Code", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_code_not_flagged() {
        let result = check(&["The footer says `Generated with Claude Code`."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_uses_word_not_flagged() {
        // "generated" in a different context should not fire
        let result = check(&["The generated files should be committed."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_windsurf_flagged() {
        let result = check(&["Created using Windsurf"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_cody_flagged() {
        let result = check(&["Built with Cody"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_prohibition_not_flagged() {
        // Instructions about NOT using attribution should not fire
        let result = check(&["Do NOT add \"Generated with Claude Code\" to commits"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_do_not_include_not_flagged() {
        let result = check(&[
            "- Do not include AI co-authoring information (e.g., \"Co-Authored-By: Claude\")",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_never_add_not_flagged() {
        let result = check(&["Never add \"Generated with Claude Code\" footers"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_avoid_not_flagged() {
        let result = check(&["Avoid adding \"Built with Cursor\" to your files"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_quoted_example_not_flagged() {
        let result = check(&["Remove lines like \"Generated with Claude Code\" from files"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_html_comment_auto_generated_not_flagged() {
        // HTML comments about auto-generation are not attribution
        let result = check(&["<!-- This section is auto-generated by claude-mem. -->"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_lines_one_per_line() {
        let result = check(&[
            "Generated with Claude Code",
            "Some normal text",
            "Built with Cursor",
        ]);
        assert_eq!(result.diagnostics.len(), 2);
    }
}
