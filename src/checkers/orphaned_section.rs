use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

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

impl Checker for OrphanedSectionChecker {
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

                // Skip if next is a child section (lower level = deeper nesting)
                if next.level > current.level {
                    continue;
                }

                // Next is same level or higher (parent) — check for content between
                let content_start = current.line; // heading line (1-indexed)
                let content_end = next.line - 1; // line before next heading

                let has_content = file
                    .raw_lines
                    .iter()
                    .skip(content_start) // skip the heading line itself
                    .take(content_end.saturating_sub(content_start))
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
}
