use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::{inside_inline_code, ScopeFilter, LIST_MARKER};
use super::Checker;

const TOOL_NAMES: &str = "\
    cargo|npm|npx|yarn|pnpm|bun|pip|pip3|python|python3|\
    go|ruby|gem|bundle|make|cmake|\
    git|gh|docker|docker-compose|podman|kubectl|helm|terraform|\
    curl|wget|brew|apt|apt-get|yum|dnf|pacman|\
    rustup|rustc|node|deno|mvn|gradle|dotnet|mix|\
    swift|xcodebuild|pod|flutter|dart";

static COMMAND_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?x) ^ (?:\$\s+)? (?:{TOOL_NAMES}) \s+\S+")).unwrap());

static PROSE_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^(?:you\s+|if\s+|when\s+|always\s+|make\s+sure|use\s+|the\s+|note|for\s+)|should\s+|before\s+|after\s+|instead\s+|rather\s+|need\s+to\s+|\.\s+[A-Z])")
        .unwrap()
});

static TOOL_PROSE_VERB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)^(?:\$\s+)?(?:{TOOL_NAMES})\s+(?:is|are|was|were|has|have|had|does|do|did|can|could|will|would|shall|should|may|might|manages?|handles?|provides?|supports?|allows?|enables?|requires?|includes?|contains?|works?|helps?|offers?|serves?|stores?)\b"
    ))
    .unwrap()
});

pub(crate) struct CommandWithoutCodeblockChecker {
    scope: ScopeFilter,
}

impl CommandWithoutCodeblockChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for CommandWithoutCodeblockChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "command-without-codeblock",
            description: "Flags bare shell commands not in code blocks",
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

            for (i, line) in non_code_lines_masked(&file.raw_lines, &file.in_code_block) {
                let line_num = i + 1;

                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('|') {
                    continue;
                }

                // Strip list markers
                let marker_len = LIST_MARKER.find(trimmed).map_or(0, |m| m.end());
                let text = trimmed[marker_len..].trim();

                if text.is_empty() {
                    continue;
                }

                // Skip tool-name + prose-verb lines ("npm manages dependencies")
                if TOOL_PROSE_VERB.is_match(text) {
                    continue;
                }

                if let Some(m) = COMMAND_PATTERN.find(text) {
                    // Offset of `text` within `line` via pointer arithmetic
                    let text_offset = text.as_ptr() as usize - line.as_ptr() as usize;
                    let absolute_pos = text_offset + m.start();
                    if inside_inline_code(line, absolute_pos) {
                        continue;
                    }

                    if PROSE_SIGNAL.is_match(text) {
                        continue;
                    }

                    if text.split_whitespace().count() > 8 {
                        continue;
                    }

                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Info,
                        Category::CommandWithoutCodeblock,
                        suggest: "Wrap the command in a fenced code block (```) or inline backticks for copy-paste clarity",
                        "bare command outside a code block: `{:.60}`",
                        text
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

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        CommandWithoutCodeblockChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_bare_cargo_test_flagged() {
        let result = check(&["cargo test --release"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].category,
            Category::CommandWithoutCodeblock
        );
    }

    #[test]
    fn test_bare_npm_install_flagged() {
        let result = check(&["npm install express"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_bare_docker_compose_flagged() {
        let result = check(&["docker-compose up -d"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_bare_git_command_flagged() {
        let result = check(&["git push origin main"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_dollar_prompt_flagged() {
        let result = check(&["$ cargo build --release"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_in_list_item_flagged() {
        let result = check(&["- npm run build"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_numbered_list_flagged() {
        let result = check(&["1. pip install -r requirements.txt"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_code_block_not_flagged() {
        let result = check(&["```", "cargo test --release", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_inline_backticks_not_flagged() {
        let result = check(&["`cargo test --release`"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_prose_with_command_not_flagged() {
        let result =
            check(&["You should run cargo test before committing any changes to the repo"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_prose_use_not_flagged() {
        let result = check(&["Use npm install to add dependencies to your project"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_long_sentence_not_flagged() {
        let result = check(&["Make sure to run cargo test after making changes to any of the checker modules in the project"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_normal_text_not_flagged() {
        let result = check(&["Always verify your changes before pushing."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_heading_not_flagged() {
        let result = check(&["## cargo test"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_kubectl_flagged() {
        let result = check(&["kubectl get pods -n production"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_make_command_flagged() {
        let result = check(&["make build"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_brew_install_flagged() {
        let result = check(&["brew install spectralint"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_table_row_not_flagged() {
        let result = check(&["| cargo test | Run all tests |"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_single_word_not_flagged() {
        // "cargo" alone without a subcommand should not flag
        let result = check(&["cargo"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_backtick_command_in_prose_not_flagged() {
        let result = check(&["Run `cargo test` to verify."]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_tool_prose_verb_not_flagged() {
        let result = check(&["- npm manages JavaScript dependencies"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_docker_provides_not_flagged() {
        let result = check(&["docker provides container isolation"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_cargo_is_not_flagged() {
        let result = check(&["cargo is the Rust package manager"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_bare_commands() {
        let result = check(&["cargo build --release", "Some normal text", "npm run test"]);
        assert_eq!(result.diagnostics.len(), 2);
    }
}
