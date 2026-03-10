use regex::Regex;
use std::sync::LazyLock;

use crate::config::EmphasisOveruseConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

pub(crate) struct EmphasisOveruseChecker {
    max_emphasis: usize,
    scope: ScopeFilter,
}

impl EmphasisOveruseChecker {
    pub(crate) fn new(config: &EmphasisOveruseConfig) -> Self {
        Self {
            max_emphasis: config.max_emphasis,
            scope: ScopeFilter::new(&config.scope),
        }
    }
}

/// Bold emphasis markers: **IMPORTANT**, **CRITICAL**, **WARNING**, **CAUTION**, **NOTE**
static BOLD_EMPHASIS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\*\*(?:IMPORTANT|CRITICAL|WARNING|CAUTION|NOTE)\*\*").unwrap()
});

/// Standalone all-caps emphasis markers (not inside bold, not in headings).
/// Must be preceded by start-of-line/whitespace and followed by end-of-line/whitespace/colon/punctuation.
static CAPS_EMPHASIS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s>*_-])(?:IMPORTANT|CRITICAL|WARNING|CAUTION)(?:[:\s.,!]|$)").unwrap()
});

impl Checker for EmphasisOveruseChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "emphasis-overuse",
            description: "Flags files with excessive emphasis markers",
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

            let mut count = 0usize;

            for (_, line) in file.non_code_lines() {
                if is_heading(line) {
                    continue;
                }

                count += BOLD_EMPHASIS.find_iter(line).count();

                // Count standalone all-caps, but skip if already counted as bold
                if !line.contains("**") {
                    count += CAPS_EMPHASIS.find_iter(line).count();
                }
            }

            if count > self.max_emphasis {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::EmphasisOveruse,
                    suggest: "Reduce emphasis markers to highlight only the most critical instructions",
                    "File contains {} emphasis markers (threshold: {}). \
                     Excessive emphasis creates alert fatigue — agents can't prioritize \
                     when everything screams for attention.",
                    count,
                    self.max_emphasis
                );
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check_with_threshold(lines: &[&str], max_emphasis: usize) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        let config = EmphasisOveruseConfig {
            enabled: true,
            max_emphasis,
            scope: Vec::new(),
            severity: None,
        };
        EmphasisOveruseChecker::new(&config).check(&ctx)
    }

    fn run_check(lines: &[&str]) -> CheckResult {
        run_check_with_threshold(lines, 3)
    }

    #[test]
    fn test_bold_emphasis_detected() {
        let result = run_check(&[
            "**IMPORTANT**: Do this",
            "**CRITICAL**: And this",
            "**WARNING**: Also this",
            "**CAUTION**: Plus this",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains('4'));
    }

    #[test]
    fn test_caps_emphasis_detected() {
        let result = run_check(&[
            "IMPORTANT: Do this",
            "CRITICAL: And this",
            "WARNING: Also this",
            "CAUTION: Plus this",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_below_threshold_no_flag() {
        let result = run_check(&["**IMPORTANT**: Do this", "**WARNING**: And this"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_excluded() {
        let result = run_check(&[
            "```",
            "**IMPORTANT**: In code",
            "**CRITICAL**: In code",
            "**WARNING**: In code",
            "**CAUTION**: In code",
            "```",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_heading_excluded() {
        let result = run_check(&["## IMPORTANT", "## CRITICAL", "## WARNING", "## CAUTION"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_case_insensitive_bold() {
        let result = run_check(&[
            "**Important**: a",
            "**important**: b",
            "**IMPORTANT**: c",
            "**critical**: d",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_note_bold_counted() {
        let result = run_check(&["**NOTE**: a", "**NOTE**: b", "**NOTE**: c", "**NOTE**: d"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_default_threshold_10() {
        // 10 markers should not flag (threshold is >10)
        let mut lines: Vec<&str> = Vec::new();
        lines.resize(10, "**IMPORTANT**: something");
        let result = run_check_with_threshold(&lines, 10);
        assert!(result.diagnostics.is_empty());

        // 11 markers should flag
        lines.push("**CRITICAL**: more");
        let result = run_check_with_threshold(&lines, 10);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_mixed_bold_and_caps() {
        let result = run_check(&[
            "**IMPORTANT**: a",
            "CRITICAL: b",
            "**WARNING**: c",
            "CAUTION: d",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_caps_in_bold_line_not_double_counted() {
        // A line with ** should only count the bold matches, not also the caps
        let result = run_check_with_threshold(
            &[
                "**IMPORTANT**: this is **CRITICAL**",
                "**WARNING**: also",
                "**CAUTION**: also",
            ],
            3,
        );
        assert_eq!(result.diagnostics.len(), 1);
        // Should count 4 bold markers (2+1+1), not 4 bold + 2 caps
        assert!(result.diagnostics[0].message.contains('4'));
    }
}
