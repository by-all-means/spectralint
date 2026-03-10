use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::types::{Category, Diagnostic, Fix, Severity};

const CACHE_VERSION: &str = "1";
const CACHE_FILE: &str = ".spectralint-cache.json";
/// Maximum cache file size (50 MiB) to prevent memory exhaustion from crafted caches.
const MAX_CACHE_SIZE: u64 = 50 * 1024 * 1024;

/// On-disk cache format.
#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    version: String,
    spectralint_version: String,
    config_hash: u64,
    files_hash: u64,
    diagnostics: Vec<CachedDiagnostic>,
}

/// A diagnostic stored in the cache (uses owned `String` path instead of `Arc<PathBuf>`).
#[derive(Debug, Serialize, Deserialize)]
struct CachedDiagnostic {
    file: String,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_column: Option<usize>,
    severity: Severity,
    category: Category,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<Box<Fix>>,
}

impl CachedDiagnostic {
    fn from_diagnostic(d: &Diagnostic) -> Self {
        Self {
            file: d.file.display().to_string(),
            line: d.line,
            column: d.column,
            end_line: d.end_line,
            end_column: d.end_column,
            severity: d.severity,
            category: d.category.clone(),
            message: d.message.clone(),
            suggestion: d.suggestion.clone(),
            fix: d.fix.clone(),
        }
    }

    /// Used in tests for roundtrip verification; the main load path interns Arc paths instead.
    #[cfg(test)]
    fn into_diagnostic(self) -> Diagnostic {
        Diagnostic {
            file: Arc::new(PathBuf::from(self.file)),
            line: self.line,
            column: self.column,
            end_line: self.end_line,
            end_column: self.end_column,
            severity: self.severity,
            category: self.category,
            message: self.message,
            suggestion: self.suggestion,
            fix: self.fix,
        }
    }
}

/// Stable FNV-1a hash (deterministic across Rust versions, unlike DefaultHasher).
fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Hash a string using FNV-1a.
fn hash_str(s: &str) -> u64 {
    fnv1a_hash(s.as_bytes())
}

/// Compute a combined hash of all scanned file paths and their metadata (mtime + size).
/// Uses metadata instead of reading file contents to avoid a redundant double-read
/// (files are read again later during parsing). Files are sorted by path for determinism.
pub(crate) fn compute_files_hash(files: &[PathBuf]) -> u64 {
    debug_assert!(
        files.windows(2).all(|w| w[0] <= w[1]),
        "files must be sorted"
    );

    let mut combined: u64 = 0xcbf2_9ce4_8422_2325;
    for path in files {
        // Hash the path bytes directly (no allocation via display().to_string())
        let path_hash = fnv1a_hash(path.as_os_str().as_encoded_bytes());
        combined ^= path_hash;
        combined = combined.wrapping_mul(0x0100_0000_01b3);

        match std::fs::metadata(path) {
            Ok(meta) => {
                // Hash file size
                let size = meta.len();
                combined ^= size;
                combined = combined.wrapping_mul(0x0100_0000_01b3);

                // Hash mtime as seconds since UNIX_EPOCH
                if let Ok(mtime) = meta.modified() {
                    let secs = mtime
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    combined ^= secs;
                    combined = combined.wrapping_mul(0x0100_0000_01b3);
                }
            }
            Err(_) => {
                // File unreadable: incorporate a sentinel so the hash differs
                // from the case where the file simply doesn't exist in the list.
                combined ^= 0xdead_beef_dead_beef;
                combined = combined.wrapping_mul(0x0100_0000_01b3);
                tracing::warn!(
                    "Cannot stat {}, cache hash may be inaccurate",
                    path.display()
                );
            }
        }
    }

    combined
}

/// Compute a hash of the config by serializing it to a canonical string.
/// We hash the TOML config file content directly if available, otherwise
/// hash the serialized default.
pub(crate) fn compute_config_hash(config_path: Option<&Path>, project_root: &Path) -> u64 {
    // Try explicit config path first, then auto-discovered path
    let default_path;
    let path = match config_path {
        Some(p) => p,
        None => {
            default_path = project_root.join(".spectralintrc.toml");
            &default_path
        }
    };

    match std::fs::read_to_string(path) {
        Ok(content) => hash_str(&content),
        Err(_) => hash_str("__default_config__"),
    }
}

