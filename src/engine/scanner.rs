use globset::GlobSet;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::cross_ref::build_glob_set;
use crate::checkers::utils::SKIP_DIRS;
use crate::config::Config;

const MAX_WALK_DEPTH: usize = 256;

/// Result of scanning a project tree: matched `.md` files plus a filename index.
pub(crate) struct ScanResult {
    pub files: Vec<PathBuf>,
    pub filename_index: HashSet<String>,
    pub canonical_root: Option<PathBuf>,
}

/// Immutable configuration for a single walk pass.
struct WalkConfig {
    root: PathBuf,
    canonical_root: Option<PathBuf>,
    ignore: GlobSet,
    ignore_files: GlobSet,
    include: GlobSet,
}

pub(crate) fn scan(root: &Path, config: &Config) -> ScanResult {
    let canonical_root = root.canonicalize().ok();
    let walk = WalkConfig {
        root: root.to_path_buf(),
        canonical_root: canonical_root.clone(),
        ignore: build_glob_set(&config.ignore),
        ignore_files: build_glob_set(&config.ignore_files),
        include: build_glob_set(&config.include),
    };
    let mut files = Vec::new();
    let mut filename_index = HashSet::new();
    walk_dir(root, &walk, &mut files, &mut filename_index, 0);
    files.sort();
    ScanResult {
        files,
        filename_index,
        canonical_root,
    }
}

#[must_use]
pub(crate) fn matches_glob(path: &Path, root: &Path, set: &GlobSet) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| set.is_match(name))
        || path.strip_prefix(root).is_ok_and(|rel| set.is_match(rel))
}

