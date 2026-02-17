use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct SessionJournalChecker {
    scope: ScopeFilter,
}

impl SessionJournalChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Strong markers: unambiguously indicate a session journal.
/// These patterns are highly unlikely to appear in legitimate instruction files.
static STRONG_MARKERS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (
            r"(?i)\bwhat we (?:accomplished|completed|did|built|fixed)\b",
            "retrospective heading",
        ),
        (
            r"(?i)\bsession (?:progress|summary|notes|log)\b",
            "session log heading",
        ),
        (r"(?i)\bwhat we just\b", "recent-action heading"),
        (r"(?i)\bprevious session\b", "session reference"),
        (
            r"(?i)\bfiles? (?:modified|changed|updated|created|removed|touched)\s+(?:this|last|in this)\b",
            "session file changelog",
        ),
    ]
    .iter()
    .map(|(p, label)| (Regex::new(p).unwrap(), *label))
    .collect()
});

/// Weak markers: could appear in legitimate files but reinforce the signal
/// when combined with strong markers.
static WEAK_MARKERS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (
            r"(?i)\bfiles? (?:modified|changed|updated|created|removed|touched)\b",
            "file changelog",
        ),
        (r"(?i)\b(?:PR|pull request) (?:status|#\d+)\b", "PR status"),
        (r"(?i)\bnext steps? after\b", "post-session todo"),
        (r"(?i)\bcurrent status\b", "status section"),
        (
            r"(?i)\bexpected (?:performance |)impact\b",
            "impact assessment",
        ),
        (r"(?i)\bkey decisions? made\b", "decision log"),
    ]
    .iter()
    .map(|(p, label)| (Regex::new(p).unwrap(), *label))
    .collect()
});

static CHECKMARK_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[✅❌]").unwrap());

/// Minimum strong markers required. At least 2 strong markers must be present
/// so generic markers alone cannot cause a false positive.
const STRONG_MARKER_THRESHOLD: usize = 2;

/// Minimum number of checkmarks to count as a weak "checkmark density" marker.
const CHECKMARK_THRESHOLD: usize = 8;

