use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{inside_inline_code, ScopeFilter};
use super::Checker;

/// Matches `[text](#anchor)` links — captures the anchor slug (group 1).
static ANCHOR_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(?:[^\]]*)\]\(#([^)]+)\)").unwrap());

/// Convert a heading title to a GitHub-flavored markdown anchor slug.
///
/// Mirrors GitHub's `github-slugger` algorithm: lowercase, keep only
/// alphanumerics/spaces/hyphens, replace spaces with hyphens.
/// Consecutive hyphens are NOT collapsed — GitHub preserves them.
/// e.g. `Phase 0 — Discovery` → `phase-0--discovery` (em-dash stripped,
/// surrounding spaces each become a hyphen).
fn heading_to_anchor(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for c in title.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() {
                slug.push(lc);
            }
        } else if c == ' ' || c == '-' {
            slug.push('-');
        }
        // All other characters (em-dash, punctuation, etc.) are silently dropped
    }
    // Trim leading/trailing hyphens
    slug.trim_matches('-').to_string()
}

pub(crate) struct BrokenAnchorLinkChecker {
    scope: ScopeFilter,
}

impl BrokenAnchorLinkChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for BrokenAnchorLinkChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "broken-anchor-link",
            description: "Flags in-file anchor links that don't match any heading",
            default_severity: Severity::Error,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let anchors: HashSet<String> = file
                .sections
                .iter()
                .map(|s| heading_to_anchor(&s.title))
                .collect();

            for (i, line) in non_code_lines_masked(&file.raw_lines, &file.in_code_block) {
                let line_num = i + 1;

                for caps in ANCHOR_LINK.captures_iter(line) {
                    let full_match = caps.get(0).unwrap();
                    // Skip if the link is inside inline code
                    if inside_inline_code(line, full_match.start()) {
                        continue;
                    }

                    let anchor = &caps[1];
                    let anchor_lower = anchor.to_lowercase();

                    // Check if this anchor matches any heading
                    if !anchors.contains(&anchor_lower) {
                        emit!(
                            result,
                            file.path,
                            line_num,
                            Severity::Error,
                            Category::BrokenAnchorLink,
                            suggest: "Fix the anchor to match an existing heading, or add the missing heading",
                            "anchor link `#{}` does not match any heading in this file",
                            anchor
                        );
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
    use crate::checkers::utils::test_helpers::single_file_ctx_with_sections;
    use crate::parser::types::Section;

    fn check_with_sections(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        BrokenAnchorLinkChecker::new(&[]).check(&ctx)
    }

    fn sample_sections() -> Vec<Section> {
        vec![
            Section {
                level: 1,
                title: "Getting Started".to_string(),
                line: 1,
                end_line: 5,
            },
            Section {
                level: 2,
                title: "Build Commands".to_string(),
                line: 6,
                end_line: 10,
            },
            Section {
                level: 2,
                title: "Testing".to_string(),
                line: 11,
                end_line: 15,
            },
        ]
    }

    #[test]
    fn test_valid_anchor_not_flagged() {
        let result = check_with_sections(
            &[
                "# Getting Started",
                "",
                "See [build commands](#build-commands) below.",
                "",
                "",
                "## Build Commands",
                "",
                "cargo build",
                "",
                "",
                "## Testing",
                "",
                "cargo test",
                "",
                "",
            ],
            sample_sections(),
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_broken_anchor_flagged() {
        let result = check_with_sections(
            &[
                "# Getting Started",
                "",
                "See [setup](#setup-guide) for details.",
                "",
                "",
                "## Build Commands",
                "",
                "cargo build",
                "",
                "",
                "## Testing",
                "",
                "cargo test",
                "",
                "",
            ],
            sample_sections(),
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::BrokenAnchorLink);
        assert!(result.diagnostics[0].message.contains("setup-guide"));
    }

    #[test]
    fn test_case_insensitive_match() {
        let result = check_with_sections(
            &[
                "# Getting Started",
                "",
                "See [testing](#Testing) section.",
                "",
                "",
                "## Build Commands",
                "",
                "cargo build",
                "",
                "",
                "## Testing",
                "",
                "cargo test",
                "",
                "",
            ],
            sample_sections(),
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check_with_sections(
            &[
                "# Getting Started",
                "",
                "```",
                "[link](#nonexistent)",
                "```",
                "",
                "## Build Commands",
                "",
                "cargo build",
                "",
                "",
                "## Testing",
                "",
                "cargo test",
                "",
            ],
            sample_sections(),
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_code_not_flagged() {
        let result = check_with_sections(
            &[
                "# Getting Started",
                "",
                "Use `[link](#nonexistent)` syntax.",
                "",
                "",
                "## Build Commands",
                "",
                "cargo build",
                "",
                "",
                "## Testing",
                "",
                "cargo test",
                "",
                "",
            ],
            sample_sections(),
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_broken_anchors() {
        let result = check_with_sections(
            &[
                "# Getting Started",
                "",
                "See [foo](#foo) and [bar](#bar).",
                "",
                "",
                "## Build Commands",
                "",
                "cargo build",
                "",
                "",
                "## Testing",
                "",
                "cargo test",
                "",
                "",
            ],
            sample_sections(),
        );
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_heading_to_anchor_basic() {
        assert_eq!(heading_to_anchor("Getting Started"), "getting-started");
        assert_eq!(heading_to_anchor("Build Commands"), "build-commands");
        assert_eq!(heading_to_anchor("Testing"), "testing");
    }

    #[test]
    fn test_heading_to_anchor_special_chars() {
        // GitHub doesn't collapse consecutive hyphens — `&` is stripped,
        // leaving two spaces that each become a hyphen.
        assert_eq!(
            heading_to_anchor("Build & Test Commands"),
            "build--test-commands"
        );
        assert_eq!(heading_to_anchor("What's New?"), "whats-new");
    }

    #[test]
    fn test_heading_to_anchor_em_dash() {
        // Em-dash (U+2014) is stripped; surrounding spaces each become hyphens.
        assert_eq!(
            heading_to_anchor("Phase 0 — Intelligence & Discovery"),
            "phase-0--intelligence--discovery"
        );
        // En-dash (U+2013) same behavior.
        assert_eq!(heading_to_anchor("Pages 1–10"), "pages-110");
    }

    #[test]
    fn test_heading_to_anchor_numbers() {
        assert_eq!(heading_to_anchor("Step 1: Setup"), "step-1-setup");
    }

    #[test]
    fn test_heading_to_anchor_unicode() {
        assert_eq!(heading_to_anchor("Café Setup"), "café-setup");
    }

    #[test]
    fn test_no_sections_no_crash() {
        let result = check_with_sections(&["See [link](#somewhere)."], vec![]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_external_link_not_flagged() {
        // [text](https://example.com) should not be checked
        let result = check_with_sections(
            &["# Intro", "", "See [docs](https://example.com).", "", ""],
            vec![Section {
                level: 1,
                title: "Intro".to_string(),
                line: 1,
                end_line: 5,
            }],
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_file_link_with_anchor_not_flagged() {
        // [text](other.md#section) links to another file — not our problem
        let result = check_with_sections(
            &["# Intro", "", "See [guide](guide.md#setup).", "", ""],
            vec![Section {
                level: 1,
                title: "Intro".to_string(),
                line: 1,
                end_line: 5,
            }],
        );
        assert_eq!(result.diagnostics.len(), 0);
    }
}
