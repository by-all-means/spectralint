use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// A structured autofix: a description plus one or more text replacements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    pub description: String,
    pub replacements: Vec<Replacement>,
}

/// A single text replacement within a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replacement {
    pub line: usize,      // 1-based line number
    pub start_col: usize, // 0-based byte offset in line
    pub end_col: usize,   // 0-based byte offset in line (exclusive)
    pub new_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Metadata that every checker provides, enabling auto-registration
/// and eliminating the need to manually update multiple files when
/// adding a new checker.
#[derive(Debug, Clone)]
pub struct RuleMeta {
    /// Kebab-case rule name (e.g. "dead-reference").
    pub name: &'static str,
    /// One-line description shown in `spectralint list`.
    pub description: &'static str,
    /// The default severity emitted by this checker.
    pub default_severity: Severity,
    /// If true, the checker is only enabled when `--strict` is passed
    /// (or when explicitly enabled in config).
    pub strict_only: bool,
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
    ConflictingDirectives,
    MissingRoleDefinition,
    RedundantDirective,
    InstructionDensity,
    MissingExamples,
    UnboundedScope,
    CircularReference,
    LargeCodeBlock,
    DuplicateSection,
    AbsolutePath,
    GenericInstruction,
    MisorderedSteps,
    SectionLengthImbalance,
    UnclosedFence,
    UntaggedCodeBlock,
    DuplicateInstructionFile,
    OutdatedModelReference,
    BrokenTable,
    PlaceholderUrl,
    EmphasisOveruse,
    BoilerplateTemplate,
    OrphanedSection,
    ExcessiveNesting,
    ContextWindowWaste,
    AmbiguousScopeReference,
    InstructionWithoutContext,
    CrossFileContradiction,
    StaleStyleRule,
    HardcodedFileStructure,
    UnversionedStackReference,
    MissingStandardFile,
    BareUrl,
    RepeatedWord,
    UndocumentedEnvVar,
    EmptyCodeBlock,
    ClickHereLink,
    DoubleNegation,
    ImperativeHeading,
    InconsistentCommandPrefix,
    EmptyHeading,
    CopiedMetaInstructions,
    XmlDocumentWrapper,
    GeneratedAttribution,
    CommandWithoutCodeblock,
    MissingVerificationStep,
    BrokenAnchorLink,
    LongParagraph,
    HardcodedWindowsPath,
    StaleFileTree,
    CommandValidation,
    TokenBudget,
    InvalidSuppression,
    UnusedSuppression,
    CustomPattern(Box<str>),
}