impl Checker for SessionJournalChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut strong_markers: Vec<&str> = Vec::new();
            let mut weak_markers: Vec<&str> = Vec::new();
            let mut checkmark_count = 0;

            for (_, line) in non_code_lines(&file.raw_lines) {
                checkmark_count += CHECKMARK_PATTERN.find_iter(line).count();

                for (pat, label) in STRONG_MARKERS.iter() {
                    if pat.is_match(line) && !strong_markers.contains(label) {
                        strong_markers.push(label);
                    }
                }

                for (pat, label) in WEAK_MARKERS.iter() {
                    if pat.is_match(line) && !weak_markers.contains(label) {
                        weak_markers.push(label);
                    }
                }
            }

            // High checkmark density is a weak marker
            if checkmark_count >= CHECKMARK_THRESHOLD {
                weak_markers.push("checkmark density");
            }

            // Fire if we have 3+ strong markers, OR
            // 2+ strong markers AND at least one additional signal (weak marker or any checkmarks)
            let has_additional_signal = !weak_markers.is_empty() || checkmark_count > 0;
            if strong_markers.len() >= 3
                || (strong_markers.len() >= STRONG_MARKER_THRESHOLD && has_additional_signal)
            {
                let mut all_markers: Vec<&str> = Vec::new();
                all_markers.extend(&strong_markers);
                all_markers.extend(&weak_markers);
                all_markers.sort();
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Warning,
                    Category::SessionJournal,
                    suggest: "Rewrite as forward-looking instructions — tell the agent what to do, not what was done",
                    "File appears to be a session journal, not an instruction file. \
                     Detected {} markers: {}. Rewrite with imperative instructions \
                     (commands, constraints, conventions).",
                    all_markers.len(),
                    all_markers.join(", ")
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
        SessionJournalChecker::new(&[]).check(&ctx)
    }

    // ── Detection tests (true positives) ─────────────────────────────────

    #[test]
    fn test_session_journal_detected() {
        let result = run_check(&[
            "# Session Progress",
            "## What We Accomplished",
            "- ✅ Fixed the bug",
            "- ✅ Updated tests",
            "## Files Modified",
            "- src/main.rs",
            "## Next Steps After Reboot",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[0].category, Category::SessionJournal);
    }

    #[test]
    fn test_bam_web_fancy_style_journal() {
        let result = run_check(&[
            "# Performance Optimization - Context for Claude",
            "## LATEST SESSION PROGRESS (2025-01-05)",
            "### What We Just Completed",
            "- ✅ Fixed font loading",
            "- ✅ Image optimization",
            "- ✅ Mobile viewport",
            "- ✅ PWA enhancements",
            "- ✅ Resource loading",
            "- ✅ CSS cleanup",
            "- ✅ Type errors",
            "- ✅ Meta tags",
            "### Files Modified This Session",
            "- app.html",
            "- AnimatedCard.svelte",
            "### Expected Performance Impact",
            "- LCP: significant improvement",
            "### Key Decisions Made",
            "- Font preloading strategy",
            "### PR Status",
            "- PR #260: optimized images",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_two_strong_plus_one_weak_fires() {
        let result = run_check(&[
            "## What We Accomplished",
            "## Session Summary",
            "## Current Status",
        ]);
        assert_eq!(result.diagnostics.len(), 1, "2 strong + 1 weak = fires");
    }

    #[test]
    fn test_three_strong_markers_fires() {
        let result = run_check(&[
            "## What We Accomplished",
            "## Session Summary",
            "## What We Just Did",
        ]);
        assert_eq!(result.diagnostics.len(), 1, "3 strong markers should fire");
    }

    #[test]
    fn test_pr_status_as_strong_evidence() {
        let result = run_check(&[
            "## What We Completed",
            "## Previous Session",
            "## PR #260 merged",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_checkmark_density_as_weak_marker() {
        let result = run_check(&[
            "# What We Accomplished",
            "# Session Progress",
            "- ✅ Task 1",
            "- ✅ Task 2",
            "- ✅ Task 3",
            "- ✅ Task 4",
            "- ✅ Task 5",
            "- ✅ Task 6",
            "- ✅ Task 7",
            "- ✅ Task 8",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "2 strong + checkmark density (weak) = 3 total, fires"
        );
    }

    #[test]
    fn test_message_lists_markers() {
        let result = run_check(&[
            "# Session Progress",
            "## What We Accomplished",
            "## Files Modified",
        ]);
        let msg = &result.diagnostics[0].message;
        assert!(msg.contains("session log heading"));
        assert!(msg.contains("retrospective heading"));
    }

    // ── False positive prevention (true negatives) ───────────────────────

    #[test]
    fn test_proper_instruction_file_no_diagnostic() {
        let result = run_check(&[
            "# Project Instructions",
            "## Commands",
            "- `cargo test` to run tests",
            "- `cargo build` to compile",
            "## Conventions",
            "- Use snake_case for all functions",
            "- Never commit directly to main",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_three_weak_markers_no_diagnostic() {
        // This is the key false-positive prevention test.
        // A legitimate instruction file can have "Current Status",
        // "Expected Impact", and "Key Decisions Made" without being a journal.
        let result = run_check(&[
            "# Architecture Decision Record",
            "## Current Status",
            "The project uses a microservices architecture.",
            "## Expected Impact",
            "Reduced latency by 40%.",
            "## Key Decisions Made",
            "- Use gRPC instead of REST",
            "- Deploy to Kubernetes",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "3 weak markers without any strong marker should NOT trigger"
        );
    }

    #[test]
    fn test_adr_style_file_no_diagnostic() {
        // Architectural Decision Record with generic markers
        let result = run_check(&[
            "# ADR-001: Database Selection",
            "## Current Status",
            "Accepted.",
            "## Context",
            "We need a database for the new service.",
            "## Expected Performance Impact",
            "Sub-millisecond reads with Redis.",
            "## Key Decisions Made",
            "- Use Redis for caching",
            "- PostgreSQL for persistence",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "ADR-style file with only weak markers should NOT trigger"
        );
    }

    #[test]
    fn test_project_brief_no_diagnostic() {
        // Project brief that mentions status and impact
        let result = run_check(&[
            "# Project Brief",
            "## Current Status",
            "In active development.",
            "## PR Status",
            "All PRs must be reviewed by 2 people.",
            "## Expected Impact",
            "10x improvement in build times.",
            "## Next Steps After Migration",
            "Run the full test suite.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Project brief with only weak markers should NOT trigger"
        );
    }

    #[test]
    fn test_single_strong_marker_no_diagnostic() {
        let result = run_check(&["# Session Progress", "Some content about the project."]);
        assert!(
            result.diagnostics.is_empty(),
            "A single strong marker should not trigger"
        );
    }

    #[test]
    fn test_single_weak_marker_no_diagnostic() {
        let result = run_check(&[
            "# Instructions",
            "## Current Status",
            "The project uses Rust.",
            "Always run tests before committing.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "A single weak marker should not trigger"
        );
    }

    #[test]
    fn test_one_strong_plus_many_weak_no_diagnostic() {
        // Only 1 strong marker — even with many weak ones, should NOT fire
        let result = run_check(&[
            "## What We Accomplished",
            "## Current Status",
            "## Expected Impact",
            "## Key Decisions Made",
            "## PR Status",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "1 strong + 4 weak should NOT trigger (requires 2+ strong)"
        );
    }

    #[test]
    fn test_checkmarks_plus_weak_markers_no_diagnostic() {
        // Checkmarks are weak, so even 8+ checkmarks + 2 weak markers = no strong
        let result = run_check(&[
            "# Feature Checklist",
            "- ✅ Auth implemented",
            "- ✅ Database set up",
            "- ✅ API endpoints done",
            "- ✅ Tests written",
            "- ✅ CI configured",
            "- ✅ Docs updated",
            "- ✅ Security reviewed",
            "- ✅ Performance tested",
            "## Current Status",
            "Ready for launch.",
            "## Expected Impact",
            "Faster onboarding.",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Checkmarks (weak) + 2 weak markers with 0 strong should NOT trigger"
        );
    }

    #[test]
    fn test_seven_checkmarks_below_threshold() {
        // 7 checkmarks = below 8 threshold, so no checkmark density marker
        let result = run_check(&[
            "# What We Accomplished",
            "# Session Summary",
            "- ✅ Task 1",
            "- ✅ Task 2",
            "- ✅ Task 3",
            "- ✅ Task 4",
            "- ✅ Task 5",
            "- ✅ Task 6",
            "- ✅ Task 7",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "2 strong markers + file changelog = fires even without checkmark density"
        );
    }

    // ── Edge cases ───────────────────────────────────────────────────────

    #[test]
    fn test_in_code_block_not_counted() {
        let result = run_check(&[
            "# Instructions",
            "```",
            "## Session Progress",
            "## What We Accomplished",
            "## Files Modified This Session",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Markers inside code blocks should not be counted"
        );
    }

    #[test]
    fn test_two_strong_markers_alone_no_diagnostic() {
        // 2 strong but total < 3
        let result = run_check(&["# Session Progress", "## What We Accomplished"]);
        assert!(
            result.diagnostics.is_empty(),
            "2 strong markers but total=2 (below 3) should NOT trigger"
        );
    }

    #[test]
    fn test_files_modified_without_session_context_is_weak() {
        // "Files Modified" (bare) is weak, "Files Modified This Session" is strong
        let result = run_check(&[
            "## Files Modified",
            "## Current Status",
            "## Expected Impact",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Bare 'Files Modified' (weak) + 2 weak = 0 strong, should NOT trigger"
        );
    }

    #[test]
    fn test_files_modified_this_session_is_strong() {
        let result = run_check(&[
            "## What We Accomplished",
            "## Files Modified This Session",
            "## Current Status",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "'Files Modified This Session' is strong, + 1 strong + 1 weak = fires"
        );
    }
}