fn walk_dir(
    dir: &Path,
    cfg: &WalkConfig,
    files: &mut Vec<PathBuf>,
    filename_index: &mut HashSet<String>,
    depth: usize,
) {
    if depth >= MAX_WALK_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        let Ok(ft) = entry.file_type() else {
            continue;
        };

        // Symlinked directories are skipped to prevent cycles.
        // Symlinked files are allowed only when they resolve within the project.
        if ft.is_symlink() {
            let within_root = !path.is_dir()
                && cfg.canonical_root.as_ref().is_some_and(|root| {
                    path.canonicalize()
                        .is_ok_and(|resolved| resolved.starts_with(root))
                });
            if !within_root {
                continue;
            }
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if matches_glob(&path, &cfg.root, &cfg.ignore) {
            continue;
        }

        if ft.is_dir() {
            if !SKIP_DIRS.contains(&name_str.as_ref()) {
                walk_dir(&path, cfg, files, filename_index, depth + 1);
            }
        } else {
            filename_index.insert(name_str.into_owned());

            if path.extension().and_then(|e| e.to_str()) == Some("md")
                && !matches_glob(&path, &cfg.root, &cfg.ignore_files)
                && matches_glob(&path, &cfg.root, &cfg.include)
            {
                files.push(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn all_md_config() -> Config {
        Config {
            include: vec!["**/*.md".to_string()],
            ..Config::default()
        }
    }

    #[test]
    fn test_scan_finds_md_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Hello").unwrap();
        fs::write(dir.path().join("notes.txt"), "not markdown").unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();

        let config = Config::default();
        let files = scan(dir.path(), &config).files;
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
        let files = scan(dir.path(), &config).files;
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
        let files = scan(dir.path(), &config).files;
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
        let files = scan(dir.path(), &config).files;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap().to_str().unwrap(), "readme.md");
    }

    #[test]
    fn test_scan_include_filters_non_matching() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();
        fs::create_dir(dir.path().join("reports")).unwrap();
        fs::write(dir.path().join("reports/notes.md"), "# Notes").unwrap();

        let config = Config::default();
        let files = scan(dir.path(), &config).files;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap().to_str().unwrap(), "CLAUDE.md");
    }

    #[test]
    fn test_scan_include_all_md() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/guide.md"), "# Guide").unwrap();

        let config = all_md_config();
        let files = scan(dir.path(), &config).files;
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_scan_include_dot_claude_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        fs::write(dir.path().join(".claude/settings.md"), "# Settings").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();

        let config = Config::default();
        let files = scan(dir.path(), &config).files;
        assert_eq!(files.len(), 1);
        assert!(files[0].to_str().unwrap().contains(".claude"));
    }

    #[cfg(unix)]
    #[test]
    fn test_scan_skips_symlinked_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Root").unwrap();
        fs::create_dir(dir.path().join("real")).unwrap();
        fs::write(dir.path().join("real/CLAUDE.md"), "# Real").unwrap();

        // Create a symlink pointing to `real/`
        std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("linked")).unwrap();

        let config = all_md_config();
        let files = scan(dir.path(), &config).files;

        // Only root and real/ should be scanned, not linked/
        assert!(
            !files.iter().any(|f| f.to_str().unwrap().contains("linked")),
            "Symlinked directories should be skipped"
        );
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_scan_include_empty_scans_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Instructions").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();

        let config = Config {
            include: vec![],
            ..Config::default()
        };
        let files = scan(dir.path(), &config).files;
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = tempfile::tempdir().unwrap();

        let config = all_md_config();
        let result = scan(dir.path(), &config);

        assert!(result.files.is_empty());
        assert!(result.filename_index.is_empty());
    }

    #[test]
    fn test_scan_deeply_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b/c/d/e")).unwrap();
        fs::write(dir.path().join("a/b/c/d/e/deep.md"), "# Deep").unwrap();
        fs::write(dir.path().join("a/mid.md"), "# Mid").unwrap();
        fs::write(dir.path().join("root.md"), "# Root").unwrap();

        let config = all_md_config();
        let files = scan(dir.path(), &config).files;

        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.ends_with("deep.md")));
        assert!(files.iter().any(|f| f.ends_with("mid.md")));
        assert!(files.iter().any(|f| f.ends_with("root.md")));
    }

    #[test]
    fn test_scan_filename_index_includes_non_md_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Hello").unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "// lib").unwrap();

        let config = all_md_config();
        let result = scan(dir.path(), &config);

        // Only .md files in the files list
        assert_eq!(result.files.len(), 1);
        // But the filename_index has all files
        assert!(result.filename_index.contains("CLAUDE.md"));
        assert!(result.filename_index.contains("main.rs"));
        assert!(result.filename_index.contains("Cargo.toml"));
        assert!(result.filename_index.contains("lib.rs"));
        assert_eq!(result.filename_index.len(), 4);
    }

    #[test]
    fn test_scan_canonical_root_is_populated() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Hello").unwrap();

        let config = Config::default();
        let result = scan(dir.path(), &config);

        // The canonical root should be set (tempdir exists on disk)
        assert!(result.canonical_root.is_some());
    }

    #[test]
    fn test_scan_files_are_sorted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("z_last.md"), "# Z").unwrap();
        fs::write(dir.path().join("a_first.md"), "# A").unwrap();
        fs::write(dir.path().join("m_middle.md"), "# M").unwrap();

        let config = all_md_config();
        let files = scan(dir.path(), &config).files;

        assert_eq!(files.len(), 3);
        // Files should be sorted
        let names: Vec<&str> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_str().unwrap())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn test_scan_skips_all_known_skip_dirs() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("root.md"), "# Root").unwrap();

        for skip_dir in SKIP_DIRS {
            fs::create_dir(dir.path().join(skip_dir)).unwrap();
            fs::write(dir.path().join(skip_dir).join("hidden.md"), "# Hidden").unwrap();
        }

        let config = all_md_config();
        let files = scan(dir.path(), &config).files;

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("root.md"));
    }

    #[test]
    fn test_scan_include_specific_pattern() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Claude").unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Agents").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();
        fs::write(dir.path().join("notes.md"), "# Notes").unwrap();

        // Only include CLAUDE.md pattern
        let config = Config {
            include: vec!["CLAUDE.md".to_string()],
            ..Config::default()
        };
        let files = scan(dir.path(), &config).files;

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("CLAUDE.md"));
    }

    #[test]
    fn test_scan_no_md_files_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        fs::write(dir.path().join("data.json"), "{}").unwrap();

        let config = all_md_config();
        let files = scan(dir.path(), &config).files;

        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_ignore_directory_pattern_does_not_affect_sibling() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/guide.md"), "# Guide").unwrap();
        fs::create_dir(dir.path().join("tmp")).unwrap();
        fs::write(dir.path().join("tmp/scratch.md"), "# Scratch").unwrap();
        fs::write(dir.path().join("root.md"), "# Root").unwrap();

        let mut config = all_md_config();
        config.ignore.push("tmp".to_string());
        let files = scan(dir.path(), &config).files;

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.ends_with("guide.md")));
        assert!(files.iter().any(|f| f.ends_with("root.md")));
        assert!(!files.iter().any(|f| f.ends_with("scratch.md")));
    }

    #[test]
    fn test_scan_nested_include_pattern() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs/api")).unwrap();
        fs::write(dir.path().join("docs/api/spec.md"), "# Spec").unwrap();
        fs::write(dir.path().join("docs/guide.md"), "# Guide").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();

        let config = Config {
            include: vec!["docs/**".to_string()],
            ..Config::default()
        };
        let files = scan(dir.path(), &config).files;

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.ends_with("spec.md")));
        assert!(files.iter().any(|f| f.ends_with("guide.md")));
    }

    #[cfg(unix)]
    #[test]
    fn test_scan_symlinked_file_within_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("real.md"), "# Real").unwrap();

        // Create a symlink to a file within the project
        std::os::unix::fs::symlink(dir.path().join("real.md"), dir.path().join("link.md")).unwrap();

        let config = all_md_config();
        let files = scan(dir.path(), &config).files;

        // The symlinked file should be included since it resolves within the project
        assert!(files.iter().any(|f| f.ends_with("real.md")));
        assert!(files.iter().any(|f| f.ends_with("link.md")));
        assert_eq!(files.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn test_scan_symlinked_file_outside_project() {
        let project_dir = tempfile::tempdir().unwrap();
        let external_dir = tempfile::tempdir().unwrap();

        fs::write(project_dir.path().join("CLAUDE.md"), "# Root").unwrap();
        fs::write(external_dir.path().join("external.md"), "# External").unwrap();

        // Create a symlink pointing outside the project
        std::os::unix::fs::symlink(
            external_dir.path().join("external.md"),
            project_dir.path().join("ext_link.md"),
        )
        .unwrap();

        let config = all_md_config();
        let files = scan(project_dir.path(), &config).files;

        // Symlinked file outside project should be skipped
        assert!(!files.iter().any(|f| f.ends_with("ext_link.md")));
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_matches_glob_by_filename() {
        let root = Path::new("/project");
        let set = build_glob_set(&["*.md".to_string()]);

        assert!(matches_glob(Path::new("/project/readme.md"), root, &set));
        assert!(!matches_glob(Path::new("/project/main.rs"), root, &set));
    }

    #[test]
    fn test_matches_glob_by_relative_path() {
        let root = Path::new("/project");
        let set = build_glob_set(&["docs/**".to_string()]);

        assert!(matches_glob(
            Path::new("/project/docs/guide.md"),
            root,
            &set
        ));
        assert!(!matches_glob(Path::new("/project/src/main.rs"), root, &set));
    }

    #[test]
    fn test_matches_glob_empty_set() {
        let root = Path::new("/project");
        let set = build_glob_set(&[]);

        assert!(!matches_glob(Path::new("/project/anything.md"), root, &set));
    }

    #[test]
    fn test_scan_ignore_files_does_not_remove_from_filename_index() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.md"), "# Hello").unwrap();
        fs::write(dir.path().join("changelog.md"), "# Changes").unwrap();

        let mut config = all_md_config();
        config.ignore_files.push("changelog.md".to_string());
        let result = scan(dir.path(), &config);

        // changelog.md should be excluded from files but remain in the filename index
        assert_eq!(result.files.len(), 1);
        assert!(result.filename_index.contains("changelog.md"));
        assert!(result.filename_index.contains("readme.md"));
    }
}
