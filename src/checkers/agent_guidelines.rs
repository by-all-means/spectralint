use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::types::ParsedFile;
use crate::parser::{is_directive_line, non_code_lines};
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct AgentGuidelinesChecker {
    scope: ScopeFilter,
}

impl AgentGuidelinesChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

static POSITIVE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\balways\b",
        r"(?i)\bmust\b",
        r"(?i)\bshould\b",
        r"(?i)\bmake sure\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static NEGATIVE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\bnever\b",
        r"(?i)\bdo not\b",
        r"(?i)\bdon'?t\b",
        r"(?i)\bavoid\b",
        r"(?i)\bmust not\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static DELEGATION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\bdo whatever\b",
        r"(?i)\bhandle everything\b",
        r"(?i)\bfull autonomy\b",
        r"(?i)\bcomplete freedom\b",
        r"(?i)\buse your best judgm?ent\b",
        r"(?i)\bfigure it out\b",
        r"(?i)\bas you see fit\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

/// Words that address an agent. Required for ambiguous delegation phrases
/// ("full autonomy", "complete freedom") to avoid flagging project descriptions
/// like "Full autonomy, no external dependencies".
static AGENT_ADDRESSING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:you|your|agent|claude|assistant|copilot|have|grant|give|with)\b").unwrap()
});

static OUTPUT_FORMAT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:output|format|return|respond|response format|structure)\b").unwrap()
});

// ── Responsibility categories (keyword → category) ──────────────────────

const RESPONSIBILITY_KEYWORDS: &[(&[&str], &str)] = &[
    (&["build", "compile"], "build/compile"),
    (&["test", "qa"], "test/qa"),
    (&["deploy", "release"], "deploy/release"),
    (&["review", "audit"], "review/audit"),
    (&["write", "create"], "write/create"),
    (&["debug", "fix"], "debug/fix"),
    (&["security"], "security"),
    (&["performance"], "performance"),
    (&["documentation"], "documentation"),
    (&["formatting", "style"], "formatting/style"),
];

impl Checker for AgentGuidelinesChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            check_missing_negative_constraints(file, &mut result);
            check_multi_responsibility(file, &mut result);
            check_unconstrained_delegation(file, &mut result);
            check_missing_output_format(file, &mut result);
        }

        result
    }
}

fn any_pattern_matches(patterns: &[Regex], line: &str) -> bool {
    patterns.iter().any(|p| p.is_match(line))
}

/// Scan directive lines for positive imperatives and negative constraints.
/// Only fire when: positive_count >= 3 && !has_negative && directive_lines >= 5
const MIN_POSITIVES: usize = 3;
const MIN_DIRECTIVE_LINES: usize = 5;

fn check_missing_negative_constraints(file: &ParsedFile, result: &mut CheckResult) {
    let mut positive_count = 0;
    let mut directive_line_count = 0;

    for (_, line) in non_code_lines(&file.raw_lines) {
        if !is_directive_line(line) {
            continue;
        }

        directive_line_count += 1;

        if any_pattern_matches(&POSITIVE_PATTERNS, line) {
            positive_count += 1;
        }
        if any_pattern_matches(&NEGATIVE_PATTERNS, line) {
            return;
        }
    }

    if positive_count >= MIN_POSITIVES && directive_line_count >= MIN_DIRECTIVE_LINES {
        emit!(
            result,
            file.path,
            1,
            Severity::Info,
            Category::AgentGuidelines,
            suggest: "Add negative constraints (Never/Do not/Avoid) to define clear boundaries",
            "File has positive imperatives (Always/Must/Should) but no negative constraints \
             (Never/Do not/Avoid). Consider adding boundaries to clarify what the agent should NOT do."
        );
    }
}

/// If 4+ distinct responsibility categories appear in section headings,
/// emit once at line 1 listing the categories.
fn check_multi_responsibility(file: &ParsedFile, result: &mut CheckResult) {
    let found_categories: BTreeSet<&str> = file
        .sections
        .iter()
        .flat_map(|section| {
            let title_lower = section.title.to_lowercase();
            RESPONSIBILITY_KEYWORDS
                .iter()
                .filter(move |(keywords, _)| keywords.iter().any(|kw| title_lower.contains(kw)))
                .map(|(_, category)| *category)
        })
        .collect();

    if found_categories.len() >= 4 {
        let cats: Vec<&str> = found_categories.into_iter().collect();
        emit!(
            result,
            file.path,
            1,
            Severity::Info,
            Category::AgentGuidelines,
            suggest: "Split into focused single-responsibility agent files",
            "File covers {} responsibility areas ({}). Consider splitting into \
             focused single-responsibility agent files.",
            cats.len(),
            cats.join(", ")
        );
    }
}

