use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::emit;
use crate::parser::types::{InlineSuppress, ParsedFile, SuppressKind};
use crate::types::Category;

#[derive(Debug)]
pub(super) struct SuppressedRange {
    pub(super) rule: Option<String>,
    pub(super) start_line: usize,
    pub(super) end_line: usize,
    pub(super) used: Cell<bool>,
    /// The line where the disable comment appears (for unused suppression diagnostics).
    pub(super) comment_line: usize,
}

pub(super) fn build_suppression_set(
    files: &[ParsedFile],
) -> HashMap<Arc<PathBuf>, Vec<SuppressedRange>> {
    files
        .iter()
        .filter_map(|file| {
            let ranges = build_ranges(&file.suppress_comments, file.raw_lines.len());
            (!ranges.is_empty()).then(|| (file.path.clone(), ranges))
        })
        .collect()
}

fn build_ranges(comments: &[InlineSuppress], total_lines: usize) -> Vec<SuppressedRange> {
    let mut ranges = Vec::new();
    let mut open_blocks = Vec::new();

    for comment in comments {
        match &comment.kind {
            SuppressKind::Disable => {
                open_blocks.push((comment.rule.clone(), comment.line));
            }
            SuppressKind::Enable => {
                if let Some(pos) = open_blocks
                    .iter()
                    .rposition(|(rule, _)| *rule == comment.rule)
                {
                    let (rule, start) = open_blocks.remove(pos);
                    ranges.push(SuppressedRange {
                        rule,
                        start_line: start,
                        end_line: comment.line,
                        used: Cell::new(false),
                        comment_line: start,
                    });
                }
            }
            SuppressKind::DisableNextLine => {
                ranges.push(SuppressedRange {
                    rule: comment.rule.clone(),
                    start_line: comment.line + 1,
                    end_line: comment.line + 1,
                    used: Cell::new(false),
                    comment_line: comment.line,
                });
            }
        }
    }

    for (rule, start) in open_blocks {
        ranges.push(SuppressedRange {
            rule,
            start_line: start,
            end_line: total_lines,
            used: Cell::new(false),
            comment_line: start,
        });
    }

    ranges
}

pub(super) fn is_suppressed(
    suppressions: &HashMap<Arc<PathBuf>, Vec<SuppressedRange>>,
    file: &Arc<PathBuf>,
    line: usize,
    category: &Category,
) -> bool {
    let Some(ranges) = suppressions.get(file) else {
        return false;
    };
    let mut matched = false;
    for range in ranges {
        if line >= range.start_line && line <= range.end_line {
            let rule_matches = match range.rule.as_deref() {
                None => true,
                Some(rule) => match category {
                    Category::CustomPattern(_) => {
                        rule.strip_prefix("custom:") == Some(category.as_str())
                    }
                    _ => category.as_str() == rule,
                },
            };
            if rule_matches {
                range.used.set(true);
                matched = true;
            }
        }
    }
    matched
}

/// Collect all known rule names from built-in categories and custom patterns.
/// Derives from `AVAILABLE_RULES` in explain.rs to avoid maintaining a separate list.
pub(super) fn all_known_rule_names(
    custom_patterns: &[crate::config::CustomPattern],
) -> HashSet<String> {
    let mut names: HashSet<String> = crate::cli::explain::AVAILABLE_RULES
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect();
    for pat in custom_patterns {
        names.insert(format!("custom:{}", pat.name));
    }
    names
}

/// Validate that all rule names used in suppress comments are recognized.
/// Returns diagnostics for unrecognized rule names.
pub(super) fn validate_suppress_rules(
    files: &[ParsedFile],
    known_rules: &HashSet<String>,
) -> Vec<crate::types::Diagnostic> {
    let mut result = crate::types::CheckResult::default();
    for file in files {
        for comment in &file.suppress_comments {
            if let Some(ref rule) = comment.rule {
                if !known_rules.contains(rule.as_str()) {
                    emit!(
                        result,
                        file.path,
                        comment.line,
                        crate::types::Severity::Warning,
                        Category::InvalidSuppression,
                        suggest: "Check for typos — run `spectralint explain` to see all available rules",
                        "Unrecognized rule name in suppress comment: \"{rule}\""
                    );
                }
            }
        }
    }
    result.diagnostics
}

