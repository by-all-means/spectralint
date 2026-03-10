use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct StaleFileTreeChecker {
    scope: ScopeFilter,
}

impl StaleFileTreeChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Matches Unicode box-drawing tree prefixes: ├── , └── , │   , etc.
static TREE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*(?:[├└│─|+\-\s])*(?:[├└][\s─]*|[|+][\s\-]*))\s*(.+?)\s*$").unwrap()
});

/// Root line of a tree: a bare word ending with /
static TREE_ROOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\S+/)\s*$").unwrap());

/// Lines that indicate the tree is illustrative/example.
static EXAMPLE_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:example|e\.g\.|such as|for instance|for example|like|sample|suggested|recommended|proposed|planned|will have|should look)\b").unwrap()
});

/// Creation verb context — the structure is being created, not asserted.
static CREATION_VERB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:create[sd]?|generat(?:e[sd]?|ing)|add(?:ed|ing)?|build(?:ing)?|scaffold)\b",
    )
    .unwrap()
});

/// Returns true if a tree line looks like a tree drawing character (not a filename).
fn is_tree_char_line(name: &str) -> bool {
    name.trim()
        .chars()
        .all(|c| matches!(c, '─' | '│' | '├' | '└' | '-' | '|' | '+' | ' '))
}

/// Returns true if the name contains placeholder patterns.
fn is_placeholder(name: &str) -> bool {
    name.contains("...")
        || name.contains("<")
        || name.contains("{")
        || name.to_lowercase().contains("xxx")
}

/// Parse a code block into (depth, name) entries for tree lines.
/// Returns None if the block doesn't look like a directory tree.
fn parse_tree_block(lines: &[&str]) -> Option<Vec<(usize, String, bool)>> {
    let mut entries: Vec<(usize, String, bool)> = Vec::new();
    let mut tree_lines = 0;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check for root line (e.g., "src/")
        if entries.is_empty() {
            if let Some(caps) = TREE_ROOT.captures(trimmed) {
                let root = caps[1].trim_end_matches('/');
                entries.push((0, root.to_string(), true));
                tree_lines += 1;
                continue;
            }
        }

        // Check for tree-structured line
        if let Some(caps) = TREE_LINE.captures(line) {
            let prefix = &caps[1];
            let name = caps[2].trim().to_string();

            // Skip pure decoration lines
            if is_tree_char_line(&name) {
                continue;
            }

            // Strip trailing comment like "# main entry point"
            let name = name
                .split_once('#')
                .or_else(|| name.split_once("//"))
                .map(|(n, _)| n.trim().to_string())
                .unwrap_or(name);

            if name.is_empty() {
                continue;
            }

            // Calculate depth from prefix width (each tree level is ~4 chars)
            let depth = (prefix.len() + 2) / 4; // rough estimation
            let is_dir = name.ends_with('/');
            let clean_name = name.trim_end_matches('/').to_string();

            entries.push((depth, clean_name, is_dir));
            tree_lines += 1;
        }
    }

    // Need at least 2 tree-structured lines to be a tree
    if tree_lines >= 2 {
        Some(entries)
    } else {
        None
    }
}

/// Convert parsed tree entries into relative file paths.
fn build_paths(entries: &[(usize, String, bool)]) -> Vec<(String, bool)> {
    let mut paths: Vec<(String, bool)> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();

    for (depth, name, is_dir) in entries {
        // Pop stack to parent level
        while stack.last().is_some_and(|(d, _)| *d >= *depth) {
            stack.pop();
        }
        stack.push((*depth, name.clone()));

        let full_path: String = stack
            .iter()
            .map(|(_, n)| n.as_str())
            .collect::<Vec<_>>()
            .join("/");
        paths.push((full_path, *is_dir));
    }

    paths
}