/// Try to load cached diagnostics. Returns `Some(diagnostics)` if the cache
/// is valid (same version, config, and file contents), or `None` if the cache
/// is missing, corrupt, or stale.
pub(crate) fn load(
    project_root: &Path,
    files_hash: u64,
    config_hash: u64,
) -> Option<Vec<Diagnostic>> {
    let cache_path = project_root.join(CACHE_FILE);
    let meta = std::fs::metadata(&cache_path).ok()?;
    if meta.len() > MAX_CACHE_SIZE {
        tracing::warn!(
            "Cache file too large ({:.1} MiB), ignoring",
            meta.len() as f64 / (1024.0 * 1024.0)
        );
        return None;
    }
    let content = std::fs::read_to_string(&cache_path).ok()?;
    let cache: CacheFile = serde_json::from_str(&content).ok()?;

    if cache.version != CACHE_VERSION
        || cache.spectralint_version != env!("CARGO_PKG_VERSION")
        || cache.config_hash != config_hash
        || cache.files_hash != files_hash
    {
        return None;
    }

    let mut path_cache = std::collections::HashMap::<String, Arc<PathBuf>>::new();
    Some(
        cache
            .diagnostics
            .into_iter()
            .map(|cd| {
                let file = path_cache
                    .entry(cd.file.clone())
                    .or_insert_with(|| Arc::new(PathBuf::from(&cd.file)))
                    .clone();
                Diagnostic {
                    file,
                    line: cd.line,
                    column: cd.column,
                    end_line: cd.end_line,
                    end_column: cd.end_column,
                    severity: cd.severity,
                    category: cd.category,
                    message: cd.message,
                    suggestion: cd.suggestion,
                    fix: cd.fix,
                }
            })
            .collect(),
    )
}