impl Category {
    /// Returns the kebab-case name of this category without allocating
    /// for built-in variants. For `CustomPattern`, returns the pattern
    /// name (without the `custom:` prefix).
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Category::DeadReference => "dead-reference",
            Category::VagueDirective => "vague-directive",
            Category::NamingInconsistency => "naming-inconsistency",
            Category::EnumDrift => "enum-drift",
            Category::AgentGuidelines => "agent-guidelines",
            Category::PlaceholderText => "placeholder-text",
            Category::FileSize => "file-size",
            Category::CredentialExposure => "credential-exposure",
            Category::HeadingHierarchy => "heading-hierarchy",
            Category::DangerousCommand => "dangerous-command",
            Category::StaleReference => "stale-reference",
            Category::EmojiDensity => "emoji-density",
            Category::SessionJournal => "session-journal",
            Category::MissingEssentialSections => "missing-essential-sections",
            Category::PromptInjectionVector => "prompt-injection-vector",
            Category::MissingVerification => "missing-verification",
            Category::NegativeOnlyFraming => "negative-only-framing",
            Category::ConflictingDirectives => "conflicting-directives",
            Category::MissingRoleDefinition => "missing-role-definition",
            Category::RedundantDirective => "redundant-directive",
            Category::InstructionDensity => "instruction-density",
            Category::MissingExamples => "missing-examples",
            Category::UnboundedScope => "unbounded-scope",
            Category::CircularReference => "circular-reference",
            Category::LargeCodeBlock => "large-code-block",
            Category::DuplicateSection => "duplicate-section",
            Category::AbsolutePath => "absolute-path",
            Category::GenericInstruction => "generic-instruction",
            Category::MisorderedSteps => "misordered-steps",
            Category::SectionLengthImbalance => "section-length-imbalance",
            Category::UnclosedFence => "unclosed-fence",
            Category::UntaggedCodeBlock => "untagged-code-block",
            Category::DuplicateInstructionFile => "duplicate-instruction-file",
            Category::OutdatedModelReference => "outdated-model-reference",
            Category::BrokenTable => "broken-table",
            Category::PlaceholderUrl => "placeholder-url",
            Category::EmphasisOveruse => "emphasis-overuse",
            Category::BoilerplateTemplate => "boilerplate-template",
            Category::OrphanedSection => "orphaned-section",
            Category::ExcessiveNesting => "excessive-nesting",
            Category::ContextWindowWaste => "context-window-waste",
            Category::AmbiguousScopeReference => "ambiguous-scope-reference",
            Category::InstructionWithoutContext => "instruction-without-context",
            Category::CrossFileContradiction => "cross-file-contradiction",
            Category::StaleStyleRule => "stale-style-rule",
            Category::HardcodedFileStructure => "hardcoded-file-structure",
            Category::UnversionedStackReference => "unversioned-stack-reference",
            Category::MissingStandardFile => "missing-standard-file",
            Category::BareUrl => "bare-url",
            Category::RepeatedWord => "repeated-word",
            Category::UndocumentedEnvVar => "undocumented-env-var",
            Category::EmptyCodeBlock => "empty-code-block",
            Category::ClickHereLink => "click-here-link",
            Category::DoubleNegation => "double-negation",
            Category::ImperativeHeading => "imperative-heading",
            Category::InconsistentCommandPrefix => "inconsistent-command-prefix",
            Category::EmptyHeading => "empty-heading",
            Category::CopiedMetaInstructions => "copied-meta-instructions",
            Category::XmlDocumentWrapper => "xml-document-wrapper",
            Category::GeneratedAttribution => "generated-attribution",
            Category::CommandWithoutCodeblock => "command-without-codeblock",
            Category::MissingVerificationStep => "missing-verification-step",
            Category::BrokenAnchorLink => "broken-anchor-link",
            Category::LongParagraph => "long-paragraph",
            Category::HardcodedWindowsPath => "hardcoded-windows-path",
            Category::StaleFileTree => "stale-file-tree",
            Category::CommandValidation => "command-validation",
            Category::TokenBudget => "token-budget",
            Category::InvalidSuppression => "invalid-suppression",
            Category::UnusedSuppression => "unused-suppression",
            Category::CustomPattern(name) => name,
        }
    }
}

impl Serialize for Category {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Category {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::CustomPattern(_) => write!(f, "custom:{}", self.as_str()),
            _ => f.write_str(self.as_str()),
        }
    }
}

