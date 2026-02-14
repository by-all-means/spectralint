use std::collections::HashSet;

use strsim::jaro_winkler;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::types::Table;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{normalize, ScopeFilter};
use super::Checker;

pub struct EnumDriftChecker {
    scope: ScopeFilter,
}

impl EnumDriftChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

struct TableRef<'a> {
    table: &'a Table,
    file_idx: usize,
    normalized_headers: Vec<String>,
}

impl Checker for EnumDriftChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();
        let mut seen = HashSet::new();

        let table_refs: Vec<TableRef> = ctx
            .files
            .iter()
            .enumerate()
            .filter(|(file_idx, f)| {
                !ctx.historical_indices.contains(file_idx)
                    && self.scope.includes(&f.path, &ctx.project_root)
            })
            .flat_map(|(file_idx, file)| {
                file.tables.iter().map(move |table| TableRef {
                    normalized_headers: table.headers.iter().map(|h| normalize(h)).collect(),
                    table,
                    file_idx,
                })
            })
            .collect();

        for i in 0..table_refs.len() {
            for j in (i + 1)..table_refs.len() {
                let a = &table_refs[i];
                let b = &table_refs[j];

                if a.file_idx == b.file_idx || !tables_match(a, b) {
                    continue;
                }

                check_drift(ctx, a, b, &mut result, &mut seen);
            }
        }

        result
    }
}

fn tables_match(a: &TableRef, b: &TableRef) -> bool {
    let shared_count = a
        .normalized_headers
        .iter()
        .filter(|h| b.normalized_headers.contains(h))
        .count();

    if shared_count >= 2 {
        return true;
    }

    if shared_count >= 1 {
        if let (Some(sec_a), Some(sec_b)) = (&a.table.parent_section, &b.table.parent_section) {
            if jaro_winkler(sec_a, sec_b) >= 0.8 {
                return true;
            }
        }
    }

    false
}

