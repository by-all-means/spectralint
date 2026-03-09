use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

/// Returns true if the line looks like a shell command (not a comment or output).
fn is_command_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('#')
        && !trimmed.starts_with("//")
        && !trimmed.starts_with("<!--")
}

/// Returns true if the line starts with a shell prompt character.
fn has_prompt_prefix(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("$ ") || trimmed.starts_with("> ")
}

pub(crate) struct InconsistentCommandPrefixChecker {
    scope: ScopeFilter,
}

impl InconsistentCommandPrefixChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for InconsistentCommandPrefixChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "inconsistent-command-prefix",
            description: "Flags mixed $ prefix styles in shell code blocks",
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

            let lines = &file.raw_lines;
            let mut i = 0;

            while i < lines.len() {
                let trimmed = lines[i].trim_start();
                let is_shell_block = trimmed.starts_with("```bash")
                    || trimmed.starts_with("```sh")
                    || trimmed.starts_with("```shell")
                    || trimmed.starts_with("```zsh")
                    || trimmed == "```"; // untagged blocks often contain commands

                if !is_shell_block {
                    i += 1;
                    continue;
                }

                let fence_line = i + 1; // 1-indexed
                i += 1;

                // Collect command lines within this block
                let mut with_prefix = 0usize;
                let mut without_prefix = 0usize;
                let mut command_lines = Vec::new();

                while i < lines.len() {
                    let t = lines[i].trim_start();
                    if t.starts_with("```") {
                        break;
                    }

                    if is_command_line(&lines[i]) {
                        if has_prompt_prefix(&lines[i]) {
                            with_prefix += 1;
                        } else {
                            without_prefix += 1;
                        }
                        command_lines.push(i);
                    }
                    i += 1;
                }

                // Flag if the block has a mix of prefixed and unprefixed commands
                // Require at least 2 command lines and at least 1 of each style
                if command_lines.len() >= 2 && with_prefix > 0 && without_prefix > 0 {
                    emit!(
                        result,
                        file.path,
                        fence_line,
                        Severity::Info,
                        Category::InconsistentCommandPrefix,
                        suggest: "Use a consistent style: either all commands with $ prefix or none",
                        "inconsistent command prefix — {with_prefix} lines with $ and {without_prefix} without"
                    );
                }

                i += 1;
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
        InconsistentCommandPrefixChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_mixed_prefix_flagged() {
        let result = check(&["```bash", "$ npm install", "npm start", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].category,
            Category::InconsistentCommandPrefix
        );
    }

    #[test]
    fn test_all_prefixed_not_flagged() {
        let result = check(&["```bash", "$ npm install", "$ npm start", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_no_prefix_not_flagged() {
        let result = check(&["```bash", "npm install", "npm start", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_single_command_not_flagged() {
        let result = check(&["```bash", "$ npm install", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_comments_ignored() {
        let result = check(&[
            "```bash",
            "# Install deps",
            "$ npm install",
            "npm start",
            "```",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_non_shell_block_not_flagged() {
        let result = check(&["```python", "$ npm install", "npm start", "```"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_untagged_block_checked() {
        let result = check(&["```", "$ npm install", "npm start", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_greater_than_prefix() {
        let result = check(&["```sh", "> npm install", "npm start", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
