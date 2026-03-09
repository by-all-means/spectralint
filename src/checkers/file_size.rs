use crate::config::FileSizeConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::Checker;

pub(crate) struct FileSizeChecker {
    warn_lines: usize,
    max_lines: usize,
    strict: bool,
}

impl FileSizeChecker {
    pub(crate) fn new(config: &FileSizeConfig, strict: bool) -> Self {
        Self {
            warn_lines: config.warn_lines,
            max_lines: config.max_lines,
            strict,
        }
    }
}

impl Checker for FileSizeChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "file-size",
            description: "Warns when instruction files exceed recommended length",
            default_severity: Severity::Warning,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            let line_count = file.raw_lines.len();

            if line_count >= self.max_lines {
                let severity = if self.strict {
                    Severity::Warning
                } else {
                    Severity::Info
                };
                emit!(
                    result,
                    file.path,
                    1,
                    severity,
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
        run_check_with_lines_strict(line_count, false)
    }

    fn run_check_with_lines_strict(line_count: usize, strict: bool) -> CheckResult {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let lines: Vec<String> = (0..line_count).map(|i| format!("Line {i}")).collect();
        let file = ParsedFile {
            path: std::sync::Arc::new(root.join("CLAUDE.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines,
            in_code_block: vec![],
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        let config = FileSizeConfig::default();
        FileSizeChecker::new(&config, strict).check(&ctx)
    }

    #[test]
    fn test_small_file_no_diagnostic() {
        let result = run_check_with_lines(100);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_warn_threshold_info() {
        let result = run_check_with_lines(500);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn test_below_warn_threshold_no_diagnostic() {
        let result = run_check_with_lines(499);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_max_threshold_info_in_default_mode() {
        let result = run_check_with_lines(750);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn test_above_max_info_in_default_mode() {
        let result = run_check_with_lines(800);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn test_max_threshold_warning_in_strict_mode() {
        let result = run_check_with_lines_strict(750, true);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    }
}
