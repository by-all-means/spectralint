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
    let paths = scanner::scan(project_root, config);
    if paths.is_empty() {
        anyhow::bail!("No markdown files found in {}", project_root.display());
    }

    let parsed: Vec<_> = paths
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
    let context = CheckerContext::build(parsed, project_root, &config.historical_files);

    let mut result = CheckResult::default();
    for checker in checkers::all_checkers(config) {
        result
            .diagnostics
            .extend(checker.check(&context).diagnostics);
    }

    result.diagnostics.retain(|d| {
        !suppress::is_suppressed(&suppressions, &d.file, d.line, &d.category.to_string())
    });

    result.diagnostics.sort_by(|a, b| {
        (&a.file, a.line, &a.category, &a.message).cmp(&(&b.file, b.line, &b.category, &b.message))
    });
    result.diagnostics.dedup_by(|a, b| {
        a.file == b.file && a.line == b.line && a.category == b.category && a.message == b.message
    });

    Ok(result)
}
