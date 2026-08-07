//! Baseline file support: record existing findings so `check` only fails on
//! new ones, letting existing repos adopt spectralint incrementally.
//!
//! Matching is count-based on (relative file, category, digit-masked message)
//! — no line numbers, so ordinary edits don't invalidate entries. Messages are
//! digit-masked because several rules embed volatile numbers ("~1200 estimated
//! tokens", "File has 803 lines") that change on every edit.

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::LazyLock;

use crate::emit;
use crate::types::{Category, CheckResult, Diagnostic, Severity};

/// Default baseline filename, resolved relative to the project root.
pub const DEFAULT_BASELINE_FILE: &str = ".spectralint-baseline.json";

const BASELINE_VERSION: u32 = 1;

/// How `engine::run` resolves the baseline file.
#[derive(Debug, Clone, Default)]
pub enum BaselineMode {
    /// Use `<project_root>/.spectralint-baseline.json` if it exists.
    #[default]
    Auto,
    /// Use an explicit path; a missing or unreadable file is a hard error.
    Path(PathBuf),
    /// Ignore any baseline.
    Disabled,
}

#[derive(Debug, Serialize, Deserialize)]
struct BaselineFile {
    version: u32,
    entries: Vec<BaselineEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BaselineEntry {
    file: String,
    category: String,
    message: String,
    count: usize,
}

/// (relative file with `/` separators, category display string, masked message)
type Key = (String, String, String);

pub(crate) struct BaselineOutcome {
    pub suppressed: usize,
    pub stale: Vec<Diagnostic>,
}

/// Result of writing a baseline file.
pub struct WriteStats {
    pub findings: usize,
    pub files: usize,
    /// Diagnostics skipped because their path is outside the project root
    /// (machine-specific absolute paths must not leak into a committed file).
    pub skipped: usize,
}

static DIGIT_RUNS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]+(\.[0-9]+)?").unwrap());

/// Mask digit runs so volatile counts don't invalidate entries on every edit —
/// but only OUTSIDE backtick/quote-delimited spans. Quoted text is an
/// identifier (`chapter1.md`, "Step 1"): masking it would conflate distinct
/// findings and silently hide new ones (a fixed ref to `chapter1.md` would
/// absorb a brand-new broken ref to `chapter2.md`). Unquoted digits are
/// volatile counts ("File has 803 lines"). An unbalanced delimiter leaves the
/// remainder unmasked — failing toward a stale entry, never a hidden finding.
fn mask_message(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut unquoted = String::new();
    let mut delimiter: Option<char> = None;
    for ch in message.chars() {
        match delimiter {
            Some(d) => {
                out.push(ch);
                if ch == d {
                    delimiter = None;
                }
            }
            None if ch == '`' || ch == '"' => {
                out.push_str(&DIGIT_RUNS.replace_all(&unquoted, "#"));
                unquoted.clear();
                out.push(ch);
                delimiter = Some(ch);
            }
            None => unquoted.push(ch),
        }
    }
    out.push_str(&DIGIT_RUNS.replace_all(&unquoted, "#"));
    out
}

