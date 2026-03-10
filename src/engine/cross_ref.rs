use std::collections::HashSet;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::parser::types::ParsedFile;

use super::scanner::matches_glob;

pub struct CheckerContext {
    pub(crate) files: Vec<ParsedFile>,
    pub(crate) project_root: PathBuf,
    pub(crate) canonical_root: Option<PathBuf>,
    pub(crate) filename_index: HashSet<String>,
    pub(crate) historical_indices: HashSet<usize>,
}

impl CheckerContext {
    pub(crate) fn build(
        files: Vec<ParsedFile>,
        project_root: &Path,
        historical_patterns: &[String],
        filename_index: HashSet<String>,
        canonical_root: Option<PathBuf>,
    ) -> Self {
        let historical_set = build_glob_set(historical_patterns);
        let historical_indices = files
            .iter()
            .enumerate()
            .filter(|(_, f)| matches_glob(&f.path, project_root, &historical_set))
            .map(|(i, _)| i)
            .collect();
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
    use std::sync::Arc;

    fn make_parsed_file(root: &Path, name: &str) -> ParsedFile {
        ParsedFile {
            path: Arc::new(root.join(name)),
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

        let ctx = CheckerContext::build(files, root, &patterns, HashSet::new(), None);

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

        let ctx = CheckerContext::build(files, root, &patterns, HashSet::new(), None);

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

        let ctx = CheckerContext::build(files, root, &[], HashSet::new(), None);

        assert!(
            ctx.historical_indices.is_empty(),
            "No patterns means no historical files"
        );
    }

    #[test]
    fn test_build_context_from_multiple_files() {
        let root = Path::new("/project");
        let files = vec![
            make_parsed_file(root, "CLAUDE.md"),
            make_parsed_file(root, "docs/AGENTS.md"),
            make_parsed_file(root, "sub/deep/notes.md"),
        ];

        let ctx = CheckerContext::build(files, root, &[], HashSet::new(), None);

        assert_eq!(ctx.files.len(), 3);
        assert_eq!(ctx.project_root, root);
        assert!(ctx.historical_indices.is_empty());
        assert_eq!(*ctx.files[0].path, root.join("CLAUDE.md"));
        assert_eq!(*ctx.files[1].path, root.join("docs/AGENTS.md"));
        assert_eq!(*ctx.files[2].path, root.join("sub/deep/notes.md"));
    }

    #[test]
    fn test_empty_file_list() {
        let root = Path::new("/project");
        let files: Vec<ParsedFile> = vec![];

        let ctx = CheckerContext::build(
            files,
            root,
            &["changelog*".to_string()],
            HashSet::new(),
            None,
        );

        assert!(ctx.files.is_empty());
        assert!(ctx.historical_indices.is_empty());
        assert_eq!(ctx.project_root, root);
    }

    #[test]
    fn test_filename_index_population() {
        let root = Path::new("/project");
        let files = vec![make_parsed_file(root, "CLAUDE.md")];

        let mut index = HashSet::new();
        index.insert("CLAUDE.md".to_string());
        index.insert("utils.rs".to_string());
        index.insert("Cargo.toml".to_string());

        let ctx = CheckerContext::build(files, root, &[], index, None);

        assert_eq!(ctx.filename_index.len(), 3);
        assert!(ctx.filename_index.contains("CLAUDE.md"));
        assert!(ctx.filename_index.contains("utils.rs"));
        assert!(ctx.filename_index.contains("Cargo.toml"));
        assert!(!ctx.filename_index.contains("nonexistent.txt"));
    }

    #[test]
    fn test_filename_index_empty() {
        let root = Path::new("/project");
        let files = vec![make_parsed_file(root, "CLAUDE.md")];

        let ctx = CheckerContext::build(files, root, &[], HashSet::new(), None);

        assert!(ctx.filename_index.is_empty());
    }

    #[test]
    fn test_canonical_root_some() {
        let root = Path::new("/project");
        let files = vec![make_parsed_file(root, "CLAUDE.md")];
        let canonical = PathBuf::from("/resolved/project");

        let ctx = CheckerContext::build(files, root, &[], HashSet::new(), Some(canonical.clone()));

        assert_eq!(ctx.canonical_root, Some(canonical));
    }

    #[test]
    fn test_canonical_root_none() {
        let root = Path::new("/project");
        let files = vec![make_parsed_file(root, "CLAUDE.md")];

        let ctx = CheckerContext::build(files, root, &[], HashSet::new(), None);

        assert!(ctx.canonical_root.is_none());
    }

    #[test]
    fn test_historical_detection_mixed_patterns() {
        let root = Path::new("/project");
        let files = vec![
            make_parsed_file(root, "CLAUDE.md"),
            make_parsed_file(root, "changelog.md"),
            make_parsed_file(root, "archive-2023.md"),
            make_parsed_file(root, "docs/retro-q1.md"),
            make_parsed_file(root, "AGENTS.md"),
            make_parsed_file(root, "history-old.md"),
        ];

        let patterns = vec![
            "changelog*".to_string(),
            "archive*".to_string(),
            "retro*".to_string(),
            "history*".to_string(),
        ];

        let ctx = CheckerContext::build(files, root, &patterns, HashSet::new(), None);

        assert!(
            !ctx.historical_indices.contains(&0),
            "CLAUDE.md not historical"
        );
        assert!(
            ctx.historical_indices.contains(&1),
            "changelog.md is historical"
        );
        assert!(
            ctx.historical_indices.contains(&2),
            "archive-2023.md is historical"
        );
        assert!(
            ctx.historical_indices.contains(&3),
            "docs/retro-q1.md is historical"
        );
        assert!(
            !ctx.historical_indices.contains(&4),
            "AGENTS.md not historical"
        );
        assert!(
            ctx.historical_indices.contains(&5),
            "history-old.md is historical"
        );
        assert_eq!(ctx.historical_indices.len(), 4);
    }

    #[test]
    fn test_historical_wildcard_star_star_pattern() {
        let root = Path::new("/project");
        let files = vec![
            make_parsed_file(root, "docs/archive/old.md"),
            make_parsed_file(root, "CLAUDE.md"),
        ];

        let patterns = vec!["docs/archive/**".to_string()];

        let ctx = CheckerContext::build(files, root, &patterns, HashSet::new(), None);

        assert!(
            ctx.historical_indices.contains(&0),
            "docs/archive/old.md should match docs/archive/**"
        );
        assert!(!ctx.historical_indices.contains(&1));
    }

    #[test]
    fn test_build_glob_set_empty() {
        let set = build_glob_set(&[]);
        assert!(!set.is_match("anything"));
    }

    #[test]
    fn test_build_glob_set_case_insensitive() {
        let set = build_glob_set(&["changelog*".to_string()]);
        assert!(set.is_match("CHANGELOG.md"));
        assert!(set.is_match("changelog.md"));
        assert!(set.is_match("Changelog.md"));
    }

    #[test]
    fn test_build_glob_set_multiple_patterns() {
        let set = build_glob_set(&["*.md".to_string(), "*.txt".to_string()]);
        assert!(set.is_match("readme.md"));
        assert!(set.is_match("notes.txt"));
        assert!(!set.is_match("code.rs"));
    }

    #[test]
    fn test_build_filename_index_with_tempdir() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Hello").unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/helper.py"), "pass").unwrap();

        let index = build_filename_index(dir.path());

        assert!(index.contains("CLAUDE.md"));
        assert!(index.contains("main.rs"));
        assert!(index.contains("helper.py"));
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_build_filename_index_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let index = build_filename_index(dir.path());
        assert!(index.is_empty());
    }

    #[test]
    fn test_build_filename_index_nested_dirs() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
        fs::write(dir.path().join("a/b/c/deep.txt"), "deep").unwrap();
        fs::write(dir.path().join("a/shallow.txt"), "shallow").unwrap();

        let index = build_filename_index(dir.path());

        assert!(index.contains("deep.txt"));
        assert!(index.contains("shallow.txt"));
        assert_eq!(index.len(), 2);
    }
}
