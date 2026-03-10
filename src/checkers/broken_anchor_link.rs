use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

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
/// Leading/trailing hyphens are NOT trimmed — GitHub preserves them
/// (e.g. heading `🚀 Contributing` → anchor `-contributing`).
fn heading_to_anchor(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for c in title.chars() {
        if c.is_alphanumeric() || c == '_' {
            for lc in c.to_lowercase() {
                slug.push(lc);
            }
        } else if c == ' ' || c == '-' {
            slug.push('-');
        }
        // All other characters (em-dash, punctuation, emoji, etc.) are silently dropped
    }
    slug
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

            // Build anchor set with GitHub-style deduplication (-1, -2, etc.)
            let mut anchor_counts: HashMap<String, usize> =
                HashMap::with_capacity(file.sections.len());
            let mut anchors: HashSet<String> = HashSet::with_capacity(file.sections.len());
            for section in &file.sections {
                let base = heading_to_anchor(&section.title);
                let count = anchor_counts.entry(base.clone()).or_insert(0);
                if *count == 0 {
                    anchors.insert(base);
                } else {
                    anchors.insert(format!("{base}-{count}"));
                }
                *count += 1;
            }

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
    fn test_heading_to_anchor_emoji_prefix() {
        // GitHub preserves leading hyphens from stripped emoji
        assert_eq!(heading_to_anchor("\u{1f680} Contributing"), "-contributing");
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
    fn test_heading_to_anchor_underscore() {
        // GitHub preserves underscores in anchors
        assert_eq!(heading_to_anchor("my_function"), "my_function");
        assert_eq!(heading_to_anchor("api_key_name"), "api_key_name");
    }

    #[test]
    fn test_duplicate_heading_anchors() {
        // GitHub appends -1, -2 for duplicate headings
        let sections = vec![
            Section {
                level: 2,
                title: "API".to_string(),
                line: 1,
                end_line: 5,
            },
            Section {
                level: 2,
                title: "API".to_string(),
                line: 6,
                end_line: 10,
            },
            Section {
                level: 2,
                title: "API".to_string(),
                line: 11,
                end_line: 15,
            },
        ];
        // Link to #api-1 (second occurrence) should be valid
        let result = check_with_sections(
            &[
                "## API",
                "",
                "first",
                "",
                "",
                "## API",
                "",
                "second",
                "",
                "",
                "## API",
                "",
                "third",
                "",
                "",
                "See [second](#api-1) and [third](#api-2).",
            ],
            sections,
        );
        assert_eq!(
            result.diagnostics.len(),
            0,
            "Links to duplicate heading anchors (#api-1, #api-2) should be valid"
        );
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

    #[test]
    fn test_heading_to_anchor_period_dropped() {
        assert_eq!(heading_to_anchor("v2.0 Release"), "v20-release");
    }

    #[test]
    fn test_heading_to_anchor_backticks_dropped() {
        assert_eq!(heading_to_anchor("Using `cargo test`"), "using-cargo-test");
    }

    #[test]
    fn test_heading_to_anchor_tilde_dropped() {
        assert_eq!(heading_to_anchor("~/.config Setup"), "config-setup");
    }

    #[test]
    fn test_duplicate_heading_zero_suffix_invalid() {
        // GitHub never produces #api-0 — first occurrence is just #api
        let sections = vec![
            Section {
                level: 2,
                title: "API".to_string(),
                line: 1,
                end_line: 5,
            },
            Section {
                level: 2,
                title: "API".to_string(),
                line: 6,
                end_line: 10,
            },
        ];
        let result = check_with_sections(
            &[
                "## API",
                "",
                "first",
                "",
                "",
                "## API",
                "",
                "second",
                "",
                "",
                "See [wrong](#api-0).",
            ],
            sections,
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "#api-0 should be invalid — first occurrence is just #api"
        );
    }

    #[test]
    fn test_heading_with_special_characters_anchor() {
        // Heading "What's New?" → anchor "whats-new"
        let sections = vec![Section {
            level: 2,
            title: "What's New?".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &["## What's New?", "", "See [updates](#whats-new).", "", ""],
            sections,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_with_special_chars_broken_link() {
        // Heading "What's New?" → anchor "whats-new", NOT "whats-new?"
        let sections = vec![Section {
            level: 2,
            title: "What's New?".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &["## What's New?", "", "See [updates](#whats-new?).", "", ""],
            sections,
        );
        // The regex won't capture the `?` as part of the anchor inside parens,
        // so the anchor captured is "whats-new?" — but actually the regex `#([^)]+)`
        // captures everything until `)`, so the `?` IS included.
        // "whats-new?" is not in anchors, so it flags.
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_case_sensitivity_anchor_uppercase_heading() {
        // Heading "BUILD Commands" → anchor "build-commands"
        // Link to "#BUILD-Commands" → lowercased to "build-commands" → match
        let sections = vec![Section {
            level: 2,
            title: "BUILD Commands".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &[
                "## BUILD Commands",
                "",
                "See [link](#BUILD-Commands).",
                "",
                "",
            ],
            sections,
        );
        assert_eq!(
            result.diagnostics.len(),
            0,
            "anchor matching should be case-insensitive"
        );
    }

    #[test]
    fn test_anchor_with_unicode_characters() {
        // Heading "Café Setup" → anchor "café-setup" (unicode alphanumerics preserved)
        let sections = vec![Section {
            level: 2,
            title: "Caf\u{00e9} Setup".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &[
                "## Caf\u{00e9} Setup",
                "",
                "See [setup](#caf\u{00e9}-setup).",
                "",
                "",
            ],
            sections,
        );
        assert_eq!(
            result.diagnostics.len(),
            0,
            "unicode characters in anchors should be preserved and matched"
        );
    }

    #[test]
    fn test_deeply_nested_heading() {
        // h5 heading should still produce a valid anchor
        let sections = vec![Section {
            level: 5,
            title: "Deep Nested Section".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &[
                "##### Deep Nested Section",
                "",
                "See [deep](#deep-nested-section).",
                "",
                "",
            ],
            sections,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_anchor_to_third_duplicate_heading() {
        // Three headings with same name: #api, #api-1, #api-2
        let sections = vec![
            Section {
                level: 2,
                title: "API".to_string(),
                line: 1,
                end_line: 5,
            },
            Section {
                level: 2,
                title: "API".to_string(),
                line: 6,
                end_line: 10,
            },
            Section {
                level: 2,
                title: "API".to_string(),
                line: 11,
                end_line: 15,
            },
        ];
        // Link to the first occurrence (#api) should be valid
        let result = check_with_sections(
            &[
                "## API",
                "",
                "first",
                "",
                "",
                "## API",
                "",
                "second",
                "",
                "",
                "## API",
                "",
                "third",
                "",
                "",
                "See [first api](#api).",
            ],
            sections,
        );
        assert_eq!(
            result.diagnostics.len(),
            0,
            "Link to first occurrence (#api) should be valid"
        );
    }

    #[test]
    fn test_heading_with_colons_and_numbers() {
        // "Step 1: Setup" → "step-1-setup"
        let sections = vec![Section {
            level: 2,
            title: "Step 1: Setup".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &["## Step 1: Setup", "", "See [step](#step-1-setup).", "", ""],
            sections,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_with_backticks() {
        // "Using `cargo test`" → "using-cargo-test"
        let sections = vec![Section {
            level: 2,
            title: "Using `cargo test`".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &[
                "## Using `cargo test`",
                "",
                "See [tests](#using-cargo-test).",
                "",
                "",
            ],
            sections,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_with_em_dash() {
        // "Phase 0 — Discovery" → "phase-0--discovery"
        let sections = vec![Section {
            level: 2,
            title: "Phase 0 \u{2014} Discovery".to_string(),
            line: 1,
            end_line: 5,
        }];
        let result = check_with_sections(
            &[
                "## Phase 0 \u{2014} Discovery",
                "",
                "See [phase](#phase-0--discovery).",
                "",
                "",
            ],
            sections,
        );
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_anchors_same_line_mixed_validity() {
        // One valid and one broken anchor on the same line
        let sections = vec![
            Section {
                level: 2,
                title: "Build".to_string(),
                line: 1,
                end_line: 5,
            },
            Section {
                level: 2,
                title: "Test".to_string(),
                line: 6,
                end_line: 10,
            },
        ];
        let result = check_with_sections(
            &[
                "## Build",
                "",
                "content",
                "",
                "",
                "## Test",
                "",
                "content",
                "",
                "",
                "See [build](#build) and [deploy](#deploy).",
            ],
            sections,
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "only the broken #deploy anchor should flag"
        );
        assert!(result.diagnostics[0].message.contains("deploy"));
    }
}
