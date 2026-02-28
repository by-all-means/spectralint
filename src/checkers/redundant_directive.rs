use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::config::RedundantDirectiveConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{normalize_directive, ScopeFilter};
use super::Checker;

pub struct RedundantDirectiveChecker {
    scope: ScopeFilter,
    similarity_threshold: f64,
    min_line_length: usize,
}

impl RedundantDirectiveChecker {
    pub fn new(config: &RedundantDirectiveConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            similarity_threshold: config.similarity_threshold,
            min_line_length: config.min_line_length,
        }
    }
}

/// Lines that look like structural/reference content rather than directives:
/// table rows, file paths, link-only lines, API endpoint listings, indented sub-items,
/// bold-only structural labels, standalone XML/HTML tags, URL-containing items,
/// backtick-term definitions, numbered list items.
static NON_DIRECTIVE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^\s*\|                                          # table row
        | ^\s*(?:[-*+]|\d+\.)\s+\S+\.(?:md|rs|ts|js|py|go|yaml|toml|json|el|rb|ex)\s*$  # bare file path
        | ^\s*(?:[-*+]|\d+\.)\s+`[^`]+\.(?:md|rs|ts|js|py|go|yaml|toml|json|el|rb|ex)`(?:\s*$|:\s)  # backtick-wrapped file path (with optional description)
        | ^\s*(?:[-*+]|\d+\.)\s+\*\*(?:GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS)\s  # API endpoint
        | ^\s*[-*+]\s+\[.*\]\(.*\)\s*$                 # link-only list item
        | ^\s{2,}[-*+]\s+                               # indented sub-item (2+ spaces before marker)
        | ^\s*(?:(?:[-*+]|\d+\.)\s+)?\*\*[^*]+\*\*:?\s*$  # bold-only structural label (with or without list marker)
        | ^\s*(?:(?:[-*+]|\d+\.)\s+)?\*\*[^*]+\*\*:\s   # bold key-value metadata (e.g. **File**: value)
        | ^\s*</?[\w-]+(?:\s+[^>]*)?\s*/?>\s*$          # standalone XML/HTML tag
        | ^\s*(?:(?:[-*+]|\d+\.)\s+)?(?:\w[\w\s]{0,20}:\s+)?https?://\S  # URL-containing item (with or without list marker)
        | ^\s*(?:[-*+]|\d+\.)\s+`[^`]+`:\s             # backtick-term definition
        | ^\s*\d+\.\s+                                  # numbered list item (procedural steps)
    ",
    )
    .unwrap()
});

const MAX_DIRECTIVE_LINES: usize = 200;

