use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct BoilerplateTemplateChecker {
    scope: ScopeFilter,
}

impl BoilerplateTemplateChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

const TEMPLATE_FINGERPRINTS: &[&str] = &[
    "this file provides guidance to claude code",
    "this file provides guidance to claude",
];

/// Max non-empty lines for a file to be considered "mostly template".
const MAX_NON_EMPTY_LINES: usize = 20;

impl Checker for BoilerplateTemplateChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let non_empty_count = file
                .raw_lines
                .iter()
                .filter(|l| !l.trim().is_empty())
                .count();

            if non_empty_count > MAX_NON_EMPTY_LINES {
                continue;
            }

            let has_fingerprint = file.raw_lines.iter().any(|line| {
                let lower = line.to_lowercase();
                TEMPLATE_FINGERPRINTS.iter().any(|fp| lower.contains(fp))
            });

            if has_fingerprint {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::BoilerplateTemplate,
                    suggest: "Customize this file with project-specific instructions for better agent performance",
                    "File appears to be an unchanged default template. \
                     Default templates provide minimal agent value."
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

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        BoilerplateTemplateChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_default_template_flagged() {
        let result = run_check(&[
            "# CLAUDE.md",
            "",
            "This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.",
            "",
            "## Build",
            "npm run build",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].category,
            Category::BoilerplateTemplate
        );
    }

    #[test]
    fn test_customized_template_no_flag() {
        // File has the fingerprint but > 20 non-empty lines = substantial content added
        let mut lines = vec![
            "# CLAUDE.md",
            "",
            "This file provides guidance to Claude Code when working with this repo.",
        ];
        for _ in 0..20 {
            lines.push("- Do something specific");
        }
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_fingerprint_no_flag() {
        let result = run_check(&[
            "# Project Instructions",
            "",
            "## Build",
            "Run `cargo build` to compile.",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_case_insensitive() {
        let result = run_check(&["This File Provides Guidance To Claude Code for this repo."]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_empty_file_no_flag() {
        let result = run_check(&[]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_exactly_20_non_empty_still_flags() {
        let mut lines: Vec<&str> = Vec::new();
        lines.push("This file provides guidance to Claude Code.");
        for _ in 0..19 {
            lines.push("- item");
        }
        let result = run_check(&lines);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_21_non_empty_no_flag() {
        let mut lines: Vec<&str> = Vec::new();
        lines.push("This file provides guidance to Claude Code.");
        for _ in 0..20 {
            lines.push("- item");
        }
        let result = run_check(&lines);
        assert!(result.diagnostics.is_empty());
    }
}
