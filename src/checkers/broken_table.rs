use regex::Regex;
use std::sync::LazyLock;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct BrokenTableChecker {
    scope: ScopeFilter,
}

impl BrokenTableChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// A separator row contains only `|`, `-`, `:`, and whitespace.
static SEPARATOR_ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\|[\s|:\-]+\|\s*$").unwrap());

/// Count the number of columns in a table row based on pipe delimiters.
fn column_count(line: &str) -> usize {
    let trimmed = line.trim();
    let pipes = trimmed.matches('|').count();
    if pipes == 0 {
        return 0;
    }
    if trimmed.starts_with('|') && trimmed.ends_with('|') {
        pipes - 1
    } else if trimmed.starts_with('|') || trimmed.ends_with('|') {
        pipes
    } else {
        pipes + 1
    }
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.len() > 1
}

impl Checker for BrokenTableChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let mut table_region: Vec<(usize, &str)> = Vec::new();

            let check_region = |result: &mut CheckResult, region: &[(usize, &str)]| {
                if region.len() < 2 {
                    return;
                }

                let header_line = region[0].1;
                let header_cols = column_count(header_line);

                if !SEPARATOR_ROW.is_match(region[1].1) {
                    emit!(
                        result,
                        file.path,
                        region[0].0 + 1,
                        Severity::Warning,
                        Category::BrokenTable,
                        suggest: "Fix the table formatting — agents may misread data from malformed tables",
                        "Table is missing a separator row (e.g., |---|---|)"
                    );
                    return;
                }

                for &(line_idx, line) in &region[2..] {
                    if SEPARATOR_ROW.is_match(line) {
                        continue;
                    }
                    let cols = column_count(line);
                    if cols != header_cols {
                        emit!(
                            result,
                            file.path,
                            line_idx + 1,
                            Severity::Warning,
                            Category::BrokenTable,
                            suggest: "Fix the table formatting — agents may misread data from malformed tables",
                            "Table row has {} columns but header has {}",
                            cols,
                            header_cols
                        );
                    }
                }
            };

            for (idx, line) in file.raw_lines.iter().enumerate() {
                if file.is_code(idx) {
                    if !table_region.is_empty() {
                        check_region(&mut result, &table_region);
                        table_region.clear();
                    }
                    continue;
                }

                if is_table_line(line) {
                    table_region.push((idx, line));
                } else if !table_region.is_empty() {
                    check_region(&mut result, &table_region);
                    table_region.clear();
                }
            }

            if !table_region.is_empty() {
                check_region(&mut result, &table_region);
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
        BrokenTableChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_well_formed_table_no_flag() {
        let result = run_check(&[
            "| Name | Value |",
            "|------|-------|",
            "| foo  | 1     |",
            "| bar  | 2     |",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_column_count_mismatch() {
        let result = run_check(&[
            "| Name | Value |",
            "|------|-------|",
            "| foo  | 1     | extra |",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("3 columns"));
        assert!(result.diagnostics[0].message.contains("header has 2"));
    }

    #[test]
    fn test_missing_separator_row() {
        let result = run_check(&["| Name | Value |", "| foo  | 1     |", "| bar  | 2     |"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("separator"));
    }

    #[test]
    fn test_table_inside_code_block_no_flag() {
        let result = run_check(&["```", "| Name | Value |", "| foo  | 1     |", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_single_pipe_line_no_flag() {
        let result = run_check(&["| just one line"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_empty_file_no_flag() {
        let result = run_check(&[]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_alignment_colons_valid() {
        let result = run_check(&[
            "| Left | Center | Right |",
            "|:-----|:------:|------:|",
            "| a    | b      | c     |",
        ]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_multiple_tables_one_broken() {
        let result = run_check(&[
            "| A | B |",
            "|---|---|",
            "| 1 | 2 |",
            "",
            "| X | Y |",
            "| 1 | 2 |",
        ]);
        // First table is fine, second is missing separator
        assert_eq!(result.diagnostics.len(), 1);
    }
}