/// Relativize against the project root and join with `/` regardless of OS so
/// committed baselines are portable. Returns `None` for paths outside root.
fn normalized_rel_path(file: &Path, project_root: &Path) -> Option<String> {
    let rel = file.strip_prefix(project_root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

fn key_for(diagnostic: &Diagnostic, project_root: &Path) -> Option<Key> {
    let file = normalized_rel_path(&diagnostic.file, project_root)?;
    Some((
        file,
        diagnostic.category.to_string(),
        mask_message(&diagnostic.message),
    ))
}

/// Resolve the baseline mode to a concrete path, if any.
pub(crate) fn resolve(mode: &BaselineMode, project_root: &Path) -> Result<Option<PathBuf>> {
    match mode {
        BaselineMode::Disabled => Ok(None),
        BaselineMode::Auto => {
            let path = project_root.join(DEFAULT_BASELINE_FILE);
            Ok(path.exists().then_some(path))
        }
        BaselineMode::Path(path) => {
            if !path.exists() {
                bail!("baseline file not found: {}", path.display());
            }
            Ok(Some(path.canonicalize().unwrap_or_else(|_| path.clone())))
        }
    }
}

/// Load and validate a baseline file. Duplicate keys are merged by summing
/// counts; messages are re-masked so hand-edited entries with raw numbers
/// still match. Category strings are NOT parsed — a baseline written by a
/// newer spectralint degrades to stale entries instead of a hard error.
pub(crate) fn load(path: &Path) -> Result<HashMap<Key, usize>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read baseline file {}", path.display()))?;
    let parsed: BaselineFile = serde_json::from_str(&content)
        .with_context(|| format!("invalid baseline file {}", path.display()))?;
    if parsed.version != BASELINE_VERSION {
        bail!(
            "unsupported baseline version {} in {} (this spectralint supports version {})",
            parsed.version,
            path.display(),
            BASELINE_VERSION
        );
    }

    let mut entries: HashMap<Key, usize> = HashMap::new();
    for entry in parsed.entries {
        if entry.count < 1 {
            bail!(
                "invalid count {} for baseline entry {}: {} in {}",
                entry.count,
                entry.file,
                entry.category,
                path.display()
            );
        }
        let key = (entry.file, entry.category, mask_message(&entry.message));
        *entries.entry(key).or_insert(0) += entry.count;
    }
    Ok(entries)
}

/// Filter baselined findings out of `diagnostics`. Each entry suppresses up to
/// `count` matching findings, lowest lines first (`diagnostics` is sorted).
/// Entries with leftover count get one `stale-baseline-entry` diagnostic each,
/// anchored to the baseline file itself.
pub(crate) fn apply(
    diagnostics: &mut Vec<Diagnostic>,
    project_root: &Path,
    baseline_path: &Path,
    entries: &HashMap<Key, usize>,
) -> BaselineOutcome {
    let mut remaining = entries.clone();
    let before = diagnostics.len();

    diagnostics.retain(|d| {
        let Some(key) = key_for(d, project_root) else {
            return true;
        };
        match remaining.get_mut(&key) {
            Some(count) if *count > 0 => {
                *count -= 1;
                false
            }
            _ => true,
        }
    });
    let suppressed = before - diagnostics.len();

    let mut stale = CheckResult::default();
    let baseline_file = Arc::new(baseline_path.to_path_buf());
    let mut leftover: Vec<(&Key, usize)> = remaining
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(key, count)| (key, *count))
        .collect();
    leftover.sort();
    for ((file, category, message), count) in leftover {
        emit!(
            stale,
            baseline_file,
            1,
            Severity::Info,
            Category::StaleBaselineEntry,
            suggest: "Run `spectralint check . --write-baseline` to refresh the baseline",
            "baseline entry no longer matches any finding: {}: {}: \"{}\" ({} unmatched)",
            file,
            category,
            message,
            count
        );
    }

    BaselineOutcome {
        suppressed,
        stale: stale.diagnostics,
    }
}

