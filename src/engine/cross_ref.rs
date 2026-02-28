use std::collections::HashSet;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::parser::types::ParsedFile;

use super::scanner::matches_glob;

pub struct CheckerContext {
    pub files: Vec<ParsedFile>,
    pub project_root: PathBuf,
    pub canonical_root: Option<PathBuf>,
    pub filename_index: HashSet<String>,
    pub historical_indices: HashSet<usize>,
}

impl CheckerContext {
    pub(crate) fn build(
        files: Vec<ParsedFile>,
        project_root: &Path,
        historical_patterns: &[String],
        filename_index: HashSet<String>,
    ) -> Self {
        let historical_set = build_glob_set(historical_patterns);
        let historical_indices = files
            .iter()
            .enumerate()
            .filter(|(_, f)| matches_glob(&f.path, project_root, &historical_set))
            .map(|(i, _)| i)
            .collect();
        let canonical_root = project_root.canonicalize().ok();
        Self {
            files,
            project_root: project_root.to_path_buf(),
            canonical_root,
            filename_index,
            historical_indices,
        }
    }
}

/// Simple filename index builder for tests (walks the tree collecting basenames).
#[cfg(test)]
pub(crate) fn build_filename_index(root: &Path) -> HashSet<String> {
    let mut index = HashSet::new();
    fn collect(dir: &Path, index: &mut HashSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            if ft.is_dir() {
                collect(&entry.path(), index);
            } else {
                index.insert(name.to_string_lossy().into_owned());
            }
        }
    }
    collect(root, &mut index);
    index
}

pub(crate) fn build_glob_set(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = GlobBuilder::new(pattern).case_insensitive(true).build() {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_parsed_file(root: &Path, name: &str) -> ParsedFile {
        ParsedFile {
            path: root.join(name),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: vec![],
            in_code_block: vec![],
        }
    }

    #[test]
    fn test_historical_by_filename_glob() {
        let root = Path::new("/project");
        let files = vec![
            make_parsed_file(root, "CLAUDE.md"),
            make_parsed_file(root, "changelog.md"),
            make_parsed_file(root, "CHANGELOG.md"),
            make_parsed_file(root, "retro-2024.md"),
        ];

        let patterns = vec!["changelog*".to_string(), "retro*".to_string()];

        let ctx = CheckerContext::build(files, root, &patterns, HashSet::new());

        assert!(
            !ctx.historical_indices.contains(&0),
            "CLAUDE.md should not be historical"
        );
        assert!(
            ctx.historical_indices.contains(&1),
            "changelog.md should be historical"
        );
        assert!(
            ctx.historical_indices.contains(&2),
            "CHANGELOG.md should be historical (case-insensitive)"
        );
        assert!(
            ctx.historical_indices.contains(&3),
            "retro-2024.md should be historical"
        );
    }

    #[test]
    fn test_historical_by_path_pattern() {
        let root = Path::new("/project");
        let files = vec![
            make_parsed_file(root, "CLAUDE.md"),
            make_parsed_file(root, "docs/history.md"),
            make_parsed_file(root, "history.md"),
        ];

        let patterns = vec!["docs/history.md".to_string()];

        let ctx = CheckerContext::build(files, root, &patterns, HashSet::new());

        assert!(!ctx.historical_indices.contains(&0));
        assert!(
            ctx.historical_indices.contains(&1),
            "docs/history.md should match path pattern"
        );
        assert!(
            !ctx.historical_indices.contains(&2),
            "history.md at root should NOT match docs/history.md pattern"
        );
    }

    #[test]
    fn test_empty_historical_patterns() {
        let root = Path::new("/project");
        let files = vec![
            make_parsed_file(root, "changelog.md"),
            make_parsed_file(root, "CLAUDE.md"),
        ];

        let ctx = CheckerContext::build(files, root, &[], HashSet::new());

        assert!(
            ctx.historical_indices.is_empty(),
            "No patterns means no historical files"
        );
    }
}
