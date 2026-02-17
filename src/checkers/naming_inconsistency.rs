use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use strsim::jaro_winkler;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{normalize, ScopeFilter};
use super::Checker;

/// Patterns to skip: dates, timestamps, and YAML-like frontmatter lines.
static DATE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:\d{4}[-/]\d{2}[-/]\d{2}|(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)\s+\d{1,2},?\s*\d{4}|\d{1,2}:\d{2}\s*(?:am|pm))$").unwrap()
});

/// Leading numbered prefix like "1. ", "Step 3: ", "Phase 2: "
static NUMBERED_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:\d+\.\s*|(?:step|phase)\s+\d+[:\s]\s*)").unwrap());

/// YAML-like frontmatter lines (key: value, allowed-tools:, description:)
static YAML_FRONTMATTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][-a-z_]*:").unwrap());

/// Strip a leading numbered prefix from a name for comparison.
fn strip_numbered_prefix(name: &str) -> &str {
    if let Some(m) = NUMBERED_PREFIX.find(name) {
        &name[m.end()..]
    } else {
        name
    }
}

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
            let originals: Vec<&str> = unique_originals.iter().copied().collect();
            let suggestion = format!(
                "Standardize to one form: use either \"{}\" or \"{}\" consistently",
                originals[0], originals[1]
            );

            for occ in group {
                emit!(
                    result,
                    ctx.files[occ.file_idx].path,
                    occ.line,
                    Severity::Warning,
                    Category::NamingInconsistency,
                    suggest: &suggestion,
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
                if similarity < 0.95 {
                    continue;
                }

                let group_a = &groups[key_a];
                let group_b = &groups[key_b];

                // Skip dates/timestamps (e.g. "Dec 5, 2025" vs "Dec 4, 2025")
                if DATE_PATTERN.is_match(&group_a[0].original)
                    || DATE_PATTERN.is_match(&group_b[0].original)
                {
                    continue;
                }

                // Skip YAML frontmatter lines (e.g. "allowed-tools: ...")
                if YAML_FRONTMATTER.is_match(&group_a[0].original)
                    || YAML_FRONTMATTER.is_match(&group_b[0].original)
                {
                    continue;
                }

                // Skip if names only differ by numbered prefix
                // (e.g. "3. Validation Checklist" vs "4. Validation Checklist")
                let stripped_a = strip_numbered_prefix(&group_a[0].original);
                let stripped_b = strip_numbered_prefix(&group_b[0].original);
                if !stripped_a.is_empty() && stripped_a.eq_ignore_ascii_case(stripped_b) {
                    continue;
                }

                // Skip if names differ only in digits/version numbers
                // (e.g. "Output 1: Gate YAML" vs "Output 2: Gate YAML",
                //  "Algorithm v6" vs "Algorithm v7")
                let no_digits_a: String = group_a[0]
                    .original
                    .chars()
                    .filter(|c| !c.is_ascii_digit())
                    .collect();
                let no_digits_b: String = group_b[0]
                    .original
                    .chars()
                    .filter(|c| !c.is_ascii_digit())
                    .collect();
                if !no_digits_a.is_empty() && no_digits_a.eq_ignore_ascii_case(&no_digits_b) {
                    continue;
                }

                let files_a: HashSet<_> = group_a.iter().map(|o| o.file_idx).collect();
                let files_b: HashSet<_> = group_b.iter().map(|o| o.file_idx).collect();
                if files_a == files_b {
                    continue;
                }

                let msg = format!(
                    "Similar names: \"{}\" and \"{}\" might refer to the same concept (similarity: {:.0}%)",
                    group_a[0].original, group_b[0].original, similarity * 100.0
                );
                let suggestion = format!(
                    "Standardize to one form: use either \"{}\" or \"{}\" consistently",
                    group_a[0].original, group_b[0].original
                );

                for occ in group_a.iter().chain(group_b.iter()) {
                    emit!(
                        result,
                        ctx.files[occ.file_idx].path,
                        occ.line,
                        Severity::Info,
                        Category::NamingInconsistency,
                        suggest: &suggestion,
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

        // "api_configs" vs "api_config" → very high Jaro-Winkler similarity (>0.95)
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["api_configs".to_string()],
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
                headers: vec!["api_config".to_string()],
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

    #[test]
    fn test_dates_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        use crate::parser::types::Section;
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![Section {
                title: "Dec 5, 2025".to_string(),
                level: 2,
                line: 5,
                end_line: 5,
            }],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![Section {
                title: "Dec 4, 2025".to_string(),
                level: 2,
                line: 3,
                end_line: 3,
            }],
            tables: vec![],
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
            infos.is_empty(),
            "Dates should not be flagged as similar names"
        );
    }

    #[test]
    fn test_iso_dates_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        use crate::parser::types::Section;
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![Section {
                title: "2025-10-27".to_string(),
                level: 2,
                line: 5,
                end_line: 5,
            }],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![Section {
                title: "2025-08-20".to_string(),
                level: 2,
                line: 3,
                end_line: 3,
            }],
            tables: vec![],
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
            infos.is_empty(),
            "ISO dates should not be flagged as similar names"
        );
    }

    #[test]
    fn test_numbered_prefixes_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        use crate::parser::types::Section;
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![Section {
                title: "3. Validation Checklist".to_string(),
                level: 2,
                line: 5,
                end_line: 5,
            }],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![Section {
                title: "4. Validation Checklist".to_string(),
                level: 2,
                line: 3,
                end_line: 3,
            }],
            tables: vec![],
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
            infos.is_empty(),
            "Numbered prefixes with same body should not be flagged"
        );
    }

    #[test]
    fn test_step_prefixes_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        use crate::parser::types::Section;
        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![Section {
                title: "Step 3: Check for shared content".to_string(),
                level: 2,
                line: 5,
                end_line: 5,
            }],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        };

        let file2 = ParsedFile {
            path: root.join("AGENTS.md"),
            sections: vec![Section {
                title: "Step 4: Check for shared content".to_string(),
                level: 2,
                line: 3,
                end_line: 3,
            }],
            tables: vec![],
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
            infos.is_empty(),
            "Step N: prefixes with same body should not be flagged"
        );
    }

    #[test]
    fn test_yaml_frontmatter_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let file1 = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections: vec![],
            tables: vec![Table {
                headers: vec!["allowed-tools: Bash(gh pr comment:)".to_string()],
                rows: vec![],
                line: 1,
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
                headers: vec!["allowed-tools: Bash(gh pr diff:)".to_string()],
                rows: vec![],
                line: 1,
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
            infos.is_empty(),
            "YAML frontmatter lines should not be flagged as similar names"
        );
    }
}
