use regex::Regex;
use std::sync::LazyLock;

use crate::config::InstructionDensityConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub struct InstructionDensityChecker {
    scope: ScopeFilter,
    max_consecutive_bullets: usize,
}

impl InstructionDensityChecker {
    pub fn new(config: &InstructionDensityConfig) -> Self {
        Self {
            scope: ScopeFilter::new(&config.scope),
            max_consecutive_bullets: config.max_consecutive_bullets,
        }
    }
}

/// Matches bullet-point lines (-, *, +, or numbered lists).
static BULLET_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[-*+]|\d+\.)\s+").unwrap());

/// Section titles that are navigation/reference/inventory, not instruction lists.
static SKIP_SECTION_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        ^(?:table\s+of\s+contents|contents|toc|index|navigation|changelog|release\s+notes)$
        | \b(?:structure|files?|modules?|components?|directories|architecture)\s*(?:\(|$)
        | \b(?:directory|file|folder|project)\s+(?:structure|layout|tree|organization)\b
        | \b(?:key|core|main)\s+(?:component|module|file)s?\b
        | \b(?:test\s+files?|test\s+coverage)\b
        | \b(?:future\s+enhancement|ideas?|roadmap|wishlist|enhancements?)\b
        | \b(?:philosophy|principles|values|tenets)\b
        | ^\S+\.(?:js|ts|py|rs|rb|go|el|ex)\b
        | \bcommands?\b
        | \bscripts?\b
        | \bquick\s+reference\b
        | \bapi\s+(?:endpoint|route|reference)s?\b
        | ^reference$
        | \b(?:data\s+)?(?:persistence|schema|tables?|models?|database)\b
        | \([^)]*(?:src|lib|pkg|app|cmd)/
        | 文件结构
        ",
    )
    .unwrap()
});

