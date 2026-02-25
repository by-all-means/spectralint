use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, Severity};

use super::utils::{is_template_ref, ScopeFilter};
use super::Checker;

pub struct CircularReferenceChecker {
    scope: ScopeFilter,
}

impl CircularReferenceChecker {
    pub fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Try to resolve a file ref path to an index in `path_to_idx`.
fn resolve_ref(
    ref_path: &str,
    source_file: &Path,
    project_root: &Path,
    path_to_idx: &HashMap<PathBuf, usize>,
) -> Option<usize> {
    let source_dir = source_file.parent().unwrap_or(project_root);

    // Try relative to source file first, then project root
    [source_dir, project_root].into_iter().find_map(|base| {
        base.join(ref_path)
            .canonicalize()
            .ok()
            .and_then(|canonical| path_to_idx.get(&canonical).copied())
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DfsState {
    Unvisited,
    InProgress,
    Done,
}

impl Checker for CircularReferenceChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        // Build canonical path -> index map
        let path_to_idx: HashMap<PathBuf, usize> = ctx
            .files
            .iter()
            .enumerate()
            .map(|(idx, file)| {
                let key = file
                    .path
                    .canonicalize()
                    .unwrap_or_else(|_| file.path.clone());
                (key, idx)
            })
            .collect();

        // Build adjacency list: adj[src] = [(target, ref_idx), ...]
        let n = ctx.files.len();
        let mut adj: Vec<Vec<(usize, usize)>> = vec![vec![]; n];

        for (src_idx, file) in ctx.files.iter().enumerate() {
            for (ref_idx, file_ref) in file.file_refs.iter().enumerate() {
                if is_template_ref(&file_ref.path) {
                    continue;
                }
                if let Some(target_idx) = resolve_ref(
                    &file_ref.path,
                    &file_ref.source_file,
                    &ctx.project_root,
                    &path_to_idx,
                ) {
                    // Skip self-references — a file mentioning its own name
                    // (e.g. "# CLAUDE.md" or "Update CLAUDE.md when:") is not
                    // a circular dependency.
                    if target_idx == src_idx {
                        continue;
                    }
                    adj[src_idx].push((target_idx, ref_idx));
                }
            }
        }

        // DFS cycle detection
        let mut state = vec![DfsState::Unvisited; n];
        let mut stack: Vec<usize> = Vec::new();

        for start in 0..n {
            if state[start] != DfsState::Unvisited {
                continue;
            }
            dfs_detect_cycles(
                start,
                &adj,
                &mut state,
                &mut stack,
                ctx,
                &self.scope,
                &mut result,
            );
        }

        result
    }
}

fn dfs_detect_cycles(
    node: usize,
    adj: &[Vec<(usize, usize)>],
    state: &mut [DfsState],
    stack: &mut Vec<usize>,
    ctx: &CheckerContext,
    scope: &ScopeFilter,
    result: &mut CheckResult,
) {
    state[node] = DfsState::InProgress;
    stack.push(node);

    for &(target, ref_idx) in &adj[node] {
        match state[target] {
            DfsState::InProgress => {
                // Found a cycle! Report on the file that closes the cycle.
                let file = &ctx.files[node];
                if !scope.includes(&file.path, &ctx.project_root) {
                    continue;
                }
                let file_ref = &file.file_refs[ref_idx];

                // Build cycle description
                let cycle_start = stack.iter().position(|&n| n == target).unwrap();
                let cycle_nodes: Vec<String> = stack[cycle_start..]
                    .iter()
                    .map(|&i| {
                        ctx.files[i]
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    })
                    .collect();
                let cycle_desc = format!("{} → {}", cycle_nodes.join(" → "), cycle_nodes[0]);

                emit!(
                    result,
                    file_ref.source_file,
                    file_ref.line,
                    Severity::Warning,
                    Category::CircularReference,
                    suggest: "Break the cycle by removing or restructuring one of the references",
                    "Circular reference chain: {}",
                    cycle_desc
                );
            }
            DfsState::Unvisited => {
                dfs_detect_cycles(target, adj, state, stack, ctx, scope, result);
            }
            DfsState::Done => {}
        }
    }

    stack.pop();
    state[node] = DfsState::Done;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::{FileRef, ParsedFile};
    use std::collections::HashSet;
    use std::fs;

    fn make_file(root: &std::path::Path, name: &str, refs: Vec<(&str, usize)>) -> ParsedFile {
        let path = root.join(name);
        ParsedFile {
            path: path.clone(),
            sections: vec![],
            tables: vec![],
            file_refs: refs
                .into_iter()
                .map(|(r, line)| FileRef {
                    path: r.to_string(),
                    line,
                    source_file: path.clone(),
                })
                .collect(),
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
        }
    }

    #[test]
    fn test_simple_cycle_a_b_a() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.md"), "# A\nSee b.md").unwrap();
        fs::write(root.join("b.md"), "# B\nSee a.md").unwrap();

        let files = vec![
            make_file(root, "a.md", vec![("b.md", 2)]),
            make_file(root, "b.md", vec![("a.md", 2)]),
        ];

        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = CircularReferenceChecker::new(&[]);
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Warning);
        assert_eq!(result.diagnostics[0].category, Category::CircularReference);
        assert!(result.diagnostics[0]
            .message
            .contains("Circular reference chain"));
    }

    #[test]
    fn test_three_node_cycle_a_b_c_a() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.md"), "# A").unwrap();
        fs::write(root.join("b.md"), "# B").unwrap();
        fs::write(root.join("c.md"), "# C").unwrap();

        let files = vec![
            make_file(root, "a.md", vec![("b.md", 1)]),
            make_file(root, "b.md", vec![("c.md", 1)]),
            make_file(root, "c.md", vec![("a.md", 1)]),
        ];

        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = CircularReferenceChecker::new(&[]);
        let result = checker.check(&ctx);

        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("→"));
    }

    #[test]
    fn test_no_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.md"), "# A").unwrap();
        fs::write(root.join("b.md"), "# B").unwrap();
        fs::write(root.join("c.md"), "# C").unwrap();

        let files = vec![
            make_file(root, "a.md", vec![("b.md", 1)]),
            make_file(root, "b.md", vec![("c.md", 1)]),
            make_file(root, "c.md", vec![]),
        ];

        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = CircularReferenceChecker::new(&[]);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "DAG with no cycle should produce no diagnostics"
        );
    }

    #[test]
    fn test_unresolved_refs_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.md"), "# A").unwrap();

        let files = vec![make_file(root, "a.md", vec![("nonexistent.md", 1)])];

        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = CircularReferenceChecker::new(&[]);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "Unresolved refs should not cause diagnostics (dead-reference handles those)"
        );
    }

    #[test]
    fn test_self_reference_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.md"), "# A\nSee a.md").unwrap();

        let files = vec![make_file(root, "a.md", vec![("a.md", 2)])];

        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = CircularReferenceChecker::new(&[]);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "Self-references (file mentioning its own name) should be ignored"
        );
    }

    #[test]
    fn test_template_refs_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.md"), "# A").unwrap();

        let files = vec![make_file(
            root,
            "a.md",
            vec![("commands/*.md", 1), ("path/to/file.md", 2)],
        )];

        let ctx = CheckerContext {
            files,
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };

        let checker = CircularReferenceChecker::new(&[]);
        let result = checker.check(&ctx);

        assert!(
            result.diagnostics.is_empty(),
            "Template/glob refs should be skipped"
        );
    }
}
