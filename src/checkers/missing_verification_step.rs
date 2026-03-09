use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

/// Workflow verbs that indicate the file describes procedural steps.
static WORKFLOW_VERB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:run|execute|build|deploy|install|configure|set\s+up|start|compile|generate|create|migrate|push|publish|release|launch|ship)\b").unwrap()
});

/// Verification signals — test commands, verification verbs, success phrases.
static VERIFY_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:verify|validate|test|assert|expect|confirm|check|ensure|lint|spec|pytest|jest|mocha|rspec|cargo\s+test|npm\s+test|go\s+test|make\s+test|dotnet\s+test|mix\s+test|gradle\s+test|mvn\s+test|flutter\s+test)\b").unwrap()
});

/// Success criteria phrases.
static SUCCESS_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:expected\s+output|should\s+(?:see|return|output|produce|display|show|pass)|success\s+criteria|looks?\s+like|tests?\s+pass|all\s+green|no\s+errors?)").unwrap()
});

/// Minimum workflow verbs to consider the file procedural enough to need verification.
const MIN_WORKFLOW_VERBS: usize = 5;

pub(crate) struct MissingVerificationStepChecker {
    scope: ScopeFilter,
}

impl MissingVerificationStepChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for MissingVerificationStepChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "missing-verification-step",
            description: "Flags files with workflow steps but no verification",
            default_severity: Severity::Info,
            strict_only: true,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Skip very small files
            if file.raw_lines.len() < 10 {
                continue;
            }

            let mut workflow_count = 0;
            let mut has_verification = false;

            for (i, line) in file.raw_lines.iter().enumerate() {
                let in_code = *file.in_code_block.get(i).unwrap_or(&false);

                if VERIFY_SIGNAL.is_match(line) || (!in_code && SUCCESS_PHRASE.is_match(line)) {
                    has_verification = true;
                    break;
                }

                if !in_code && WORKFLOW_VERB.is_match(line) {
                    workflow_count += 1;
                }
            }

            if workflow_count >= MIN_WORKFLOW_VERBS && !has_verification {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::MissingVerificationStep,
                    suggest: "Add a verification step: a test command, expected output, or success criteria",
                    "file has {} workflow directives but no verification or test step anywhere",
                    workflow_count
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

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        MissingVerificationStepChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_workflow_without_verification_flagged() {
        let result = check(&[
            "# Setup",
            "",
            "1. Run the install script.",
            "2. Execute the migration.",
            "3. Build the frontend.",
            "4. Deploy to staging.",
            "5. Configure the load balancer.",
            "6. Start the background workers.",
            "",
            "# Notes",
            "",
            "See the wiki for details.",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].category,
            Category::MissingVerificationStep
        );
    }

    #[test]
    fn test_workflow_with_verification_passes() {
        let result = check(&[
            "# Setup",
            "",
            "1. Run the install script.",
            "2. Execute the migration.",
            "3. Build the frontend.",
            "4. Deploy to staging.",
            "5. Configure the load balancer.",
            "6. Start the background workers.",
            "7. Verify everything is running.",
            "",
            "# Notes",
            "",
            "See the wiki for details.",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_workflow_with_test_command_passes() {
        let result = check(&[
            "# Setup",
            "",
            "1. Run the install script.",
            "2. Execute the migration.",
            "3. Build the frontend.",
            "4. Deploy to staging.",
            "5. Configure the load balancer.",
            "",
            "```bash",
            "cargo test",
            "```",
            "",
            "# Notes",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_workflow_with_success_phrase_passes() {
        let result = check(&[
            "# Setup",
            "",
            "1. Run the install script.",
            "2. Execute the migration.",
            "3. Build the frontend.",
            "4. Deploy to staging.",
            "5. Configure the load balancer.",
            "",
            "You should see a success message in the terminal.",
            "",
            "# Notes",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_few_workflow_verbs_not_flagged() {
        let result = check(&[
            "# Overview",
            "",
            "Run the build command.",
            "Deploy the app.",
            "",
            "# Architecture",
            "",
            "The system uses microservices.",
            "Each service handles a specific domain.",
            "Data flows through the event bus.",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_small_file_not_flagged() {
        let result = check(&[
            "# Build",
            "",
            "Run the build.",
            "Execute deploy.",
            "Install deps.",
            "Configure env.",
            "Start server.",
        ]);
        // Only 7 lines, below the 10-line minimum
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_ensure_counts_as_verification() {
        let result = check(&[
            "# Setup",
            "",
            "1. Run the install script.",
            "2. Execute the migration.",
            "3. Build the frontend.",
            "4. Deploy to staging.",
            "5. Configure the load balancer.",
            "",
            "Ensure all services are healthy.",
            "",
            "# Notes",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_tests_pass_phrase_counts() {
        let result = check(&[
            "# Setup",
            "",
            "1. Run the install script.",
            "2. Execute the migration.",
            "3. Build the frontend.",
            "4. Deploy to staging.",
            "5. Configure the load balancer.",
            "",
            "All tests should pass before merging.",
            "",
            "# Notes",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }
}
