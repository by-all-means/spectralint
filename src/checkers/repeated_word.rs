use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, Fix, Replacement, RuleMeta, Severity};

use super::utils::{inside_inline_code, ScopeFilter};
use super::Checker;

/// Pattern to extract word tokens from a line.
static WORD_TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\w+\b").unwrap());

/// Words that are grammatically valid as consecutive duplicates.
const ALLOWLIST: &[&str] = &["that", "had"];

pub(crate) struct RepeatedWordChecker {
    scope: ScopeFilter,
}

impl RepeatedWordChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for RepeatedWordChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "repeated-word",
            description: "Flags accidental consecutive duplicate words",
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

                // Skip table rows
                if line.trim_start().starts_with('|') {
                    continue;
                }

                let mut prev: Option<regex::Match<'_>> = None;
                for m in WORD_TOKEN.find_iter(line) {
                    if let Some(first) = prev {
                        let second = m;

                        if first.as_str().eq_ignore_ascii_case(second.as_str()) {
                            let word = first.as_str();
                            let is_allowed =
                                ALLOWLIST.iter().any(|&w| w.eq_ignore_ascii_case(word));
                            let between = &line[first.end()..second.start()];
                            let whitespace_only = between.chars().all(|c| c.is_whitespace());

                            if !is_allowed
                                && whitespace_only
                                && !inside_inline_code(line, first.start())
                            {
                                let lower = word.to_lowercase();
                                // Fix: remove from end of first word to end of second word
                                // (removes the whitespace + the duplicate word)
                                let fix = Fix {
                                    description: format!("Remove the duplicate \"{lower}\""),
                                    replacements: vec![Replacement {
                                        line: line_num,
                                        start_col: first.end(),
                                        end_col: second.end(),
                                        new_text: String::new(),
                                    }],
                                };
                                emit!(
                                    result,
                                    file.path,
                                    line_num,
                                    Severity::Info,
                                    Category::RepeatedWord,
                                    fix: fix,
                                    suggest: &format!("Remove the duplicate \"{lower}\""),
                                    "repeated word: \"{lower} {lower}\""
                                );
                            }
                        }
                    }
                    prev = Some(m);
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
        RepeatedWordChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_repeated_word_flagged() {
        let result = check(&["The the dog ran"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::RepeatedWord);
    }

    #[test]
    fn test_case_insensitive() {
        let result = check(&["The THE dog ran"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_allowlisted_word_not_flagged() {
        let result = check(&["He said that that was fine"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_had_had_not_flagged() {
        let result = check(&["She had had enough"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_clean_line() {
        let result = check(&["This is a normal sentence"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "echo echo hello", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_table_row_not_flagged() {
        let result = check(&["| value value | other |"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_code_not_flagged() {
        let result = check(&["Use `test test` for checking"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_is_is_flagged() {
        let result = check(&["The question is is it ready"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_fix_data_present() {
        let result = check(&["The the dog ran"]);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        let fix = d.fix.as_ref().expect("fix should be present");
        assert_eq!(fix.replacements.len(), 1);
        let r = &fix.replacements[0];
        assert_eq!(r.line, 1);
        // "The the dog ran" — first "The" is 0..3, second "the" is 4..7
        // Fix removes from end of first (3) to end of second (7) = " the"
        assert_eq!(r.start_col, 3);
        assert_eq!(r.end_col, 7);
        assert_eq!(r.new_text, "");
    }
}
