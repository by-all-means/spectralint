use owo_colors::{OwoColorize, Stream};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use crate::types::{CheckResult, Severity};

/// Conditionally apply styling, respecting `owo_colors::set_override()`.
macro_rules! styled {
    ($val:expr, $first:ident $(.$rest:ident)*) => {
        $val.if_supports_color(Stream::Stdout, |v| v.$first()$(.$rest())*.to_string())
    };
}

pub fn render(result: &CheckResult, project_root: &Path) {
    if result.diagnostics.is_empty() {
        println!();
        println!("  {}", styled!("\u{2501}".repeat(50), dimmed));
        println!("  {}", styled!("no issues found", green));
        println!();
        return;
    }

    let mut by_category: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for d in &result.diagnostics {
        by_category.entry(d.category.as_str()).or_default().push(d);
    }

    let errors = result.error_count();
    let warnings = result.warning_count();
    let infos = result.info_count();

    println!();
    println!("  {}", styled!("\u{2501}".repeat(50), dimmed));
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(styled!(format!("{errors} errors"), red.bold).to_string());
    }
    if warnings > 0 {
        parts.push(styled!(format!("{warnings} warnings"), yellow.bold).to_string());
    }
    if infos > 0 {
        parts.push(styled!(format!("{infos} info"), blue).to_string());
    }
    let file_count = result
        .diagnostics
        .iter()
        .map(|d| &d.file)
        .collect::<HashSet<_>>()
        .len();
    println!(
        "  {} across {} files",
        parts.join(", "),
        styled!(file_count, bold)
    );
    println!("  {}", styled!("\u{2501}".repeat(50), dimmed));

    let max_severity = |cat: &str| -> Severity {
        by_category
            .get(cat)
            .and_then(|diags| diags.iter().map(|d| d.severity).max())
            .unwrap_or(Severity::Info)
    };

    let mut categories: Vec<_> = by_category.keys().collect();
    categories.sort_by(|&a, &b| max_severity(b).cmp(&max_severity(a)).then_with(|| a.cmp(b)));

    for cat_name in categories {
        let diags = &by_category[cat_name];
        let count = diags.len();
        let severity = diags[0].severity;

        let (icon, label) = match severity {
            Severity::Error => (
                styled!("\u{2717}", red).to_string(),
                styled!(*cat_name, red.bold).to_string(),
            ),
            Severity::Warning => (
                styled!("\u{26a0}", yellow).to_string(),
                styled!(*cat_name, yellow.bold).to_string(),
            ),
            Severity::Info => (
                styled!("\u{2139}", blue).to_string(),
                styled!(*cat_name, blue.bold).to_string(),
            ),
        };

        println!();
        println!(
            "  {} {} {}",
            icon,
            label,
            styled!(format!("({count})"), dimmed)
        );

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
            println!("    {}", styled!(file.as_str(), dimmed));
            for d in file_diags {
                if shown >= limit {
                    break;
                }
                println!("      L{:<4} {}", d.line, d.message);
                if let Some(suggestion) = &d.suggestion {
                    println!("      {}", styled!(format!("help: {suggestion}"), dimmed));
                }
                shown += 1;
            }
        }
        if count > limit {
            println!(
                "    {}",
                styled!(
                    format!(
                        "... and {} more (use --format json for full list)",
                        count - limit
                    ),
                    dimmed
                )
            );
        }
    }

    println!();
}
