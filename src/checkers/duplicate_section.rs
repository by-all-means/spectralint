use std::collections::HashMap;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct DuplicateSectionChecker {
    scope: ScopeFilter,
}

impl DuplicateSectionChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for DuplicateSectionChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "duplicate-section",
            description: "Flags repeated section headings within a file",
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

            // parents[level] = most recent heading at that level (scopes duplicate detection)
            let mut parents: [String; 7] = Default::default();
            let mut seen: HashMap<String, usize> = HashMap::new();

            for section in &file.sections {
                let level = section.level as usize;

                for parent in parents.iter_mut().skip(level) {
                    parent.clear();
                }

                // h3+ scoped to parent (parallel structure), h2 compared globally
                let parent = if level > 2 { &parents[level - 1] } else { "" };
                let key = format!(
                    "h{}:{}:{}",
                    level,
                    parent.to_lowercase(),
                    section.title.to_lowercase()
                );

                if let Some(&first_line) = seen.get(&key) {
                    emit!(
                        result,
                        file.path,
                        section.line,
                        Severity::Warning,
                        Category::DuplicateSection,
                        suggest: "Merge duplicate sections or rename one to be more specific",
                        "Duplicate section heading \"{}\" (first occurrence at line {})",
                        section.title,
                        first_line
                    );
                } else {
                    seen.insert(key, section.line);
                }

                parents[level] = section.title.clone();
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::{
        section_with_end as section, single_file_ctx_with_sections,
    };
    use crate::parser::types::Section;

    fn run_check(sections: Vec<Section>) -> CheckResult {
        let lines: Vec<&str> = vec!["# Doc"];
        let (_dir, ctx) = single_file_ctx_with_sections(&lines, sections);
        DuplicateSectionChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_duplicate_detected() {
        let result = run_check(vec![
            section("Testing", 2, 1, 5),
            section("Build", 2, 6, 10),
            section("Testing", 2, 11, 15),
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[0].line, 11);
        assert!(result.diagnostics[0].message.contains("line 1"));
    }

    #[test]
    fn test_same_heading_different_levels_no_flag() {
        let result = run_check(vec![
            section("Testing", 2, 1, 5),
            section("Testing", 3, 6, 10),
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_unique_headings_no_flag() {
        let result = run_check(vec![
            section("Build", 2, 1, 5),
            section("Test", 2, 6, 10),
            section("Deploy", 2, 11, 15),
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_case_insensitive_duplicate() {
        let result = run_check(vec![
            section("Testing", 2, 1, 5),
            section("testing", 2, 6, 10),
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_empty_sections_no_flag() {
        let result = run_check(vec![]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_triple_duplicate_flags_second_and_third() {
        let result = run_check(vec![
            section("Testing", 2, 1, 5),
            section("Testing", 2, 6, 10),
            section("Testing", 2, 11, 15),
        ]);
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].line, 6);
        assert_eq!(result.diagnostics[1].line, 11);
    }

    #[test]
    fn test_same_subsection_under_different_parents_no_flag() {
        // ### Testing under ## Build and ### Testing under ## Deploy
        // These are intentional parallel structure, not duplicates
        let result = run_check(vec![
            section("Build", 2, 1, 20),
            section("Testing", 3, 5, 15),
            section("Deploy", 2, 20, 40),
            section("Testing", 3, 25, 35),
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Subsections under different parents should not flag as duplicates"
        );
    }

    #[test]
    fn test_same_subsection_under_same_parent_flags() {
        // Two ### Testing under the same ## Build parent
        let result = run_check(vec![
            section("Build", 2, 1, 30),
            section("Testing", 3, 5, 15),
            section("Testing", 3, 16, 25),
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_multiple_different_duplicates() {
        let result = run_check(vec![
            section("Build", 2, 1, 5),
            section("Test", 2, 6, 10),
            section("Build", 2, 11, 15),
            section("Test", 2, 16, 20),
        ]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_single_section_no_flag() {
        let result = run_check(vec![section("Build", 2, 1, 10)]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_h2_under_different_h1_parents_still_flags() {
        // ## Python under # Development Guidelines and ## Python under # Code Style
        // should still flag — h2 sections are compared globally regardless of h1 parent
        let result = run_check(vec![
            section("Development Guidelines", 1, 1, 100),
            section("Python", 2, 5, 50),
            section("Code Style", 1, 100, 200),
            section("Python", 2, 105, 150),
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Duplicate h2 sections should flag even under different h1 parents"
        );
    }
}
