use crate::config::FileSizeConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::Checker;

pub struct FileSizeChecker {
    warn_lines: usize,
    max_lines: usize,
}

impl FileSizeChecker {
    pub fn new(config: &FileSizeConfig) -> Self {
        Self {
            warn_lines: config.warn_lines,
            max_lines: config.max_lines,
        }
    }
}

impl Checker for FileSizeChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            let line_count = file.raw_lines.len();

            if line_count >= self.max_lines {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Warning,
                    Category::FileSize,
                    suggest: "Split into focused sub-files and use file references for progressive disclosure",
                    "File has {} lines (exceeds {} line limit). Large instruction files cause \
                     LLM \"lost in the middle\" degradation.",
                    line_count,
                    self.max_lines
                );
            } else if line_count >= self.warn_lines {
                emit!(
                    result,
                    file.path,
                    1,
                    Severity::Info,
                    Category::FileSize,
                    suggest: "Split into focused sub-files and use file references for progressive disclosure",
                    "File has {} lines (approaching {} line limit). Consider splitting \
                     to avoid LLM context degradation.",
                    line_count,
                    self.max_lines
                );
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::ParsedFile;
    use std::collections::HashSet;

    fn run_check_with_lines(line_count: usize) -> CheckResult {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let lines: Vec<String> = (0..line_count).map(|i| format!("Line {i}")).collect();
        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines,
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };
        let config = FileSizeConfig::default();
        FileSizeChecker::new(&config).check(&ctx)
    }

    #[test]
    fn test_small_file_no_diagnostic() {
        let result = run_check_with_lines(100);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_warn_threshold_info() {
        let result = run_check_with_lines(300);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn test_max_threshold_warning() {
        let result = run_check_with_lines(500);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_above_max_warning() {
        let result = run_check_with_lines(600);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn test_just_below_warn_no_diagnostic() {
        let result = run_check_with_lines(299);
        assert!(result.diagnostics.is_empty());
    }
}