/// Write a baseline recording all `diagnostics`, sorted for stable diffs.
/// Atomic tmp+rename so a crash can't leave a truncated file.
pub fn write(path: &Path, diagnostics: &[Diagnostic], project_root: &Path) -> Result<WriteStats> {
    let mut grouped: HashMap<Key, usize> = HashMap::new();
    let mut skipped = 0usize;
    for d in diagnostics {
        match key_for(d, project_root) {
            Some(key) => *grouped.entry(key).or_insert(0) += 1,
            None => skipped += 1,
        }
    }

    let findings: usize = grouped.values().sum();
    let files: usize = {
        let mut names: Vec<&str> = grouped.keys().map(|(f, _, _)| f.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names.len()
    };

    let mut entries: Vec<BaselineEntry> = grouped
        .into_iter()
        .map(|((file, category, message), count)| BaselineEntry {
            file,
            category,
            message,
            count,
        })
        .collect();
    entries.sort_by(|a, b| {
        (&a.file, &a.category, &a.message).cmp(&(&b.file, &b.category, &b.message))
    });

    let content = serde_json::to_string_pretty(&BaselineFile {
        version: BASELINE_VERSION,
        entries,
    })
    .context("failed to serialize baseline")?;

    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content + "\n")
        .with_context(|| format!("failed to write baseline file {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to move baseline file into place at {}",
            path.display()
        )
    })?;

    Ok(WriteStats {
        findings,
        files,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(file: &str, line: usize, category: Category, message: &str) -> Diagnostic {
        Diagnostic {
            file: Arc::new(PathBuf::from(file)),
            line,
            column: None,
            end_line: None,
            end_column: None,
            severity: Severity::Warning,
            category,
            message: message.to_string(),
            suggestion: None,
            fix: None,
        }
    }

    #[test]
    fn test_mask_message_digits() {
        assert_eq!(
            mask_message("File has ~1200 estimated tokens (exceeds 1000 token budget)"),
            "File has ~# estimated tokens (exceeds # token budget)"
        );
        assert_eq!(
            mask_message("Section is 3.5x the median"),
            "Section is #x the median"
        );
        assert_eq!(mask_message("no digits here"), "no digits here");
    }

    #[test]
    fn test_mask_message_preserves_quoted_identifiers() {
        // Digits inside quotes/backticks are identifiers, not volatile counts:
        // masking them would conflate distinct findings and hide new ones.
        assert_eq!(
            mask_message("\"chapter1.md\" does not exist"),
            "\"chapter1.md\" does not exist"
        );
        assert_eq!(
            mask_message("reference to `v2/notes.md` is dead"),
            "reference to `v2/notes.md` is dead"
        );
        // Mixed: quoted identifier kept, unquoted count masked.
        assert_eq!(
            mask_message("Section \"Step 1\" is 45 lines (3.2x the median of 14 lines)"),
            "Section \"Step 1\" is # lines (#x the median of # lines)"
        );
        // Unbalanced delimiter: remainder unmasked (fails toward stale, not hidden).
        assert_eq!(mask_message("odd \"quote 42"), "odd \"quote 42");
    }

    #[test]
    fn test_distinct_quoted_identifiers_do_not_conflate() {
        // Regression: a baselined dead ref to chapter1.md must NOT absorb a
        // new dead ref to chapter2.md.
        assert_ne!(
            mask_message("\"chapter1.md\" does not exist"),
            mask_message("\"chapter2.md\" does not exist")
        );
    }

    #[test]
    fn test_normalized_rel_path_uses_forward_slashes() {
        let root = Path::new("/project");
        let file = Path::new("/project/docs/sub/CLAUDE.md");
        assert_eq!(
            normalized_rel_path(file, root),
            Some("docs/sub/CLAUDE.md".to_string())
        );
    }

    #[test]
    fn test_path_outside_root_not_normalized() {
        let root = Path::new("/project");
        assert_eq!(
            normalized_rel_path(Path::new("/elsewhere/a.md"), root),
            None
        );
    }

    #[test]
    fn test_custom_pattern_key_distinct_from_builtin() {
        let root = Path::new("/p");
        let builtin = diag("/p/a.md", 1, Category::DeadReference, "m");
        let custom = diag(
            "/p/a.md",
            1,
            Category::CustomPattern("dead-reference".into()),
            "m",
        );
        assert_ne!(key_for(&builtin, root), key_for(&custom, root));
    }

    #[test]
    fn test_write_then_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let path = root.join(DEFAULT_BASELINE_FILE);
        let diags = vec![
            diag(
                &root.join("CLAUDE.md").display().to_string(),
                3,
                Category::DeadReference,
                "reference to `x.md` not found",
            ),
            diag(
                &root.join("CLAUDE.md").display().to_string(),
                9,
                Category::DeadReference,
                "reference to `x.md` not found",
            ),
            diag(
                &root.join("docs/AGENTS.md").display().to_string(),
                1,
                Category::TokenBudget,
                "File has ~1200 estimated tokens",
            ),
        ];

        let stats = write(&path, &diags, root).unwrap();
        assert_eq!(stats.findings, 3);
        assert_eq!(stats.files, 2);
        assert_eq!(stats.skipped, 0);

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded[&(
                "CLAUDE.md".to_string(),
                "dead-reference".to_string(),
                "reference to `x.md` not found".to_string()
            )],
            2
        );
        assert_eq!(
            loaded[&(
                "docs/AGENTS.md".to_string(),
                "token-budget".to_string(),
                "File has ~# estimated tokens".to_string()
            )],
            1
        );
    }

    #[test]
    fn test_write_skips_paths_outside_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let path = root.join(DEFAULT_BASELINE_FILE);
        let diags = vec![diag("/somewhere/else.md", 1, Category::DeadReference, "m")];
        let stats = write(&path, &diags, root).unwrap();
        assert_eq!(stats.findings, 0);
        assert_eq!(stats.skipped, 1);
        assert!(load(&path).unwrap().is_empty());
    }

    #[test]
    fn test_apply_suppresses_up_to_count_lowest_lines_first() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = root.join("CLAUDE.md").display().to_string();
        let mut diags = vec![
            diag(&file, 2, Category::DeadReference, "broken"),
            diag(&file, 5, Category::DeadReference, "broken"),
            diag(&file, 9, Category::DeadReference, "broken"),
        ];
        let mut entries = HashMap::new();
        entries.insert(
            (
                "CLAUDE.md".to_string(),
                "dead-reference".to_string(),
                "broken".to_string(),
            ),
            2,
        );

        let baseline_path = root.join(DEFAULT_BASELINE_FILE);
        let outcome = apply(&mut diags, root, &baseline_path, &entries);
        assert_eq!(outcome.suppressed, 2);
        assert!(outcome.stale.is_empty());
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].line, 9,
            "excess should be the highest-line occurrence"
        );
    }

    #[test]
    fn test_apply_reports_stale_entries() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut diags: Vec<Diagnostic> = vec![];
        let mut entries = HashMap::new();
        entries.insert(
            (
                "CLAUDE.md".to_string(),
                "dead-reference".to_string(),
                "gone".to_string(),
            ),
            3,
        );

        let baseline_path = root.join(DEFAULT_BASELINE_FILE);
        let outcome = apply(&mut diags, root, &baseline_path, &entries);
        assert_eq!(outcome.suppressed, 0);
        assert_eq!(outcome.stale.len(), 1);
        let stale = &outcome.stale[0];
        assert_eq!(stale.category, Category::StaleBaselineEntry);
        assert_eq!(stale.severity, Severity::Info);
        assert_eq!(*stale.file, baseline_path);
        assert!(stale.message.contains("3 unmatched"));
    }

    #[test]
    fn test_apply_masks_current_messages_before_matching() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = root.join("CLAUDE.md").display().to_string();
        // Baseline recorded at 803 lines; the file has since grown to 812.
        let mut diags = vec![diag(&file, 1, Category::FileSize, "File has 812 lines")];
        let mut entries = HashMap::new();
        entries.insert(
            (
                "CLAUDE.md".to_string(),
                "file-size".to_string(),
                "File has # lines".to_string(),
            ),
            1,
        );

        let baseline_path = root.join(DEFAULT_BASELINE_FILE);
        let outcome = apply(&mut diags, root, &baseline_path, &entries);
        assert_eq!(outcome.suppressed, 1);
        assert!(diags.is_empty());
        assert!(outcome.stale.is_empty());
    }

    #[test]
    fn test_load_rejects_future_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.json");
        std::fs::write(&path, r#"{"version": 2, "entries": []}"#).unwrap();
        let err = load(&path).unwrap_err().to_string();
        assert!(err.contains("unsupported baseline version 2"), "{err}");
    }

    #[test]
    fn test_load_rejects_zero_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.json");
        std::fs::write(
            &path,
            r#"{"version": 1, "entries": [{"file": "a.md", "category": "dead-reference", "message": "m", "count": 0}]}"#,
        )
        .unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn test_load_rejects_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn test_load_merges_duplicate_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.json");
        std::fs::write(
            &path,
            r#"{"version": 1, "entries": [
                {"file": "a.md", "category": "dead-reference", "message": "m", "count": 1},
                {"file": "a.md", "category": "dead-reference", "message": "m", "count": 2}
            ]}"#,
        )
        .unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[&(
                "a.md".to_string(),
                "dead-reference".to_string(),
                "m".to_string()
            )],
            3
        );
    }

    #[test]
    fn test_load_remasks_hand_edited_messages() {
        // A hand-added entry with raw digits must still match.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.json");
        std::fs::write(
            &path,
            r#"{"version": 1, "entries": [{"file": "a.md", "category": "file-size", "message": "File has 803 lines", "count": 1}]}"#,
        )
        .unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.contains_key(&(
            "a.md".to_string(),
            "file-size".to_string(),
            "File has # lines".to_string()
        )));
    }

    #[test]
    fn test_resolve_auto_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(resolve(&BaselineMode::Auto, dir.path()).unwrap().is_none());
    }

    #[test]
    fn test_resolve_explicit_missing_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert!(resolve(&BaselineMode::Path(missing), dir.path()).is_err());
    }

    #[test]
    fn test_resolve_disabled_is_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(DEFAULT_BASELINE_FILE), "{}").unwrap();
        assert!(resolve(&BaselineMode::Disabled, dir.path())
            .unwrap()
            .is_none());
    }
}
