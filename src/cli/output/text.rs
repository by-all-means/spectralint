use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use crate::types::{CheckResult, Severity};

pub fn render(result: &CheckResult, project_root: &Path) {
    if result.diagnostics.is_empty() {
        println!();
        println!("  {}", "\u{2501}".repeat(50).dimmed());
        println!("  {}", "no issues found".green());
        println!();
        return;
    }

    let mut by_category: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for d in &result.diagnostics {
        by_category
            .entry(d.category.to_string())
            .or_default()
            .push(d);
    }

    let errors = result.error_count();
    let warnings = result.warning_count();
    let infos = result.info_count();

    println!();
    println!("  {}", "\u{2501}".repeat(50).dimmed());
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!("{errors} errors").red().bold().to_string());
    }
    if warnings > 0 {
        parts.push(format!("{warnings} warnings").yellow().bold().to_string());
    }
    if infos > 0 {
        parts.push(format!("{infos} info").blue().to_string());
    }
    let file_count = result
        .diagnostics
        .iter()
        .map(|d| &d.file)
        .collect::<HashSet<_>>()
        .len();
    println!("  {} across {} files", parts.join(", "), file_count.bold());
    println!("  {}", "\u{2501}".repeat(50).dimmed());

    let severity_order = |cat: &str| -> u8 {
        by_category.get(cat).map_or(3, |diags| {
            diags
                .iter()
                .map(|d| match d.severity {
                    Severity::Error => 0,
                    Severity::Warning => 1,
                    Severity::Info => 2,
                })
                .min()
                .unwrap_or(3)
        })
    };

    let mut categories: Vec<_> = by_category.keys().collect();
    categories.sort_by_key(|&cat| (severity_order(cat), cat));

    for cat_name in categories {
        let diags = &by_category[cat_name];
        let count = diags.len();
        let severity = diags[0].severity;

        let (icon, label) = match severity {
            Severity::Error => (
                "\u{2717}".red().to_string(),
                cat_name.red().bold().to_string(),
            ),
            Severity::Warning => (
                "\u{26a0}".yellow().to_string(),
                cat_name.yellow().bold().to_string(),
            ),
            Severity::Info => (
                "\u{2139}".blue().to_string(),
                cat_name.blue().bold().to_string(),
            ),
        };

        println!();
        println!("  {} {} {}", icon, label, format!("({count})").dimmed());

        let mut by_file: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for d in diags {
            let rel = super::relative_path(&d.file, project_root);
            by_file.entry(rel).or_default().push(d);
        }

        let limit = if severity == Severity::Info && count > 20 {
            10
        } else {
            usize::MAX
        };

        let mut shown = 0;
        for (file, file_diags) in &by_file {
            if shown >= limit {
                break;
            }
            println!("    {}", file.dimmed());
            for d in file_diags {
                if shown >= limit {
                    break;
                }
                println!("      L{:<4} {}", d.line, d.message);
                shown += 1;
            }
        }
        if count > limit {
            println!(
                "    {}",
                format!(
                    "... and {} more (use --format json for full list)",
                    count - limit
                )
                .dimmed()
            );
        }
    }

    println!();
}