/// Save diagnostics to the cache file.
pub(crate) fn save(
    project_root: &Path,
    files_hash: u64,
    config_hash: u64,
    diagnostics: &[Diagnostic],
) {
    let cache = CacheFile {
        version: CACHE_VERSION.to_string(),
        spectralint_version: env!("CARGO_PKG_VERSION").to_string(),
        config_hash,
        files_hash,
        diagnostics: diagnostics
            .iter()
            .map(CachedDiagnostic::from_diagnostic)
            .collect(),
    };

    let cache_path = project_root.join(CACHE_FILE);
    if let Ok(json) = serde_json::to_string(&cache) {
        // Atomic write: write to temp file then rename to avoid corruption
        let tmp_path = cache_path.with_extension("tmp");
        if std::fs::write(&tmp_path, json).is_ok() {
            let _ = std::fs::rename(&tmp_path, &cache_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_str_deterministic() {
        let h1 = hash_str("hello world");
        let h2 = hash_str("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_str_different_inputs() {
        let h1 = hash_str("hello");
        let h2 = hash_str("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_cached_diagnostic_roundtrip() {
        let diag = Diagnostic {
            file: Arc::new(PathBuf::from("test.md")),
            line: 42,
            column: Some(5),
            end_line: Some(42),
            end_column: Some(20),
            severity: Severity::Warning,
            category: Category::DeadReference,
            message: "broken ref".to_string(),
            suggestion: Some("fix it".to_string()),
            fix: None,
        };

        let cached = CachedDiagnostic::from_diagnostic(&diag);
        let restored = cached.into_diagnostic();

        assert_eq!(*restored.file, PathBuf::from("test.md"));
        assert_eq!(restored.line, 42);
        assert_eq!(restored.column, Some(5));
        assert_eq!(restored.end_line, Some(42));
        assert_eq!(restored.end_column, Some(20));
        assert_eq!(restored.severity, Severity::Warning);
        assert_eq!(restored.category, Category::DeadReference);
        assert_eq!(restored.message, "broken ref");
        assert_eq!(restored.suggestion, Some("fix it".to_string()));
    }

    #[test]
    fn test_cache_load_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let result = load(dir.path(), 0, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let diags = vec![Diagnostic {
            file: Arc::new(PathBuf::from("CLAUDE.md")),
            line: 10,
            column: None,
            end_line: None,
            end_column: None,
            severity: Severity::Error,
            category: Category::VagueDirective,
            message: "vague".to_string(),
            suggestion: None,
            fix: None,
        }];

        save(dir.path(), 123, 456, &diags);

        let loaded = load(dir.path(), 123, 456);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].line, 10);
        assert_eq!(loaded[0].category, Category::VagueDirective);
    }

    #[test]
    fn test_cache_invalidated_by_files_hash() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), 123, 456, &[]);

        // Different files_hash should invalidate
        let result = load(dir.path(), 999, 456);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_invalidated_by_config_hash() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), 123, 456, &[]);

        // Different config_hash should invalidate
        let result = load(dir.path(), 123, 999);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_invalidated_by_corrupt_json() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join(CACHE_FILE);
        std::fs::write(&cache_path, "not valid json{{{").unwrap();

        let result = load(dir.path(), 0, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_invalidated_by_version_change() {
        // Simulate a cache with a different CACHE_VERSION
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join(CACHE_FILE);
        let cache = CacheFile {
            version: "0".to_string(), // old version
            spectralint_version: env!("CARGO_PKG_VERSION").to_string(),
            config_hash: 100,
            files_hash: 200,
            diagnostics: vec![],
        };
        std::fs::write(&cache_path, serde_json::to_string(&cache).unwrap()).unwrap();
        let result = load(dir.path(), 200, 100);
        assert!(
            result.is_none(),
            "Different CACHE_VERSION should invalidate cache"
        );
    }

    #[test]
    fn test_cache_invalidated_by_spectralint_version() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join(CACHE_FILE);
        let cache = CacheFile {
            version: CACHE_VERSION.to_string(),
            spectralint_version: "0.0.0-fake".to_string(), // different binary version
            config_hash: 100,
            files_hash: 200,
            diagnostics: vec![],
        };
        std::fs::write(&cache_path, serde_json::to_string(&cache).unwrap()).unwrap();
        let result = load(dir.path(), 200, 100);
        assert!(
            result.is_none(),
            "Different spectralint version should invalidate cache"
        );
    }

    #[test]
    fn test_cached_diagnostic_with_fix_roundtrip() {
        use crate::types::{Fix, Replacement};
        let diag = Diagnostic {
            file: Arc::new(PathBuf::from("CLAUDE.md")),
            line: 5,
            column: Some(10),
            end_line: Some(5),
            end_column: Some(20),
            severity: Severity::Info,
            category: Category::RepeatedWord,
            message: "repeated 'the'".to_string(),
            suggestion: Some("Remove the duplicate word".to_string()),
            fix: Some(Box::new(Fix {
                description: "Remove duplicate".to_string(),
                replacements: vec![Replacement {
                    line: 5,
                    start_col: 10,
                    end_col: 14,
                    new_text: String::new(),
                }],
            })),
        };

        let cached = CachedDiagnostic::from_diagnostic(&diag);
        let json = serde_json::to_string(&cached).unwrap();
        let deserialized: CachedDiagnostic = serde_json::from_str(&json).unwrap();
        let restored = deserialized.into_diagnostic();

        assert_eq!(restored.line, 5);
        assert_eq!(restored.column, Some(10));
        assert!(restored.fix.is_some());
        let fix = restored.fix.unwrap();
        assert_eq!(fix.replacements.len(), 1);
        assert_eq!(fix.replacements[0].start_col, 10);
        assert_eq!(fix.replacements[0].end_col, 14);
        assert_eq!(fix.replacements[0].new_text, "");
    }

    #[test]
    fn test_cache_save_and_load_multiple_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let diags = vec![
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 1,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Error,
                category: Category::DeadReference,
                message: "dead ref 1".to_string(),
                suggestion: None,
                fix: None,
            },
            Diagnostic {
                file: Arc::new(PathBuf::from("b.md")),
                line: 10,
                column: Some(5),
                end_line: Some(10),
                end_column: Some(15),
                severity: Severity::Warning,
                category: Category::VagueDirective,
                message: "vague".to_string(),
                suggestion: Some("be specific".to_string()),
                fix: None,
            },
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 20,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Info,
                category: Category::TokenBudget,
                message: "big file".to_string(),
                suggestion: None,
                fix: None,
            },
        ];

        save(dir.path(), 111, 222, &diags);
        let loaded = load(dir.path(), 111, 222).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].category, Category::DeadReference);
        assert_eq!(loaded[1].category, Category::VagueDirective);
        assert_eq!(loaded[1].column, Some(5));
        assert_eq!(loaded[2].category, Category::TokenBudget);
    }

    #[test]
    fn test_cache_empty_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), 42, 84, &[]);
        let loaded = load(dir.path(), 42, 84).unwrap();
        assert!(loaded.is_empty(), "Empty diagnostics should roundtrip");
    }

    #[test]
    fn test_compute_config_hash_no_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let hash = compute_config_hash(None, dir.path());
        // Should use the default sentinel
        assert_eq!(hash, hash_str("__default_config__"));
    }

    #[test]
    fn test_compute_config_hash_with_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_content = "[checkers.dead_reference]\nenabled = false\n";
        std::fs::write(dir.path().join(".spectralintrc.toml"), config_content).unwrap();
        let hash = compute_config_hash(None, dir.path());
        assert_eq!(hash, hash_str(config_content));
    }

    #[test]
    fn test_compute_config_hash_different_content_different_hash() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".spectralintrc.toml"), "version1").unwrap();
        let h1 = compute_config_hash(None, dir.path());
        std::fs::write(dir.path().join(".spectralintrc.toml"), "version2").unwrap();
        let h2 = compute_config_hash(None, dir.path());
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_fnv1a_hash_known_value() {
        // FNV-1a 64-bit hash of empty string should be the offset basis
        assert_eq!(fnv1a_hash(b""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn test_compute_files_hash_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.md");
        let f2 = dir.path().join("b.md");
        std::fs::write(&f1, "hello").unwrap();
        std::fs::write(&f2, "world").unwrap();

        let files = vec![f1.clone(), f2.clone()];
        let h1 = compute_files_hash(&files);
        let h2 = compute_files_hash(&files);
        assert_eq!(h1, h2, "Same files should produce same hash");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "files must be sorted")]
    fn test_compute_files_hash_rejects_unsorted() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.md");
        let f2 = dir.path().join("b.md");
        std::fs::write(&f1, "hello").unwrap();
        std::fs::write(&f2, "world").unwrap();

        // Unsorted input should trigger debug assertion
        compute_files_hash(&[f2, f1]);
    }
}
