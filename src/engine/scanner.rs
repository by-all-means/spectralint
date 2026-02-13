use globset::GlobSet;
use std::path::{Path, PathBuf};

use super::cross_ref::build_glob_set;
use crate::config::Config;

pub fn scan(root: &Path, config: &Config) -> Vec<PathBuf> {
    let ignore_set = build_glob_set(&config.ignore);
    let ignore_files_set = build_glob_set(&config.ignore_files);
    let include_set = build_glob_set(&config.include);
    let mut files = Vec::new();
    walk_dir(
        root,
        root,
        &ignore_set,
        &ignore_files_set,
        &include_set,
        &mut files,
    );
    files.sort();
    files
}

pub(crate) fn matches_glob(path: &Path, root: &Path, set: &GlobSet) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| set.is_match(name))
        || path.strip_prefix(root).is_ok_and(|rel| set.is_match(rel))
}

fn walk_dir(
    dir: &Path,
    root: &Path,
    ignore: &GlobSet,
    ignore_files: &GlobSet,
    include: &GlobSet,
    files: &mut Vec<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if matches_glob(&path, root, ignore) {
            continue;
        }

        if path.is_dir() {
            walk_dir(&path, root, ignore, ignore_files, include, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md")
            && !matches_glob(&path, root, ignore_files)
            && matches_glob(&path, root, include)
        {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn all_md_config() -> Config {
        let mut config = Config::default();
        config.include = vec!["**/*.md".to_string()];
        config
    }

    #[test]
    fn test_scan_finds_md_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Hello").unwrap();
        fs::write(dir.path().join("notes.txt"), "not markdown").unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let config = Config::default();
        let files = scan(dir.path(), &config);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.extension().unwrap() == "md"));
    }

    #[test]
    fn test_scan_ignores_directories() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Hello").unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/bad.md"), "# Bad").unwrap();

        let config = Config::default();
        let files = scan(dir.path(), &config);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_scan_glob_patterns() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.md"), "# Hello").unwrap();
        fs::create_dir(dir.path().join("build_output")).unwrap();
        fs::write(dir.path().join("build_output/doc.md"), "# Build").unwrap();
        fs::create_dir(dir.path().join("build_artifacts")).unwrap();
        fs::write(dir.path().join("build_artifacts/notes.md"), "# Notes").unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/guide.md"), "# Guide").unwrap();

        let mut config = all_md_config();
        config.ignore.push("build_*".to_string());
        let files = scan(dir.path(), &config);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| {
            let name = f.file_name().unwrap().to_str().unwrap();
            name == "readme.md" || name == "guide.md"
        }));
    }

    #[test]
    fn test_scan_ignore_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.md"), "# Hello").unwrap();
        fs::write(dir.path().join("changelog.md"), "# Changes").unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/history.md"), "# History").unwrap();

        let mut config = all_md_config();
        config.ignore_files.push("changelog.md".to_string());
        config.ignore_files.push("docs/history.md".to_string());
        let files = scan(dir.path(), &config);
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "readme.md");
    }

    #[test]
    fn test_scan_include_filters_non_matching() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();
        fs::create_dir(dir.path().join("reports")).unwrap();
        fs::write(dir.path().join("reports/notes.md"), "# Notes").unwrap();

        let config = Config::default();
        let files = scan(dir.path(), &config);
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "CLAUDE.md");
    }

    #[test]
    fn test_scan_include_all_md() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/guide.md"), "# Guide").unwrap();

        let config = all_md_config();
        let files = scan(dir.path(), &config);
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_scan_include_dot_claude_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        fs::write(dir.path().join(".claude/settings.md"), "# Settings").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();

        let config = Config::default();
        let files = scan(dir.path(), &config);
        assert_eq!(files.len(), 1);
        assert!(files[0].to_str().unwrap().contains(".claude"));
    }

    #[test]
    fn test_scan_include_empty_scans_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();

        let mut config = Config::default();
        config.include = vec![];
        let files = scan(dir.path(), &config);
        assert!(files.is_empty());
    }
}