impl std::str::FromStr for Category {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "dead-reference" => Ok(Category::DeadReference),
            "vague-directive" => Ok(Category::VagueDirective),
            "naming-inconsistency" => Ok(Category::NamingInconsistency),
            "enum-drift" => Ok(Category::EnumDrift),
            "agent-guidelines" => Ok(Category::AgentGuidelines),
            "placeholder-text" => Ok(Category::PlaceholderText),
            "file-size" => Ok(Category::FileSize),
            "credential-exposure" => Ok(Category::CredentialExposure),
            "heading-hierarchy" => Ok(Category::HeadingHierarchy),
            "dangerous-command" => Ok(Category::DangerousCommand),
            "stale-reference" => Ok(Category::StaleReference),
            "emoji-density" => Ok(Category::EmojiDensity),
            "session-journal" => Ok(Category::SessionJournal),
            "missing-essential-sections" => Ok(Category::MissingEssentialSections),
            "prompt-injection-vector" => Ok(Category::PromptInjectionVector),
            "missing-verification" => Ok(Category::MissingVerification),
            "negative-only-framing" => Ok(Category::NegativeOnlyFraming),
            "conflicting-directives" => Ok(Category::ConflictingDirectives),
            "missing-role-definition" => Ok(Category::MissingRoleDefinition),
            "redundant-directive" => Ok(Category::RedundantDirective),
            "instruction-density" => Ok(Category::InstructionDensity),
            "missing-examples" => Ok(Category::MissingExamples),
            "unbounded-scope" => Ok(Category::UnboundedScope),
            "circular-reference" => Ok(Category::CircularReference),
            "large-code-block" => Ok(Category::LargeCodeBlock),
            "duplicate-section" => Ok(Category::DuplicateSection),
            "absolute-path" => Ok(Category::AbsolutePath),
            "generic-instruction" => Ok(Category::GenericInstruction),
            "misordered-steps" => Ok(Category::MisorderedSteps),
            "section-length-imbalance" => Ok(Category::SectionLengthImbalance),
            "unclosed-fence" => Ok(Category::UnclosedFence),
            "untagged-code-block" => Ok(Category::UntaggedCodeBlock),
            "duplicate-instruction-file" => Ok(Category::DuplicateInstructionFile),
            "outdated-model-reference" => Ok(Category::OutdatedModelReference),
            "broken-table" => Ok(Category::BrokenTable),
            "placeholder-url" => Ok(Category::PlaceholderUrl),
            "emphasis-overuse" => Ok(Category::EmphasisOveruse),
            "boilerplate-template" => Ok(Category::BoilerplateTemplate),
            "orphaned-section" => Ok(Category::OrphanedSection),
            "excessive-nesting" => Ok(Category::ExcessiveNesting),
            "context-window-waste" => Ok(Category::ContextWindowWaste),
            "ambiguous-scope-reference" => Ok(Category::AmbiguousScopeReference),
            "instruction-without-context" => Ok(Category::InstructionWithoutContext),
            "cross-file-contradiction" => Ok(Category::CrossFileContradiction),
            "stale-style-rule" => Ok(Category::StaleStyleRule),
            "hardcoded-file-structure" => Ok(Category::HardcodedFileStructure),
            "unversioned-stack-reference" => Ok(Category::UnversionedStackReference),
            "missing-standard-file" => Ok(Category::MissingStandardFile),
            "bare-url" => Ok(Category::BareUrl),
            "repeated-word" => Ok(Category::RepeatedWord),
            "undocumented-env-var" => Ok(Category::UndocumentedEnvVar),
            "empty-code-block" => Ok(Category::EmptyCodeBlock),
            "click-here-link" => Ok(Category::ClickHereLink),
            "double-negation" => Ok(Category::DoubleNegation),
            "imperative-heading" => Ok(Category::ImperativeHeading),
            "inconsistent-command-prefix" => Ok(Category::InconsistentCommandPrefix),
            "empty-heading" => Ok(Category::EmptyHeading),
            "copied-meta-instructions" => Ok(Category::CopiedMetaInstructions),
            "xml-document-wrapper" => Ok(Category::XmlDocumentWrapper),
            "generated-attribution" => Ok(Category::GeneratedAttribution),
            "command-without-codeblock" => Ok(Category::CommandWithoutCodeblock),
            "missing-verification-step" => Ok(Category::MissingVerificationStep),
            "broken-anchor-link" => Ok(Category::BrokenAnchorLink),
            "long-paragraph" => Ok(Category::LongParagraph),
            "hardcoded-windows-path" => Ok(Category::HardcodedWindowsPath),
            "stale-file-tree" => Ok(Category::StaleFileTree),
            "command-validation" => Ok(Category::CommandValidation),
            "token-budget" => Ok(Category::TokenBudget),
            "invalid-suppression" => Ok(Category::InvalidSuppression),
            "unused-suppression" => Ok(Category::UnusedSuppression),
            other => {
                if let Some(name) = other.strip_prefix("custom:") {
                    Ok(Category::CustomPattern(name.into()))
                } else {
                    Err(format!("unknown category: {other}"))
                }
            }
        }
    }
}

fn serialize_arc_pathbuf<S: serde::Serializer>(
    path: &Arc<PathBuf>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.collect_str(&path.display())
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    #[serde(serialize_with = "serialize_arc_pathbuf")]
    pub file: Arc<PathBuf>,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<usize>,
    pub severity: Severity,
    pub category: Category,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Box<Fix>>,
}

