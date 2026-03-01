use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{has_elaboration_after, is_heading, ScopeFilter};
use super::Checker;

pub(crate) struct AmbiguousScopeReferenceChecker {
    scope: ScopeFilter,
}

impl AmbiguousScopeReferenceChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Matches ambiguous quantifier + open-ended noun phrases.
static AMBIGUOUS_SCOPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:the|all|any|each)\s+",
        r"(?:relevant|appropriate|necessary|required|applicable|related|corresponding|associated|proper)\s+",
        r"(?:files?|tests?|modules?|components?|configuration|configs?|dependencies|packages?|services?|resources?|changes?|updates?)\b",
    ))
    .unwrap()
});

/// Returns true if the line contains inline code (backtick), a file path
/// reference, or elaboration after a colon — signals that disambiguate the scope.
fn has_concrete_reference(line: &str) -> bool {
    line.contains('`')
        || line.contains(".rs")
        || line.contains(".ts")
        || line.contains(".js")
        || line.contains(".py")
        || line.contains(".md")
        || line.contains(".toml")
        || line.contains(".json")
        || line.contains(".yaml")
        || line.contains(".yml")
}

impl Checker for AmbiguousScopeReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                let line_num = idx + 1;

                if is_heading(line) || !is_directive_line(line) {
                    continue;
                }

                if has_concrete_reference(line) {
                    continue;
                }

                if let Some(m) = AMBIGUOUS_SCOPE.find(line) {
                    if !has_elaboration_after(line, m.end()) {
                        emit!(
                            result,
                            file.path,
                            line_num,
                            Severity::Info,
                            Category::AmbiguousScopeReference,
                            suggest: "Replace with specific file paths, test commands, or concrete references",
                            "Ambiguous scope reference: \"{}\"",
                            m.as_str()
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
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        AmbiguousScopeReferenceChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_the_relevant_files() {
        let result = run_check(&["- Update the relevant files"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("the relevant files"));
    }

    #[test]
    fn test_appropriate_tests() {
        let result = run_check(&["- Run the appropriate tests"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_necessary_configuration() {
        let result = run_check(&["- Update the necessary configuration"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_related_modules() {
        let result = run_check(&["- Check all related modules"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_with_backtick_no_flag() {
        let result = run_check(&["- Update the relevant files in `src/`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_with_file_extension_no_flag() {
        let result = run_check(&["- Update the relevant files like config.toml"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_with_colon_elaboration_no_flag() {
        let result = run_check(&["- Update the relevant files: src/main.rs and lib.rs"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_heading_no_flag() {
        let result = run_check(&["## Update the relevant files"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_no_flag() {
        let result = run_check(&["```", "Update the relevant files", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_specific_instruction_no_flag() {
        let result = run_check(&["- Run `cargo test` before committing"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_clean_file() {
        let result = run_check(&[
            "# Build",
            "- Run `cargo test` before committing",
            "- Update `Cargo.toml` when adding dependencies",
        ]);
        assert!(result.diagnostics.is_empty());
    }
}
