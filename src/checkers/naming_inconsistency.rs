use std::collections::{BTreeSet, HashMap, HashSet};

use strsim::jaro_winkler;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{normalize, ScopeFilter};
use super::Checker;

pub struct NamingInconsistencyChecker {
    scope: ScopeFilter,
}

impl NamingInconsistencyChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

#[derive(Debug, Clone)]
struct NameOccurrence {
    original: String,
    file_idx: usize,
    line: usize,
}

impl Checker for NamingInconsistencyChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        let mut occurrences = Vec::new();

        for (file_idx, file) in ctx.files.iter().enumerate() {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }
            for table in &file.tables {
                for header in &table.headers {
                    let trimmed = header.trim();
                    if !trimmed.is_empty() {
                        occurrences.push(NameOccurrence {
                            original: trimmed.to_string(),
                            file_idx,
                            line: table.line,
                        });
                    }
                }
            }
            for section in &file.sections {
                let trimmed = section.title.trim();
                if !trimmed.is_empty() {
                    occurrences.push(NameOccurrence {
                        original: trimmed.to_string(),
                        file_idx,
                        line: section.line,
                    });
                }
            }
        }

        let mut groups: HashMap<_, Vec<_>> = HashMap::new();
        for occ in &occurrences {
            groups
                .entry(normalize(&occ.original))
                .or_default()
                .push(occ);
        }

        for group in groups.values() {
            let unique_originals: BTreeSet<&str> =
                group.iter().map(|o| o.original.as_str()).collect();
            if unique_originals.len() < 2 {
                continue;
            }

            // Skip if all variants only differ by case (e.g. "Input" vs "INPUT")
            let lowered: HashSet<String> =
                unique_originals.iter().map(|s| s.to_lowercase()).collect();
            if lowered.len() < 2 {
                continue;
            }

            if !group.iter().any(|o| o.file_idx != group[0].file_idx) {
                continue;
            }

            let variants: Vec<_> = unique_originals
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect();
            let msg = format!(
                "Inconsistent naming: {} refer to the same concept",
                variants.join(" vs ")
            );

            for occ in group {
                emit!(
                    result,
                    ctx.files[occ.file_idx].path,
                    occ.line,
                    Severity::Warning,
                    Category::NamingInconsistency,
                    "{msg}"
                );
            }
        }

        let group_keys: Vec<_> = groups
            .keys()
            .filter(|k| k.split('_').count() >= 2)
            .collect();
        for i in 0..group_keys.len() {
            for j in (i + 1)..group_keys.len() {
                let key_a = group_keys[i];
                let key_b = group_keys[j];

                let similarity = jaro_winkler(key_a, key_b);
                if similarity < 0.92 {
                    continue;
                }

                let group_a = &groups[key_a];
                let group_b = &groups[key_b];

                let files_a: HashSet<_> = group_a.iter().map(|o| o.file_idx).collect();
                let files_b: HashSet<_> = group_b.iter().map(|o| o.file_idx).collect();
                if files_a == files_b {
                    continue;
                }

                let msg = format!(
                    "Similar names: \"{}\" and \"{}\" might refer to the same concept (similarity: {:.0}%)",
                    group_a[0].original, group_b[0].original, similarity * 100.0
                );

                for occ in group_a.iter().chain(group_b.iter()) {
                    emit!(
                        result,
                        ctx.files[occ.file_idx].path,
                        occ.line,
                        Severity::Info,
                        Category::NamingInconsistency,
                        "{msg}"
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
    use crate::parser::types::{ParsedFile, Table};

    #[test]
    fn test_naming_inconsistency_detected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["api_key".to_string(), "Value".to_string()],
                rows: vec![],
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
                headers: vec!["apiKey".to_string(), "Value".to_string()],
                rows: vec![],
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

        let checker = NamingInconsistencyChecker::new(&[]);
        let result = checker.check(&ctx);

        let warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert!(
            !warnings.is_empty(),
            "Expected naming inconsistency warnings"
        );
        assert!(warnings[0].message.contains("api_key"));
        assert!(warnings[0].message.contains("apiKey"));
    }

    #[test]
    fn test_case_only_difference_no_warning() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["Input".to_string(), "Action".to_string()],
                rows: vec![],
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
                headers: vec!["INPUT".to_string(), "Action".to_string()],
                rows: vec![],
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

        let checker = NamingInconsistencyChecker::new(&[]);
        let result = checker.check(&ctx);

        let warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert!(
            warnings.is_empty(),
            "Case-only difference (Input vs INPUT) should not produce a warning"
        );
    }

    #[test]
    fn test_same_file_no_warning() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![
                Table {
                    headers: vec!["api_key".to_string()],
                    rows: vec![],
                    line: 2,
                    parent_section: None,
                },
                Table {
                    headers: vec!["apiKey".to_string()],
                    rows: vec![],
                    line: 8,
                    parent_section: None,
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

        let checker = NamingInconsistencyChecker::new(&[]);
        let result = checker.check(&ctx);

        let warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert!(
            warnings.is_empty(),
            "Same-file inconsistency should not warn"
        );
    }

    #[test]
    fn test_scope_limits_comparison() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["api_key".to_string(), "Value".to_string()],
                rows: vec![],
                line: 5,
                parent_section: None,
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
                headers: vec!["apiKey".to_string(), "Value".to_string()],
                rows: vec![],
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

        // Scope limited to CLAUDE.md — reports/output.md excluded
        let checker = NamingInconsistencyChecker::new(&["CLAUDE.md".to_string()]);
        let result = checker.check(&ctx);

        let warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert!(
            warnings.is_empty(),
            "Out-of-scope file should not participate in naming inconsistency comparison"
        );
    }

    // ── Item 8: Similarity threshold boundary tests ──────────────────────

    #[test]
    fn test_similar_names_flagged_as_info() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // "error_handler" vs "error_handling" → high Jaro-Winkler similarity
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["error_handler".to_string()],
                rows: vec![],
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
                headers: vec!["error_handling".to_string()],
                rows: vec![],
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

        let checker = NamingInconsistencyChecker::new(&[]);
        let result = checker.check(&ctx);

        let infos: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
            .collect();
        assert!(
            !infos.is_empty(),
            "Similar names above threshold should produce info diagnostics"
        );
    }

    #[test]
    fn test_dissimilar_names_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // "api_key" vs "user_name" → very different, should not flag
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["api_key".to_string()],
                rows: vec![],
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
                headers: vec!["user_name".to_string()],
                rows: vec![],
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

        let checker = NamingInconsistencyChecker::new(&[]);
        let result = checker.check(&ctx);
        assert!(
            result.diagnostics.is_empty(),
            "Dissimilar names should not produce any diagnostics"
        );
    }
}
