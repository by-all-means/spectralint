use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::is_directive_line;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_heading, ScopeFilter};
use super::Checker;

pub struct UnversionedStackReferenceChecker {
    scope: ScopeFilter,
}

impl UnversionedStackReferenceChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Well-known frameworks, languages, databases, and runtimes.
static FRAMEWORK_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\b(?:",
        // JS/TS ecosystem
        r"React|Next\.js|Nuxt|Vue|Angular|Svelte|Remix|Astro|Gatsby",
        r"|Node(?:\.js)?|Deno|Bun",
        r"|TypeScript|JavaScript|ES\d+",
        // Python
        r"|Python|Django|Flask|FastAPI",
        // Ruby
        r"|Ruby|Rails",
        // JVM
        r"|Java|Kotlin|Spring\s+Boot|Spring",
        // Systems
        r"|Rust|Go|C\+\+|Swift|Zig",
        // .NET
        r"|\.NET|C#",
        // PHP
        r"|PHP|Laravel",
        // Databases
        r"|PostgreSQL|MySQL|MongoDB|Redis|SQLite|Elasticsearch",
        // Mobile
        r"|React\s+Native|Flutter|Dart",
        // Infrastructure
        r"|Docker|Kubernetes|Terraform|Ansible",
        // CSS / UI
        r"|Tailwind(?:\s+CSS)?|Bootstrap",
        r")\b",
    ))
    .unwrap()
});

/// Stack-description context: phrases indicating a tech stack declaration.
/// Intentionally excludes "uses/using" (too common in tool instructions like
/// "don't use dotenv") — only matches unambiguous stack declarations.
static STACK_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:",
        r"built\s+(?:with|on|using)",
        r"|powered\s+by",
        r"|running\s+on",
        r"|written\s+in",
        r"|requires?\s+",
        r"|depends?\s+on",
        r")",
    ))
    .unwrap()
});

/// Label-style stack declarations ("Backend:", "Runtime:", etc.).
/// Anchored to line start (after bullet marker) to avoid FPs like
/// "In the JavaScript runtime:" where the colon is mid-sentence.
static STACK_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)^\s*(?:[-*>]\s*|(?:\d+[.)]\s+))?(?:",
        r"stack",
        r"|tech(?:nology)?\s*(?:stack)?",
        r"|frontend",
        r"|backend",
        r"|database",
        r"|framework",
        r"|runtime",
        r"|platform",
        r"|infrastructure",
        r")\s*:",
    ))
    .unwrap()
});

static VERSION_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bv?\d+(?:\.\d+)*\b").unwrap());

/// Project self-description — "This is the X repo/project/tool" describes what
/// the project IS, not a dependency to pin.
static PROJECT_DESCRIPTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:-\s+)?this\s+is\s+(?:the|a)\b").unwrap());

fn should_skip(line: &str) -> bool {
    is_heading(line)
        || !is_directive_line(line)
        || line.contains('`')
        || PROJECT_DESCRIPTION.is_match(line)
}

impl Checker for UnversionedStackReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                let line_num = idx + 1;

                if should_skip(line) {
                    continue;
                }

                // Must have both a framework name AND stack context
                let Some(fw_match) = FRAMEWORK_NAME.find(line) else {
                    continue;
                };
                if !STACK_CONTEXT.is_match(line) && !STACK_LABEL.is_match(line) {
                    continue;
                }

                // If there's already a version number on the line, it's fine
                if VERSION_NUMBER.is_match(line) {
                    continue;
                }

                emit!(
                    result,
                    file.path,
                    line_num,
                    Severity::Info,
                    Category::UnversionedStackReference,
                    suggest: "Pin a version number (e.g., \"React 18\") to prevent drift.",
                    "Unversioned stack reference: \"{}\"",
                    fw_match.as_str()
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
        UnversionedStackReferenceChecker::new(&[]).check(&ctx)
    }

    // ── Positive cases ──

    #[test]
    fn test_built_with_react() {
        let result = run_check(&["- Built with React"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("React"));
    }

    #[test]
    fn test_stack_colon() {
        let result = run_check(&["- Stack: React, Node, PostgreSQL"]);
        // Should flag at least one (the first match)
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn test_powered_by_django() {
        let result = run_check(&["- Powered by Django"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_frontend_colon() {
        let result = run_check(&["- Frontend: React"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_backend_colon() {
        let result = run_check(&["- Backend: Django"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_written_in_rust() {
        let result = run_check(&["- Written in Rust"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_running_on_node() {
        let result = run_check(&["- Running on Node.js"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    // ── Versioned (no flag) ──

    #[test]
    fn test_react_18_no_flag() {
        let result = run_check(&["- Built with React 18"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_python_3_11_no_flag() {
        let result = run_check(&["- Written in Python v3.11"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_node_20_no_flag() {
        let result = run_check(&["- Running on Node.js v20"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_next_14_2_no_flag() {
        let result = run_check(&["- Built with Next.js 14.2"]);
        assert!(result.diagnostics.is_empty());
    }

    // ── No stack context (no flag) ──

    #[test]
    fn test_check_react_docs_no_flag() {
        let result = run_check(&["- Check the React docs for details"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_react_in_prose_no_flag() {
        let result = run_check(&["- React components should be pure functions"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_heading_no_flag() {
        let result = run_check(&["## Built with React"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_no_flag() {
        let result = run_check(&["```", "Built with React", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_requires_docker() {
        let result = run_check(&["- Requires Docker"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_depends_on_redis() {
        let result = run_check(&["- Depends on Redis"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_based_on_no_flag() {
        // "based on" is too ambiguous ("based on user's OS" ≠ stack declaration)
        let result = run_check(&["- Based on Spring Boot"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_uses_python_no_flag() {
        // "uses" alone is too common (tool instructions), not flagged
        let result = run_check(&["- Uses Python"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_no_match_plain_prose() {
        let result = run_check(&["- Always run tests before committing"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_backtick_context_no_flag() {
        let result = run_check(&["- Built with `React` and `Node`"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_project_description_no_flag() {
        let result = run_check(&["- This is the React repository"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_this_is_a_project_no_flag() {
        let result = run_check(&["- This is a Django application"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_runtime_mid_sentence_no_flag() {
        // "runtime:" mid-sentence is not a stack label declaration
        let result = run_check(&["- In the JavaScript runtime:"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_runtime_label_at_start() {
        // "Runtime: Node.js" at start of bullet IS a stack declaration
        let result = run_check(&["- Runtime: Node.js"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_frontend_label() {
        let result = run_check(&["- Frontend: React"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_backend_label() {
        let result = run_check(&["- Backend: Django"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