impl Checker for StaleFileTreeChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "stale-file-tree",
            description: "Flags ASCII directory trees with non-existent paths",
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

            // Find code blocks and check for tree structures
            let mut i = 0;
            let lines = &file.raw_lines;
            let mut code_block_start_line;

            while i < lines.len() {
                let trimmed = lines[i].trim();

                // Look for code block start
                if trimmed.starts_with("```") && (i == 0 || !file.is_code(i - 1)) {
                    code_block_start_line = i;
                    let block_start = i + 1;
                    i += 1;

                    // Find end of code block
                    while i < lines.len() && !lines[i].trim().starts_with("```") {
                        i += 1;
                    }
                    let block_end = i;

                    if block_start >= block_end {
                        i += 1;
                        continue;
                    }

                    // Check context line before code block for example/creation verbs
                    let context_line = if code_block_start_line > 0 {
                        &lines[code_block_start_line - 1]
                    } else {
                        ""
                    };
                    if EXAMPLE_CONTEXT.is_match(context_line)
                        || CREATION_VERB.is_match(context_line)
                    {
                        i += 1;
                        continue;
                    }

                    let block_lines: Vec<&str> = lines[block_start..block_end]
                        .iter()
                        .map(|s| s.as_str())
                        .collect();

                    // Check if any line contains ellipsis (incomplete tree)
                    if block_lines.iter().any(|l| l.contains("...")) {
                        i += 1;
                        continue;
                    }

                    if let Some(entries) = parse_tree_block(&block_lines) {
                        let paths = build_paths(&entries);

                        // Build set of parent paths (directories that have children)
                        let parent_paths: std::collections::HashSet<&str> = paths
                            .iter()
                            .filter_map(|(p, _)| p.rsplit_once('/').map(|(parent, _)| parent))
                            .collect();

                        // Only flag leaf entries (files) — skip intermediate directories
                        for (path, is_dir) in &paths {
                            // Skip directories that are parents of other entries
                            if *is_dir && parent_paths.contains(path.as_str()) {
                                continue;
                            }

                            if is_placeholder(path) {
                                continue;
                            }

                            // Check root-relative
                            let full_path = ctx.project_root.join(path);
                            if *is_dir {
                                if full_path.is_dir() {
                                    continue;
                                }
                            } else if full_path.exists() {
                                continue;
                            }

                            // Check if basename exists in filename index (for files)
                            if !*is_dir {
                                let basename = path.rsplit('/').next().unwrap_or(path);
                                if ctx.filename_index.contains(basename) {
                                    continue;
                                }
                            }

                            emit!(
                                result,
                                file.path,
                                code_block_start_line + 1,
                                Severity::Warning,
                                Category::StaleFileTree,
                                suggest: "Update the directory tree to match the current project structure",
                                "Path in directory tree does not exist: `{}`",
                                path
                            );
                        }
                    }

                    i += 1;
                    continue;
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

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        StaleFileTreeChecker::new(&[]).check(&ctx)
    }

    fn run_check_with_files(lines: &[&str], files: &[&str]) -> CheckResult {
        let (dir, ctx) = single_file_ctx(lines);
        for f in files {
            let path = dir.path().join(f);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, "").unwrap();
        }
        StaleFileTreeChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_missing_file_in_tree() {
        let result = run_check(&["```", "src/", "├── main.rs", "└── lib.rs", "```"]);
        assert!(
            result.diagnostics.len() >= 1,
            "Should flag missing files in tree: got {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_existing_files_no_flag() {
        let result = run_check_with_files(
            &["```", "src/", "├── main.rs", "└── lib.rs", "```"],
            &["src/main.rs", "src/lib.rs"],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Should not flag when files exist"
        );
    }

    #[test]
    fn test_example_context_no_flag() {
        let result = run_check(&[
            "For example, a typical structure:",
            "```",
            "src/",
            "├── main.rs",
            "└── lib.rs",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Should skip trees preceded by example context"
        );
    }

    #[test]
    fn test_creation_context_no_flag() {
        let result = run_check(&[
            "Create the following structure:",
            "```",
            "src/",
            "├── main.rs",
            "└── lib.rs",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Should skip trees preceded by creation verbs"
        );
    }

    #[test]
    fn test_ellipsis_no_flag() {
        let result = run_check(&["```", "src/", "├── main.rs", "├── ...", "└── lib.rs", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Should skip trees with ellipsis (incomplete)"
        );
    }

    #[test]
    fn test_non_tree_code_block_no_flag() {
        let result = run_check(&["```bash", "cargo build", "cargo test", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Non-tree code blocks should not be flagged"
        );
    }

    #[test]
    fn test_placeholder_path_no_flag() {
        let result = run_check(&[
            "```",
            "src/",
            "├── <module>/",
            "│   └── handler.rs",
            "└── main.rs",
            "```",
        ]);
        // The <module> path should be skipped as placeholder
        let flagged: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("<module>"))
            .collect();
        assert!(
            flagged.is_empty(),
            "Placeholder paths should not be flagged"
        );
    }

    #[test]
    fn test_single_line_not_tree() {
        let result = run_check(&["```", "src/main.rs", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Single-line code blocks are not trees"
        );
    }
}