impl Checker for RedundantDirectiveChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let directives: Vec<(usize, String)> = file
                .non_code_lines()
                .filter(|(_, line)| {
                    let trimmed = line.trim();
                    is_directive_line(line)
                        && !trimmed.is_empty()
                        && !trimmed.starts_with('#')
                        && !NON_DIRECTIVE_LINE.is_match(line)
                })
                .map(|(i, line)| (i + 1, normalize_directive(line)))
                .filter(|(_, norm)| norm.len() >= self.min_line_length)
                .take(MAX_DIRECTIVE_LINES)
                .collect();

            // Track which target lines have already been flagged to avoid
            // emitting multiple diagnostics for the same line.
            let mut flagged: HashSet<usize> = HashSet::new();

            for i in 0..directives.len() {
                for j in (i + 1)..directives.len() {
                    if !flagged.insert(directives[j].0) {
                        // Already flagged this target line — skip.
                        continue;
                    }
                    let sim = strsim::jaro_winkler(&directives[i].1, &directives[j].1);
                    if sim >= self.similarity_threshold {
                        emit!(
                            result,
                            file.path,
                            directives[j].0,
                            Severity::Info,
                            Category::RedundantDirective,
                            suggest: "Remove or merge the duplicate directive",
                            "Line {} is {:.0}% similar to line {} — likely redundant",
                            directives[j].0,
                            sim * 100.0,
                            directives[i].0
                        );
                    } else {
                        // Not similar — remove from flagged so it can match a later pair.
                        flagged.remove(&directives[j].0);
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

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        let config = RedundantDirectiveConfig {
            enabled: true,
            similarity_threshold: 0.95,
            min_line_length: 15,
            scope: Vec::new(),
        };
        RedundantDirectiveChecker::new(&config).check(&ctx)
    }

    #[test]
    fn test_duplicate_lines_detected() {
        let result = run_check(&[
            "- Always run the full test suite before committing code.",
            "- Always run the full test suite before committing changes.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::RedundantDirective);
    }

    #[test]
    fn test_exact_duplicate_detected() {
        let result = run_check(&[
            "- Never skip the continuous integration pipeline.",
            "- Never skip the continuous integration pipeline.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_different_lines_no_flag() {
        let result = run_check(&[
            "Always run tests before committing.",
            "Never modify production databases directly.",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_short_lines_skipped() {
        let result = run_check(&["- Run tests.", "- Run tests."]);
        assert!(
            result.diagnostics.is_empty(),
            "Lines shorter than min_line_length should be skipped"
        );
    }

    #[test]
    fn test_headings_skipped() {
        let result = run_check(&[
            "# Always run the full test suite before committing code.",
            "## Always run the full test suite before committing code.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Heading lines should be skipped"
        );
    }

    #[test]
    fn test_code_block_lines_skipped() {
        let result = run_check(&[
            "```",
            "- Always run the full test suite before committing code.",
            "- Always run the full test suite before committing changes.",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Lines in code blocks should be skipped"
        );
    }

    #[test]
    fn test_table_rows_skipped() {
        let result = run_check(&[
            "| PUT /{list_id}/transfer-ownership | Transfer list ownership |",
            "| PUT /{group_id}/transfer-ownership | Transfer group ownership |",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Table rows should be skipped"
        );
    }

    #[test]
    fn test_bare_file_paths_skipped() {
        let result = run_check(&["- docs/ai/AI_CONTEXT.md", "- docs/ai/AI_BACKEND.md"]);
        assert!(
            result.diagnostics.is_empty(),
            "Bare file path list items should be skipped"
        );
    }

    #[test]
    fn test_api_endpoint_lines_skipped() {
        let result = run_check(&[
            "- **POST /{list_id}/permissions** - Grant list permissions",
            "- **POST /{group_id}/members** - Add group member",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "API endpoint list items should be skipped"
        );
    }

    #[test]
    fn test_backtick_file_paths_skipped() {
        let result = run_check(&[
            "- `tests/unit/utils/code-tools/codex-uninstaller.test.ts`",
            "- `tests/unit/utils/code-tools/codex-uninstall-enhanced.test.ts`",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Backtick-wrapped file path list items should be skipped"
        );
    }

    #[test]
    fn test_indented_sub_items_skipped() {
        let result = run_check(&[
            "  - Connection test across all databases",
            "  - Connection test for the primary database",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Indented sub-items should be skipped"
        );
    }

    #[test]
    fn test_link_only_lines_skipped() {
        let result = run_check(&[
            "- [Eliminating Waterfalls](#1-eliminating-waterfalls)",
            "- [Bundle Size Optimization](#2-bundle-size-optimization)",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Link-only list items should be skipped"
        );
    }

    #[test]
    fn test_list_marker_normalization() {
        let result = run_check(&[
            "- Always run the full test suite before committing code.",
            "* Always run the full test suite before committing code.",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Different list markers should still match"
        );
    }

    #[test]
    fn test_bold_only_labels_skipped() {
        let result = run_check(&["- **Capabilities:**", "- **Dependencies:**"]);
        assert!(
            result.diagnostics.is_empty(),
            "Bold-only structural labels should be skipped"
        );
    }

    #[test]
    fn test_xml_tags_skipped() {
        let result = run_check(&["<claude-mem-context>", "</claude-mem-context>"]);
        assert!(
            result.diagnostics.is_empty(),
            "Standalone XML/HTML tags should be skipped"
        );
    }

    #[test]
    fn test_bold_metadata_without_list_marker_skipped() {
        let result = run_check(&["**Impact: MEDIUM**", "**Impact: HIGH**"]);
        assert!(
            result.diagnostics.is_empty(),
            "Bold metadata lines without list markers should be skipped"
        );
    }

    #[test]
    fn test_url_list_items_skipped() {
        let result = run_check(&[
            "- Reference: https://docs.example.com/indexes-types",
            "- Reference: https://docs.example.com/indexes-multicolumn",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "URL-containing list items should be skipped"
        );
    }

    #[test]
    fn test_bare_url_list_items_skipped() {
        let result = run_check(&[
            "- https://github.com/org/repo/blob/main/src/foo.rs",
            "- https://github.com/org/repo/blob/main/src/bar.rs",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Bare URL list items should be skipped"
        );
    }

    #[test]
    fn test_backtick_term_definition_skipped() {
        let result = run_check(&[
            "- `label50x30`: 440x240 dots (fixed label)",
            "- `label50x40`: 440x320 dots (fixed label)",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Backtick-term definitions should be skipped"
        );
    }

    #[test]
    fn test_backtick_file_with_description_skipped() {
        let result = run_check(&[
            "- `tests/unit/utils/code-tools/codex-uninstaller.test.ts`: Uninstall tests",
            "- `tests/unit/utils/code-tools/codex-uninstall-enhanced.test.ts`: Enhanced tests",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Backtick file paths with descriptions should be skipped"
        );
    }

    #[test]
    fn test_numbered_list_items_skipped() {
        let result = run_check(&[
            "5. Run the parity script to verify all changes.",
            "5. Run the parity script to validate all changes.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Numbered list items should be skipped"
        );
    }

    #[test]
    fn test_bullet_directive_still_detected() {
        let result = run_check(&[
            "- Always run the full test suite before committing code.",
            "- Always run the full test suite before committing changes.",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Genuine duplicate bullet directives should still be detected"
        );
    }

    #[test]
    fn test_bold_key_value_metadata_skipped() {
        let result = run_check(&[
            "- **File**: tests/commands/ccr.test.ts",
            "- **File**: tests/commands/ccu.test.ts",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Bold key-value metadata lines should be skipped"
        );
    }

    #[test]
    fn test_bare_reference_url_skipped() {
        let result = run_check(&[
            "Reference: https://docs.example.com/indexes-types",
            "Reference: https://docs.example.com/indexes-multicolumn",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Bare reference URL lines without list marker should be skipped"
        );
    }

    #[test]
    fn test_dedup_same_line_only_flagged_once() {
        // Line C is similar to both A and B — should only produce 1 diagnostic for C.
        let result = run_check(&[
            "- Always run the full integration test suite before committing code to main.",
            "- Always run the full integration test suite before pushing code to main.",
            "- Always run the full integration test suite before merging code to main.",
        ]);
        // Line 2 matches line 1. Line 3 matches line 1 and 2 — but dedup means
        // line 3 only emits once (first match wins).
        let line3_diags: Vec<_> = result.diagnostics.iter().filter(|d| d.line == 3).collect();
        assert_eq!(
            line3_diags.len(),
            1,
            "Each target line should only be flagged once, got {} for line 3",
            line3_diags.len()
        );
    }
}
