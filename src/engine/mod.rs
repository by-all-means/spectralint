pub(crate) mod cross_ref;
pub(crate) mod scanner;
mod suppress;

use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

use crate::checkers;
use crate::config::Config;
use crate::types::CheckResult;
use cross_ref::CheckerContext;

pub fn run(project_root: &Path, config: &Config) -> Result<CheckResult> {
    let scan_result = scanner::scan(project_root, config);
    if scan_result.files.is_empty() {
        anyhow::bail!("No markdown files found in {}", project_root.display());
    }

    let parsed: Vec<_> = scan_result
        .files
        .par_iter()
        .filter_map(|p| match crate::parser::parse_file(p) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {e}", p.display());
                None
            }
        })
        .collect();

    let suppressions = suppress::build_suppression_set(&parsed);
    let context = CheckerContext::build(
        parsed,
        project_root,
        &config.historical_files,
        scan_result.filename_index,
    );

    let all = checkers::all_checkers(config);
    let mut diagnostics: Vec<_> = all
        .par_iter()
        .flat_map(|checker| checker.check(&context).diagnostics)
        .collect();

    diagnostics.retain(|d| !suppress::is_suppressed(&suppressions, &d.file, d.line, &d.category));

    diagnostics.sort_by(|a, b| {
        (&a.file, a.line, &a.category, &a.message).cmp(&(&b.file, b.line, &b.category, &b.message))
    });
    diagnostics.dedup_by(|a, b| {
        a.file == b.file && a.line == b.line && a.category == b.category && a.message == b.message
    });

    Ok(CheckResult { diagnostics })
}