#[derive(Debug, Default)]
pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    #[must_use]
    fn count_severity(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.count_severity(Severity::Error)
    }

    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.count_severity(Severity::Warning)
    }

    #[must_use]
    pub fn info_count(&self) -> usize {
        self.count_severity(Severity::Info)
    }

    /// Returns (errors, warnings, info) in a single pass.
    #[must_use]
    pub fn severity_counts(&self) -> (usize, usize, usize) {
        let mut e = 0;
        let mut w = 0;
        let mut i = 0;
        for d in &self.diagnostics {
            match d.severity {
                Severity::Error => e += 1,
                Severity::Warning => w += 1,
                Severity::Info => i += 1,
            }
        }
        (e, w, i)
    }

    #[must_use]
    pub fn has_severity_at_least(&self, threshold: Severity) -> bool {
        self.diagnostics.iter().any(|d| d.severity >= threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diagnostic(severity: Severity) -> Diagnostic {
        Diagnostic {
            file: Arc::new(PathBuf::from("test.md")),
            line: 1,
            column: None,
            end_line: None,
            end_column: None,
            severity,
            category: Category::DeadReference,
            message: "test".to_string(),
            suggestion: None,
            fix: None,
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
            Category::ConflictingDirectives.to_string(),
            "conflicting-directives"
        );
        assert_eq!(
            Category::MissingRoleDefinition.to_string(),
            "missing-role-definition"
        );
        assert_eq!(
            Category::RedundantDirective.to_string(),
            "redundant-directive"
        );
        assert_eq!(
            Category::InstructionDensity.to_string(),
            "instruction-density"
        );
        assert_eq!(Category::MissingExamples.to_string(), "missing-examples");
        assert_eq!(Category::UnboundedScope.to_string(), "unbounded-scope");
        assert_eq!(
            Category::CircularReference.to_string(),
            "circular-reference"
        );
        assert_eq!(Category::LargeCodeBlock.to_string(), "large-code-block");
        assert_eq!(Category::DuplicateSection.to_string(), "duplicate-section");
        assert_eq!(Category::AbsolutePath.to_string(), "absolute-path");
        assert_eq!(
            Category::GenericInstruction.to_string(),
            "generic-instruction"
        );
        assert_eq!(Category::MisorderedSteps.to_string(), "misordered-steps");
        assert_eq!(
            Category::SectionLengthImbalance.to_string(),
            "section-length-imbalance"
        );
        assert_eq!(Category::UnclosedFence.to_string(), "unclosed-fence");
        assert_eq!(
            Category::UntaggedCodeBlock.to_string(),
            "untagged-code-block"
        );
        assert_eq!(
            Category::DuplicateInstructionFile.to_string(),
            "duplicate-instruction-file"
        );
        assert_eq!(
            Category::OutdatedModelReference.to_string(),
            "outdated-model-reference"
        );
        assert_eq!(Category::BrokenTable.to_string(), "broken-table");
        assert_eq!(Category::PlaceholderUrl.to_string(), "placeholder-url");
        assert_eq!(Category::EmphasisOveruse.to_string(), "emphasis-overuse");
        assert_eq!(
            Category::BoilerplateTemplate.to_string(),
            "boilerplate-template"
        );
        assert_eq!(Category::OrphanedSection.to_string(), "orphaned-section");
        assert_eq!(Category::ExcessiveNesting.to_string(), "excessive-nesting");
        assert_eq!(
            Category::ContextWindowWaste.to_string(),
            "context-window-waste"
        );
        assert_eq!(
            Category::AmbiguousScopeReference.to_string(),
            "ambiguous-scope-reference"
        );
        assert_eq!(
            Category::InstructionWithoutContext.to_string(),
            "instruction-without-context"
        );
        assert_eq!(
            Category::CrossFileContradiction.to_string(),
            "cross-file-contradiction"
        );
        assert_eq!(Category::StaleStyleRule.to_string(), "stale-style-rule");
        assert_eq!(
            Category::HardcodedFileStructure.to_string(),
            "hardcoded-file-structure"
        );
        assert_eq!(
            Category::UnversionedStackReference.to_string(),
            "unversioned-stack-reference"
        );
        assert_eq!(
            Category::MissingStandardFile.to_string(),
            "missing-standard-file"
        );
        assert_eq!(Category::BareUrl.to_string(), "bare-url");
        assert_eq!(Category::RepeatedWord.to_string(), "repeated-word");
        assert_eq!(
            Category::UndocumentedEnvVar.to_string(),
            "undocumented-env-var"
        );
        assert_eq!(Category::EmptyCodeBlock.to_string(), "empty-code-block");
        assert_eq!(Category::ClickHereLink.to_string(), "click-here-link");
        assert_eq!(Category::DoubleNegation.to_string(), "double-negation");
        assert_eq!(
            Category::ImperativeHeading.to_string(),
            "imperative-heading"
        );
        assert_eq!(
            Category::InconsistentCommandPrefix.to_string(),
            "inconsistent-command-prefix"
        );
        assert_eq!(Category::EmptyHeading.to_string(), "empty-heading");
        assert_eq!(
            Category::CopiedMetaInstructions.to_string(),
            "copied-meta-instructions"
        );
        assert_eq!(
            Category::XmlDocumentWrapper.to_string(),
            "xml-document-wrapper"
        );
        assert_eq!(
            Category::GeneratedAttribution.to_string(),
            "generated-attribution"
        );
        assert_eq!(
            Category::CommandWithoutCodeblock.to_string(),
            "command-without-codeblock"
        );
        assert_eq!(
            Category::MissingVerificationStep.to_string(),
            "missing-verification-step"
        );
        assert_eq!(Category::BrokenAnchorLink.to_string(), "broken-anchor-link");
        assert_eq!(Category::LongParagraph.to_string(), "long-paragraph");
        assert_eq!(
            Category::HardcodedWindowsPath.to_string(),
            "hardcoded-windows-path"
        );
        assert_eq!(Category::StaleFileTree.to_string(), "stale-file-tree");
        assert_eq!(
            Category::CommandValidation.to_string(),
            "command-validation"
        );
        assert_eq!(Category::TokenBudget.to_string(), "token-budget");
        assert_eq!(
            Category::InvalidSuppression.to_string(),
            "invalid-suppression"
        );
        assert_eq!(
            Category::UnusedSuppression.to_string(),
            "unused-suppression"
        );
        assert_eq!(
            Category::CustomPattern("todo".into()).to_string(),
            "custom:todo"
        );
    }

    #[test]
    fn test_category_as_str() {
        assert_eq!(Category::DeadReference.as_str(), "dead-reference");
        assert_eq!(Category::VagueDirective.as_str(), "vague-directive");
        assert_eq!(Category::TokenBudget.as_str(), "token-budget");
        assert_eq!(Category::CustomPattern("todo".into()).as_str(), "todo");
        // as_str() for built-in variants matches Display output
        assert_eq!(
            Category::DeadReference.as_str(),
            Category::DeadReference.to_string()
        );
        // as_str() for CustomPattern returns just the name (no "custom:" prefix)
        assert_ne!(
            Category::CustomPattern("todo".into()).as_str(),
            Category::CustomPattern("todo".into()).to_string()
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

    #[test]
    fn test_diagnostic_sort_by_file_then_line() {
        let mut diagnostics = [
            Diagnostic {
                file: Arc::new(PathBuf::from("b.md")),
                line: 10,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Error,
                category: Category::DeadReference,
                message: "msg1".to_string(),
                suggestion: None,
                fix: None,
            },
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 5,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Warning,
                category: Category::VagueDirective,
                message: "msg2".to_string(),
                suggestion: None,
                fix: None,
            },
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 1,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Info,
                category: Category::EnumDrift,
                message: "msg3".to_string(),
                suggestion: None,
                fix: None,
            },
        ];

        diagnostics.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

        assert_eq!(*diagnostics[0].file, PathBuf::from("a.md"));
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(*diagnostics[1].file, PathBuf::from("a.md"));
        assert_eq!(diagnostics[1].line, 5);
        assert_eq!(*diagnostics[2].file, PathBuf::from("b.md"));
        assert_eq!(diagnostics[2].line, 10);
    }

    #[test]
    fn test_structural_dedup() {
        let mut diagnostics = vec![
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 5,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Warning,
                category: Category::DeadReference,
                message: "first".to_string(),
                suggestion: None,
                fix: None,
            },
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 5,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Warning,
                category: Category::DeadReference,
                message: "first".to_string(),
                suggestion: None,
                fix: None,
            },
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 5,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Info,
                category: Category::VagueDirective,
                message: "different rule same line".to_string(),
                suggestion: None,
                fix: None,
            },
            Diagnostic {
                file: Arc::new(PathBuf::from("a.md")),
                line: 10,
                column: None,
                end_line: None,
                end_column: None,
                severity: Severity::Warning,
                category: Category::DeadReference,
                message: "different line".to_string(),
                suggestion: None,
                fix: None,
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

    #[test]
    fn test_category_fromstr_roundtrip() {
        // All built-in categories should round-trip through Display and FromStr
        let categories = [
            Category::DeadReference,
            Category::VagueDirective,
            Category::NamingInconsistency,
            Category::EnumDrift,
            Category::AgentGuidelines,
            Category::PlaceholderText,
            Category::FileSize,
            Category::CredentialExposure,
            Category::HeadingHierarchy,
            Category::DangerousCommand,
            Category::StaleReference,
            Category::EmojiDensity,
            Category::SessionJournal,
            Category::MissingEssentialSections,
            Category::PromptInjectionVector,
            Category::MissingVerification,
            Category::NegativeOnlyFraming,
            Category::ConflictingDirectives,
            Category::MissingRoleDefinition,
            Category::RedundantDirective,
            Category::InstructionDensity,
            Category::MissingExamples,
            Category::UnboundedScope,
            Category::CircularReference,
            Category::LargeCodeBlock,
            Category::DuplicateSection,
            Category::AbsolutePath,
            Category::GenericInstruction,
            Category::MisorderedSteps,
            Category::SectionLengthImbalance,
            Category::UnclosedFence,
            Category::UntaggedCodeBlock,
            Category::DuplicateInstructionFile,
            Category::OutdatedModelReference,
            Category::BrokenTable,
            Category::PlaceholderUrl,
            Category::EmphasisOveruse,
            Category::BoilerplateTemplate,
            Category::OrphanedSection,
            Category::ExcessiveNesting,
            Category::ContextWindowWaste,
            Category::AmbiguousScopeReference,
            Category::InstructionWithoutContext,
            Category::CrossFileContradiction,
            Category::StaleStyleRule,
            Category::HardcodedFileStructure,
            Category::UnversionedStackReference,
            Category::MissingStandardFile,
            Category::BareUrl,
            Category::RepeatedWord,
            Category::UndocumentedEnvVar,
            Category::EmptyCodeBlock,
            Category::ClickHereLink,
            Category::DoubleNegation,
            Category::ImperativeHeading,
            Category::InconsistentCommandPrefix,
            Category::EmptyHeading,
            Category::CopiedMetaInstructions,
            Category::XmlDocumentWrapper,
            Category::GeneratedAttribution,
            Category::CommandWithoutCodeblock,
            Category::MissingVerificationStep,
            Category::BrokenAnchorLink,
            Category::LongParagraph,
            Category::HardcodedWindowsPath,
            Category::StaleFileTree,
            Category::CommandValidation,
            Category::TokenBudget,
            Category::InvalidSuppression,
            Category::UnusedSuppression,
        ];

        for cat in &categories {
            let s = cat.to_string();
            let parsed: Category = s
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse '{}' back to Category: {}", s, e));
            assert_eq!(
                &parsed, cat,
                "Round-trip failed for '{s}': got {parsed:?}, expected {cat:?}"
            );
        }
    }

    #[test]
    fn test_category_fromstr_custom_pattern() {
        let parsed: Category = "custom:todo".parse().unwrap();
        assert_eq!(parsed, Category::CustomPattern("todo".into()));
        assert_eq!(parsed.to_string(), "custom:todo");
    }

    #[test]
    fn test_category_fromstr_unknown() {
        let result: std::result::Result<Category, _> = "not-a-real-rule".parse();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown category"));
    }

    #[test]
    fn test_category_serde_json_roundtrip() {
        // All built-in categories should survive JSON serialization
        let categories = [
            Category::DeadReference,
            Category::VagueDirective,
            Category::TokenBudget,
            Category::RepeatedWord,
            Category::CustomPattern("my-rule".into()),
        ];
        for cat in &categories {
            let json = serde_json::to_string(cat).unwrap();
            let parsed: Category = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, cat, "JSON roundtrip failed for {cat:?}");
        }
    }

    #[test]
    fn test_category_fromstr_empty_custom() {
        // "custom:" with empty name
        let parsed: Category = "custom:".parse().unwrap();
        assert_eq!(parsed, Category::CustomPattern("".into()));
    }

    #[test]
    fn test_severity_counts_single_pass() {
        let result = CheckResult {
            diagnostics: vec![
                make_diagnostic(Severity::Error),
                make_diagnostic(Severity::Warning),
                make_diagnostic(Severity::Warning),
                make_diagnostic(Severity::Info),
            ],
        };
        let (e, w, i) = result.severity_counts();
        assert_eq!(e, 1);
        assert_eq!(w, 2);
        assert_eq!(i, 1);
    }

    #[test]
    fn test_severity_counts_empty() {
        let result = CheckResult::default();
        assert_eq!(result.severity_counts(), (0, 0, 0));
    }

    #[test]
    fn test_diagnostic_json_skips_none_fields() {
        let d = Diagnostic {
            file: Arc::new(PathBuf::from("test.md")),
            line: 1,
            column: None,
            end_line: None,
            end_column: None,
            severity: Severity::Info,
            category: Category::DeadReference,
            message: "test".to_string(),
            suggestion: None,
            fix: None,
        };
        let json = serde_json::to_value(&d).unwrap();
        assert!(
            !json.as_object().unwrap().contains_key("column"),
            "None column should be skipped"
        );
        assert!(
            !json.as_object().unwrap().contains_key("end_line"),
            "None end_line should be skipped"
        );
        assert!(
            !json.as_object().unwrap().contains_key("suggestion"),
            "None suggestion should be skipped"
        );
        assert!(
            !json.as_object().unwrap().contains_key("fix"),
            "None fix should be skipped"
        );
    }

    #[test]
    fn test_diagnostic_json_includes_present_fields() {
        use crate::types::{Fix, Replacement};
        let d = Diagnostic {
            file: Arc::new(PathBuf::from("test.md")),
            line: 5,
            column: Some(10),
            end_line: Some(5),
            end_column: Some(20),
            severity: Severity::Warning,
            category: Category::RepeatedWord,
            message: "dup word".to_string(),
            suggestion: Some("remove it".to_string()),
            fix: Some(Box::new(Fix {
                description: "auto-fix".to_string(),
                replacements: vec![Replacement {
                    line: 5,
                    start_col: 10,
                    end_col: 14,
                    new_text: String::new(),
                }],
            })),
        };
        let json = serde_json::to_value(&d).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj["column"], 10);
        assert_eq!(obj["end_line"], 5);
        assert_eq!(obj["end_column"], 20);
        assert_eq!(obj["suggestion"], "remove it");
        assert!(obj.contains_key("fix"));
        assert_eq!(obj["fix"]["replacements"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_diagnostic_file_serialized_as_string() {
        let d = make_diagnostic(Severity::Error);
        let json = serde_json::to_value(&d).unwrap();
        assert!(
            json["file"].is_string(),
            "Arc<PathBuf> should serialize as string"
        );
        assert_eq!(json["file"].as_str().unwrap(), "test.md");
    }

    #[test]
    fn test_fix_equality() {
        use crate::types::{Fix, Replacement};
        let f1 = Fix {
            description: "fix".to_string(),
            replacements: vec![Replacement {
                line: 1,
                start_col: 0,
                end_col: 5,
                new_text: "x".to_string(),
            }],
        };
        let f2 = f1.clone();
        assert_eq!(f1, f2);
    }

    #[test]
    fn test_check_result_default_empty() {
        let r = CheckResult::default();
        assert!(r.diagnostics.is_empty());
        assert_eq!(r.error_count(), 0);
        assert_eq!(r.warning_count(), 0);
        assert_eq!(r.info_count(), 0);
    }
}
