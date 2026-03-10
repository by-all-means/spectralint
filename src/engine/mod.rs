mod cache;
pub(crate) mod cross_ref;
pub(crate) mod fix;

/// Re-export `apply_fixes` so the binary crate can use `engine::apply_fixes`.
pub use fix::apply_fixes;
pub(crate) mod scanner;
mod suppress;

use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

use crate::checkers;
use crate::config::Config;
use crate::types::CheckResult;
use cross_ref::CheckerContext;

/// Return the list of markdown files that would be scanned for the given project root and config.
pub fn scanned_files(project_root: &Path, config: &Config) -> Vec<std::path::PathBuf> {
    scanner::scan(project_root, config).files
}

pub fn run(
    project_root: &Path,
    config: &Config,
    use_cache: bool,
    config_path: Option<&Path>,
) -> Result<CheckResult> {
    let scan_result = scanner::scan(project_root, config);
    if scan_result.files.is_empty() {
        anyhow::bail!("No markdown files found in {}", project_root.display());
    }

    // Compute cache keys and try to load from cache
    let (files_hash, config_hash) = if use_cache {
        let fh = cache::compute_files_hash(&scan_result.files);
        let ch = cache::compute_config_hash(config_path, project_root);
        if let Some(diagnostics) = cache::load(project_root, fh, ch) {
            return Ok(CheckResult { diagnostics });
        }
        (fh, ch)
    } else {
        (0, 0)
    };

    let total_files = scan_result.files.len();
    let parsed: Vec<_> = scan_result
        .files
        .par_iter()
        .filter_map(|p| match crate::parser::parse_file(p) {
            Ok(f) => Some(f),
            Err(e) => {
                tracing::warn!("Failed to parse {}: {e}", p.display());
                None
            }
        })
        .collect();

    let parse_failures = total_files - parsed.len();
    if parsed.is_empty() {
        anyhow::bail!(
            "All {} markdown file(s) failed to parse in {}",
            total_files,
            project_root.display()
        );
    }
    if parse_failures > 0 {
        tracing::warn!(
            "Checked {}/{} files ({} failed to parse)",
            parsed.len(),
            total_files,
            parse_failures
        );
    }

    // Validate suppress comment rule names
    let known_rules = suppress::all_known_rule_names(&config.checkers.custom_patterns);
    let mut invalid_suppress_diags = suppress::validate_suppress_rules(&parsed, &known_rules);

    let suppressions = suppress::build_suppression_set(&parsed);
    let context = CheckerContext::build(
        parsed,
        project_root,
        &config.historical_files,
        scan_result.filename_index,
        scan_result.canonical_root,
    );

    let all = checkers::all_checkers(config);
    let mut diagnostics: Vec<_> = all
        .par_iter()
        .flat_map(|checker| checker.check(&context).diagnostics)
        .collect();

    diagnostics.retain(|d| !suppress::is_suppressed(&suppressions, &d.file, d.line, &d.category));

    // Detect unused suppressions (must run after suppression filtering)
    let mut unused_suppress_diags = suppress::find_unused_suppressions(&suppressions);

    diagnostics.append(&mut invalid_suppress_diags);
    diagnostics.append(&mut unused_suppress_diags);

    // Apply per-checker severity overrides
    for d in &mut diagnostics {
        if let Some(sev) = config.severity_override(&d.category) {
            d.severity = sev;
        }
    }

    diagnostics.sort_by(|a, b| {
        (&a.file, a.line, a.column, &a.category, &a.message).cmp(&(
            &b.file,
            b.line,
            b.column,
            &b.category,
            &b.message,
        ))
    });
    diagnostics.dedup_by(|a, b| {
        a.file == b.file
            && a.line == b.line
            && a.column == b.column
            && a.category == b.category
            && a.message == b.message
    });

    // Save to cache
    if use_cache {
        cache::save(project_root, files_hash, config_hash, &diagnostics);
    }

    Ok(CheckResult { diagnostics })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, Diagnostic, Severity};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn make_diag(
        file: &str,
        line: usize,
        column: Option<usize>,
        severity: Severity,
        category: Category,
        message: &str,
    ) -> Diagnostic {
        Diagnostic {
            file: Arc::new(PathBuf::from(file)),
            line,
            column,
            end_line: None,
            end_column: None,
            severity,
            category,
            message: message.to_string(),
            suggestion: None,
            fix: None,
        }
    }

    // ── 1. Deduplication logic ──────────────────────────────────────────

    #[test]
    fn dedup_removes_exact_duplicates() {
        let mut diagnostics = vec![
            make_diag(
                "a.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "broken link",
            ),
            make_diag(
                "a.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "broken link",
            ),
            make_diag(
                "a.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "broken link",
            ),
        ];

        diagnostics.sort_by(|a, b| {
            (&a.file, a.line, a.column, &a.category, &a.message).cmp(&(
                &b.file,
                b.line,
                b.column,
                &b.category,
                &b.message,
            ))
        });
        diagnostics.dedup_by(|a, b| {
            a.file == b.file
                && a.line == b.line
                && a.column == b.column
                && a.category == b.category
                && a.message == b.message
        });

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn dedup_keeps_different_categories_same_line() {
        let mut diagnostics = vec![
            make_diag(
                "a.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "msg",
            ),
            make_diag(
                "a.md",
                5,
                None,
                Severity::Info,
                Category::VagueDirective,
                "msg",
            ),
        ];

        diagnostics.sort_by(|a, b| {
            (&a.file, a.line, a.column, &a.category, &a.message).cmp(&(
                &b.file,
                b.line,
                b.column,
                &b.category,
                &b.message,
            ))
        });
        diagnostics.dedup_by(|a, b| {
            a.file == b.file
                && a.line == b.line
                && a.column == b.column
                && a.category == b.category
                && a.message == b.message
        });

        assert_eq!(
            diagnostics.len(),
            2,
            "Different categories on the same line should be kept"
        );
    }

    #[test]
    fn dedup_keeps_different_messages_same_category() {
        let mut diagnostics = vec![
            make_diag(
                "a.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "ref 1",
            ),
            make_diag(
                "a.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "ref 2",
            ),
        ];

        diagnostics.sort_by(|a, b| {
            (&a.file, a.line, a.column, &a.category, &a.message).cmp(&(
                &b.file,
                b.line,
                b.column,
                &b.category,
                &b.message,
            ))
        });
        diagnostics.dedup_by(|a, b| {
            a.file == b.file
                && a.line == b.line
                && a.column == b.column
                && a.category == b.category
                && a.message == b.message
        });

        assert_eq!(
            diagnostics.len(),
            2,
            "Different messages should be kept even with same category/line"
        );
    }

    #[test]
    fn dedup_keeps_different_files_same_line() {
        let mut diagnostics = vec![
            make_diag(
                "a.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "msg",
            ),
            make_diag(
                "b.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "msg",
            ),
        ];

        diagnostics.sort_by(|a, b| {
            (&a.file, a.line, a.column, &a.category, &a.message).cmp(&(
                &b.file,
                b.line,
                b.column,
                &b.category,
                &b.message,
            ))
        });
        diagnostics.dedup_by(|a, b| {
            a.file == b.file
                && a.line == b.line
                && a.column == b.column
                && a.category == b.category
                && a.message == b.message
        });

        assert_eq!(
            diagnostics.len(),
            2,
            "Same line/category in different files should be kept"
        );
    }

    #[test]
    fn dedup_distinguishes_by_column() {
        let mut diagnostics = vec![
            make_diag(
                "a.md",
                5,
                Some(1),
                Severity::Warning,
                Category::DeadReference,
                "msg",
            ),
            make_diag(
                "a.md",
                5,
                Some(10),
                Severity::Warning,
                Category::DeadReference,
                "msg",
            ),
        ];

        diagnostics.sort_by(|a, b| {
            (&a.file, a.line, a.column, &a.category, &a.message).cmp(&(
                &b.file,
                b.line,
                b.column,
                &b.category,
                &b.message,
            ))
        });
        diagnostics.dedup_by(|a, b| {
            a.file == b.file
                && a.line == b.line
                && a.column == b.column
                && a.category == b.category
                && a.message == b.message
        });

        assert_eq!(diagnostics.len(), 2, "Different columns should be kept");
    }

    // ── 2. Severity override ────────────────────────────────────────────

    #[test]
    fn severity_override_changes_diagnostic_severity() {
        let mut config = Config::default();
        config.checkers.dead_reference.severity = Some(Severity::Info);

        let mut diagnostics = vec![make_diag(
            "a.md",
            1,
            None,
            Severity::Error,
            Category::DeadReference,
            "msg",
        )];

        for d in &mut diagnostics {
            if let Some(sev) = config.severity_override(&d.category) {
                d.severity = sev;
            }
        }

        assert_eq!(diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn severity_override_none_preserves_original() {
        let config = Config::default();
        // Default config should have no severity override for dead_reference
        assert!(config.severity_override(&Category::DeadReference).is_none());

        let mut diagnostics = vec![make_diag(
            "a.md",
            1,
            None,
            Severity::Warning,
            Category::DeadReference,
            "msg",
        )];

        for d in &mut diagnostics {
            if let Some(sev) = config.severity_override(&d.category) {
                d.severity = sev;
            }
        }

        assert_eq!(
            diagnostics[0].severity,
            Severity::Warning,
            "Without override, original severity should be preserved"
        );
    }

    #[test]
    fn severity_override_promotes_to_error() {
        let mut config = Config::default();
        config.checkers.vague_directive.severity = Some(Severity::Error);

        let mut diagnostics = vec![make_diag(
            "a.md",
            3,
            None,
            Severity::Info,
            Category::VagueDirective,
            "vague",
        )];

        for d in &mut diagnostics {
            if let Some(sev) = config.severity_override(&d.category) {
                d.severity = sev;
            }
        }

        assert_eq!(diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn severity_override_applies_per_category() {
        let mut config = Config::default();
        config.checkers.dead_reference.severity = Some(Severity::Info);
        // vague_directive left at None (no override)

        let mut diagnostics = vec![
            make_diag(
                "a.md",
                1,
                None,
                Severity::Error,
                Category::DeadReference,
                "dead",
            ),
            make_diag(
                "a.md",
                2,
                None,
                Severity::Warning,
                Category::VagueDirective,
                "vague",
            ),
        ];

        for d in &mut diagnostics {
            if let Some(sev) = config.severity_override(&d.category) {
                d.severity = sev;
            }
        }

        assert_eq!(
            diagnostics[0].severity,
            Severity::Info,
            "DeadReference should be overridden"
        );
        assert_eq!(
            diagnostics[1].severity,
            Severity::Warning,
            "VagueDirective should stay unchanged"
        );
    }

    // ── 3. Empty project — no matching files ────────────────────────────

    #[test]
    fn run_on_empty_project_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        // Create a non-markdown file so the directory is not empty but has no .md files
        std::fs::write(dir.path().join("readme.txt"), "hello").unwrap();

        let config = Config::default();
        let result = run(dir.path(), &config, false, None);

        assert!(
            result.is_err(),
            "run() should fail when no markdown files are found"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("No markdown files found"),
            "Error should mention no markdown files found, got: {err_msg}"
        );
    }

    #[test]
    fn scanned_files_empty_for_no_md_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not markdown").unwrap();

        let config = Config::default();
        let files = scanned_files(dir.path(), &config);
        assert!(
            files.is_empty(),
            "scanned_files should return empty for directory with no .md files"
        );
    }

    #[test]
    fn scanned_files_finds_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Hello").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not markdown").unwrap();

        let config = Config::default();
        let files = scanned_files(dir.path(), &config);
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "CLAUDE.md");
    }

    // ── 4. Suppression filtering ────────────────────────────────────────

    #[test]
    fn suppression_filters_matching_diagnostics() {
        use std::collections::HashMap;

        let file_path = Arc::new(PathBuf::from("test.md"));
        let ranges = vec![suppress::SuppressedRange {
            rule: Some("dead-reference".to_string()),
            start_line: 5,
            end_line: 5,
            used: std::cell::Cell::new(false),
            comment_line: 4,
        }];

        let mut suppressions = HashMap::new();
        suppressions.insert(file_path.clone(), ranges);

        let diagnostics = vec![
            make_diag(
                "test.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "broken",
            ),
            make_diag(
                "test.md",
                10,
                None,
                Severity::Warning,
                Category::DeadReference,
                "another",
            ),
            make_diag(
                "test.md",
                5,
                None,
                Severity::Warning,
                Category::VagueDirective,
                "vague",
            ),
        ];

        let filtered: Vec<_> = diagnostics
            .into_iter()
            .filter(|d| !suppress::is_suppressed(&suppressions, &d.file, d.line, &d.category))
            .collect();

        assert_eq!(
            filtered.len(),
            2,
            "Suppressed diagnostic should be filtered out"
        );
        assert_eq!(
            filtered[0].line, 10,
            "Non-suppressed same-category diagnostic should remain"
        );
        assert_eq!(
            filtered[1].category,
            Category::VagueDirective,
            "Different category on suppressed line should remain"
        );
    }

    #[test]
    fn suppression_with_no_rule_filters_all_categories() {
        use std::collections::HashMap;

        let file_path = Arc::new(PathBuf::from("test.md"));
        let ranges = vec![suppress::SuppressedRange {
            rule: None, // no rule means suppress all
            start_line: 3,
            end_line: 8,
            used: std::cell::Cell::new(false),
            comment_line: 3,
        }];

        let mut suppressions = HashMap::new();
        suppressions.insert(file_path.clone(), ranges);

        let diagnostics = vec![
            make_diag(
                "test.md",
                5,
                None,
                Severity::Warning,
                Category::DeadReference,
                "dead",
            ),
            make_diag(
                "test.md",
                5,
                None,
                Severity::Warning,
                Category::VagueDirective,
                "vague",
            ),
            make_diag(
                "test.md",
                5,
                None,
                Severity::Error,
                Category::PlaceholderText,
                "placeholder",
            ),
            make_diag(
                "test.md",
                10,
                None,
                Severity::Warning,
                Category::DeadReference,
                "outside",
            ),
        ];

        let filtered: Vec<_> = diagnostics
            .into_iter()
            .filter(|d| !suppress::is_suppressed(&suppressions, &d.file, d.line, &d.category))
            .collect();

        assert_eq!(
            filtered.len(),
            1,
            "Only diagnostic outside suppressed range should remain"
        );
        assert_eq!(filtered[0].line, 10);
    }

    // ── 5. End-to-end run() produces correct results ────────────────────

    #[test]
    fn run_produces_sorted_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("CLAUDE.md"),
            "# Test\n\nSome content here.\n",
        )
        .unwrap();

        let config = Config::default();
        let result = run(dir.path(), &config, false, None);
        assert!(result.is_ok(), "run() should succeed on a valid project");

        let diagnostics = result.unwrap().diagnostics;
        // Verify diagnostics are sorted by file, line, column, category, message
        for pair in diagnostics.windows(2) {
            let ord = (
                &pair[0].file,
                pair[0].line,
                pair[0].column,
                &pair[0].category,
                &pair[0].message,
            )
                .cmp(&(
                    &pair[1].file,
                    pair[1].line,
                    pair[1].column,
                    &pair[1].category,
                    &pair[1].message,
                ));
            assert!(
                ord != std::cmp::Ordering::Greater,
                "Diagnostics should be sorted, but found {:?} before {:?}",
                pair[0].message,
                pair[1].message
            );
        }
    }

    #[test]
    fn run_produces_no_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("CLAUDE.md"),
            "# Test\n\nSome content here.\n",
        )
        .unwrap();

        let config = Config::default();
        let result = run(dir.path(), &config, false, None).unwrap();

        // Check no adjacent diagnostics are identical
        for pair in result.diagnostics.windows(2) {
            let is_dup = pair[0].file == pair[1].file
                && pair[0].line == pair[1].line
                && pair[0].column == pair[1].column
                && pair[0].category == pair[1].category
                && pair[0].message == pair[1].message;
            assert!(
                !is_dup,
                "Found duplicate diagnostic: {} at line {}",
                pair[0].message, pair[0].line
            );
        }
    }

    #[test]
    fn run_with_suppress_comment_filters_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        // Use dead-reference checker: write a file with a reference to a non-existent file,
        // and suppress it with an inline comment.
        std::fs::write(
            dir.path().join("CLAUDE.md"),
            "# Test\n\n\
             <!-- spectralint-disable dead-reference -->\n\
             See [missing](./does-not-exist.md) for details.\n\
             <!-- spectralint-enable dead-reference -->\n\n\
             Some other content.\n",
        )
        .unwrap();

        let config = Config::default();
        let result = run(dir.path(), &config, false, None).unwrap();

        // The dead-reference diagnostic for line 4 should be suppressed
        let dead_refs: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.category == Category::DeadReference)
            .collect();
        assert!(
            dead_refs.is_empty(),
            "Dead reference diagnostic should be suppressed, but found: {:?}",
            dead_refs.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn run_without_cache_does_not_create_cache_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Test\n").unwrap();

        let config = Config::default();
        let _ = run(dir.path(), &config, false, None);

        let cache_path = dir.path().join(".spectralint-cache.json");
        assert!(
            !cache_path.exists(),
            "Cache file should not be created when use_cache is false"
        );
    }

    #[test]
    fn run_with_cache_creates_cache_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Test\n").unwrap();

        let config = Config::default();
        let _ = run(dir.path(), &config, true, None);

        let cache_path = dir.path().join(".spectralint-cache.json");
        assert!(
            cache_path.exists(),
            "Cache file should be created when use_cache is true"
        );
    }
}
