use rayon::prelude::*;

use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Diagnostic, RuleMeta, Severity};

use super::utils::{match_conflict_patterns, ScopeFilter, CONFLICT_PAIRS};
use super::Checker;

pub(crate) struct CrossFileContradictionChecker {
    scope: ScopeFilter,
}

impl CrossFileContradictionChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Returns true if `ancestor` is a directory ancestor of `descendant`.
/// Both paths should be absolute or both relative to the same root.
fn is_ancestor_descendant(ancestor: &std::path::Path, descendant: &std::path::Path) -> bool {
    let Some(ancestor_dir) = ancestor.parent() else {
        return false;
    };
    let Some(descendant_dir) = descendant.parent() else {
        return false;
    };

    // descendant_dir must start with ancestor_dir and be deeper
    descendant_dir != ancestor_dir && descendant_dir.starts_with(ancestor_dir)
}

impl Checker for CrossFileContradictionChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "cross-file-contradiction",
            description: "Detects contradictory instructions across files",
            default_severity: Severity::Warning,
            strict_only: true,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        if ctx.files.len() < 2 {
            return result;
        }

        // Pre-collect directive lines for each file (borrows from ctx.files)
        let file_lines: Vec<Vec<(usize, &str)>> = ctx
            .files
            .iter()
            .map(|f| f.non_code_lines().map(|(i, line)| (i + 1, line)).collect())
            .collect();

        // Pre-compute bitmasks using RegexSet for fast batch matching.
        // Bit (2*N) = matches pair[N].a, bit (2*N+1) = matches pair[N].b.
        let file_masks: Vec<u64> = file_lines
            .iter()
            .map(|lines| {
                let mut mask: u64 = 0;
                for (_, line) in lines {
                    let matches = match_conflict_patterns(line);
                    for idx in matches.iter() {
                        if idx < 64 {
                            mask |= 1u64 << idx;
                        }
                    }
                }
                mask
            })
            .collect();

        // Generate all (i, j) pairs where i < j and both are in scope
        let pairs: Vec<(usize, usize)> = (0..ctx.files.len())
            .filter(|&i| self.scope.includes(&ctx.files[i].path, &ctx.project_root))
            .flat_map(|i| {
                ((i + 1)..ctx.files.len())
                    .filter(|&j| self.scope.includes(&ctx.files[j].path, &ctx.project_root))
                    .map(move |j| (i, j))
            })
            .collect();

        let pair_diagnostics: Vec<Diagnostic> = pairs
            .par_iter()
            .filter_map(|&(i, j)| {
                // Only compare ancestor-descendant pairs
                if !is_ancestor_descendant(&ctx.files[i].path, &ctx.files[j].path)
                    && !is_ancestor_descendant(&ctx.files[j].path, &ctx.files[i].path)
                {
                    return None;
                }

                // Skip if no overlapping conflict patterns between the two files.
                // For each pair N, a_bit = 2*N, b_bit = 2*N+1.
                // Contradiction requires (i.a & j.b) or (i.b & j.a).
                // Build "even" and "odd" masks by shifting: even bits = side A, odd bits = side B.
                let mi = file_masks[i];
                let mj = file_masks[j];
                // Check if any pair has i's A matching j's B or i's B matching j's A
                let even_mask: u64 = 0x5555_5555_5555_5555; // bits 0,2,4,...
                let odd_mask: u64 = 0xAAAA_AAAA_AAAA_AAAA; // bits 1,3,5,...
                let i_has_a = mi & even_mask;
                let i_has_b = mi & odd_mask;
                let j_has_a = mj & even_mask;
                let j_has_b = mj & odd_mask;
                // Shift odd bits down to align with even bits for comparison
                let has_overlap =
                    (i_has_a & (j_has_b >> 1)) != 0 || ((i_has_b >> 1) & j_has_a) != 0;
                if !has_overlap {
                    return None;
                }

                // Find the first contradiction (at most one diagnostic per file pair)
                for (pair_idx, pair) in CONFLICT_PAIRS.iter().enumerate() {
                    if pair_idx >= 32 {
                        break;
                    }
                    let a_bit = 1u64 << (2 * pair_idx);
                    let b_bit = 1u64 << (2 * pair_idx + 1);

                    // Check both directions: (i=A, j=B) and (i=B, j=A)
                    for (side_i_bit, side_j_bit, side_i, side_j) in [
                        (a_bit, b_bit, &pair.a, &pair.b),
                        (b_bit, a_bit, &pair.b, &pair.a),
                    ] {
                        if file_masks[i] & side_i_bit == 0 || file_masks[j] & side_j_bit == 0 {
                            continue;
                        }

                        let i_match = file_lines[i].iter().find(|(_, line)| side_i.is_match(line));
                        let j_match = file_lines[j].iter().find(|(_, line)| side_j.is_match(line));

                        if let (Some((line_i, _)), Some((line_j, _))) = (i_match, j_match) {
                            let rel_i = ctx.files[i]
                                .path
                                .strip_prefix(&ctx.project_root)
                                .unwrap_or(&ctx.files[i].path);
                            let rel_j = ctx.files[j]
                                .path
                                .strip_prefix(&ctx.project_root)
                                .unwrap_or(&ctx.files[j].path);
                            return Some(Diagnostic {
                                file: ctx.files[i].path.clone(),
                                line: *line_i,
                                column: None,
                                end_line: None,
                                end_column: None,
                                severity: Severity::Warning,
                                category: Category::CrossFileContradiction,
                                message: format!(
                                    "Cross-file contradiction ({}): {} line {} vs {} line {}",
                                    pair.description,
                                    rel_i.display(),
                                    line_i,
                                    rel_j.display(),
                                    line_j
                                ),
                                suggestion: Some(
                                    "Resolve the contradiction or add a comment explaining the intentional override".to_string(),
                                ),
                                fix: None,
                            });
                        }
                    }
                }

                None
            })
            .collect();

        result.diagnostics.extend(pair_diagnostics);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::ParsedFile;
    use std::collections::HashSet;

    fn make_file(root: &std::path::Path, rel: &str, lines: &[&str]) -> ParsedFile {
        let raw_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let in_code_block = crate::parser::build_code_block_mask(&raw_lines);
        ParsedFile {
            path: std::sync::Arc::new(root.join(rel)),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines,
            in_code_block,
        }
    }

    fn run_check(files: Vec<ParsedFile>, root: &std::path::Path) -> CheckResult {
        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            canonical_root: None,
            filename_index: HashSet::new(),
            historical_indices: HashSet::new(),
        };
        CrossFileContradictionChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_ancestor_descendant_contradiction() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "CLAUDE.md", &["Always use formal tone."]),
            make_file(root, "backend/CLAUDE.md", &["Keep it casual and friendly."]),
        ];
        let result = run_check(files, root);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("tone"));
    }

    #[test]
    fn test_sibling_dirs_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "frontend/CLAUDE.md", &["Always use formal tone."]),
            make_file(root, "backend/CLAUDE.md", &["Keep it casual and friendly."]),
        ];
        let result = run_check(files, root);
        assert!(
            result.diagnostics.is_empty(),
            "Sibling directories should not trigger cross-file contradiction"
        );
    }

    #[test]
    fn test_no_contradiction_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "CLAUDE.md", &["Always use formal tone."]),
            make_file(root, "backend/CLAUDE.md", &["Run tests before committing."]),
        ];
        let result = run_check(files, root);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_deep_nesting_contradiction() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "CLAUDE.md", &["Always write tests for new code."]),
            make_file(
                root,
                "src/backend/CLAUDE.md",
                &["Skip tests for trivial changes."],
            ),
        ];
        let result = run_check(files, root);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("testing"));
    }

    #[test]
    fn test_reverse_direction_detected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "CLAUDE.md", &["Keep it casual and friendly."]),
            make_file(root, "backend/CLAUDE.md", &["Always use formal tone."]),
        ];
        let result = run_check(files, root);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("tone"));
    }

    #[test]
    fn test_single_file_no_flag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![make_file(root, "CLAUDE.md", &["Always use formal tone."])];
        let result = run_check(files, root);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_is_ancestor_descendant_fn() {
        let root = std::path::Path::new("/project");
        assert!(is_ancestor_descendant(
            &root.join("CLAUDE.md"),
            &root.join("backend/CLAUDE.md")
        ));
        assert!(!is_ancestor_descendant(
            &root.join("frontend/CLAUDE.md"),
            &root.join("backend/CLAUDE.md")
        ));
        assert!(is_ancestor_descendant(
            &root.join("CLAUDE.md"),
            &root.join("src/backend/CLAUDE.md")
        ));
        assert!(!is_ancestor_descendant(
            &root.join("CLAUDE.md"),
            &root.join("AGENTS.md")
        ));
    }

    #[test]
    fn test_files_with_no_overlapping_topics() {
        // Two ancestor-descendant files that talk about completely different things
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "CLAUDE.md", &["Use Python 3.11 for all scripts."]),
            make_file(
                root,
                "backend/CLAUDE.md",
                &["Database migrations go in the db/ folder."],
            ),
        ];
        let result = run_check(files, root);
        assert!(
            result.diagnostics.is_empty(),
            "Files with no overlapping conflict topics should not flag"
        );
    }

    #[test]
    fn test_files_with_exact_same_content() {
        // Two ancestor-descendant files with identical content — no contradiction
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "CLAUDE.md", &["Always use formal tone."]),
            make_file(root, "backend/CLAUDE.md", &["Always use formal tone."]),
        ];
        let result = run_check(files, root);
        assert!(
            result.diagnostics.is_empty(),
            "Files with the exact same content should not contradict each other"
        );
    }

    #[test]
    fn test_three_way_contradiction() {
        // Root says formal, child says casual, grandchild says formal again.
        // Should detect contradiction between root<->child and child<->grandchild.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let files = vec![
            make_file(root, "CLAUDE.md", &["Always use formal tone."]),
            make_file(root, "src/CLAUDE.md", &["Keep it casual and friendly."]),
            make_file(root, "src/api/CLAUDE.md", &["Always use formal tone."]),
        ];
        let result = run_check(files, root);
        // Root<->src/CLAUDE.md: formal vs casual (contradiction)
        // src/CLAUDE.md<->src/api/CLAUDE.md: casual vs formal (contradiction)
        // Root<->src/api/CLAUDE.md: same direction (no contradiction)
        assert!(
            result.diagnostics.len() >= 2,
            "Three-way contradiction should detect at least 2 contradictions, got {}",
            result.diagnostics.len()
        );
        // All diagnostics should be about tone
        for d in &result.diagnostics {
            assert!(
                d.message.contains("tone"),
                "Expected tone contradiction, got: {}",
                d.message
            );
        }
    }
}
