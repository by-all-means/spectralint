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
    severity: &'a Severity,
    category: &'a Category,
    message: &'a str,
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
            severity: &d.severity,
            category: &d.category,
            message: &d.message,
        })
        .collect();

    JsonOutput {
        diagnostics,
        summary: JsonSummary {
            errors: result.error_count(),
            warnings: result.warning_count(),
            info: result.info_count(),
        },
    }
}

pub fn render(result: &CheckResult, project_root: &Path) {
    let output = build_output(result, project_root);
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, Diagnostic, Severity};
    use std::path::PathBuf;

    #[test]
    fn test_json_output_is_valid() {
        let result = CheckResult {
            diagnostics: vec![Diagnostic {
                file: PathBuf::from("/project/CLAUDE.md"),
                line: 10,
                severity: Severity::Error,
                category: Category::DeadReference,
                message: "file not found".to_string(),
            }],
        };

        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["summary"]["errors"], 1);
    }
}