/// Detect open-ended delegation phrases on directive lines.
fn check_unconstrained_delegation(file: &ParsedFile, result: &mut CheckResult) {
    for (i, line) in non_code_lines(&file.raw_lines) {
        if !is_directive_line(line) {
            continue;
        }

        for pat in DELEGATION_PATTERNS.iter() {
            if let Some(m) = pat.find(line) {
                let matched_lower = m.as_str().to_lowercase();
                // "full autonomy" and "complete freedom" are ambiguous — they can
                // describe a project ("Full autonomy, no external dependencies")
                // rather than granting agent freedom. Require agent-addressing context.
                if (matched_lower.contains("autonomy") || matched_lower.contains("freedom"))
                    && !AGENT_ADDRESSING.is_match(line)
                {
                    continue;
                }

                emit!(
                    result,
                    file.path,
                    i + 1,
                    Severity::Info,
                    Category::AgentGuidelines,
                    suggest: "Provide specific boundaries instead of open-ended autonomy",
                    "Unconstrained delegation: \"{}\". \
                     Provide specific boundaries instead of open-ended autonomy.",
                    m.as_str()
                );
            }
        }
    }
}

/// Emit if no non-code line mentions output/format/return/respond/structure.
fn check_missing_output_format(file: &ParsedFile, result: &mut CheckResult) {
    let has_content = file.raw_lines.iter().any(|l| !l.trim().is_empty());
    if !has_content {
        return;
    }

    if non_code_lines(&file.raw_lines).any(|(_, line)| OUTPUT_FORMAT_PATTERN.is_match(line)) {
        return;
    }

    emit!(
        result,
        file.path,
        1,
        Severity::Info,
        Category::AgentGuidelines,
        suggest: "Describe the expected response format or structure",
        "No output format specification found. Consider describing the expected \
         response format or structure."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::{
        count_matching, single_file_ctx, single_file_ctx_with_sections,
    };
    use crate::parser::types::Section;
    use std::collections::HashSet;

    fn make_ctx(root: &std::path::Path, files: Vec<ParsedFile>) -> CheckerContext {
        CheckerContext {
            files,
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        }
    }

    fn make_file(
        root: &std::path::Path,
        name: &str,
        lines: &[&str],
        sections: Vec<Section>,
    ) -> ParsedFile {
        ParsedFile {
            path: root.join(name),
            sections,
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn section(title: &str, level: u8, line: usize) -> Section {
        Section {
            level,
            title: title.to_string(),
            line,
            end_line: 0,
        }
    }

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        AgentGuidelinesChecker::new(&[]).check(&ctx)
    }

    fn run_check_with_sections(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        AgentGuidelinesChecker::new(&[]).check(&ctx)
    }

    // ── Missing Negative Constraints ─────────────────────────────────────

    #[test]
    fn test_positive_without_negative_flags() {
        let result = run_check(&[
            "Always run tests.",
            "Must check types.",
            "Should lint code.",
            "Check the output.",
            "Verify all results.",
        ]);
        assert_eq!(count_matching(&result, "negative constraints"), 1);
    }

    #[test]
    fn test_few_positives_no_flag() {
        // Only 2 positives and 2 directive lines — below thresholds
        let result = run_check(&["Always run tests.", "Must check types."]);
        assert_eq!(
            count_matching(&result, "negative constraints"),
            0,
            "Files with < 3 positives or < 5 directive lines should not flag"
        );
    }

    #[test]
    fn test_positive_with_negative_no_flag() {
        let result = run_check(&["Always run tests.", "Never skip linting."]);
        assert_eq!(count_matching(&result, "negative constraints"), 0);
    }

    #[test]
    fn test_no_positives_no_flag() {
        let result = run_check(&["Run tests.", "Check output."]);
        assert_eq!(count_matching(&result, "negative constraints"), 0);
    }

    #[test]
    fn test_negative_constraint_do_not() {
        let result = run_check(&["Should check code.", "Do not modify tests."]);
        assert_eq!(count_matching(&result, "negative constraints"), 0);
    }

    #[test]
    fn test_negative_constraint_dont() {
        let result = run_check(&["Must review code.", "Don't change the API."]);
        assert_eq!(count_matching(&result, "negative constraints"), 0);
    }

    #[test]
    fn test_positive_in_code_block_skipped() {
        let result = run_check(&["```", "Always run tests.", "```", "Check output."]);
        assert_eq!(
            count_matching(&result, "negative constraints"),
            0,
            "Positives inside code blocks should be ignored"
        );
    }

    #[test]
    fn test_negative_constraint_avoid() {
        let result = run_check(&["Always check types.", "Avoid mutation."]);
        assert_eq!(count_matching(&result, "negative constraints"), 0);
    }

    // ── Multi-Responsibility ─────────────────────────────────────────────

    #[test]
    fn test_multi_responsibility_flags_at_4() {
        let result = run_check_with_sections(
            &["# Build", "# Testing", "# Deploy", "# Review"],
            vec![
                section("Build", 1, 1),
                section("Testing", 1, 2),
                section("Deploy", 1, 3),
                section("Review", 1, 4),
            ],
        );
        assert_eq!(count_matching(&result, "responsibility"), 1);
    }

    #[test]
    fn test_multi_responsibility_no_flag_at_3() {
        let result = run_check_with_sections(
            &["# Build", "# Testing", "# Deploy"],
            vec![
                section("Build", 1, 1),
                section("Testing", 1, 2),
                section("Deploy", 1, 3),
            ],
        );
        assert_eq!(count_matching(&result, "responsibility"), 0);
    }

    #[test]
    fn test_multi_responsibility_lists_categories() {
        let result = run_check_with_sections(
            &[
                "# Build Process",
                "# Test Suite",
                "# Deployment",
                "# Security Checks",
            ],
            vec![
                section("Build Process", 1, 1),
                section("Test Suite", 1, 2),
                section("Deployment", 1, 3),
                section("Security Checks", 1, 4),
            ],
        );

        let diag = result
            .diagnostics
            .iter()
            .find(|d| d.message.contains("responsibility"))
            .expect("Should have multi-responsibility diagnostic");
        assert!(diag.message.contains("build/compile"));
        assert!(diag.message.contains("security"));
    }

    #[test]
    fn test_multi_responsibility_same_category_deduped() {
        let result = run_check_with_sections(
            &["# Build", "# Compile", "# Testing"],
            vec![
                section("Build", 1, 1),
                section("Compile", 1, 2),
                section("Testing", 1, 3),
            ],
        );
        assert_eq!(
            count_matching(&result, "responsibility"),
            0,
            "build and compile are same category, only 2 distinct"
        );
    }

    // ── Unconstrained Delegation ─────────────────────────────────────────

    #[test]
    fn test_delegation_do_whatever() {
        let result = run_check(&["You can do whatever you want."]);
        assert_eq!(count_matching(&result, "Unconstrained delegation"), 1);
    }

    #[test]
    fn test_delegation_figure_it_out() {
        let result = run_check(&["Just figure it out."]);
        assert_eq!(count_matching(&result, "Unconstrained delegation"), 1);
    }

    #[test]
    fn test_delegation_use_best_judgment() {
        let result = run_check(&["Use your best judgment here."]);
        assert_eq!(count_matching(&result, "Unconstrained delegation"), 1);
    }

    #[test]
    fn test_delegation_in_code_block_skipped() {
        let result = run_check(&["```", "Do whatever you want.", "```"]);
        assert_eq!(count_matching(&result, "Unconstrained delegation"), 0);
    }

    #[test]
    fn test_delegation_in_blockquote_skipped() {
        let result = run_check(&["> Do whatever you want."]);
        assert_eq!(count_matching(&result, "Unconstrained delegation"), 0);
    }

    #[test]
    fn test_delegation_no_false_positive() {
        let result = run_check(&["Handle errors gracefully.", "Do the right thing."]);
        assert_eq!(count_matching(&result, "Unconstrained delegation"), 0);
    }

    #[test]
    fn test_delegation_full_autonomy_project_description_skipped() {
        // "Full autonomy" describing a project, not granting agent freedom
        let result = run_check(&["- Full autonomy, no external dependencies"]);
        assert_eq!(
            count_matching(&result, "Unconstrained delegation"),
            0,
            "\"Full autonomy\" without agent-addressing context should not flag"
        );
    }

    #[test]
    fn test_delegation_full_autonomy_with_agent_context_flags() {
        // "Full autonomy" addressed to the agent
        let result = run_check(&["You have full autonomy to make changes."]);
        assert_eq!(
            count_matching(&result, "Unconstrained delegation"),
            1,
            "\"Full autonomy\" with agent-addressing context should flag"
        );
    }

    #[test]
    fn test_delegation_complete_freedom_project_description_skipped() {
        let result = run_check(&["- Complete freedom from vendor lock-in"]);
        assert_eq!(
            count_matching(&result, "Unconstrained delegation"),
            0,
            "\"Complete freedom\" without agent-addressing context should not flag"
        );
    }

    #[test]
    fn test_delegation_complete_freedom_with_agent_context_flags() {
        let result = run_check(&["Give the agent complete freedom."]);
        assert_eq!(
            count_matching(&result, "Unconstrained delegation"),
            1,
            "\"Complete freedom\" with agent-addressing context should flag"
        );
    }

    #[test]
    fn test_delegation_multiple_per_file() {
        let result = run_check(&["Do whatever you think is best.", "Figure it out yourself."]);
        assert_eq!(count_matching(&result, "Unconstrained delegation"), 2);
    }

    // ── Missing Output Format ────────────────────────────────────────────

    #[test]
    fn test_missing_output_format_flags() {
        let result = run_check(&["# Instructions", "Run the tests."]);
        assert_eq!(count_matching(&result, "output format"), 1);
    }

    #[test]
    fn test_output_format_present_no_flag() {
        let result = run_check(&["# Instructions", "Return JSON output."]);
        assert_eq!(count_matching(&result, "output format"), 0);
    }

    #[test]
    fn test_output_format_in_heading_no_flag() {
        let result = run_check(&["# Response Format", "Use markdown."]);
        assert_eq!(count_matching(&result, "output format"), 0);
    }

    #[test]
    fn test_empty_file_no_output_format_flag() {
        let result = run_check(&[]);
        assert_eq!(count_matching(&result, "output format"), 0);
    }

    #[test]
    fn test_whitespace_only_file_no_output_format_flag() {
        let result = run_check(&["  ", "", "  "]);
        assert_eq!(count_matching(&result, "output format"), 0);
    }

    #[test]
    fn test_output_format_only_in_code_block_still_flags() {
        let result = run_check(&["# Guide", "```", "output: json", "```", "Do the work."]);
        assert_eq!(
            count_matching(&result, "output format"),
            1,
            "Output keyword only in code block should still flag"
        );
    }

    // ── Scope filtering ──────────────────────────────────────────────────

    #[test]
    fn test_scope_excludes_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = make_file(root, "reports/output.md", &["Always run tests."], vec![]);
        let ctx = make_ctx(root, vec![file]);
        let checker = AgentGuidelinesChecker::new(&["CLAUDE.md".to_string()]);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "Out-of-scope file should produce no diagnostics"
        );
    }

    #[test]
    fn test_scope_includes_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Needs enough content to trigger at least one sub-check (e.g., missing output format)
        let file = make_file(
            root,
            "CLAUDE.md",
            &["# Instructions", "Run the tests."],
            vec![],
        );
        let ctx = make_ctx(root, vec![file]);
        let checker = AgentGuidelinesChecker::new(&["CLAUDE.md".to_string()]);
        let result = checker.check(&ctx);

        assert!(
            !result.diagnostics.is_empty(),
            "In-scope file should produce diagnostics"
        );
    }

    // ── Severity ─────────────────────────────────────────────────────────

    #[test]
    fn test_all_diagnostics_are_info() {
        // "Do whatever" triggers delegation; missing output format triggers too
        let result = run_check(&["# Guide", "Do whatever you want."]);

        assert!(
            !result.diagnostics.is_empty(),
            "Should produce at least one diagnostic"
        );
        for d in &result.diagnostics {
            assert_eq!(d.severity, Severity::Info);
            assert_eq!(d.category, Category::AgentGuidelines);
        }
    }
}
