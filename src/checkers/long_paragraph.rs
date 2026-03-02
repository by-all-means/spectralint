use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct LongParagraphChecker {
    scope: ScopeFilter,
    max_lines: usize,
}

impl LongParagraphChecker {
    pub(crate) fn new(config: &crate::config::LongParagraphConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            max_lines: config.max_lines,
        }
    }
}

impl Checker for LongParagraphChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut para_start: Option<usize> = None;
            let mut para_len: usize = 0;

            // Flush the current paragraph if it exceeds the limit.
            let flush = |result: &mut CheckResult, start: Option<usize>, len: usize| {
                if let Some(s) = start {
                    if len > self.max_lines {
                        emit_long_para(result, file, s, len, self.max_lines);
                    }
                }
            };

            for (i, line) in file.raw_lines.iter().enumerate() {
                let in_code = *file.in_code_block.get(i).unwrap_or(&false);
                let trimmed = line.trim();

                let breaks_paragraph = in_code
                    || trimmed.is_empty()
                    || trimmed.starts_with('#')
                    || trimmed.starts_with("```")
                    || trimmed.starts_with("---")
                    || trimmed.starts_with("===")
                    || trimmed.starts_with('|')
                    || trimmed.starts_with("<!--")
                    || trimmed.starts_with("- ")
                    || trimmed.starts_with("* ")
                    || trimmed.starts_with("+ ")
                    || trimmed.starts_with("> ")
                    || (trimmed
                        .as_bytes()
                        .first()
                        .is_some_and(|b| b.is_ascii_digit())
                        && trimmed.contains(". "));

                if breaks_paragraph {
                    flush(&mut result, para_start, para_len);
                    para_start = None;
                    para_len = 0;
                } else {
                    if para_start.is_none() {
                        para_start = Some(i);
                    }
                    para_len += 1;
                }
            }

            flush(&mut result, para_start, para_len);
        }

        result
    }
}

fn emit_long_para(
    result: &mut CheckResult,
    file: &crate::parser::types::ParsedFile,
    start: usize,
    len: usize,
    max: usize,
) {
    emit!(
        result,
        file.path,
        start + 1,
        Severity::Info,
        Category::LongParagraph,
        suggest: "Break into shorter paragraphs or use bullet points for easier agent parsing",
        "paragraph spans {} consecutive lines (limit: {}) — dense text blocks are harder for agents to parse",
        len,
        max
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;
    use crate::config::LongParagraphConfig;

    fn check_with_max(lines: &[&str], max_lines: usize) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        let config = LongParagraphConfig {
            enabled: true,
            max_lines,
            scope: Vec::new(),
            severity: None,
        };
        LongParagraphChecker::new(&config).check(&ctx)
    }

    fn check(lines: &[&str]) -> CheckResult {
        check_with_max(lines, 8)
    }

    #[test]
    fn test_long_paragraph_flagged() {
        let result = check(&[
            "This is a very long paragraph that goes on and on.",
            "It continues with more text on this line.",
            "And yet more text follows here.",
            "The paragraph keeps going without a break.",
            "Still more content in this dense block.",
            "Even more text is added to this paragraph.",
            "This line makes it seven consecutive lines.",
            "Eight lines now, still no break.",
            "Nine lines — this exceeds the default threshold.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::LongParagraph);
    }

    #[test]
    fn test_short_paragraph_not_flagged() {
        let result = check(&[
            "This is a short paragraph.",
            "It only has three lines.",
            "That's well within the limit.",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_exactly_at_limit_not_flagged() {
        let result = check_with_max(&["Line one.", "Line two.", "Line three.", "Line four."], 4);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_one_over_limit_flagged() {
        let result = check_with_max(
            &[
                "Line one.",
                "Line two.",
                "Line three.",
                "Line four.",
                "Line five.",
            ],
            4,
        );
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_blank_line_breaks_paragraph() {
        let result = check_with_max(
            &[
                "Line one.",
                "Line two.",
                "Line three.",
                "",
                "Line four.",
                "Line five.",
                "Line six.",
            ],
            4,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_breaks_paragraph() {
        let result = check_with_max(
            &[
                "Line one.",
                "Line two.",
                "Line three.",
                "## Heading",
                "Line four.",
                "Line five.",
            ],
            4,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_list_items_break_paragraph() {
        let result = check_with_max(
            &[
                "Line one.",
                "Line two.",
                "Line three.",
                "- List item",
                "Line four.",
                "Line five.",
            ],
            4,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_counted() {
        let result = check_with_max(
            &[
                "```",
                "this is code line 1",
                "this is code line 2",
                "this is code line 3",
                "this is code line 4",
                "this is code line 5",
                "```",
            ],
            3,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_long_paragraphs() {
        let result = check_with_max(
            &[
                "Para one line one.",
                "Para one line two.",
                "Para one line three.",
                "Para one line four.",
                "",
                "Para two line one.",
                "Para two line two.",
                "Para two line three.",
                "Para two line four.",
            ],
            3,
        );
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_table_breaks_paragraph() {
        let result = check_with_max(
            &[
                "Line one.",
                "Line two.",
                "| col1 | col2 |",
                "Line three.",
                "Line four.",
            ],
            3,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_line_number_correct() {
        let result = check_with_max(
            &[
                "# Heading",
                "",
                "Short para.",
                "",
                "Long para line 1.",
                "Long para line 2.",
                "Long para line 3.",
                "Long para line 4.",
            ],
            3,
        );
        assert_eq!(result.diagnostics.len(), 1);
        // Paragraph starts at line 5 (0-indexed: 4, 1-indexed: 5)
        assert_eq!(result.diagnostics[0].line, 5);
    }
}
