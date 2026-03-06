use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{inside_inline_code, ScopeFilter};
use super::Checker;

pub(crate) struct AbsolutePathChecker {
    scope: ScopeFilter,
}

impl AbsolutePathChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// System prefixes that are legitimate cross-machine paths.
const SYSTEM_PREFIXES: &[&str] = &[
    "/etc/", "/usr/", "/dev/", "/tmp/", "/var/", "/proc/", "/sys/", "/opt/", "/bin/", "/sbin/",
    "/lib/",
];

static UNIX_PERSONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/(?:home|Users|root)/[a-zA-Z0-9_.-]+").unwrap());
static WINDOWS_PERSONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[A-Z]:\\(?:Users|home)\\[a-zA-Z0-9_.-]+").unwrap());

fn is_system_path(line: &str, match_start: usize) -> bool {
    let from_match = &line[match_start..];
    SYSTEM_PREFIXES
        .iter()
        .any(|prefix| from_match.starts_with(prefix))
}

/// Returns true if ALL `~/` on this line are inside URLs or markdown link text.
fn all_tildes_in_urls(line: &str) -> bool {
    for (i, _) in line.match_indices("~/") {
        let before_tilde = &line[..i];

        let open_brackets = before_tilde.matches('[').count();
        let close_brackets = before_tilde.matches(']').count();
        if open_brackets > close_brackets {
            continue;
        }

        if let Some(http_pos) = before_tilde
            .rfind("http://")
            .or_else(|| before_tilde.rfind("https://"))
        {
            if !before_tilde[http_pos..].contains(' ') {
                let url_portion = &line[http_pos..];
                if let Some(end_pos) = url_portion.find([' ', ')', ']']) {
                    if i < http_pos + end_pos {
                        continue;
                    }
                } else {
                    continue;
                }
            }
        }

        return false;
    }
    true
}

/// Detect a personal or tilde path on a single line. Returns a message fragment
/// if found, or `None` if the line is clean.
fn detect_personal_path(line: &str) -> Option<String> {
    if let Some(m) = UNIX_PERSONAL.find(line) {
        if !is_system_path(line, m.start()) && !inside_inline_code(line, m.start()) {
            return Some(format!("Hardcoded personal path: {}", m.as_str()));
        }
    }
    if let Some(m) = WINDOWS_PERSONAL.find(line) {
        if !inside_inline_code(line, m.start()) {
            return Some(format!("Hardcoded personal path: {}", m.as_str()));
        }
    }
    if line.contains("~/") && !all_tildes_in_urls(line) {
        // Check if all tilde occurrences are inside inline code or reference
        // well-known hidden config dirs (~/.config/, ~/.claude/, etc.)
        let all_benign = line
            .match_indices("~/")
            .all(|(i, _)| inside_inline_code(line, i) || line[i..].starts_with("~/."));
        if !all_benign {
            return Some("Tilde home path detected".to_string());
        }
    }
    None
}

impl Checker for AbsolutePathChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (idx, line) in file.non_code_lines() {
                if let Some(msg) = detect_personal_path(line) {
                    emit!(
                        result,
                        file.path,
                        idx + 1,
                        Severity::Warning,
                        Category::AbsolutePath,
                        suggest: "Replace with a relative path or environment variable",
                        "{}",
                        msg
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
        AbsolutePathChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_unix_home_path() {
        let result = run_check(&["Use /home/john/project/src for the source"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("/home/john"));
    }

    #[test]
    fn test_macos_users_path() {
        let result = run_check(&["Located at /Users/alice/dev/myapp"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("/Users/alice"));
    }

    #[test]
    fn test_windows_path() {
        let result = run_check(&[r"Open C:\Users\Bob\projects\"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains(r"C:\Users\"));
    }

    #[test]
    fn test_tilde_home_path() {
        let result = run_check(&["Save to ~/Documents/project"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Tilde home path"));
    }

    #[test]
    fn test_system_path_no_flag() {
        let result = run_check(&[
            "Edit /etc/hosts",
            "Use /usr/bin/env",
            "Write to /tmp/cache",
            "Check /var/log/syslog",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_clean_file_no_flag() {
        let result = run_check(&[
            "# Build Instructions",
            "Run `cargo build`",
            "Use relative paths like ./src/main.rs",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_path_in_code_block_skipped() {
        let result = run_check(&["```bash", "cd /home/john/project", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Paths inside code blocks should not be flagged"
        );
    }

    #[test]
    fn test_root_path_flags() {
        let result = run_check(&["Config at /root/config"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_url_with_tilde_no_flag() {
        let result = run_check(&[
            "Visit [npmjs.com/settings/~/tokens](https://www.npmjs.com/settings/~/tokens)",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Tilde inside a URL should not flag"
        );
    }

    #[test]
    fn test_tilde_not_in_url_still_flags() {
        let result = run_check(&["Save to ~/projects and check https://example.com"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_multiple_paths_one_line_single_diagnostic() {
        let result = run_check(&["Copy from /home/john/src to /Users/alice/dst"]);
        // First match (Unix) triggers, rest skipped due to early return
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_home_dir_without_username_no_flag() {
        // /home/ alone without a username following should not flag
        let result = run_check(&["Look in /home/ for users"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_tilde_hidden_config_dir_no_flag() {
        let result = run_check(&["Config stored in ~/.crystal/config.json"]);
        assert!(
            result.diagnostics.is_empty(),
            "Tilde paths to hidden config dirs (~/.something) should not flag"
        );
    }

    #[test]
    fn test_tilde_visible_dir_still_flags() {
        let result = run_check(&["Save to ~/Documents/project"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Tilde paths to visible dirs (~/Documents) should still flag"
        );
    }

    #[test]
    fn test_bare_tilde_no_flag() {
        let result = run_check(&["Use ~ for home"]);
        assert!(
            result.diagnostics.is_empty(),
            "Bare ~ without / should not flag"
        );
    }

    #[test]
    fn test_url_containing_users_path() {
        // /Users/ inside a URL is still flagged (it's a personal path pattern)
        let result = run_check(&["See https://example.com/Users/alice/profile"]);
        assert_eq!(result.diagnostics.len(), 1);
    }
}
