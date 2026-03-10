use owo_colors::{OwoColorize, Stream};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use crate::types::{CheckResult, Severity};

/// Conditionally apply styling, respecting `owo_colors::set_override()`.
macro_rules! styled {
    ($val:expr, $first:ident $(.$rest:ident)*) => {
        $val.if_supports_color(Stream::Stdout, |v| v.$first()$(.$rest())*.to_string())
    };
}

/// Render diagnostics into a `String` (colors are still controlled by
/// `owo_colors::set_override`).  Extracted so unit tests can inspect
/// the output without capturing stdout.
pub(crate) fn render_to_string(result: &CheckResult, project_root: &Path) -> String {
    let mut out = String::new();
    render_into(&mut out, result, project_root);
    out
}

pub fn render(result: &CheckResult, project_root: &Path) {
    print!("{}", render_to_string(result, project_root));
}

fn render_into(out: &mut String, result: &CheckResult, project_root: &Path) {
    if result.diagnostics.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, "  {}", styled!("\u{2501}".repeat(50), dimmed)).unwrap();
        writeln!(out, "  {}", styled!("no issues found", green)).unwrap();
        writeln!(out).unwrap();
        return;
    }

    let mut by_category: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for d in &result.diagnostics {
        by_category.entry(d.category.as_str()).or_default().push(d);
    }

    let (errors, warnings, infos) = result.severity_counts();

    writeln!(out).unwrap();
    writeln!(out, "  {}", styled!("\u{2501}".repeat(50), dimmed)).unwrap();
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
    writeln!(
        out,
        "  {} across {} files",
        parts.join(", "),
        styled!(file_count, bold)
    )
    .unwrap();
    writeln!(out, "  {}", styled!("\u{2501}".repeat(50), dimmed)).unwrap();

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

        writeln!(out).unwrap();
        writeln!(
            out,
            "  {} {} {}",
            icon,
            label,
            styled!(format!("({count})"), dimmed)
        )
        .unwrap();

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
            writeln!(out, "    {}", styled!(file.as_str(), dimmed)).unwrap();
            for d in file_diags {
                if shown >= limit {
                    break;
                }
                writeln!(out, "      L{:<4} {}", d.line, d.message).unwrap();
                if let Some(suggestion) = &d.suggestion {
                    writeln!(
                        out,
                        "      {}",
                        styled!(format!("help: {suggestion}"), dimmed)
                    )
                    .unwrap();
                }
                shown += 1;
            }
        }
        if count > limit {
            writeln!(
                out,
                "    {}",
                styled!(
                    format!(
                        "... and {} more (use --format json for full list)",
                        count - limit
                    ),
                    dimmed
                )
            )
            .unwrap();
        }
    }

    writeln!(out).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, CheckResult, Diagnostic, Severity};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn make_diag(
        file: &str,
        line: usize,
        severity: Severity,
        category: Category,
        message: &str,
    ) -> Diagnostic {
        Diagnostic {
            file: Arc::new(PathBuf::from(file)),
            line,
            column: None,
            end_line: None,
            end_column: None,
            severity,
            category,
            message: message.to_string(),
            suggestion: None,
            fix: None,
        }
    }

    /// Disable color output so tests get plain text.
    fn no_color() {
        owo_colors::set_override(false);
    }

    #[test]
    fn empty_diagnostics_shows_no_issues() {
        no_color();
        let result = CheckResult::default();
        let out = render_to_string(&result, Path::new("/project"));
        assert!(
            out.contains("no issues found"),
            "Empty result should say 'no issues found', got:\n{out}"
        );
    }

    #[test]
    fn single_error_summary_line() {
        no_color();
        let result = CheckResult {
            diagnostics: vec![make_diag(
                "/project/CLAUDE.md",
                10,
                Severity::Error,
                Category::DeadReference,
                "file not found",
            )],
        };
        let out = render_to_string(&result, Path::new("/project"));
        assert!(out.contains("1 errors"), "Should show '1 errors'");
        assert!(
            out.contains("across 1 files"),
            "Should show 'across 1 files'"
        );
    }

    #[test]
    fn mixed_severity_summary_line() {
        no_color();
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Error,
                    Category::DeadReference,
                    "err1",
                ),
                make_diag(
                    "/project/a.md",
                    2,
                    Severity::Warning,
                    Category::VagueDirective,
                    "warn1",
                ),
                make_diag(
                    "/project/b.md",
                    3,
                    Severity::Info,
                    Category::PlaceholderText,
                    "info1",
                ),
            ],
        };
        let out = render_to_string(&result, Path::new("/project"));
        assert!(out.contains("1 errors"), "Should contain error count");
        assert!(out.contains("1 warnings"), "Should contain warning count");
        assert!(out.contains("1 info"), "Should contain info count");
        assert!(
            out.contains("across 2 files"),
            "Should show 2 distinct files"
        );
    }

    #[test]
    fn diagnostics_grouped_by_category() {
        no_color();
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Warning,
                    Category::VagueDirective,
                    "vague1",
                ),
                make_diag(
                    "/project/a.md",
                    5,
                    Severity::Warning,
                    Category::VagueDirective,
                    "vague2",
                ),
                make_diag(
                    "/project/b.md",
                    2,
                    Severity::Error,
                    Category::DeadReference,
                    "dead1",
                ),
            ],
        };
        let out = render_to_string(&result, Path::new("/project"));
        // Both categories should appear
        assert!(
            out.contains("vague-directive"),
            "Should list vague-directive"
        );
        assert!(out.contains("dead-reference"), "Should list dead-reference");
        // Count annotations
        assert!(out.contains("(2)"), "vague-directive should show count (2)");
        assert!(out.contains("(1)"), "dead-reference should show count (1)");
    }

    #[test]
    fn errors_listed_before_warnings_before_info() {
        no_color();
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Info,
                    Category::PlaceholderText,
                    "info msg",
                ),
                make_diag(
                    "/project/a.md",
                    2,
                    Severity::Error,
                    Category::DeadReference,
                    "error msg",
                ),
                make_diag(
                    "/project/a.md",
                    3,
                    Severity::Warning,
                    Category::VagueDirective,
                    "warning msg",
                ),
            ],
        };
        let out = render_to_string(&result, Path::new("/project"));
        let error_pos = out
            .find("dead-reference")
            .expect("should contain dead-reference");
        let warning_pos = out
            .find("vague-directive")
            .expect("should contain vague-directive");
        let info_pos = out
            .find("placeholder-text")
            .expect("should contain placeholder-text");
        assert!(
            error_pos < warning_pos,
            "Errors should appear before warnings"
        );
        assert!(warning_pos < info_pos, "Warnings should appear before info");
    }

    #[test]
    fn suggestion_appears_as_help() {
        no_color();
        let mut diag = make_diag(
            "/project/a.md",
            1,
            Severity::Warning,
            Category::VagueDirective,
            "too vague",
        );
        diag.suggestion = Some("be more specific".to_string());
        let result = CheckResult {
            diagnostics: vec![diag],
        };
        let out = render_to_string(&result, Path::new("/project"));
        assert!(
            out.contains("help: be more specific"),
            "Suggestion should render as 'help: ...'"
        );
    }

    #[test]
    fn relative_path_displayed() {
        no_color();
        let result = CheckResult {
            diagnostics: vec![make_diag(
                "/project/subdir/CLAUDE.md",
                1,
                Severity::Error,
                Category::DeadReference,
                "msg",
            )],
        };
        let out = render_to_string(&result, Path::new("/project"));
        assert!(
            out.contains("subdir/CLAUDE.md"),
            "Should show relative path, got:\n{out}"
        );
        assert!(
            !out.contains("/project/subdir/CLAUDE.md"),
            "Should not show absolute path"
        );
    }

    #[test]
    fn line_number_formatting() {
        no_color();
        let result = CheckResult {
            diagnostics: vec![make_diag(
                "/project/a.md",
                42,
                Severity::Warning,
                Category::VagueDirective,
                "my message",
            )],
        };
        let out = render_to_string(&result, Path::new("/project"));
        assert!(
            out.contains("L42"),
            "Should contain line number L42, got:\n{out}"
        );
        assert!(
            out.contains("my message"),
            "Should contain the diagnostic message"
        );
    }
}
