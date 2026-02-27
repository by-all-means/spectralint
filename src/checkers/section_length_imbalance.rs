use crate::config::SectionLengthImbalanceConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct SectionLengthImbalanceChecker {
    scope: ScopeFilter,
    min_section_lines: usize,
    imbalance_ratio: f64,
}

impl SectionLengthImbalanceChecker {
    pub fn new(config: &SectionLengthImbalanceConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            min_section_lines: config.min_section_lines,
            imbalance_ratio: config.imbalance_ratio,
        }
    }
}

/// Sections that are inherently long by nature and should be excluded.
fn is_toc_section(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower == "table of contents" || lower == "toc" || lower == "index" || lower == "contents"
}

/// Minimum median below which ratio comparisons become meaningless.
const MIN_MEDIAN_FLOOR: usize = 5;

impl Checker for SectionLengthImbalanceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            let h2_sections: Vec<_> = file
                .sections
                .iter()
                .filter(|s| s.level == 2 && !is_toc_section(&s.title))
                .collect();

            if h2_sections.len() < 3 {
                continue;
            }

            let line_counts: Vec<(usize, usize, &str)> = h2_sections
                .iter()
                .map(|s| {
                    let count = s.end_line.saturating_sub(s.line);
                    (s.line, count, s.title.as_str())
                })
                .collect();

            let mut sorted_counts: Vec<usize> = line_counts.iter().map(|&(_, c, _)| c).collect();
            sorted_counts.sort_unstable();
            let median = sorted_counts[sorted_counts.len() / 2];

            if median < MIN_MEDIAN_FLOOR {
                continue;
            }

            for &(line, count, title) in &line_counts {
                let ratio = count as f64 / median as f64;
                if count >= self.min_section_lines && ratio >= self.imbalance_ratio {
                    emit!(
                        result,
                        file.path,
                        line,
                        Severity::Info,
                        Category::SectionLengthImbalance,
                        suggest: format!(
                            "This section is {:.1}x longer than its siblings — consider splitting into sub-sections or a separate file",
                            ratio
                        ),
                        "Section \"{}\" is {} lines ({:.1}x the median of {} lines)",
                        title,
                        count,
                        ratio,
                        median
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
    use crate::checkers::utils::test_helpers::single_file_ctx_with_sections;
    use crate::config::SectionLengthImbalanceConfig;
    use crate::parser::types::Section;

    fn section(title: &str, level: u8, line: usize, end_line: usize) -> Section {
        Section {
            level,
            title: title.to_string(),
            line,
            end_line,
        }
    }

    fn checker_with(min_lines: usize, ratio: f64) -> SectionLengthImbalanceChecker {
        SectionLengthImbalanceChecker::new(&SectionLengthImbalanceConfig {
            enabled: true,
            min_section_lines: min_lines,
            imbalance_ratio: ratio,
            scope: vec![],
        })
    }

    fn run_check_with(sections: Vec<Section>, min_lines: usize, ratio: f64) -> CheckResult {
        let lines: Vec<&str> = vec!["# Doc"];
        let (_dir, ctx) = single_file_ctx_with_sections(&lines, sections);
        checker_with(min_lines, ratio).check(&ctx)
    }

    #[test]
    fn test_imbalanced_section_detected() {
        // Sections of 10, 10, 10, 80 lines — 80-line section should flag
        let result = run_check_with(
            vec![
                section("A", 2, 1, 11),   // 10 lines
                section("B", 2, 11, 21),  // 10 lines
                section("C", 2, 21, 31),  // 10 lines
                section("D", 2, 31, 111), // 80 lines
            ],
            50,
            4.0,
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("\"D\""));
        assert!(result.diagnostics[0].message.contains("80 lines"));
    }

    #[test]
    fn test_three_siblings_with_outlier() {
        // 10, 10, 80 lines — median is 10, ratio is 8x
        let result = run_check_with(
            vec![
                section("A", 2, 1, 11),   // 10 lines
                section("B", 2, 11, 21),  // 10 lines
                section("C", 2, 21, 101), // 80 lines
            ],
            50,
            4.0,
        );
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn test_fewer_than_three_siblings_no_flag() {
        let result = run_check_with(
            vec![section("A", 2, 1, 11), section("B", 2, 11, 111)],
            10,
            4.0,
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_balanced_sections_no_flag() {
        let result = run_check_with(
            vec![
                section("A", 2, 1, 41),   // 40 lines
                section("B", 2, 41, 91),  // 50 lines
                section("C", 2, 91, 151), // 60 lines
            ],
            50,
            4.0,
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_different_heading_levels_separate() {
        // Only h2 sections are compared — h3 sections are ignored
        let result = run_check_with(
            vec![
                section("A", 2, 1, 11),
                section("B", 2, 11, 21),
                section("C", 3, 21, 101), // h3, not compared
            ],
            10,
            4.0,
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_below_min_section_lines_no_flag() {
        // Section is 30 lines (3x median of 10) but below min_section_lines of 50
        let result = run_check_with(
            vec![
                section("A", 2, 1, 11),
                section("B", 2, 11, 21),
                section("C", 2, 21, 51), // 30 lines
            ],
            50,
            2.0,
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_toc_section_excluded() {
        // "Table of Contents" section should be excluded from comparison
        let result = run_check_with(
            vec![
                section("A", 2, 1, 11),                   // 10 lines
                section("B", 2, 11, 21),                  // 10 lines
                section("Table of Contents", 2, 21, 121), // 100 lines (excluded)
                section("C", 2, 121, 131),                // 10 lines
            ],
            10,
            4.0,
        );
        assert!(
            result.diagnostics.is_empty(),
            "Table of Contents section should be excluded"
        );
    }

    #[test]
    fn test_low_median_skipped() {
        // When median is below MIN_MEDIAN_FLOOR (5), don't compare
        let result = run_check_with(
            vec![
                section("A", 2, 1, 3),   // 2 lines
                section("B", 2, 3, 5),   // 2 lines
                section("C", 2, 5, 105), // 100 lines
            ],
            10,
            4.0,
        );
        assert!(
            result.diagnostics.is_empty(),
            "Should not flag when median is below floor"
        );
    }

    #[test]
    fn test_multiple_outliers_flagged() {
        // Sections of 10, 10, 10, 80, 80 lines
        // Median = 10, so 80-line sections should flag (ratio 8.0)
        let result = run_check_with(
            vec![
                section("A", 2, 1, 11),    // 10 lines
                section("B", 2, 11, 21),   // 10 lines
                section("C", 2, 21, 31),   // 10 lines
                section("D", 2, 31, 111),  // 80 lines
                section("E", 2, 111, 191), // 80 lines
            ],
            50,
            4.0,
        );
        assert_eq!(
            result.diagnostics.len(),
            2,
            "Both oversized sections should flag"
        );
    }

    #[test]
    fn test_h3_only_no_flag() {
        // Only h2 sections are analyzed
        let result = run_check_with(
            vec![
                section("A", 3, 1, 11),
                section("B", 3, 11, 21),
                section("C", 3, 21, 121),
            ],
            10,
            4.0,
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_exact_ratio_boundary_flags() {
        // Median 10, section 40 lines = exactly 4.0x ratio with ratio threshold 4.0
        let result = run_check_with(
            vec![
                section("A", 2, 1, 11),  // 10 lines
                section("B", 2, 11, 21), // 10 lines
                section("C", 2, 21, 61), // 40 lines → ratio 4.0x
            ],
            10,
            4.0,
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Exactly at ratio threshold should flag (>= comparison)"
        );
    }

    #[test]
    fn test_single_h2_section_no_flag() {
        let result = run_check_with(vec![section("A", 2, 1, 200)], 10, 4.0);
        assert!(result.diagnostics.is_empty());
    }
}
