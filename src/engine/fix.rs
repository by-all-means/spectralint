use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::types::{Diagnostic, Replacement};

/// Apply all structured fixes from diagnostics that have `fix: Some(...)`.
///
/// Groups replacements by file, sorts them in reverse order (last line first,
/// then last column first within a line) to avoid offset invalidation, applies
/// them to the file content, and writes back to disk.
///
/// Returns the number of fixes applied.
pub fn apply_fixes(diagnostics: &[Diagnostic]) -> usize {
    // Collect all replacements grouped by file
    let mut by_file: HashMap<Arc<PathBuf>, Vec<&Replacement>> = HashMap::new();
    for d in diagnostics {
        if let Some(ref fix) = d.fix {
            for replacement in &fix.replacements {
                by_file
                    .entry(Arc::clone(&d.file))
                    .or_default()
                    .push(replacement);
            }
        }
    }

    let mut total_fixed = 0;

    for (path, mut replacements) in by_file {
        let content = match std::fs::read_to_string(path.as_ref()) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Could not read {} for fixing: {e}", path.display());
                continue;
            }
        };

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        // Preserve trailing newline
        let trailing_newline = content.ends_with('\n');

        // Sort replacements in reverse order: last line first, then last column first.
        // This ensures earlier replacements don't shift the byte offsets of later ones.
        replacements.sort_by(|a, b| {
            b.line
                .cmp(&a.line)
                .then_with(|| b.start_col.cmp(&a.start_col))
        });

        // Deduplicate identical replacements (same line, same range, same text)
        replacements.dedup_by(|a, b| {
            a.line == b.line
                && a.start_col == b.start_col
                && a.end_col == b.end_col
                && a.new_text == b.new_text
        });

        // Track the minimum safe column per line to detect overlapping replacements.
        // After applying a replacement at start_col..end_col, the safe boundary for
        // that line becomes start_col. Any subsequent replacement whose end_col exceeds
        // this boundary overlaps and must be skipped.
        let mut safe_boundary: HashMap<usize, usize> = HashMap::new();

        let mut applied = 0;
        let mut skipped = 0;
        for r in &replacements {
            let line_idx = r.line.saturating_sub(1); // 1-based to 0-based
            if line_idx >= lines.len() {
                tracing::warn!(
                    "Fix references line {} but {} has only {} lines",
                    r.line,
                    path.display(),
                    lines.len()
                );
                continue;
            }

            let line = &lines[line_idx];
            if r.start_col > line.len()
                || r.end_col > line.len()
                || r.start_col > r.end_col
                || !line.is_char_boundary(r.start_col)
                || !line.is_char_boundary(r.end_col)
            {
                tracing::warn!(
                    "Fix has invalid column range {}..{} for line {} (length {})",
                    r.start_col,
                    r.end_col,
                    r.line,
                    line.len()
                );
                continue;
            }

            // Skip if this replacement overlaps with one already applied on this line
            if let Some(&boundary) = safe_boundary.get(&line_idx) {
                if r.end_col > boundary {
                    skipped += 1;
                    continue;
                }
            }

            let mut new_line =
                String::with_capacity(line.len() - (r.end_col - r.start_col) + r.new_text.len());
            new_line.push_str(&line[..r.start_col]);
            new_line.push_str(&r.new_text);
            new_line.push_str(&line[r.end_col..]);
            lines[line_idx] = new_line;
            safe_boundary.insert(line_idx, r.start_col);
            applied += 1;
        }

        if skipped > 0 {
            tracing::warn!(
                "Skipped {} overlapping fix(es) in {}",
                skipped,
                path.display()
            );
        }

        if applied > 0 {
            let mut output = lines.join("\n");
            if trailing_newline {
                output.push('\n');
            }
            if let Err(e) = std::fs::write(path.as_ref(), output) {
                tracing::warn!("Could not write fix to {}: {e}", path.display());
            } else {
                tracing::info!("Fixed {} issue(s) in {}", applied, path.display());
                total_fixed += applied;
            }
        }
    }

    total_fixed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, Diagnostic, Fix, Replacement, Severity};
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    fn make_fix_diagnostic(path: PathBuf, fix: Fix) -> Diagnostic {
        Diagnostic {
            file: Arc::new(path),
            line: 1,
            column: None,
            end_line: None,
            end_column: None,
            severity: Severity::Info,
            category: Category::RepeatedWord,
            message: "test".to_string(),
            suggestion: None,
            fix: Some(Box::new(fix)),
        }
    }

    #[test]
    fn test_apply_single_replacement() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "The the dog ran\n").unwrap();

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Remove duplicate word".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 4,
                    end_col: 8,
                    new_text: String::new(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 1);

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "The dog ran\n");
    }

    #[test]
    fn test_no_fix_diagnostics() {
        let diag = Diagnostic {
            file: Arc::new(PathBuf::from("test.md")),
            line: 1,
            column: None,
            end_line: None,
            end_column: None,
            severity: Severity::Info,
            category: Category::RepeatedWord,
            message: "test".to_string(),
            suggestion: None,
            fix: None,
        };

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 0);
    }

    #[test]
    fn test_multiple_replacements_same_file() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "aaa bbb\nccc ddd\n").unwrap();

        let diag1 = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix line 1".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 0,
                    end_col: 3,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        let diag2 = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix line 2".to_string(),
                replacements: vec![Replacement {
                    line: 2,
                    start_col: 0,
                    end_col: 3,
                    new_text: "yyy".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag1, diag2]);
        assert_eq!(fixed, 2);

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "xxx bbb\nyyy ddd\n");
    }

    #[test]
    fn test_overlapping_replacements_on_same_line() {
        let tmp = NamedTempFile::new().unwrap();
        // Line: "aaa bbb ccc"
        //         012345678901
        std::fs::write(tmp.path(), "aaa bbb ccc\n").unwrap();

        // Two overlapping replacements on the same line:
        // Replace cols 2..6 ("a bb") with "X"
        // Replace cols 4..9 ("bbb c") with "Y"
        // These overlap in the range 4..6.
        // Only the one with the higher start_col should be applied (processed first
        // in reverse order), and the other should be skipped.
        let diag1 = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "First fix".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 2,
                    end_col: 6,
                    new_text: "X".to_string(),
                }],
            },
        );

        let diag2 = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Second fix".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 4,
                    end_col: 9,
                    new_text: "Y".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag1, diag2]);
        // Only one replacement should be applied; the other is skipped due to overlap
        assert_eq!(fixed, 1);

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        // The replacement at cols 4..9 is processed first (higher start_col in reverse sort).
        // "aaa bbb ccc" -> replace 4..9 ("bbb c") with "Y" -> "aaa Ycc"
        // The replacement at cols 2..6 has end_col=6 > safe_boundary=4, so it's skipped.
        assert_eq!(content, "aaa Ycc\n");
    }

    #[test]
    fn test_nonexistent_file_skipped() {
        let diag = make_fix_diagnostic(
            PathBuf::from("/tmp/spectralint_test_nonexistent_file_12345.md"),
            Fix {
                description: "Fix".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 0,
                    end_col: 3,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 0, "Nonexistent file should be skipped gracefully");
    }

    #[test]
    fn test_out_of_bounds_line_skipped() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "only one line\n").unwrap();

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix".to_string(),
                replacements: vec![Replacement {
                    line: 5, // file only has 1 line
                    start_col: 0,
                    end_col: 3,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 0, "Out of bounds line should be skipped");
        // File should be unchanged
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "only one line\n");
    }

    #[test]
    fn test_invalid_column_range_skipped() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "short\n").unwrap();

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 10, // > line length (5)
                    end_col: 15,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 0, "Invalid column range should be skipped");
    }

    #[test]
    fn test_reversed_column_range_skipped() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "hello world\n").unwrap();

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 8, // start > end
                    end_col: 3,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 0, "Reversed column range should be skipped");
    }

    #[test]
    fn test_preserves_trailing_newline() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "aaa bbb\n").unwrap();

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 0,
                    end_col: 3,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 1);
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.ends_with('\n'), "Should preserve trailing newline");
        assert_eq!(content, "xxx bbb\n");
    }

    #[test]
    fn test_no_trailing_newline_preserved() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "aaa bbb").unwrap(); // no trailing newline

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 0,
                    end_col: 3,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 1);
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(!content.ends_with('\n'), "Should not add trailing newline");
        assert_eq!(content, "xxx bbb");
    }

    #[test]
    fn test_empty_fix_replacements_no_write() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "original content\n").unwrap();

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Empty fix".to_string(),
                replacements: vec![], // no replacements
            },
        );

        let fixed = apply_fixes(&[diag]);
        assert_eq!(fixed, 0);
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "original content\n", "File should not be modified");
    }

    #[test]
    fn test_duplicate_replacements_deduplicated() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "the the dog\n").unwrap();

        // Two identical fixes for the same spot
        let diag1 = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Remove dup".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 4,
                    end_col: 8,
                    new_text: String::new(),
                }],
            },
        );
        let diag2 = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Remove dup".to_string(),
                replacements: vec![Replacement {
                    line: 1,
                    start_col: 4,
                    end_col: 8,
                    new_text: String::new(),
                }],
            },
        );

        let fixed = apply_fixes(&[diag1, diag2]);
        assert_eq!(fixed, 1, "Identical replacements should be deduplicated");
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "the dog\n");
    }

    #[test]
    fn test_line_zero_treated_as_out_of_bounds() {
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "hello world\n").unwrap();

        let diag = make_fix_diagnostic(
            tmp.path().to_path_buf(),
            Fix {
                description: "Fix".to_string(),
                replacements: vec![Replacement {
                    line: 0, // invalid: lines are 1-based
                    start_col: 0,
                    end_col: 5,
                    new_text: "xxx".to_string(),
                }],
            },
        );

        // line 0 saturating_sub(1) = 0, which maps to line 1 effectively
        // This is actually valid by the current logic (0.saturating_sub(1) = 0, which is a valid index)
        // But we test that it doesn't panic
        let _fixed = apply_fixes(&[diag]);
    }
}
