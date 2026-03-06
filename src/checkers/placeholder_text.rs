use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{inside_inline_code, ScopeFilter};
use super::Checker;

pub(crate) struct PlaceholderTextChecker {
    scope: ScopeFilter,
}

impl PlaceholderTextChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static PLACEHOLDER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?:",
        r"\bTODO\b(?:\s*:)?", // case-sensitive: only ALL CAPS
        r"|(?i:\bTBD\b)(?:\s*:)?",
        r"|(?i:\bFIXME\b)(?:\s*:)?",
        r"|(?i:\[insert .+?\])",
        r"|(?i:\betc\.?)(?:\s|$)",
        r"|(?i:\band so on\b)",
        r"|\.{3,}\s*$",
        r")",
    ))
    .unwrap()
});

/// Matches a proper enumeration before "etc." — 2+ comma-separated items.
/// Uses `[^,]+` instead of `\w+` to handle multi-word items ("code style")
/// and special characters (`@gala-chain/api`, `Optional[...]`).
static ENUMERATION_BEFORE_ETC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:[^,]+,\s*){2,}.*\betc\.?").unwrap());

/// Matches "or"-separated enumerations: "X or Y or Z etc"
static OR_ENUMERATION_BEFORE_ETC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:\w+\s+or\s+){2,}\w+\s+etc\.?").unwrap());

/// Returns true if the match falls inside a file-path-like token (e.g. `tasks/todo.md`).
fn is_inside_file_path(line: &str, match_start: usize, match_end: usize) -> bool {
    let token_start = line[..match_start]
        .rfind(char::is_whitespace)
        .map_or(0, |i| i + 1);
    let token_end = line[match_end..]
        .find(char::is_whitespace)
        .map_or(line.len(), |i| match_end + i);
    let token =
        line[token_start..token_end].trim_matches(|c: char| c == '`' || c == '"' || c == '\'');
    token.contains('/') || token.contains('\\')
}

/// Returns true if the match is immediately followed by a file extension (e.g. `TODO.md`).
fn is_file_reference(line: &str, match_end: usize) -> bool {
    static FILE_EXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\.[a-zA-Z]{1,5}\b").unwrap());
    FILE_EXT.is_match(&line[match_end..])
}

/// Returns true if TODO/TBD/FIXME is used as a noun modifier (e.g. "TODO items", "TODO list").
fn is_noun_usage(line: &str, match_end: usize) -> bool {
    static NOUN_AFTER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s+(items?|list|entries|cards?|tracker|progress|count|app)\b").unwrap()
    });
    NOUN_AFTER.is_match(&line[match_end..])
}

/// Returns true if this match is an "etc." that follows a proper enumeration.
/// `prev_line` is used to detect enumerations that wrap across lines.
fn is_etc_after_enumeration(line: &str, matched: &str, prev_line: Option<&str>) -> bool {
    let trimmed = matched.trim();
    if !trimmed
        .get(..3)
        .is_some_and(|s| s.eq_ignore_ascii_case("etc"))
    {
        return false;
    }
    if ENUMERATION_BEFORE_ETC.is_match(line) || OR_ENUMERATION_BEFORE_ETC.is_match(line) {
        return true;
    }
    // Handle wrapped lines: "analysis or review or\n      debugging etc"
    if let Some(prev) = prev_line {
        let combined = format!("{} {}", prev.trim(), line.trim());
        if ENUMERATION_BEFORE_ETC.is_match(&combined)
            || OR_ENUMERATION_BEFORE_ETC.is_match(&combined)
        {
            return true;
        }
    }
    false
}

