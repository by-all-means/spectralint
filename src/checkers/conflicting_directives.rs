use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct ConflictingDirectivesChecker {
    scope: ScopeFilter,
}

impl ConflictingDirectivesChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

struct ConflictPair {
    a: Regex,
    b: Regex,
    description: &'static str,
}

static CONFLICT_PAIRS: LazyLock<Vec<ConflictPair>> = LazyLock::new(|| {
    vec![
        // Tone
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+use\s+formal|formal\s+tone|be\s+formal)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:keep\s+it\s+casual|casual\s+tone|be\s+casual|conversational\s+tone)\b").unwrap(),
            description: "tone: formal vs casual",
        },
        // API usage
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:never\s+use\s+external\s+APIs?|no\s+external\s+(?:API|service)\s+calls?|do\s+not\s+(?:call|use)\s+external)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:fetch\s+from\s+the\s+API|call\s+the\s+(?:external\s+)?API|use\s+the\s+(?:external\s+)?API)\b").unwrap(),
            description: "API usage: no external APIs vs use the API",
        },
        // File creation
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:never\s+create\s+(?:new\s+)?files?|do\s+not\s+create\s+(?:new\s+)?files?|don'?t\s+create\s+(?:new\s+)?files?)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:create\s+(?:new\s+)?files?\s+(?:as\s+needed|when\s+needed|freely)|feel\s+free\s+to\s+create)\b").unwrap(),
            description: "file creation: never create files vs create files freely",
        },
        // Confirmation behavior
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+ask\s+(?:for\s+)?confirm|require\s+(?:user\s+)?confirm|ask\s+before\s+(?:every|each|any))\w*\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:never\s+ask\s+(?:for\s+)?confirm|don'?t\s+ask\s+(?:for\s+)?confirm|proceed\s+without\s+(?:asking|confirm)|skip\s+confirm)\w*\b").unwrap(),
            description: "confirmation: always ask vs never ask",
        },
        // Verbosity
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:be\s+(?:as\s+)?(?:brief|concise|terse)|keep\s+(?:responses?\s+)?short|minimal\s+(?:output|response))\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:be\s+(?:very\s+)?(?:detailed|verbose|thorough)|provide\s+(?:detailed|comprehensive|extensive)\s+(?:explanations?|responses?))\b").unwrap(),
            description: "verbosity: be concise vs be detailed",
        },
        // Resource modification
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:never\s+(?:modify|edit|change)\s+(?:existing|production)|read[\s-]only\s+(?:mode|access)|do\s+not\s+(?:modify|change)\s+existing)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:freely\s+(?:modify|edit|update)|modify\s+(?:any|all)\s+files?|full\s+write\s+access)\b").unwrap(),
            description: "resource modification: read-only vs full write access",
        },
    ]
});

impl Checker for ConflictingDirectivesChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let directive_lines: Vec<(usize, &str)> = non_code_lines(&file.raw_lines).collect();

            for pair in CONFLICT_PAIRS.iter() {
                let mut a_match: Option<usize> = None;
                let mut b_match: Option<usize> = None;

                for &(i, line) in &directive_lines {
                    if a_match.is_none() && pair.a.is_match(line) {
                        a_match = Some(i + 1);
                    }
                    if b_match.is_none() && pair.b.is_match(line) {
                        b_match = Some(i + 1);
                    }
                    if a_match.is_some() && b_match.is_some() {
                        break;
                    }
                }

                if let (Some(line_a), Some(line_b)) = (a_match, b_match) {
                    let report_line = line_a.min(line_b);
                    emit!(
                        result,
                        file.path,
                        report_line,
                        Severity::Warning,
                        Category::ConflictingDirectives,
                        suggest: "Remove or reconcile one of the conflicting directives",
                        "Conflicting directives ({}) at lines {} and {}",
                        pair.description,
                        line_a,
                        line_b
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
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        ConflictingDirectivesChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_tone_conflict_detected() {
        let result = run_check(&[
            "Always use formal tone when responding.",
            "Keep it casual and friendly.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(
            result.diagnostics[0].category,
            Category::ConflictingDirectives
        );
        assert!(result.diagnostics[0].message.contains("tone"));
    }

    #[test]
    fn test_api_conflict_detected() {
        let result = run_check(&[
            "Never use external APIs for data retrieval.",
            "Fetch from the API to get the latest data.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("API"));
    }

    #[test]
    fn test_confirmation_conflict_detected() {
        let result = run_check(&[
            "Always ask for confirmation before destructive actions.",
            "Don't ask for confirmation, just proceed.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("confirmation"));
    }

    #[test]
    fn test_verbosity_conflict_detected() {
        let result = run_check(&[
            "Be brief and concise in all responses.",
            "Provide detailed explanations for every change.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("verbosity"));
    }

    #[test]
    fn test_no_conflict_clean_file() {
        let result = run_check(&[
            "Always use formal tone.",
            "Run tests before committing.",
            "Never skip CI.",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_code_block_lines_excluded() {
        let result = run_check(&[
            "Always use formal tone.",
            "```",
            "Keep it casual and friendly.",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Lines in code blocks should not trigger conflicts"
        );
    }

    #[test]
    fn test_multiple_conflicts_detected() {
        let result = run_check(&[
            "Always use formal tone.",
            "Keep it casual.",
            "Be brief and concise.",
            "Provide detailed explanations.",
        ]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_file_creation_conflict() {
        let result = run_check(&["Never create new files.", "Create files as needed."]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("file creation"));
    }

    #[test]
    fn test_resource_modification_conflict() {
        let result = run_check(&[
            "Never modify existing files.",
            "Full write access to all files.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0]
            .message
            .contains("resource modification"));
    }
}
