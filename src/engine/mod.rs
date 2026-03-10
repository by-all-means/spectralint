mod cache;
pub(crate) mod cross_ref;
pub(crate) mod fix;

/// Re-export `apply_fixes` so the binary crate can use `engine::apply_fixes`.
pub use fix::apply_fixes;
pub(crate) mod scanner;
mod suppress;

use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

use crate::checkers;
use crate::config::Config;
use crate::types::CheckResult;
use cross_ref::CheckerContext;

/// Return the list of markdown files that would be scanned for the given project root and config.
pub fn scanned_files(project_root: &Path, config: &Config) -> Vec<std::path::PathBuf> {
    scanner::scan(project_root, config).files
}

pub fn run(
    project_root: &Path,
    config: &Config,
    use_cache: bool,
    config_path: Option<&Path>,
) -> Result<CheckResult> {
    let scan_result = scanner::scan(project_root, config);
    if scan_result.files.is_empty() {
        anyhow::bail!("No markdown files found in {}", project_root.display());
    }

    // Compute cache keys and try to load from cache
    let (files_hash, config_hash) = if use_cache {
        let fh = cache::compute_files_hash(&scan_result.files);
        let ch = cache::compute_config_hash(config_path, project_root);
        if let Some(diagnostics) = cache::load(project_root, fh, ch) {
            return Ok(CheckResult { diagnostics });
        }
        (fh, ch)
    } else {
        (0, 0)
    };

    let total_files = scan_result.files.len();
    let parsed: Vec<_> = scan_result
        .files
        .par_iter()
        .filter_map(|p| match crate::parser::parse_file(p) {
            Ok(f) => Some(f),
            Err(e) => {
                tracing::warn!("Failed to parse {}: {e}", p.display());
                None
            }
        })
        .collect();

    let parse_failures = total_files - parsed.len();
    if parsed.is_empty() {
        anyhow::bail!(
            "All {} markdown file(s) failed to parse in {}",
            total_files,
            project_root.display()
        );
    }
    if parse_failures > 0 {
        tracing::warn!(
            "Checked {}/{} files ({} failed to parse)",
            parsed.len(),
            total_files,
            parse_failures
        );
    }

    // Validate suppress comment rule names
    let known_rules = suppress::all_known_rule_names(&config.checkers.custom_patterns);
    let mut invalid_suppress_diags = suppress::validate_suppress_rules(&parsed, &known_rules);

    let suppressions = suppress::build_suppression_set(&parsed);
    let context = CheckerContext::build(
        parsed,
        project_root,
        &config.historical_files,
        scan_result.filename_index,
        scan_result.canonical_root,
    );

    let all = checkers::all_checkers(config);
    let mut diagnostics: Vec<_> = all
        .par_iter()
        .flat_map(|checker| checker.check(&context).diagnostics)
        .collect();

    diagnostics.retain(|d| !suppress::is_suppressed(&suppressions, &d.file, d.line, &d.category));

    // Detect unused suppressions (must run after suppression filtering)
    let mut unused_suppress_diags = suppress::find_unused_suppressions(&suppressions);

    diagnostics.append(&mut invalid_suppress_diags);
    diagnostics.append(&mut unused_suppress_diags);

    // Apply per-checker severity overrides
    for d in &mut diagnostics {
        if let Some(sev) = config.severity_override(&d.category) {
            d.severity = sev;
        }
    }

    diagnostics.sort_by(|a, b| {
        (&a.file, a.line, a.column, &a.category, &a.message).cmp(&(
            &b.file,
            b.line,
            b.column,
            &b.category,
            &b.message,
        ))
    });
    diagnostics.dedup_by(|a, b| {
        a.file == b.file
            && a.line == b.line
            && a.column == b.column
            && a.category == b.category
            && a.message == b.message
    });

    // Save to cache
    if use_cache {
        cache::save(project_root, files_hash, config_hash, &diagnostics);
    }

    Ok(CheckResult { diagnostics })
}