/// Find suppression ranges that were never used (no diagnostic was suppressed by them).
pub(super) fn find_unused_suppressions(
    suppressions: &HashMap<Arc<PathBuf>, Vec<SuppressedRange>>,
) -> Vec<crate::types::Diagnostic> {
    let mut result = crate::types::CheckResult::default();
    for (file, ranges) in suppressions {
        for range in ranges {
            if !range.used.get() {
                let rule_desc = match range.rule.as_deref() {
                    Some(r) => format!("\"{r}\""),
                    None => "all rules".to_string(),
                };
                emit!(
                    result,
                    file,
                    range.comment_line,
                    crate::types::Severity::Info,
                    Category::UnusedSuppression,
                    suggest: "Remove the unused suppress comment to keep the file clean",
                    "Suppress comment for {rule_desc} did not suppress any diagnostic"
                );
            }
        }
    }
    result.diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::InlineSuppress;

    #[test]
    fn test_disable_next_line() {
        let comments = vec![InlineSuppress {
            line: 5,
            kind: SuppressKind::DisableNextLine,
            rule: Some("dead-reference".to_string()),
        }];
        let ranges = build_ranges(&comments, 20);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 6);
        assert_eq!(ranges[0].end_line, 6);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        assert!(is_suppressed(&map, &key, 6, &Category::DeadReference));
        assert!(!is_suppressed(&map, &key, 7, &Category::DeadReference));
        assert!(!is_suppressed(&map, &key, 6, &Category::VagueDirective));
    }

    #[test]
    fn test_disable_enable_block() {
        let comments = vec![
            InlineSuppress {
                line: 3,
                kind: SuppressKind::Disable,
                rule: None,
            },
            InlineSuppress {
                line: 8,
                kind: SuppressKind::Enable,
                rule: None,
            },
        ];
        let ranges = build_ranges(&comments, 20);
        assert_eq!(ranges.len(), 1);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        assert!(is_suppressed(&map, &key, 5, &Category::DeadReference));
        assert!(is_suppressed(&map, &key, 5, &Category::VagueDirective));
        assert!(!is_suppressed(&map, &key, 9, &Category::DeadReference));
    }

    #[test]
    fn test_rule_specific_does_not_suppress_other_rules() {
        let comments = vec![
            InlineSuppress {
                line: 3,
                kind: SuppressKind::Disable,
                rule: Some("dead-reference".to_string()),
            },
            InlineSuppress {
                line: 8,
                kind: SuppressKind::Enable,
                rule: Some("dead-reference".to_string()),
            },
        ];
        let ranges = build_ranges(&comments, 20);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        assert!(is_suppressed(&map, &key, 5, &Category::DeadReference));
        assert!(!is_suppressed(&map, &key, 5, &Category::VagueDirective));
        assert!(!is_suppressed(
            &map,
            &key,
            5,
            &Category::NamingInconsistency
        ));
        assert!(!is_suppressed(&map, &key, 9, &Category::DeadReference));
    }

    #[test]
    fn test_disable_next_line_no_rule_suppresses_all() {
        let comments = vec![InlineSuppress {
            line: 5,
            kind: SuppressKind::DisableNextLine,
            rule: None,
        }];
        let ranges = build_ranges(&comments, 20);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        assert!(is_suppressed(&map, &key, 6, &Category::DeadReference));
        assert!(is_suppressed(&map, &key, 6, &Category::VagueDirective));
        assert!(is_suppressed(&map, &key, 6, &Category::EnumDrift,));
        assert!(!is_suppressed(&map, &key, 7, &Category::DeadReference));
    }

    #[test]
    fn test_unclosed_disable_extends_to_eof() {
        let comments = vec![InlineSuppress {
            line: 10,
            kind: SuppressKind::Disable,
            rule: Some("vague-directive".to_string()),
        }];
        let ranges = build_ranges(&comments, 30);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 10);
        assert_eq!(ranges[0].end_line, 30);
    }

    #[test]
    fn test_nested_disable_blocks() {
        let comments = vec![
            InlineSuppress {
                line: 3,
                kind: SuppressKind::Disable,
                rule: None,
            },
            InlineSuppress {
                line: 5,
                kind: SuppressKind::Disable,
                rule: None,
            },
            InlineSuppress {
                line: 8,
                kind: SuppressKind::Enable,
                rule: None,
            },
        ];
        let ranges = build_ranges(&comments, 20);
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_enable_without_disable() {
        let comments = vec![InlineSuppress {
            line: 10,
            kind: SuppressKind::Enable,
            rule: None,
        }];
        let ranges = build_ranges(&comments, 20);
        assert!(
            ranges.is_empty(),
            "Enable without prior disable should produce no ranges"
        );
    }

    #[test]
    fn test_consecutive_disable_next_line() {
        let comments = vec![
            InlineSuppress {
                line: 5,
                kind: SuppressKind::DisableNextLine,
                rule: Some("dead-reference".to_string()),
            },
            InlineSuppress {
                line: 7,
                kind: SuppressKind::DisableNextLine,
                rule: Some("vague-directive".to_string()),
            },
        ];
        let ranges = build_ranges(&comments, 20);
        assert_eq!(ranges.len(), 2);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        assert!(is_suppressed(&map, &key, 6, &Category::DeadReference));
        assert!(!is_suppressed(&map, &key, 6, &Category::VagueDirective));
        assert!(is_suppressed(&map, &key, 8, &Category::VagueDirective));
        assert!(!is_suppressed(&map, &key, 8, &Category::DeadReference));
    }

    #[test]
    fn test_global_disable_then_rule_specific_disable() {
        let comments = vec![
            InlineSuppress {
                line: 3,
                kind: SuppressKind::Disable,
                rule: None,
            },
            InlineSuppress {
                line: 5,
                kind: SuppressKind::Disable,
                rule: Some("dead-reference".to_string()),
            },
            InlineSuppress {
                line: 8,
                kind: SuppressKind::Enable,
                rule: None,
            },
        ];
        let ranges = build_ranges(&comments, 20);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        assert!(is_suppressed(&map, &key, 4, &Category::DeadReference));
        assert!(is_suppressed(&map, &key, 4, &Category::VagueDirective));
    }

    #[test]
    fn test_suppression_for_unknown_file() {
        let map = HashMap::new();
        let key = Arc::new(PathBuf::from("unknown.md"));
        assert!(
            !is_suppressed(&map, &key, 5, &Category::DeadReference),
            "File not in suppression map should never be suppressed"
        );
    }

    #[test]
    fn test_consecutive_same_rule_disable_next_line() {
        let comments = vec![
            InlineSuppress {
                line: 5,
                kind: SuppressKind::DisableNextLine,
                rule: Some("dead-reference".to_string()),
            },
            InlineSuppress {
                line: 6,
                kind: SuppressKind::DisableNextLine,
                rule: Some("dead-reference".to_string()),
            },
        ];
        let ranges = build_ranges(&comments, 20);
        assert_eq!(ranges.len(), 2);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        assert!(is_suppressed(&map, &key, 6, &Category::DeadReference));
        assert!(is_suppressed(&map, &key, 7, &Category::DeadReference));
        assert!(!is_suppressed(&map, &key, 8, &Category::DeadReference));
    }

    #[test]
    fn test_unused_suppression_detected() {
        let comments = vec![InlineSuppress {
            line: 5,
            kind: SuppressKind::DisableNextLine,
            rule: Some("dead-reference".to_string()),
        }];
        let ranges = build_ranges(&comments, 20);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        // Don't suppress anything — range should remain unused
        let unused = find_unused_suppressions(&map);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].category, Category::UnusedSuppression);
    }

    #[test]
    fn test_used_suppression_not_flagged() {
        let comments = vec![InlineSuppress {
            line: 5,
            kind: SuppressKind::DisableNextLine,
            rule: Some("dead-reference".to_string()),
        }];
        let ranges = build_ranges(&comments, 20);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        // Suppress a diagnostic — range should be marked used
        assert!(is_suppressed(&map, &key, 6, &Category::DeadReference));

        let unused = find_unused_suppressions(&map);
        assert!(unused.is_empty());
    }

    #[test]
    fn test_invalid_suppression_detected() {
        let known = all_known_rule_names(&[]);
        let file = ParsedFile {
            path: Arc::new(PathBuf::from("test.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![InlineSuppress {
                line: 5,
                kind: SuppressKind::DisableNextLine,
                rule: Some("typo-rule-name".to_string()),
            }],
            raw_lines: vec!["test".to_string()],
            in_code_block: vec![false],
        };

        let diags = validate_suppress_rules(&[file], &known);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].category, Category::InvalidSuppression);
    }

    #[test]
    fn test_valid_suppression_not_flagged() {
        let known = all_known_rule_names(&[]);
        let file = ParsedFile {
            path: Arc::new(PathBuf::from("test.md")),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![InlineSuppress {
                line: 5,
                kind: SuppressKind::DisableNextLine,
                rule: Some("dead-reference".to_string()),
            }],
            raw_lines: vec!["test".to_string()],
            in_code_block: vec![false],
        };

        let diags = validate_suppress_rules(&[file], &known);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_overlapping_suppress_ranges() {
        // Two disable/enable blocks that overlap:
        // Block 1: lines 3-10 (all rules)
        // Block 2: lines 5-8 (dead-reference only)
        let comments = vec![
            InlineSuppress {
                line: 3,
                kind: SuppressKind::Disable,
                rule: None,
            },
            InlineSuppress {
                line: 5,
                kind: SuppressKind::Disable,
                rule: Some("dead-reference".to_string()),
            },
            InlineSuppress {
                line: 8,
                kind: SuppressKind::Enable,
                rule: Some("dead-reference".to_string()),
            },
            InlineSuppress {
                line: 10,
                kind: SuppressKind::Enable,
                rule: None,
            },
        ];
        let ranges = build_ranges(&comments, 20);
        assert_eq!(ranges.len(), 2, "Should produce two suppression ranges");

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        // Line 4: inside global block only → all rules suppressed
        assert!(is_suppressed(&map, &key, 4, &Category::DeadReference));
        assert!(is_suppressed(&map, &key, 4, &Category::VagueDirective));

        // Line 6: inside both blocks → all rules suppressed (global covers everything)
        assert!(is_suppressed(&map, &key, 6, &Category::DeadReference));
        assert!(is_suppressed(&map, &key, 6, &Category::VagueDirective));

        // Line 9: inside global block (3-10) but outside dead-reference block (5-8)
        // → all rules still suppressed because global block is active
        assert!(is_suppressed(&map, &key, 9, &Category::DeadReference));
        assert!(is_suppressed(&map, &key, 9, &Category::VagueDirective));

        // Line 11: outside both blocks → nothing suppressed
        assert!(!is_suppressed(&map, &key, 11, &Category::DeadReference));
        assert!(!is_suppressed(&map, &key, 11, &Category::VagueDirective));
    }

    #[test]
    fn test_suppress_custom_pattern() {
        // Suppress a custom:my-rule pattern
        let comments = vec![
            InlineSuppress {
                line: 5,
                kind: SuppressKind::Disable,
                rule: Some("custom:my-rule".to_string()),
            },
            InlineSuppress {
                line: 10,
                kind: SuppressKind::Enable,
                rule: Some("custom:my-rule".to_string()),
            },
        ];
        let ranges = build_ranges(&comments, 20);
        assert_eq!(ranges.len(), 1);

        let mut map = HashMap::new();
        let key = Arc::new(PathBuf::from("test.md"));
        map.insert(key.clone(), ranges);

        let custom_category = Category::CustomPattern("my-rule".into());

        // Line 7: inside the suppress block → custom:my-rule should be suppressed
        assert!(
            is_suppressed(&map, &key, 7, &custom_category),
            "custom:my-rule should be suppressed within the disable block"
        );

        // Other categories should NOT be suppressed
        assert!(
            !is_suppressed(&map, &key, 7, &Category::DeadReference),
            "dead-reference should not be suppressed by custom:my-rule disable"
        );

        // A different custom pattern should NOT be suppressed
        let other_custom = Category::CustomPattern("other-rule".into());
        assert!(
            !is_suppressed(&map, &key, 7, &other_custom),
            "custom:other-rule should not be suppressed by custom:my-rule disable"
        );

        // Outside the block → not suppressed
        assert!(
            !is_suppressed(&map, &key, 11, &custom_category),
            "custom:my-rule should not be suppressed outside the block"
        );
    }
}