fn check_drift(
    ctx: &CheckerContext,
    a: &TableRef,
    b: &TableRef,
    result: &mut CheckResult,
    seen: &mut HashSet<String>,
) {
    let format_values = |vals: &mut Vec<&String>| -> String {
        vals.sort();
        vals.iter()
            .map(|v| match v.char_indices().nth(50) {
                Some((i, _)) => format!("\"{}...\"", &v[..i]),
                None => format!("\"{v}\""),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    for (col_a, norm_a) in a.normalized_headers.iter().enumerate() {
        let Some(col_b) = b.normalized_headers.iter().position(|h| h == norm_a) else {
            continue;
        };

        let collect_values = |table: &Table, col: usize| -> HashSet<String> {
            table
                .rows
                .iter()
                .filter_map(|row| row.get(col))
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect()
        };

        let values_a = collect_values(a.table, col_a);
        let values_b = collect_values(b.table, col_b);

        let only_in_a: Vec<_> = values_a.difference(&values_b).collect();
        let only_in_b: Vec<_> = values_b.difference(&values_a).collect();

        if only_in_a.is_empty() && only_in_b.is_empty() {
            continue;
        }

        for (mut diff, src, col, other) in [(only_in_a, a, col_a, b), (only_in_b, b, col_b, a)] {
            if diff.is_empty() {
                continue;
            }
            let msg = format!(
                "Column \"{}\" has values {} not found in {}",
                src.table.headers[col],
                format_values(&mut diff),
                ctx.files[other.file_idx]
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            let src_file = &ctx.files[src.file_idx].path;
            let key = format!("{}:{}:{}", src_file.display(), src.table.line, msg);
            if seen.insert(key) {
                emit!(
                    result,
                    src_file,
                    src.table.line,
                    Severity::Warning,
                    Category::EnumDrift,
                    suggest: "Align the value sets across files or document why they differ",
                    "{msg}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::{ParsedFile, Table};

    #[test]
    fn test_enum_drift_detected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec!["inactive".to_string(), "skip".to_string()],
                    vec!["pending".to_string(), "queue".to_string()],
                ],
                line: 5,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec!["inactive".to_string(), "skip".to_string()],
                    vec!["archived".to_string(), "delete".to_string()],
                ],
                line: 3,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),

            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);

        assert!(
            !result.diagnostics.is_empty(),
            "Expected enum drift warnings"
        );

        let warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert!(warnings.len() >= 2);
    }

    #[test]
    fn test_historical_file_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec!["pending".to_string(), "queue".to_string()],
                ],
                line: 5,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("changelog.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec!["archived".to_string(), "delete".to_string()],
                ],
                line: 3,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let mut historical = HashSet::new();
        historical.insert(1); // changelog.md is historical

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),
            historical_indices: historical,
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Historical files should be excluded from enum drift"
        );
    }

    #[test]
    fn test_dedup_no_duplicate_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create 3 files with the same table to trigger multiple comparisons
        let make_file = |name: &str, extra_value: &str| ParsedFile {
            path: root.join(name),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec![extra_value.to_string(), "handle".to_string()],
                ],
                line: 5,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![
                make_file("A.md", "pending"),
                make_file("B.md", "pending"),
                make_file("C.md", "archived"),
            ],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);

        // Verify no exact duplicates (same file + line + message)
        let mut seen = HashSet::new();
        for d in &result.diagnostics {
            let key = format!("{}:{}:{}", d.file.display(), d.line, d.message);
            assert!(
                seen.insert(key.clone()),
                "Duplicate diagnostic found: {key}"
            );
        }
    }

    #[test]
    fn test_identical_tables_no_drift() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let table = Table {
            headers: vec!["Status".to_string(), "Action".to_string()],
            rows: vec![
                vec!["active".to_string(), "process".to_string()],
                vec!["inactive".to_string(), "skip".to_string()],
            ],
            line: 5,
            parent_section: Some("Routing".to_string()),
        };

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![table.clone()],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![Table { line: 3, ..table }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),

            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_scope_limits_comparison() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec!["pending".to_string(), "queue".to_string()],
                ],
                line: 5,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("reports/output.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec!["archived".to_string(), "delete".to_string()],
                ],
                line: 3,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        // Scope limited to CLAUDE.md only — reports/output.md excluded
        let checker = EnumDriftChecker::new(&["CLAUDE.md".to_string()]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Out-of-scope file should not participate in enum drift comparison"
        );
    }

    // ── Item 9: Table matching heuristic edge cases ──────────────────────

    #[test]
    fn test_tables_zero_shared_headers_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Fruit".to_string(), "Color".to_string()],
                rows: vec![vec!["apple".to_string(), "red".to_string()]],
                line: 5,
                parent_section: Some("Food".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Country".to_string(), "Capital".to_string()],
                rows: vec![vec!["France".to_string(), "Paris".to_string()]],
                line: 3,
                parent_section: Some("Geography".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Tables with zero shared headers should not be compared"
        );
    }

    #[test]
    fn test_tables_one_shared_header_no_parent_section() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Color".to_string()],
                rows: vec![vec!["active".to_string(), "green".to_string()]],
                line: 5,
                parent_section: None,
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Priority".to_string()],
                rows: vec![vec!["inactive".to_string(), "low".to_string()]],
                line: 3,
                parent_section: None,
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "1 shared header with no parent section should not match"
        );
    }

    #[test]
    fn test_tables_one_shared_header_dissimilar_parent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Color".to_string()],
                rows: vec![vec!["active".to_string(), "green".to_string()]],
                line: 5,
                parent_section: Some("Food".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Priority".to_string()],
                rows: vec![vec!["inactive".to_string(), "low".to_string()]],
                line: 3,
                parent_section: Some("Geography".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "1 shared header with dissimilar parent sections should not match"
        );
    }

    // ── Item 18: Empty column vs populated ───────────────────────────────

    #[test]
    fn test_enum_drift_empty_column_vs_populated() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["".to_string(), "process".to_string()],
                    vec!["".to_string(), "skip".to_string()],
                ],
                line: 5,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![
                    vec!["active".to_string(), "process".to_string()],
                    vec!["inactive".to_string(), "skip".to_string()],
                ],
                line: 3,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        // Empty values are filtered out, so only "active" and "inactive" are unique to B
        let drift: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("active"))
            .collect();
        assert!(
            !drift.is_empty(),
            "Populated values not in empty column should be flagged"
        );
    }

    // ── Item 19: Long value truncation ───────────────────────────────────

    #[test]
    fn test_enum_drift_long_value_truncated_in_message() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let long_value = "a".repeat(60);
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![vec![long_value.clone(), "process".to_string()]],
                line: 5,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Status".to_string(), "Action".to_string()],
                rows: vec![vec!["active".to_string(), "process".to_string()]],
                line: 3,
                parent_section: Some("Routing".to_string()),
            }],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file1, file2],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(!result.diagnostics.is_empty());
        // The long value should be truncated with "..."
        let has_truncated = result.diagnostics.iter().any(|d| d.message.contains("..."));
        assert!(
            has_truncated,
            "Values over 50 chars should be truncated with '...'"
        );
    }

    #[test]
    fn test_same_file_tables_no_drift() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Two tables in the SAME file with different values
        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![
                Table {
                    headers: vec!["Status".to_string(), "Action".to_string()],
                    rows: vec![
                        vec!["active".to_string(), "process".to_string()],
                        vec!["pending".to_string(), "queue".to_string()],
                    ],
                    line: 5,
                    parent_section: Some("Routing".to_string()),
                },
                Table {
                    headers: vec!["Status".to_string(), "Action".to_string()],
                    rows: vec![
                        vec!["active".to_string(), "process".to_string()],
                        vec!["archived".to_string(), "delete".to_string()],
                    ],
                    line: 15,
                    parent_section: Some("Routing".to_string()),
                },
            ],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = EnumDriftChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Same-file tables should not trigger enum drift"
        );
    }
}
