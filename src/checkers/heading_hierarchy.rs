use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct HeadingHierarchyChecker {
    scope: ScopeFilter,
}

impl HeadingHierarchyChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for HeadingHierarchyChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut last_level: Option<u8> = None;

            for section in &file.sections {
                if let Some(prev) = last_level {
                    if section.level > prev + 1 {
                        emit!(
                            result,
                            file.path,
                            section.line,
                            Severity::Info,
                            Category::HeadingHierarchy,
                            suggest: "Add an intermediate heading level to maintain hierarchy",
                            "Heading level skipped: h{} to h{} (\"{}\")",
                            prev,
                            section.level,
                            section.title
                        );
                    }
                }

                last_level = Some(section.level);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::{ParsedFile, Section};
    use std::collections::HashSet;

    fn section(title: &str, level: u8, line: usize) -> Section {
        Section {
            level,
            title: title.to_string(),
            line,
            end_line: 0,
        }
    }

    fn run_check(sections: Vec<Section>) -> CheckResult {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections,
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };
        HeadingHierarchyChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_skipped_level_detected() {
        let result = run_check(vec![section("Title", 1, 1), section("Sub", 3, 3)]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert!(result.diagnostics[0].message.contains("h1 to h3"));
    }

    #[test]
    fn test_proper_hierarchy_no_diagnostic() {
        let result = run_check(vec![
            section("Title", 1, 1),
            section("Sub", 2, 3),
            section("SubSub", 3, 5),
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_going_back_up_no_diagnostic() {
        let result = run_check(vec![
            section("Title", 1, 1),
            section("Sub", 2, 3),
            section("Another", 1, 5),
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_single_heading_no_diagnostic() {
        let result = run_check(vec![section("Title", 1, 1)]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_headings_no_diagnostic() {
        let result = run_check(vec![]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_starting_at_h2_no_diagnostic() {
        let result = run_check(vec![section("Sub", 2, 1), section("SubSub", 3, 3)]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_skips_detected() {
        let result = run_check(vec![
            section("Title", 1, 1),
            section("Deep", 4, 3),
            section("Normal", 2, 5),
            section("Deeper", 5, 7),
        ]);
        assert_eq!(result.diagnostics.len(), 2);
    }
}
