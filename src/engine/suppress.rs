use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::parser::types::{InlineSuppress, ParsedFile, SuppressKind};

#[derive(Debug)]
pub struct SuppressedRange {
    rule: Option<String>,
    start_line: usize,
    end_line: usize,
}

pub fn build_suppression_set(files: &[ParsedFile]) -> HashMap<PathBuf, Vec<SuppressedRange>> {
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
                    });
                }
            }
            SuppressKind::DisableNextLine => {
                ranges.push(SuppressedRange {
                    rule: comment.rule.clone(),
                    start_line: comment.line + 1,
                    end_line: comment.line + 1,
                });
            }
        }
    }

    for (rule, start) in open_blocks {
        ranges.push(SuppressedRange {
            rule,
            start_line: start,
            end_line: total_lines,
        });
    }

    ranges
}

pub fn is_suppressed(
    suppressions: &HashMap<PathBuf, Vec<SuppressedRange>>,
    file: &Path,
    line: usize,
    category: &str,
) -> bool {
    suppressions.get(file).is_some_and(|ranges| {
        ranges.iter().any(|range| {
            line >= range.start_line
                && line <= range.end_line
                && range.rule.as_ref().is_none_or(|rule| rule == category)
        })
    })
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
        map.insert(PathBuf::from("test.md"), ranges);

        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            6,
            "dead-reference"
        ));
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            7,
            "dead-reference"
        ));
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            6,
            "vague-directive"
        ));
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
        map.insert(PathBuf::from("test.md"), ranges);

        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            5,
            "dead-reference"
        ));
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            5,
            "vague-directive"
        ));
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            9,
            "dead-reference"
        ));
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
        map.insert(PathBuf::from("test.md"), ranges);

        // dead-reference at line 5 should be suppressed
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            5,
            "dead-reference"
        ));

        // vague-directive at line 5 should NOT be suppressed
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            5,
            "vague-directive"
        ));

        // naming-inconsistency at line 5 should NOT be suppressed
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            5,
            "naming-inconsistency"
        ));

        // dead-reference at line 9 (outside range) should NOT be suppressed
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            9,
            "dead-reference"
        ));
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
        map.insert(PathBuf::from("test.md"), ranges);

        // Line 6 should be suppressed for all rules
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            6,
            "dead-reference"
        ));
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            6,
            "vague-directive"
        ));
        assert!(is_suppressed(&map, Path::new("test.md"), 6, "enum-drift"));

        // Line 7 should NOT be suppressed
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            7,
            "dead-reference"
        ));
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

    // ── Item 7: Suppress comment edge cases ──────────────────────────────

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
        // Inner disable at line 5 closed by enable at line 8
        // Outer disable at line 3 remains open → extends to EOF
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
        map.insert(PathBuf::from("test.md"), ranges);

        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            6,
            "dead-reference"
        ));
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            6,
            "vague-directive"
        ));
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            8,
            "vague-directive"
        ));
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            8,
            "dead-reference"
        ));
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
        map.insert(PathBuf::from("test.md"), ranges);

        // Line 4 inside global disable: everything suppressed
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            4,
            "dead-reference"
        ));
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            4,
            "vague-directive"
        ));
    }

    #[test]
    fn test_suppression_for_unknown_file() {
        let map = HashMap::new();
        assert!(
            !is_suppressed(&map, Path::new("unknown.md"), 5, "dead-reference"),
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
        map.insert(PathBuf::from("test.md"), ranges);

        // Line 6 suppressed by first disable-next-line
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            6,
            "dead-reference"
        ));
        // Line 7 suppressed by second disable-next-line
        assert!(is_suppressed(
            &map,
            Path::new("test.md"),
            7,
            "dead-reference"
        ));
        // Line 8 not suppressed
        assert!(!is_suppressed(
            &map,
            Path::new("test.md"),
            8,
            "dead-reference"
        ));
    }
}
