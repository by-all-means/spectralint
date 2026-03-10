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
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .expect("SARIF serialization of diagnostics cannot fail")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, Diagnostic};
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

    #[test]
    fn empty_results_produces_valid_sarif() {
        let result = CheckResult::default();
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["version"], "2.1.0");
        assert_eq!(
            json["runs"][0]["results"].as_array().unwrap().len(),
            0,
            "Empty input should produce zero results"
        );
        assert_eq!(
            json["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .unwrap()
                .len(),
            0,
            "Empty input should produce zero rules"
        );
    }

    #[test]
    fn multiple_rules_in_output() {
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Error,
                    Category::DeadReference,
                    "dead ref",
                ),
                make_diag(
                    "/project/a.md",
                    2,
                    Severity::Warning,
                    Category::VagueDirective,
                    "vague",
                ),
                make_diag(
                    "/project/b.md",
                    3,
                    Severity::Info,
                    Category::PlaceholderText,
                    "placeholder",
                ),
                // Second dead-reference — should NOT create a duplicate rule
                make_diag(
                    "/project/b.md",
                    4,
                    Severity::Error,
                    Category::DeadReference,
                    "dead ref 2",
                ),
            ],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();

        let rules = json["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert_eq!(
            rules.len(),
            3,
            "Should have 3 unique rules (dead-reference, vague-directive, placeholder-text)"
        );

        let rule_ids: Vec<&str> = rules.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert!(rule_ids.contains(&"dead-reference"));
        assert!(rule_ids.contains(&"vague-directive"));
        assert!(rule_ids.contains(&"placeholder-text"));

        // Results should still have all 4 entries
        let results = json["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn rules_are_sorted_deterministically() {
        // BTreeSet ensures sorted order of rule IDs
        let result = CheckResult {
            diagnostics: vec![
                make_diag(
                    "/project/a.md",
                    1,
                    Severity::Warning,
                    Category::VagueDirective,
                    "v",
                ),
                make_diag(
                    "/project/a.md",
                    2,
                    Severity::Error,
                    Category::DeadReference,
                    "d",
                ),
                make_diag(
                    "/project/a.md",
                    3,
                    Severity::Info,
                    Category::PlaceholderText,
                    "p",
                ),
            ],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();

        let rule_ids: Vec<&str> = json["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            rule_ids,
            vec!["dead-reference", "placeholder-text", "vague-directive"],
            "Rules should be sorted alphabetically"
        );
    }

    #[test]
    fn severity_mapped_correctly_in_results() {
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
        let results = json["runs"][0]["results"].as_array().unwrap();

        assert_eq!(results[0]["level"], "error");
        assert_eq!(results[1]["level"], "warning");
        assert_eq!(results[2]["level"], "note", "Info should map to 'note'");
    }

    #[test]
    fn region_optional_fields() {
        // Without column/end_line/end_column
        let result_no_cols = CheckResult {
            diagnostics: vec![make_diag(
                "/project/a.md",
                10,
                Severity::Error,
                Category::DeadReference,
                "msg",
            )],
        };
        let json =
            serde_json::to_value(build_output(&result_no_cols, Path::new("/project"))).unwrap();
        let region = &json["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"];
        assert_eq!(region["startLine"], 10);
        assert!(
            region.get("startColumn").is_none() || region["startColumn"].is_null(),
            "startColumn should be absent when column is None"
        );
        assert!(
            region.get("endLine").is_none() || region["endLine"].is_null(),
            "endLine should be absent when end_line is None"
        );

        // With column/end_line/end_column
        let result_with_cols = CheckResult {
            diagnostics: vec![Diagnostic {
                file: Arc::new(PathBuf::from("/project/a.md")),
                line: 5,
                column: Some(3),
                end_line: Some(8),
                end_column: Some(15),
                severity: Severity::Warning,
                category: Category::VagueDirective,
                message: "msg".to_string(),
                suggestion: None,
                fix: None,
            }],
        };
        let json =
            serde_json::to_value(build_output(&result_with_cols, Path::new("/project"))).unwrap();
        let region = &json["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"];
        assert_eq!(region["startLine"], 5);
        assert_eq!(region["startColumn"], 3);
        assert_eq!(region["endLine"], 8);
        assert_eq!(region["endColumn"], 15);
    }

    #[test]
    fn sarif_schema_and_version() {
        let result = CheckResult::default();
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["version"], "2.1.0");
        assert!(
            json["$schema"]
                .as_str()
                .unwrap()
                .contains("sarif-schema-2.1.0"),
            "Schema URL should reference SARIF 2.1.0"
        );
    }

    #[test]
    fn artifact_location_uses_relative_path() {
        let result = CheckResult {
            diagnostics: vec![make_diag(
                "/project/sub/dir/CLAUDE.md",
                1,
                Severity::Error,
                Category::DeadReference,
                "msg",
            )],
        };
        let output = build_output(&result, Path::new("/project"));
        let json = serde_json::to_value(&output).unwrap();
        let uri = json["runs"][0]["results"][0]["locations"][0]["physicalLocation"]
            ["artifactLocation"]["uri"]
            .as_str()
            .unwrap();
        assert_eq!(uri, "sub/dir/CLAUDE.md");
    }
}
