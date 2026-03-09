use serde::Serialize;
use std::path::Path;

use crate::types::{CheckResult, Severity};

#[derive(Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
struct SarifRule {
    id: String,
    #[serde(rename = "shortDescription")]
    short_description: SarifMessage,
}

#[derive(Serialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: String,
    level: &'static str,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<SarifMessage>,
}

#[derive(Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: usize,
    #[serde(rename = "startColumn", skip_serializing_if = "Option::is_none")]
    start_column: Option<usize>,
    #[serde(rename = "endLine", skip_serializing_if = "Option::is_none")]
    end_line: Option<usize>,
    #[serde(rename = "endColumn", skip_serializing_if = "Option::is_none")]
    end_column: Option<usize>,
}

fn severity_to_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

fn build_output(result: &CheckResult, project_root: &Path) -> SarifLog {
    use std::collections::BTreeSet;

    let version = env!("CARGO_PKG_VERSION");

    let rule_ids: BTreeSet<String> = result
        .diagnostics
        .iter()
        .map(|d| d.category.to_string())
        .collect();

    let rules: Vec<SarifRule> = rule_ids
        .into_iter()
        .map(|id| SarifRule {
            short_description: SarifMessage { text: id.clone() },
            id,
        })
        .collect();

    let results: Vec<SarifResult> = result
        .diagnostics
        .iter()
        .map(|d| {
            let rel = super::relative_path(&d.file, project_root);
            SarifResult {
                rule_id: d.category.to_string(),
                level: severity_to_level(d.severity),
                message: SarifMessage {
                    text: d.message.clone(),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation { uri: rel },
                        region: SarifRegion {
                            start_line: d.line,
                            start_column: d.column,
                            end_line: d.end_line,
                            end_column: d.end_column,
                        },
                    },
                }],
                help: d
                    .suggestion
                    .as_ref()
                    .map(|s| SarifMessage { text: s.clone() }),
            }
        })
        .collect();

    SarifLog {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        version: "2.1.0",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "spectralint",
                    version,
                    rules,
                },
            },
            results,
        }],
    }
}

pub fn render(result: &CheckResult, project_root: &Path) {
    let output = build_output(result, project_root);
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, Diagnostic};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_sarif_output_structure() {
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

        assert_eq!(parsed["version"], "2.1.0");
        assert_eq!(parsed["runs"][0]["tool"]["driver"]["name"], "spectralint");
        assert_eq!(parsed["runs"][0]["results"][0]["ruleId"], "dead-reference");
        assert_eq!(parsed["runs"][0]["results"][0]["level"], "error");
        assert_eq!(
            parsed["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]
                ["startLine"],
            10
        );
        // No suggestion → no help field
        assert!(parsed["runs"][0]["results"][0]["help"].is_null());
    }

    #[test]
    fn test_sarif_output_includes_suggestion_as_help() {
        let result = CheckResult {
            diagnostics: vec![Diagnostic {
                file: Arc::new(PathBuf::from("/project/CLAUDE.md")),
                line: 5,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Warning,
                category: Category::VagueDirective,
                message: "directive is too vague".to_string(),
                suggestion: Some("Be more specific about the expected behavior".to_string()),
                fix: None,
            }],
        };

        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_string_pretty(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(
            parsed["runs"][0]["results"][0]["help"]["text"],
            "Be more specific about the expected behavior"
        );
    }

    #[test]
    fn test_severity_mapping() {
        assert_eq!(severity_to_level(Severity::Error), "error");
        assert_eq!(severity_to_level(Severity::Warning), "warning");
        assert_eq!(severity_to_level(Severity::Info), "note");
    }
}
