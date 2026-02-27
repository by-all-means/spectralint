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
            b: Regex::new(r"(?i)\b(?:keep\s+it\s+casual|casual\s+tone|be\s+casual|conversational\s+tone|informal\s+tone|be\s+informal)\b").unwrap(),
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
            a: Regex::new(r"(?i)\b(?:be\s+(?:as\s+)?(?:brief|concise|short|terse|succinct)|keep\s+(?:responses?\s+)?(?:short|concise|brief)|minimal\s+(?:output|response))\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:be\s+(?:very\s+)?(?:detailed|verbose|thorough|comprehensive|elaborate)|provide\s+(?:detailed|comprehensive|extensive|thorough)\s+(?:explanations?|responses?))\b").unwrap(),
            description: "verbosity: be concise vs be detailed",
        },
        // Resource modification
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:never\s+(?:modify|edit|change)\s+(?:existing|production)|read[\s-]only\s+(?:mode|access)|do\s+not\s+(?:modify|change)\s+existing)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:freely\s+(?:modify|edit|update)|modify\s+(?:any|all)\s+files?|full\s+write\s+access)\b").unwrap(),
            description: "resource modification: read-only vs full write access",
        },
        // Testing
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+write\s+tests?|must\s+(?:include|write|add)\s+tests?|require\s+tests?)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:skip\s+tests?|no\s+tests?\s+needed|don'?t\s+(?:write|add)\s+tests?|tests?\s+are\s+not\s+(?:needed|required|necessary))\b").unwrap(),
            description: "testing: always write tests vs skip tests",
        },
        // Comments
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:comment\s+everything|document\s+everything|add\s+comments?\s+to\s+(?:every|all)|always\s+(?:add|include)\s+comments?)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:no\s+comments?|avoid\s+comments?|code\s+should\s+be\s+self[- ]documenting|self[- ]documenting\s+code|don'?t\s+(?:add|write)\s+comments?)\b").unwrap(),
            description: "comments: comment everything vs self-documenting code",
        },
        // Dependencies
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:minimize\s+dependencies|fewer\s+dependencies|avoid\s+(?:external\s+)?dependencies|reduce\s+dependencies)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:use\s+(?:existing\s+)?libraries|don'?t\s+reinvent|prefer\s+(?:existing\s+)?(?:libraries|packages)|leverage\s+(?:existing\s+)?(?:libraries|packages))\b").unwrap(),
            description: "dependencies: minimize dependencies vs use libraries",
        },
        // Error handling
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:fail\s+fast|crash\s+on\s+error|let\s+it\s+crash|panic\s+on\s+error)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:handle\s+(?:errors?\s+)?gracefully|never\s+(?:crash|panic)|recover\s+from\s+errors?|don'?t\s+(?:crash|panic))\b").unwrap(),
            description: "error handling: fail fast vs handle gracefully",
        },
        // Autonomy
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:ask\s+before|confirm\s+with\s+(?:the\s+)?user|check\s+with\s+(?:the\s+)?user|get\s+(?:user\s+)?approval)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:work\s+autonomously|don'?t\s+ask|act\s+independently|without\s+(?:asking|confirmation)|proceed\s+independently)\b").unwrap(),
            description: "autonomy: ask before acting vs work autonomously",
        },
        // Commits
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:small\s+commits?|atomic\s+commits?|frequent\s+commits?|commit\s+(?:each|every)\s+change)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:squash\s+(?:all\s+)?commits?|single\s+commit|one\s+(?:big\s+)?commit|combine\s+(?:all\s+)?commits?)\b").unwrap(),
            description: "commits: small/atomic commits vs squash into one",
        },
        // Complexity
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:keep\s+it\s+simple|KISS|simplicity\s+first|simple\s+(?:solutions?|code)|avoid\s+(?:over[- ]?engineering|complexity))\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:optimize\s+for\s+performance|maximize\s+(?:efficiency|performance)|performance\s+(?:is\s+)?(?:critical|paramount|top\s+priority))\b").unwrap(),
            description: "complexity: keep it simple vs optimize for performance",
        },
        // Git workflow
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+create\s+(?:a\s+)?(?:new\s+)?branch|work\s+on\s+(?:a\s+)?(?:feature\s+)?branch|never\s+commit\s+(?:directly\s+)?to\s+main)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:commit\s+directly\s+to\s+main|push\s+(?:directly\s+)?to\s+main|no\s+(?:feature\s+)?branch(?:es)?(?:\s+needed)?)\b").unwrap(),
            description: "git workflow: always branch vs commit to main",
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

    #[test]
    fn test_testing_conflict() {
        let result = run_check(&[
            "Always write tests for new code.",
            "Skip tests for trivial changes.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("testing"));
    }

    #[test]
    fn test_comments_conflict() {
        let result = run_check(&[
            "Comment everything thoroughly.",
            "Code should be self-documenting.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("comments"));
    }

    #[test]
    fn test_dependencies_conflict() {
        let result = run_check(&[
            "Minimize dependencies in the project.",
            "Don't reinvent the wheel, use existing libraries.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("dependencies"));
    }

    #[test]
    fn test_error_handling_conflict() {
        let result = run_check(&[
            "Fail fast on unexpected errors.",
            "Handle errors gracefully and recover.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("error handling"));
    }

    #[test]
    fn test_autonomy_conflict() {
        let result = run_check(&[
            "Ask before making any destructive changes.",
            "Work autonomously without interruptions.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("autonomy"));
    }

    #[test]
    fn test_commits_conflict() {
        let result = run_check(&[
            "Make small commits for each logical change.",
            "Squash commits before merging.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("commits"));
    }

    #[test]
    fn test_complexity_conflict() {
        let result = run_check(&[
            "Keep it simple; avoid over-engineering.",
            "Optimize for performance in all hot paths.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("complexity"));
    }

    #[test]
    fn test_git_workflow_conflict() {
        let result = run_check(&[
            "Always create a new branch for each feature.",
            "Commit directly to main for speed.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("git workflow"));
    }
}
