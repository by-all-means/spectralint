use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct OrphanedSectionChecker {
    scope: ScopeFilter,
}

impl OrphanedSectionChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Returns true if the section title looks like an intentional outline/prompt
/// heading that is expected to have no body content (e.g., numbered outlines,
/// questions for an LLM to fill in, or placeholder indicators).
fn is_intentionally_empty(title: &str) -> bool {
    let trimmed = title.trim();

    // Numbered/lettered outline headings: "1. Topic", "a. Subtopic", "12. Item"
    if let Some((prefix, _)) = trimmed.split_once(". ") {
        if !prefix.is_empty()
            && prefix
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
        {
            return true;
        }
    }

    // Question headings: "What should we build?"
    if trimmed.ends_with('?') {
        return true;
    }

    // Placeholder indicators: "..." or "[fill in]"
    if trimmed.contains("...") || trimmed.contains("[fill in]") {
        return true;
    }

    false
}

/// Returns true if the section title looks like a non-heading line that was
/// mis-parsed as a heading (MediaWiki list items, separators, slash commands).
fn is_likely_not_heading(title: &str) -> bool {
    // MediaWiki bold markup used as numbered list items: # '''Bold text'''
    title.starts_with("'''")
    // Decorative separators: # === END === or # ---
    || title.trim_matches(|c: char| c == '=' || c == '-' || c == '*' || c == ' ').is_empty()
    // Wrapped separators: # === END USER INSTRUCTIONS ===
    || (title.starts_with("===") && title.ends_with("==="))
    // Slash commands or paths mistaken for headings: # /superpowers:brainstorm
    || title.starts_with('/')
}

impl Checker for OrphanedSectionChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "orphaned-section",
            description: "Flags headings with no content before the next heading",
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

            let sections = &file.sections;
            if sections.len() < 2 {
                continue;
            }

            for i in 0..sections.len() - 1 {
                let current = &sections[i];
                let next = &sections[i + 1];

                // Skip document title (first heading at line 1)
                if current.line == 1 && current.level == 1 {
                    continue;
                }

                // Skip lines that look like they were mis-parsed as headings
                if is_likely_not_heading(&current.title) {
                    continue;
                }

                // Skip if the next "heading" isn't a real heading either —
                // can't be orphaned relative to a mis-parsed line
                if is_likely_not_heading(&next.title) {
                    continue;
                }

                // Skip intentional outline/prompt headings that are expected
                // to be empty (numbered outlines, questions, placeholders)
                if is_intentionally_empty(&current.title) {
                    continue;
                }

                // Skip if next is a child section (lower level = deeper nesting)
                if next.level > current.level {
                    continue;
                }

                // Next is same level or higher (parent) — check for content between
                let content_start = current.line; // heading line (1-indexed)
                let content_end = next.line - 1; // line before next heading

                let has_content = file.raw_lines
                    [content_start..content_start + content_end.saturating_sub(content_start)]
                    .iter()
                    .any(|line| {
                        let trimmed = line.trim();
                        !trimmed.is_empty() && !trimmed.starts_with('#')
                    });

                if !has_content {
                    emit!(
                        result,
                        file.path,
                        current.line,
                        Severity::Info,
                        Category::OrphanedSection,
                        suggest: "Add content to this section or remove the empty heading",
                        "Orphaned section \"{}\" has no content before the next heading",
                        current.title
                    );
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::{section_with_end, single_file_ctx_with_sections};

    fn run_check(lines: &[&str], sections: Vec<crate::parser::types::Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        OrphanedSectionChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_orphaned_same_level() {
        let result = run_check(
            &["## Build Commands", "## Testing"],
            vec![
                section_with_end("Build Commands", 2, 1, 1),
                section_with_end("Testing", 2, 2, 2),
            ],
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Build Commands"));
    }

    #[test]
    fn test_parent_to_child_not_flagged() {
        let result = run_check(
            &["## Build Commands", "### Development"],
            vec![
                section_with_end("Build Commands", 2, 1, 1),
                section_with_end("Development", 3, 2, 2),
            ],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_section_with_content_not_flagged() {
        let result = run_check(
            &[
                "## Build Commands",
                "Run `cargo build` to compile.",
                "## Testing",
            ],
            vec![
                section_with_end("Build Commands", 2, 1, 2),
                section_with_end("Testing", 2, 3, 3),
            ],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_section_with_only_blank_lines_flagged() {
        let result = run_check(
            &["## Build Commands", "", "", "## Testing"],
            vec![
                section_with_end("Build Commands", 2, 1, 3),
                section_with_end("Testing", 2, 4, 4),
            ],
        );
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_single_section_not_flagged() {
        let result = run_check(
            &["## Only Section"],
            vec![section_with_end("Only Section", 2, 1, 1)],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_higher_level_transition_flagged() {
        // Going from h3 back to h2 with no content
        let result = run_check(
            &["### Sub Section", "## Main Section"],
            vec![
                section_with_end("Sub Section", 3, 1, 1),
                section_with_end("Main Section", 2, 2, 2),
            ],
        );
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_multiple_orphans() {
        let result = run_check(
            &["## A", "## B", "## C", "Content here"],
            vec![
                section_with_end("A", 2, 1, 1),
                section_with_end("B", 2, 2, 2),
                section_with_end("C", 2, 3, 4),
            ],
        );
        // A and B are orphaned, C has content
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_last_section_not_checked() {
        // Last section has no "next" to compare against, so it's not flagged
        let result = run_check(&["## Section"], vec![section_with_end("Section", 2, 1, 1)]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_numbered_outline_not_flagged() {
        let result = run_check(
            &["## 1. Business Model Options", "## 2. Revenue Streams"],
            vec![
                section_with_end("1. Business Model Options", 2, 1, 1),
                section_with_end("2. Revenue Streams", 2, 2, 2),
            ],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_lettered_outline_not_flagged() {
        let result = run_check(
            &["## a. Subtopic", "## b. Another Subtopic"],
            vec![
                section_with_end("a. Subtopic", 2, 1, 1),
                section_with_end("b. Another Subtopic", 2, 2, 2),
            ],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_question_heading_not_flagged() {
        let result = run_check(
            &["## What should we build?", "## Next Steps"],
            vec![
                section_with_end("What should we build?", 2, 1, 1),
                section_with_end("Next Steps", 2, 2, 2),
            ],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_placeholder_heading_not_flagged() {
        let result = run_check(
            &["## Topics to cover...", "## Next Section"],
            vec![
                section_with_end("Topics to cover...", 2, 1, 1),
                section_with_end("Next Section", 2, 2, 2),
            ],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_fill_in_placeholder_not_flagged() {
        let result = run_check(
            &["## [fill in] your goals", "## Next Section"],
            vec![
                section_with_end("[fill in] your goals", 2, 1, 1),
                section_with_end("Next Section", 2, 2, 2),
            ],
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_plain_empty_section_still_flagged() {
        // "Empty Section" has no number, no question mark, no placeholder
        let result = run_check(
            &["## Empty Section", "## Next Section"],
            vec![
                section_with_end("Empty Section", 2, 1, 1),
                section_with_end("Next Section", 2, 2, 2),
            ],
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Empty Section"));
    }
}