impl Checker for PlaceholderTextChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in file.non_code_lines() {
                // Skip headings — "Todo" / "TBD" in a heading is a section title, not a placeholder.
                if line.starts_with('#') {
                    continue;
                }
                let prev_line = i
                    .checked_sub(1)
                    .and_then(|idx| file.raw_lines.get(idx))
                    .map(|s| s.as_str());
                for m in PLACEHOLDER_PATTERN.find_iter(line) {
                    if is_etc_after_enumeration(line, m.as_str(), prev_line)
                        || is_inside_file_path(line, m.start(), m.end())
                        || inside_inline_code(line, m.start())
                        || is_file_reference(line, m.end())
                        || is_noun_usage(line, m.end())
                    {
                        continue;
                    }
                    emit!(
                        result,
                        file.path,
                        i + 1,
                        Severity::Warning,
                        Category::PlaceholderText,
                        suggest: "Replace placeholder with actual content",
                        "Placeholder text found: \"{}\"",
                        m.as_str().trim()
                    );
                    break; // One diagnostic per line
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
        PlaceholderTextChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_todo_detected() {
        let result = run_check(&["# Title", "TODO implement this"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_todo_with_colon_detected() {
        let result = run_check(&["TODO: add error handling"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_todo_inside_word_not_detected() {
        let result = run_check(&["TODOLIST of items to complete"]);
        assert!(
            result.diagnostics.is_empty(),
            "TODO inside a word (TODOLIST) should not match"
        );
    }

    #[test]
    fn test_tbd_detected() {
        let result = run_check(&["TBD needs review"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_fixme_detected() {
        let result = run_check(&["FIXME broken"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_insert_detected() {
        let result = run_check(&["[insert your name here]"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_etc_after_short_list_detected() {
        // Only 1 item before etc. — not a proper enumeration
        let result = run_check(&["tools, etc."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_etc_after_proper_enumeration_skipped() {
        // 2+ items before etc. — proper enumeration, should not flag
        let result = run_check(&["Use tools like grep, find, sed, etc."]);
        assert!(
            result.diagnostics.is_empty(),
            "etc. after a proper enumeration (2+ items) should not flag"
        );
    }

    #[test]
    fn test_etc_after_multi_word_items_skipped() {
        // Multi-word items: "code style, testing style, etc"
        let result = run_check(&["Use existing code style, testing style, etc"]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after multi-word enumeration should not flag"
        );
    }

    #[test]
    fn test_etc_after_special_char_items_skipped() {
        // Package names with special chars
        let result = run_check(&["Import from `@gala-chain/api`, `@gala-chain/client`, etc."]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after enumeration with special chars should not flag"
        );
    }

    #[test]
    fn test_etc_after_bracket_items_skipped() {
        // Items with brackets: Optional[...], Union[...], etc.
        let result = run_check(&[
            "Use `|` instead of `Union[...]`, `type | None` instead of `Optional[...]`, etc.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after enumeration with brackets should not flag"
        );
    }

    #[test]
    fn test_etc_after_or_enumeration_skipped() {
        let result = run_check(&["perform some analysis or review or debugging etc"]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after or-separated enumeration (2+ items) should not flag"
        );
    }

    #[test]
    fn test_etc_after_wrapped_or_enumeration_skipped() {
        let result = run_check(&[
            "    - Launch another instance to perform some analysis or review or",
            "      debugging etc",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "etc after or-enumeration wrapped across lines should not flag"
        );
    }

    #[test]
    fn test_etc_without_enumeration_detected() {
        let result = run_check(&["configure everything etc."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_and_so_on_detected() {
        let result = run_check(&["Use grep, find, and so on"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_ellipsis_detected() {
        let result = run_check(&["Do something..."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_todo_in_file_path_skipped() {
        let result = run_check(&["Write plan to `tasks/todo.md` with checkable items"]);
        assert!(
            result.diagnostics.is_empty(),
            "TODO inside a file path should not flag"
        );
    }

    #[test]
    fn test_fixme_in_file_path_skipped() {
        let result = run_check(&["See docs/fixme.txt for details"]);
        assert!(
            result.diagnostics.is_empty(),
            "FIXME inside a file path should not flag"
        );
    }

    #[test]
    fn test_tbd_in_file_path_skipped() {
        let result = run_check(&["Update `notes/tbd.md` when decided"]);
        assert!(
            result.diagnostics.is_empty(),
            "TBD inside a file path should not flag"
        );
    }

    #[test]
    fn test_todo_standalone_still_detected() {
        // "TODO" not inside a path — should still flag
        let result = run_check(&["TODO: update tasks/todo.md"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_lowercase_todo_not_flagged() {
        let result = run_check(&["a simple todo app might use 5-10 agents"]);
        assert!(
            result.diagnostics.is_empty(),
            "Lowercase 'todo' used as a noun should not flag"
        );
    }

    #[test]
    fn test_mixed_case_todo_not_flagged() {
        let result = run_check(&["Todo errors cause graceful bailouts in production"]);
        assert!(
            result.diagnostics.is_empty(),
            "Mixed-case 'Todo' used as a noun should not flag"
        );
    }

    #[test]
    fn test_todo_as_feature_name_not_flagged() {
        let result = run_check(&["Shows context health, tool activity, and todo progress"]);
        assert!(
            result.diagnostics.is_empty(),
            "Lowercase 'todo' as a feature name should not flag"
        );
    }

    #[test]
    fn test_todo_file_reference_not_flagged() {
        let result = run_check(&["TODO.md (future goals)"]);
        assert!(
            result.diagnostics.is_empty(),
            "TODO.md file reference should not flag"
        );
    }

    #[test]
    fn test_todo_in_inline_code_not_flagged() {
        let result = run_check(&["Use `error.TODO-*.js` for test fixtures"]);
        assert!(
            result.diagnostics.is_empty(),
            "TODO inside inline code should not flag"
        );
    }

    #[test]
    fn test_spanish_todo_not_flagged() {
        let result = run_check(&["Como todo plugin de WordPress, el flujo comienza con la carga"]);
        assert!(
            result.diagnostics.is_empty(),
            "Spanish 'todo' (meaning 'all') should not flag"
        );
    }

    #[test]
    fn test_allcaps_todo_still_flagged() {
        let result = run_check(&["TODO implement this feature"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_allcaps_todo_with_colon_still_flagged() {
        let result = run_check(&["TODO: add error handling here"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_real_todo_after_inline_code_still_flagged() {
        let result = run_check(&["See `some.code()` — TODO: fix this"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_clean_file_no_diagnostics() {
        let result = run_check(&["# Title", "Run the tests."]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_in_code_block_skipped() {
        let result = run_check(&["```", "TODO implement", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_placeholders() {
        let result = run_check(&["TODO first", "TBD second"]);
        assert_eq!(result.diagnostics.len(), 2);
    }
}
