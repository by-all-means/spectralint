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
}
