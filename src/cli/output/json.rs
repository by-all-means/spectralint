use serde::Serialize;
use std::path::Path;

use crate::types::{Category, CheckResult, Severity};

#[derive(Serialize)]
struct JsonOutput<'a> {
    diagnostics: Vec<JsonDiagnostic<'a>>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    file: String,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_column: Option<usize>,
    severity: &'a Severity,
    category: &'a Category,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<&'a str>,
}

#[derive(Serialize)]
struct JsonSummary {
    errors: usize,
    warnings: usize,
    info: usize,
}

fn build_output<'a>(result: &'a CheckResult, project_root: &Path) -> JsonOutput<'a> {
    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            file: super::relative_path(&d.file, project_root),
            line: d.line,
            column: d.column,
            end_line: d.end_line,
            end_column: d.end_column,
            severity: &d.severity,
            category: &d.category,
            message: &d.message,
            suggestion: d.suggestion.as_deref(),
        })
        .collect();

    let (errors, warnings, info) = result.severity_counts();
    JsonOutput {
        diagnostics,
        summary: JsonSummary {
            errors,
            warnings,
            info,
        },
    }
}

pub fn render(result: &CheckResult, project_root: &Path) {
    let output = build_output(result, project_root);
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .expect("JSON serialization of diagnostics cannot fail")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, Diagnostic, Severity};
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

    #[test]
    fn test_json_output_is_valid() {
        let result = CheckResult {
            diagnostics: vec![Diagnostic {
                file: Arc::new(PathBuf::from("/project/CLAUDE.md")),
                line: 10,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Error,
                category: Category::DeadReference,
                message: "file not found".to_string(),
                suggestion: None,
                fix: None,
            }],
        };

        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["summary"]["errors"], 1);
    }

    #[test]
    fn empty_diagnostics_list() {
        let result = CheckResult::default();
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["summary"]["errors"], 0);
        assert_eq!(parsed["summary"]["warnings"], 0);
        assert_eq!(parsed["summary"]["info"], 0);
    }

    #[test]
    fn path_normalization_strips_project_root() {
        let result = CheckResult {
            diagnostics: vec![make_diag(
                "/project/subdir/CLAUDE.md",
                1,
                Severity::Warning,
                Category::VagueDirective,
                "msg",
            )],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json["diagnostics"][0]["file"], "subdir/CLAUDE.md",
            "File path should be relative to project root"
        );
    }

    #[test]
    fn path_outside_project_root_kept_as_is() {
        let result = CheckResult {
            diagnostics: vec![make_diag(
                "/other/place/CLAUDE.md",
                1,
                Severity::Info,
                Category::PlaceholderText,
                "msg",
            )],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json["diagnostics"][0]["file"], "/other/place/CLAUDE.md",
            "Path outside project root should be kept as-is"
        );
    }

    #[test]
    fn optional_fields_omitted_when_none() {
        let result = CheckResult {
            diagnostics: vec![make_diag(
                "/project/a.md",
                1,
                Severity::Error,
                Category::DeadReference,
                "msg",
            )],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();
        let diag = &json["diagnostics"][0];
        let obj = diag.as_object().unwrap();

        assert!(
            !obj.contains_key("column"),
            "column should be omitted when None"
        );
        assert!(
            !obj.contains_key("end_line"),
            "end_line should be omitted when None"
        );
        assert!(
            !obj.contains_key("end_column"),
            "end_column should be omitted when None"
        );
        assert!(
            !obj.contains_key("suggestion"),
            "suggestion should be omitted when None"
        );
    }

    #[test]
    fn optional_fields_present_when_set() {
        let result = CheckResult {
            diagnostics: vec![Diagnostic {
                file: Arc::new(PathBuf::from("/project/a.md")),
                line: 5,
                column: Some(10),
                end_line: Some(7),
                end_column: Some(20),
                severity: Severity::Warning,
                category: Category::VagueDirective,
                message: "msg".to_string(),
                suggestion: Some("try this".to_string()),
                fix: None,
            }],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();
        let diag = &json["diagnostics"][0];

        assert_eq!(diag["column"], 10);
        assert_eq!(diag["end_line"], 7);
        assert_eq!(diag["end_column"], 20);
        assert_eq!(diag["suggestion"], "try this");
    }

    #[test]
    fn summary_counts_accuracy() {
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Error,
                    Category::DeadReference,
                    "e1",
                ),
                make_diag(
                    "/project/a.md",
                    2,
                    Severity::Error,
                    Category::DeadReference,
                    "e2",
                ),
                make_diag(
                    "/project/a.md",
                    3,
                    Severity::Warning,
                    Category::VagueDirective,
                    "w1",
                ),
                make_diag(
                    "/project/b.md",
                    1,
                    Severity::Info,
                    Category::PlaceholderText,
                    "i1",
                ),
                make_diag(
                    "/project/b.md",
                    2,
                    Severity::Info,
                    Category::PlaceholderText,
                    "i2",
                ),
                make_diag(
                    "/project/b.md",
                    3,
                    Severity::Info,
                    Category::PlaceholderText,
                    "i3",
                ),
            ],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["summary"]["errors"], 2);
        assert_eq!(json["summary"]["warnings"], 1);
        assert_eq!(json["summary"]["info"], 3);
        assert_eq!(json["diagnostics"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn severity_serialized_as_lowercase_string() {
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Error,
                    Category::DeadReference,
                    "e",
                ),
                make_diag(
                    "/project/a.md",
                    2,
                    Severity::Warning,
                    Category::VagueDirective,
                    "w",
                ),
                make_diag(
                    "/project/a.md",
                    3,
                    Severity::Info,
                    Category::PlaceholderText,
                    "i",
                ),
            ],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["diagnostics"][0]["severity"], "error");
        assert_eq!(json["diagnostics"][1]["severity"], "warning");
        assert_eq!(json["diagnostics"][2]["severity"], "info");
    }

    #[test]
    fn category_serialized_as_kebab_case() {
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Error,
                    Category::DeadReference,
                    "msg",
                ),
                make_diag(
                    "/project/a.md",
                    2,
                    Severity::Warning,
                    Category::NamingInconsistency,
                    "msg",
                ),
            ],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["diagnostics"][0]["category"], "dead-reference");
        assert_eq!(json["diagnostics"][1]["category"], "naming-inconsistency");
    }

    #[test]
    fn output_is_valid_json() {
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Error,
                    Category::DeadReference,
                    "e",
                ),
                make_diag(
                    "/project/b.md",
                    2,
                    Severity::Warning,
                    Category::VagueDirective,
                    "w",
                ),
            ],
        };
        let output = build_output(&result, Path::new("/project"));
        let json_str = serde_json::to_string_pretty(&output).unwrap();
        // Verify it parses back cleanly
        let _: serde_json::Value =
            serde_json::from_str(&json_str).expect("Output should be valid JSON");
    }
}
