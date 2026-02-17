use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    DeadReference,
    VagueDirective,
    NamingInconsistency,
    EnumDrift,
    AgentGuidelines,
    PlaceholderText,
    FileSize,
    CredentialExposure,
    HeadingHierarchy,
    DangerousCommand,
    StaleReference,
    EmojiDensity,
    SessionJournal,
    MissingEssentialSections,
    PromptInjectionVector,
    MissingVerification,
    NegativeOnlyFraming,
    CustomPattern(String),
}

impl Serialize for Category {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::DeadReference => f.write_str("dead-reference"),
            Category::VagueDirective => f.write_str("vague-directive"),
            Category::NamingInconsistency => f.write_str("naming-inconsistency"),
            Category::EnumDrift => f.write_str("enum-drift"),
            Category::AgentGuidelines => f.write_str("agent-guidelines"),
            Category::PlaceholderText => f.write_str("placeholder-text"),
            Category::FileSize => f.write_str("file-size"),
            Category::CredentialExposure => f.write_str("credential-exposure"),
            Category::HeadingHierarchy => f.write_str("heading-hierarchy"),
            Category::DangerousCommand => f.write_str("dangerous-command"),
            Category::StaleReference => f.write_str("stale-reference"),
            Category::EmojiDensity => f.write_str("emoji-density"),
            Category::SessionJournal => f.write_str("session-journal"),
            Category::MissingEssentialSections => f.write_str("missing-essential-sections"),
            Category::PromptInjectionVector => f.write_str("prompt-injection-vector"),
            Category::MissingVerification => f.write_str("missing-verification"),
            Category::NegativeOnlyFraming => f.write_str("negative-only-framing"),
            Category::CustomPattern(name) => write!(f, "custom:{name}"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub file: PathBuf,
    pub line: usize,
    pub severity: Severity,
    pub category: Category,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Default)]
pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    fn count_severity(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.count_severity(Severity::Error)
    }

    pub fn warning_count(&self) -> usize {
        self.count_severity(Severity::Warning)
    }

    pub fn info_count(&self) -> usize {
        self.count_severity(Severity::Info)
    }

    pub fn has_severity_at_least(&self, threshold: Severity) -> bool {
        self.diagnostics.iter().any(|d| d.severity >= threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_diagnostic(severity: Severity) -> Diagnostic {
        Diagnostic {
            file: PathBuf::from("test.md"),
            line: 1,
            severity,
            category: Category::DeadReference,
            message: "test".to_string(),
            suggestion: None,
        }
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
        assert!(Severity::Info < Severity::Error);
    }

    #[test]
    fn test_has_severity_at_least_error() {
        let result = CheckResult {
            diagnostics: vec![make_diagnostic(Severity::Error)],
        };
        assert!(result.has_severity_at_least(Severity::Error));
        assert!(result.has_severity_at_least(Severity::Warning));
        assert!(result.has_severity_at_least(Severity::Info));
    }

    #[test]
    fn test_has_severity_at_least_warning_only() {
        let result = CheckResult {
            diagnostics: vec![make_diagnostic(Severity::Warning)],
        };
        assert!(!result.has_severity_at_least(Severity::Error));
        assert!(result.has_severity_at_least(Severity::Warning));
        assert!(result.has_severity_at_least(Severity::Info));
    }

    #[test]
    fn test_has_severity_at_least_info_only() {
        let result = CheckResult {
            diagnostics: vec![make_diagnostic(Severity::Info)],
        };
        assert!(!result.has_severity_at_least(Severity::Error));
        assert!(!result.has_severity_at_least(Severity::Warning));
        assert!(result.has_severity_at_least(Severity::Info));
    }

    #[test]
    fn test_has_severity_at_least_empty() {
        let result = CheckResult::default();
        assert!(!result.has_severity_at_least(Severity::Info));
    }

    #[test]
    fn test_count_methods() {
        let result = CheckResult {
            diagnostics: vec![
                make_diagnostic(Severity::Error),
                make_diagnostic(Severity::Error),
                make_diagnostic(Severity::Warning),
                make_diagnostic(Severity::Info),
                make_diagnostic(Severity::Info),
                make_diagnostic(Severity::Info),
            ],
        };
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 1);
        assert_eq!(result.info_count(), 3);
    }

    #[test]
    fn test_category_display() {
        assert_eq!(Category::DeadReference.to_string(), "dead-reference");
        assert_eq!(Category::VagueDirective.to_string(), "vague-directive");
        assert_eq!(
            Category::NamingInconsistency.to_string(),
            "naming-inconsistency"
        );
        assert_eq!(Category::EnumDrift.to_string(), "enum-drift");
        assert_eq!(Category::AgentGuidelines.to_string(), "agent-guidelines");
        assert_eq!(Category::PlaceholderText.to_string(), "placeholder-text");
        assert_eq!(Category::FileSize.to_string(), "file-size");
        assert_eq!(
            Category::CredentialExposure.to_string(),
            "credential-exposure"
        );
        assert_eq!(Category::HeadingHierarchy.to_string(), "heading-hierarchy");
        assert_eq!(Category::DangerousCommand.to_string(), "dangerous-command");
        assert_eq!(Category::StaleReference.to_string(), "stale-reference");
        assert_eq!(Category::EmojiDensity.to_string(), "emoji-density");
        assert_eq!(Category::SessionJournal.to_string(), "session-journal");
        assert_eq!(
            Category::MissingEssentialSections.to_string(),
            "missing-essential-sections"
        );
        assert_eq!(
            Category::PromptInjectionVector.to_string(),
            "prompt-injection-vector"
        );
        assert_eq!(
            Category::MissingVerification.to_string(),
            "missing-verification"
        );
        assert_eq!(
            Category::NegativeOnlyFraming.to_string(),
            "negative-only-framing"
        );
        assert_eq!(
            Category::CustomPattern("todo".to_string()).to_string(),
            "custom:todo"
        );
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Info.to_string(), "info");
    }

    #[test]
    fn test_category_serialization() {
        let d = make_diagnostic(Severity::Error);
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["category"], "dead-reference");
        assert_eq!(json["severity"], "error");
    }

    // ── Item 23: Severity deserialization round-trip ──────────────────────

    #[test]
    fn test_severity_deserialize_roundtrip() {
        for sev in [Severity::Info, Severity::Warning, Severity::Error] {
            let json = serde_json::to_string(&sev).unwrap();
            let parsed: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, sev);
        }
    }

    #[test]
    fn test_severity_deserialize_invalid() {
        let result: Result<Severity, _> = serde_json::from_str(r#""critical""#);
        assert!(
            result.is_err(),
            "Invalid severity should fail deserialization"
        );
    }

    // ── Item 25: Diagnostic sort stability ───────────────────────────────

    #[test]
    fn test_diagnostic_sort_by_file_then_line() {
        let mut diagnostics = [
            Diagnostic {
                file: PathBuf::from("b.md"),
                line: 10,
                severity: Severity::Error,
                category: Category::DeadReference,
                message: "msg1".to_string(),
                suggestion: None,
            },
            Diagnostic {
                file: PathBuf::from("a.md"),
                line: 5,
                severity: Severity::Warning,
                category: Category::VagueDirective,
                message: "msg2".to_string(),
                suggestion: None,
            },
            Diagnostic {
                file: PathBuf::from("a.md"),
                line: 1,
                severity: Severity::Info,
                category: Category::EnumDrift,
                message: "msg3".to_string(),
                suggestion: None,
            },
        ];

        diagnostics.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

        assert_eq!(diagnostics[0].file, PathBuf::from("a.md"));
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[1].file, PathBuf::from("a.md"));
        assert_eq!(diagnostics[1].line, 5);
        assert_eq!(diagnostics[2].file, PathBuf::from("b.md"));
        assert_eq!(diagnostics[2].line, 10);
    }

    #[test]
    fn test_structural_dedup() {
        let mut diagnostics = vec![
            Diagnostic {
                file: PathBuf::from("a.md"),
                line: 5,
                severity: Severity::Warning,
                category: Category::DeadReference,
                message: "first".to_string(),
                suggestion: None,
            },
            Diagnostic {
                file: PathBuf::from("a.md"),
                line: 5,
                severity: Severity::Warning,
                category: Category::DeadReference,
                message: "first".to_string(),
                suggestion: None,
            },
            Diagnostic {
                file: PathBuf::from("a.md"),
                line: 5,
                severity: Severity::Info,
                category: Category::VagueDirective,
                message: "different rule same line".to_string(),
                suggestion: None,
            },
            Diagnostic {
                file: PathBuf::from("a.md"),
                line: 10,
                severity: Severity::Warning,
                category: Category::DeadReference,
                message: "different line".to_string(),
                suggestion: None,
            },
        ];

        diagnostics.sort_by(|a, b| {
            (&a.file, a.line, &a.category, &a.message).cmp(&(
                &b.file,
                b.line,
                &b.category,
                &b.message,
            ))
        });
        diagnostics.dedup_by(|a, b| {
            a.file == b.file
                && a.line == b.line
                && a.category == b.category
                && a.message == b.message
        });

        assert_eq!(
            diagnostics.len(),
            3,
            "Duplicate (same category+file+line) should be removed, different category kept"
        );
        assert_eq!(diagnostics[0].message, "first");
        assert_eq!(diagnostics[1].message, "different rule same line");
        assert_eq!(diagnostics[2].message, "different line");
    }
}
