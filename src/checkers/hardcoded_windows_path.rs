use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{inside_inline_code, ScopeFilter};
use super::Checker;

/// Matches relative Windows-style paths: word\word patterns that look like
/// directory separators, not regex escapes or other backslash uses.
/// Requires at least one segment with a file-like name (letters/digits/dots).
static WINDOWS_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\b([a-zA-Z0-9_.]+(?:\\[a-zA-Z0-9_.]+){1,})\b").unwrap());

/// Known regex/escape patterns to exclude (not paths).
static REGEX_ESCAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\[nrtbdswDSW0*+?{}()\[\]|^$.\\]").unwrap());

pub(crate) struct HardcodedWindowsPathChecker {
    scope: ScopeFilter,
}

impl HardcodedWindowsPathChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for HardcodedWindowsPathChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "hardcoded-windows-path",
            description: "Flags backslash file paths that break on non-Windows",
            default_severity: Severity::Warning,
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

                // Skip table rows (often contain escaped pipes)
                if line.trim_start().starts_with('|') {
                    continue;
                }

                for m in WINDOWS_PATH.find_iter(line) {
                    let matched = m.as_str();
                    let match_start = m.start();

                    // Skip absolute Windows paths like "C:\Projects\file.py"
                    // (handled by absolute-path checker). The regex matches
                    // "Projects\file.py"; detect the preceding drive letter.
                    if match_start >= 3
                        && line.as_bytes()[match_start - 1] == b'\\'
                        && line.as_bytes()[match_start - 2] == b':'
                        && line.as_bytes()[match_start - 3].is_ascii_alphabetic()
                    {
                        continue;
                    }

                    // Skip single-backslash regex escapes (e.g., \d, \w)
                    if matched.matches('\\').count() == 1 && REGEX_ESCAPE.is_match(matched) {
                        continue;
                    }

                    if inside_inline_code(line, match_start) {
                        continue;
                    }

                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Warning,
                        Category::HardcodedWindowsPath,
                        suggest: "Use forward slashes (`/`) for cross-platform compatibility",
                        "Windows-style backslash path: `{}`",
                        matched
                    );
                    break; // one per line
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
        HardcodedWindowsPathChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_relative_backslash_path_flagged() {
        let result = check(&["Edit scripts\\helper.py to add the new function"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].category,
            Category::HardcodedWindowsPath
        );
    }

    #[test]
    fn test_nested_backslash_path_flagged() {
        let result = check(&["The file is at src\\components\\Button.tsx"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_reference_dir_flagged() {
        let result = check(&["See references\\guide.md for details"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_forward_slash_not_flagged() {
        let result = check(&["Edit scripts/helper.py to add the new function"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_absolute_windows_not_flagged() {
        // Absolute paths are handled by absolute-path checker
        let result = check(&[r"Open C:\Projects\myapp"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_regex_escape_not_flagged() {
        let result = check(&[r"Pattern: \d+\.\d+"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", r"scripts\helper.py", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_code_not_flagged() {
        let result = check(&[r"Run `scripts\helper.py` to test"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_normal_text_not_flagged() {
        let result = check(&["Always use forward slashes in file paths."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_single_backslash_not_flagged() {
        // A single backslash isn't a path
        let result = check(&[r"Use \ for line continuation"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_cursor_rules_path_flagged() {
        let result = check(&[r"Check .cursor\rules\formatting.mdc"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