impl Checker for InstructionDensityChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Skip files with < 2 sections
            if file.sections.len() < 2 {
                continue;
            }

            for section in &file.sections {
                if SKIP_SECTION_TITLE.is_match(&section.title) {
                    continue;
                }

                let start = section.line.saturating_sub(1);
                let end = section.end_line.min(file.raw_lines.len());
                if end <= start {
                    continue;
                }

                let section_lines = &file.raw_lines[start..end];

                let mut consecutive = 0usize;
                let mut run_start_line = 0usize;
                let mut in_code_block = false;

                for (offset, line) in section_lines.iter().enumerate() {
                    let trimmed = line.trim();

                    if trimmed.starts_with("```") {
                        in_code_block = !in_code_block;
                        consecutive = 0;
                        continue;
                    }

                    if in_code_block {
                        continue;
                    }

                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        consecutive = 0;
                        continue;
                    }

                    if BULLET_PATTERN.is_match(line) {
                        if consecutive == 0 {
                            run_start_line = start + offset + 1; // 1-indexed
                        }
                        consecutive += 1;

                        if consecutive > self.max_consecutive_bullets {
                            emit!(
                                result,
                                file.path,
                                run_start_line,
                                Severity::Info,
                                Category::InstructionDensity,
                                suggest: "Break up the bullet list with subheadings, blank lines, or code examples",
                                "Section \"{}\" has {} consecutive bullet points without a structural break \
                                 (max: {}). Dense instruction walls reduce agent compliance.",
                                section.title,
                                consecutive,
                                self.max_consecutive_bullets
                            );
                            break;
                        }
                    } else {
                        consecutive = 0;
                    }
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
    use crate::parser::types::Section;

    fn run_check(lines: &[&str], sections: Vec<Section>) -> CheckResult {
        let (_dir, ctx) = single_file_ctx_with_sections(lines, sections);
        let config = InstructionDensityConfig {
            enabled: true,
            max_consecutive_bullets: 3,
            scope: Vec::new(),
        };
        InstructionDensityChecker::new(&config).check(&ctx)
    }

    fn two_sections(end1: usize, end2: usize) -> Vec<Section> {
        vec![
            Section {
                level: 1,
                title: "Rules".to_string(),
                line: 1,
                end_line: end1,
            },
            Section {
                level: 1,
                title: "Other".to_string(),
                line: end1 + 1,
                end_line: end2,
            },
        ]
    }

    #[test]
    fn test_dense_bullets_detected() {
        let lines: Vec<&str> = vec![
            "# Rules",
            "- Rule one",
            "- Rule two",
            "- Rule three",
            "- Rule four",
            "# Other",
            "Info.",
        ];
        let result = run_check(&lines, two_sections(5, 7));
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
        assert_eq!(result.diagnostics[0].category, Category::InstructionDensity);
        assert!(result.diagnostics[0].message.contains("Rules"));
    }

    #[test]
    fn test_bullets_with_break_no_flag() {
        let lines: Vec<&str> = vec![
            "# Rules",
            "- Rule one",
            "- Rule two",
            "",
            "- Rule three",
            "- Rule four",
            "# Other",
            "Info.",
        ];
        let result = run_check(&lines, two_sections(6, 8));
        assert!(
            result.diagnostics.is_empty(),
            "Bullets broken by blank line should not flag"
        );
    }

    #[test]
    fn test_few_bullets_no_flag() {
        let lines: Vec<&str> = vec!["# Rules", "- Rule one", "- Rule two", "# Other", "Info."];
        let result = run_check(&lines, two_sections(3, 5));
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_single_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Rules",
            "- Rule one",
            "- Rule two",
            "- Rule three",
            "- Rule four",
        ];
        let (_dir, ctx) = single_file_ctx_with_sections(
            &lines,
            vec![Section {
                level: 1,
                title: "Rules".to_string(),
                line: 1,
                end_line: 5,
            }],
        );
        let config = InstructionDensityConfig {
            enabled: true,
            max_consecutive_bullets: 3,
            scope: Vec::new(),
        };
        let result = InstructionDensityChecker::new(&config).check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Files with < 2 sections should be skipped"
        );
    }

    #[test]
    fn test_table_of_contents_skipped() {
        let lines: Vec<&str> = vec![
            "# Table of Contents",
            "1. [First section](#first)",
            "2. [Second section](#second)",
            "3. [Third section](#third)",
            "4. [Fourth section](#fourth)",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Table of Contents".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Table of Contents sections should be skipped"
        );
    }

    #[test]
    fn test_toc_title_skipped() {
        let lines: Vec<&str> = vec![
            "# TOC",
            "- Item one",
            "- Item two",
            "- Item three",
            "- Item four",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "TOC".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "TOC sections should be skipped"
        );
    }

    #[test]
    fn test_directory_structure_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Directory Structure (76 Skills)",
            "- `01-model-architecture/` - Model architectures",
            "- `02-tokenization/` - Tokenizers",
            "- `03-fine-tuning/` - Fine-tuning frameworks",
            "- `04-interpretability/` - Interpretability tools",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Directory Structure (76 Skills)".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Directory structure sections should be skipped"
        );
    }

    #[test]
    fn test_key_component_files_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Key Component Files",
            "- `R/property-helpers.R` - defines supported properties",
            "- `R/clean-contents.R` - handles number formatting",
            "- `R/html.R` - HTML table generation",
            "- `R/latex.R` - LaTeX output",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Key Component Files".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "File inventory sections should be skipped"
        );
    }

    #[test]
    fn test_test_files_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Test Files",
            "- `tests/unit/foo.test.ts`",
            "- `tests/unit/bar.test.ts`",
            "- `tests/unit/baz.test.ts`",
            "- `tests/unit/qux.test.ts`",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Test Files".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Test Files sections should be skipped"
        );
    }

    #[test]
    fn test_frontend_structure_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Frontend Structure (frontend/src/)",
            "- `pages/` - Main route components",
            "- `components/` - Reusable UI components",
            "- `services/` - API client services",
            "- `config/` - Configuration",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Frontend Structure (frontend/src/)".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Frontend Structure sections should be skipped"
        );
    }

    #[test]
    fn test_future_enhancement_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Future Enhancement Ideas",
            "- Add dark mode support",
            "- Implement caching layer",
            "- Add WebSocket support",
            "- Migrate to new framework",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Future Enhancement Ideas".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Future Enhancement Ideas sections should be skipped"
        );
    }

    #[test]
    fn test_data_persistence_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Data Persistence",
            "- `users` table stores user accounts",
            "- `sessions` table stores active sessions",
            "- `audit_log` table stores activity",
            "- `settings` table stores config",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Data Persistence".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Data Persistence sections should be skipped"
        );
    }

    #[test]
    fn test_path_parenthetical_section_skipped() {
        let lines: Vec<&str> = vec![
            "# JavaScript Runtime (src/bun.js/)",
            "- `fetch.zig` - HTTP fetch implementation",
            "- `event_loop.zig` - Event loop",
            "- `timer.zig` - Timer implementation",
            "- `console.zig` - Console API",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "JavaScript Runtime (src/bun.js/)".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Sections with path in parentheses should be skipped"
        );
    }

    #[test]
    fn test_cjk_file_structure_skipped() {
        let lines: Vec<&str> = vec![
            "# 文件结构",
            "- `src/main.rs` - Entry point",
            "- `src/lib.rs` - Library root",
            "- `src/config.rs` - Configuration",
            "- `src/utils.rs` - Utilities",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "文件结构".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "CJK file structure sections should be skipped"
        );
    }

    #[test]
    fn test_philosophy_section_skipped() {
        let lines: Vec<&str> = vec![
            "# Project Philosophy",
            "- Simplicity over complexity",
            "- Explicit is better than implicit",
            "- Readability counts",
            "- Practicality beats purity",
            "# Other",
            "Info.",
        ];
        let result = run_check(
            &lines,
            vec![
                Section {
                    level: 1,
                    title: "Project Philosophy".to_string(),
                    line: 1,
                    end_line: 5,
                },
                Section {
                    level: 1,
                    title: "Other".to_string(),
                    line: 6,
                    end_line: 7,
                },
            ],
        );
        assert!(
            result.diagnostics.is_empty(),
            "Philosophy sections are meta-commentary and should be skipped"
        );
    }

    #[test]
    fn test_actual_rules_section_still_flags() {
        let lines: Vec<&str> = vec![
            "# Coding Standards",
            "- Always use strict mode",
            "- Run linting before commit",
            "- Follow naming conventions",
            "- Write unit tests first",
            "# Other",
            "Info.",
        ];
        let result = run_check(&lines, two_sections(5, 7));
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Actual instruction sections should still flag"
        );
    }

    #[test]
    fn test_code_block_breaks_run() {
        let lines: Vec<&str> = vec![
            "# Rules",
            "- Rule one",
            "- Rule two",
            "```",
            "code here",
            "```",
            "- Rule three",
            "- Rule four",
            "# Other",
            "Info.",
        ];
        let result = run_check(&lines, two_sections(8, 10));
        assert!(
            result.diagnostics.is_empty(),
            "Code block should break the bullet run"
        );
    }
}
