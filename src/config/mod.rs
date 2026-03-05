use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use crate::cli::OutputFormat;
use crate::types::Severity;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub format: OutputFormat,
    pub strict: bool,
    pub include: Vec<String>,
    pub ignore: Vec<String>,
    pub ignore_files: Vec<String>,
    pub historical_files: Vec<String>,
    pub checkers: CheckersConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CheckersConfig {
    pub dead_reference: ScopedCheckerConfig,
    pub vague_directive: VagueDirectiveConfig,
    pub naming_inconsistency: ScopedCheckerConfig,
    pub enum_drift: ScopedCheckerConfig,
    pub agent_guidelines: ScopedCheckerConfig,
    pub placeholder_text: ScopedCheckerConfig,
    pub file_size: FileSizeConfig,
    pub credential_exposure: ScopedCheckerConfig,
    pub heading_hierarchy: ScopedCheckerConfig,
    pub dangerous_command: ScopedCheckerConfig,
    pub stale_reference: ScopedCheckerConfig,
    pub emoji_density: EmojiDensityConfig,
    pub session_journal: ScopedCheckerConfig,
    pub missing_essential_sections: MissingEssentialSectionsConfig,
    pub prompt_injection_vector: ScopedCheckerConfig,
    pub missing_verification: MissingVerificationConfig,
    pub negative_only_framing: NegativeOnlyFramingConfig,
    pub conflicting_directives: ScopedCheckerConfig,
    pub missing_role_definition: ScopedCheckerConfig,
    pub redundant_directive: RedundantDirectiveConfig,
    pub instruction_density: InstructionDensityConfig,
    pub missing_examples: ScopedCheckerConfig,
    pub unbounded_scope: ScopedCheckerConfig,
    pub circular_reference: ScopedCheckerConfig,
    pub large_code_block: LargeCodeBlockConfig,
    pub duplicate_section: ScopedCheckerConfig,
    pub absolute_path: ScopedCheckerConfig,
    pub generic_instruction: ScopedCheckerConfig,
    pub misordered_steps: ScopedCheckerConfig,
    pub section_length_imbalance: SectionLengthImbalanceConfig,
    pub unclosed_fence: ScopedCheckerConfig,
    pub untagged_code_block: ScopedCheckerConfig,
    pub duplicate_instruction_file: ScopedCheckerConfig,
    pub outdated_model_reference: ScopedCheckerConfig,
    pub broken_anchor_link: ScopedCheckerConfig,
    pub broken_table: ScopedCheckerConfig,
    pub placeholder_url: ScopedCheckerConfig,
    pub emphasis_overuse: EmphasisOveruseConfig,
    pub boilerplate_template: ScopedCheckerConfig,
    pub generated_attribution: ScopedCheckerConfig,
    pub orphaned_section: ScopedCheckerConfig,
    pub excessive_nesting: ExcessiveNestingConfig,
    pub context_window_waste: ScopedCheckerConfig,
    pub ambiguous_scope_reference: ScopedCheckerConfig,
    pub instruction_without_context: ScopedCheckerConfig,
    pub cross_file_contradiction: ScopedCheckerConfig,
    pub stale_style_rule: ScopedCheckerConfig,
    pub hardcoded_windows_path: ScopedCheckerConfig,
    pub hardcoded_file_structure: ScopedCheckerConfig,
    pub unversioned_stack_reference: ScopedCheckerConfig,
    pub missing_standard_file: ScopedCheckerConfig,
    pub bare_url: ScopedCheckerConfig,
    pub repeated_word: ScopedCheckerConfig,
    pub undocumented_env_var: ScopedCheckerConfig,
    pub empty_code_block: ScopedCheckerConfig,
    pub click_here_link: ScopedCheckerConfig,
    pub double_negation: ScopedCheckerConfig,
    pub imperative_heading: ScopedCheckerConfig,
    pub inconsistent_command_prefix: ScopedCheckerConfig,
    pub empty_heading: ScopedCheckerConfig,
    pub copied_meta_instructions: ScopedCheckerConfig,
    pub long_paragraph: LongParagraphConfig,
    pub command_without_codeblock: ScopedCheckerConfig,
    pub missing_verification_step: ScopedCheckerConfig,
    pub xml_document_wrapper: ScopedCheckerConfig,
    pub custom_patterns: Vec<CustomPattern>,
}

impl Default for CheckersConfig {
    fn default() -> Self {
        Self {
            dead_reference: ScopedCheckerConfig::default(),
            vague_directive: VagueDirectiveConfig::default(),
            naming_inconsistency: ScopedCheckerConfig::default(),
            // Strict-only checkers: disabled by default, enabled by strict = true
            enum_drift: ScopedCheckerConfig::disabled(),
            agent_guidelines: ScopedCheckerConfig::disabled(),
            placeholder_text: ScopedCheckerConfig::default(),
            file_size: FileSizeConfig::default(),
            credential_exposure: ScopedCheckerConfig::default(),
            heading_hierarchy: ScopedCheckerConfig::disabled(),
            dangerous_command: ScopedCheckerConfig::default(),
            stale_reference: ScopedCheckerConfig::default(),
            emoji_density: EmojiDensityConfig::default(),
            session_journal: ScopedCheckerConfig::default(),
            missing_essential_sections: MissingEssentialSectionsConfig::default(),
            prompt_injection_vector: ScopedCheckerConfig::default(),
            missing_verification: MissingVerificationConfig::default(),
            negative_only_framing: NegativeOnlyFramingConfig::default(),
            conflicting_directives: ScopedCheckerConfig::default(),
            missing_role_definition: ScopedCheckerConfig::disabled(),
            redundant_directive: RedundantDirectiveConfig::default(),
            instruction_density: InstructionDensityConfig::default(),
            missing_examples: ScopedCheckerConfig::disabled(),
            unbounded_scope: ScopedCheckerConfig::disabled(),
            circular_reference: ScopedCheckerConfig::default(),
            large_code_block: LargeCodeBlockConfig::default(),
            duplicate_section: ScopedCheckerConfig::default(),
            absolute_path: ScopedCheckerConfig::default(),
            generic_instruction: ScopedCheckerConfig::default(),
            misordered_steps: ScopedCheckerConfig::default(),
            section_length_imbalance: SectionLengthImbalanceConfig::default(),
            unclosed_fence: ScopedCheckerConfig::default(),
            untagged_code_block: ScopedCheckerConfig::disabled(),
            duplicate_instruction_file: ScopedCheckerConfig::default(),
            outdated_model_reference: ScopedCheckerConfig::default(),
            broken_anchor_link: ScopedCheckerConfig::default(),
            broken_table: ScopedCheckerConfig::default(),
            placeholder_url: ScopedCheckerConfig::default(),
            emphasis_overuse: EmphasisOveruseConfig::default(),
            boilerplate_template: ScopedCheckerConfig::default(),
            generated_attribution: ScopedCheckerConfig::default(),
            orphaned_section: ScopedCheckerConfig::default(),
            excessive_nesting: ExcessiveNestingConfig::default(),
            context_window_waste: ScopedCheckerConfig::default(),
            ambiguous_scope_reference: ScopedCheckerConfig::default(),
            instruction_without_context: ScopedCheckerConfig::default(),
            cross_file_contradiction: ScopedCheckerConfig::disabled(),
            stale_style_rule: ScopedCheckerConfig::default(),
            hardcoded_windows_path: ScopedCheckerConfig::default(),
            hardcoded_file_structure: ScopedCheckerConfig::default(),
            unversioned_stack_reference: ScopedCheckerConfig::disabled(),
            missing_standard_file: ScopedCheckerConfig::disabled(),
            bare_url: ScopedCheckerConfig::disabled(),
            repeated_word: ScopedCheckerConfig::disabled(),
            undocumented_env_var: ScopedCheckerConfig::disabled(),
            empty_code_block: ScopedCheckerConfig::disabled(),
            click_here_link: ScopedCheckerConfig::disabled(),
            double_negation: ScopedCheckerConfig::disabled(),
            imperative_heading: ScopedCheckerConfig::disabled(),
            inconsistent_command_prefix: ScopedCheckerConfig::disabled(),
            empty_heading: ScopedCheckerConfig::disabled(),
            copied_meta_instructions: ScopedCheckerConfig::disabled(),
            long_paragraph: LongParagraphConfig::default(),
            command_without_codeblock: ScopedCheckerConfig::disabled(),
            missing_verification_step: ScopedCheckerConfig::disabled(),
            xml_document_wrapper: ScopedCheckerConfig::disabled(),
            custom_patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EmojiDensityConfig {
    pub enabled: bool,
    pub max_emoji: usize,
    pub severity: Option<Severity>,
}

impl Default for EmojiDensityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_emoji: 20,
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MissingEssentialSectionsConfig {
    pub enabled: bool,
    pub min_lines: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for MissingEssentialSectionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_lines: 10,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MissingVerificationConfig {
    pub enabled: bool,
    pub min_action_verbs: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for MissingVerificationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_action_verbs: 4,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NegativeOnlyFramingConfig {
    pub enabled: bool,
    pub threshold: f64,
    pub min_negative_count: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for NegativeOnlyFramingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.65,
            min_negative_count: 3,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RedundantDirectiveConfig {
    pub enabled: bool,
    pub similarity_threshold: f64,
    pub min_line_length: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for RedundantDirectiveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            similarity_threshold: 0.95,
            min_line_length: 15,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct InstructionDensityConfig {
    pub enabled: bool,
    pub max_consecutive_bullets: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for InstructionDensityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_consecutive_bullets: 15,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LargeCodeBlockConfig {
    pub enabled: bool,
    pub max_lines: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for LargeCodeBlockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 40,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SectionLengthImbalanceConfig {
    pub enabled: bool,
    pub min_section_lines: usize,
    pub imbalance_ratio: f64,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for SectionLengthImbalanceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_section_lines: 50,
            imbalance_ratio: 4.0,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EmphasisOveruseConfig {
    pub enabled: bool,
    pub max_emphasis: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for EmphasisOveruseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_emphasis: 10,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LongParagraphConfig {
    pub enabled: bool,
    pub max_lines: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for LongParagraphConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_lines: 8,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ExcessiveNestingConfig {
    pub enabled: bool,
    pub max_depth: usize,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

impl Default for ExcessiveNestingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_depth: 4,
            scope: Vec::new(),
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FileSizeConfig {
    pub enabled: bool,
    pub max_lines: usize,
    pub warn_lines: usize,
    pub severity: Option<Severity>,
}

impl Default for FileSizeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 750,
            warn_lines: 500,
            severity: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct VagueDirectiveConfig {
    pub enabled: bool,
    pub strict: bool,
    pub extra_patterns: Vec<String>,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ScopedCheckerConfig {
    pub enabled: bool,
    pub scope: Vec<String>,
    pub severity: Option<Severity>,
}

fn default_severity() -> Severity {
    Severity::Warning
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomPattern {
    pub name: String,
    pub pattern: String,
    #[serde(default = "default_severity")]
    pub severity: Severity,
    pub message: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            format: OutputFormat::Text,
            strict: false,
            include: vec![
                "CLAUDE.md".into(),
                "AGENTS.md".into(),
                ".claude/**".into(),
                ".github/copilot-instructions.md".into(),
            ],
            ignore: vec!["node_modules".into(), ".git".into(), "target".into()],
            ignore_files: Vec::new(),
            historical_files: vec![
                "changelog*".into(),
                "retro*".into(),
                "history*".into(),
                "archive*".into(),
                "restart*".into(),
            ],
            checkers: CheckersConfig::default(),
        }
    }
}

impl Default for VagueDirectiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strict: false,
            extra_patterns: Vec::new(),
            scope: Vec::new(),
            severity: None,
        }
    }
}

impl ScopedCheckerConfig {
    /// A disabled checker (strict-only checks that are off by default).
    fn disabled() -> Self {
        Self {
            enabled: false,
            scope: Vec::new(),
            severity: None,
        }
    }
}

impl Default for ScopedCheckerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scope: Vec::new(),
            severity: None,
        }
    }
}

impl Config {
    pub fn load(config_path: Option<&Path>, project_root: &Path) -> Result<Self> {
        // When an explicit --config path is given, any read error is fatal.
        if let Some(path) = config_path {
            let content = std::fs::read_to_string(path)?;
            return toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Config parse error: {e}"));
        }

        // Auto-discover: try to read the default path directly (avoids TOCTOU
        // race between exists() and read_to_string()).
        let default = project_root.join(".spectralintrc.toml");
        match std::fs::read_to_string(&default) {
            Ok(content) => {
                toml::from_str(&content).map_err(|e| anyhow::anyhow!("Config parse error: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub const fn default_toml() -> &'static str {
        r#"# spectralint configuration

# Output format: text, json, github, sarif
# format = "text"

# Which files to scan (glob patterns, case-insensitive).
# Default: known AI instruction file patterns.
# Set to ["**/*.md"] to scan all markdown files.
include = ["CLAUDE.md", "AGENTS.md", ".claude/**", ".github/copilot-instructions.md"]

# Directories to ignore when scanning
ignore = ["node_modules", ".git", "target"]

# Individual files to skip entirely (supports glob patterns)
# ignore_files = ["changelog.md", "docs/history.md"]

# Files treated as historical (dead refs and enum drift are skipped)
# Patterns are matched case-insensitively.
# historical_files = ["changelog*", "retro*", "history*", "archive*", "restart*"]

# Strict mode enables opinionated checks that go beyond documented best practices.
# These checks are off by default because they enforce opinions rather than catch bugs.
# strict = true

# Per-checker severity override example:
# [checkers.vague_directive]
# severity = "error"

[checkers.dead_reference]
enabled = true

[checkers.vague_directive]
enabled = true
# strict = true  # also flag "when possible", "when needed", "as needed", "consider"
# extra_patterns = ["(?i)\\bmaybe\\b", "(?i)\\bprobably\\b"]
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

[checkers.naming_inconsistency]
enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

[checkers.placeholder_text]
enabled = true

[checkers.file_size]
enabled = true
max_lines = 750
warn_lines = 500

[checkers.credential_exposure]
enabled = true

[checkers.dangerous_command]
enabled = true

[checkers.stale_reference]
enabled = true

[checkers.session_journal]
enabled = true

[checkers.missing_essential_sections]
enabled = true
min_lines = 10

[checkers.prompt_injection_vector]
enabled = true

# ── Strict-only checks (disabled by default, enabled by strict = true) ──
# These are opinionated or noisy checks disabled by default.

# [checkers.enum_drift]
# enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

# [checkers.agent_guidelines]
# enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

# [checkers.heading_hierarchy]
# enabled = true

# [checkers.emoji_density]
# enabled = true
# max_emoji = 20

# [checkers.missing_verification]
# enabled = true
# min_action_verbs = 4

# [checkers.negative_only_framing]
# enabled = true
# threshold = 0.65
# min_negative_count = 3

# ── Prompt-quality checks ──

[checkers.conflicting_directives]
enabled = true

# [checkers.missing_role_definition]
# enabled = true

# [checkers.redundant_directive]
# enabled = true
# similarity_threshold = 0.95
# min_line_length = 15

# [checkers.instruction_density]
# enabled = true
# max_consecutive_bullets = 15

# [checkers.missing_examples]
# enabled = true

# [checkers.unbounded_scope]
# enabled = true

[checkers.circular_reference]
enabled = true

[checkers.large_code_block]
enabled = true
max_lines = 40

[checkers.duplicate_section]
enabled = true

[checkers.absolute_path]
enabled = true

[checkers.misordered_steps]
enabled = true

# ── New strict-only checks ──

[checkers.generic_instruction]
enabled = true

# [checkers.section_length_imbalance]
# enabled = true
# min_section_lines = 50
# imbalance_ratio = 4.0

[checkers.unclosed_fence]
enabled = true

[checkers.broken_table]
enabled = true

[checkers.duplicate_instruction_file]
enabled = true

[checkers.outdated_model_reference]
enabled = true

[checkers.placeholder_url]
enabled = true

[checkers.boilerplate_template]
enabled = true

# [checkers.emphasis_overuse]
# enabled = true
# max_emphasis = 10

[checkers.orphaned_section]
enabled = true

# [checkers.excessive_nesting]
# enabled = true
# max_depth = 4

[checkers.context_window_waste]
enabled = true

[checkers.ambiguous_scope_reference]
enabled = true

[checkers.instruction_without_context]
enabled = true

# [checkers.cross_file_contradiction]
# enabled = true

# [checkers.untagged_code_block]
# enabled = true

[checkers.stale_style_rule]
enabled = true

[checkers.hardcoded_file_structure]
enabled = true

# [checkers.unversioned_stack_reference]
# enabled = true

# [checkers.bare_url]
# enabled = true

# [checkers.repeated_word]
# enabled = true

# [checkers.undocumented_env_var]
# enabled = true

# [checkers.empty_code_block]
# enabled = true

# [checkers.click_here_link]
# enabled = true

# [checkers.double_negation]
# enabled = true

# [checkers.imperative_heading]
# enabled = true

# [checkers.inconsistent_command_prefix]
# enabled = true

# [checkers.empty_heading]
# enabled = true

# [checkers.copied_meta_instructions]
# enabled = true

# [checkers.xml_document_wrapper]
# enabled = true

# Custom regex patterns:
# [[checkers.custom_patterns]]
# name = "todo-comment"
# pattern = "(?i)\\bTODO\\b"
# severity = "warning"
# message = "TODO comment found"
"#
    }

    pub const fn minimal_toml() -> &'static str {
        r#"# spectralint configuration — minimal preset
# Only the most critical checkers are enabled.

include = ["CLAUDE.md", "AGENTS.md", ".claude/**", ".github/copilot-instructions.md"]
ignore = ["node_modules", ".git", "target"]

[checkers.dead_reference]
enabled = true

[checkers.credential_exposure]
enabled = true
"#
    }

    pub const fn strict_toml() -> &'static str {
        r#"# spectralint configuration — strict preset
# All checkers enabled, including opinionated ones.

include = ["CLAUDE.md", "AGENTS.md", ".claude/**", ".github/copilot-instructions.md"]
ignore = ["node_modules", ".git", "target"]
strict = true
"#
    }

    /// Look up the per-checker severity override for a given category.
    pub fn severity_override(&self, category: &crate::types::Category) -> Option<Severity> {
        use crate::types::Category;
        match category {
            Category::DeadReference => self.checkers.dead_reference.severity,
            Category::VagueDirective => self.checkers.vague_directive.severity,
            Category::NamingInconsistency => self.checkers.naming_inconsistency.severity,
            Category::EnumDrift => self.checkers.enum_drift.severity,
            Category::AgentGuidelines => self.checkers.agent_guidelines.severity,
            Category::PlaceholderText => self.checkers.placeholder_text.severity,
            Category::FileSize => self.checkers.file_size.severity,
            Category::CredentialExposure => self.checkers.credential_exposure.severity,
            Category::HeadingHierarchy => self.checkers.heading_hierarchy.severity,
            Category::DangerousCommand => self.checkers.dangerous_command.severity,
            Category::StaleReference => self.checkers.stale_reference.severity,
            Category::EmojiDensity => self.checkers.emoji_density.severity,
            Category::SessionJournal => self.checkers.session_journal.severity,
            Category::MissingEssentialSections => self.checkers.missing_essential_sections.severity,
            Category::PromptInjectionVector => self.checkers.prompt_injection_vector.severity,
            Category::MissingVerification => self.checkers.missing_verification.severity,
            Category::NegativeOnlyFraming => self.checkers.negative_only_framing.severity,
            Category::ConflictingDirectives => self.checkers.conflicting_directives.severity,
            Category::MissingRoleDefinition => self.checkers.missing_role_definition.severity,
            Category::RedundantDirective => self.checkers.redundant_directive.severity,
            Category::InstructionDensity => self.checkers.instruction_density.severity,
            Category::MissingExamples => self.checkers.missing_examples.severity,
            Category::UnboundedScope => self.checkers.unbounded_scope.severity,
            Category::CircularReference => self.checkers.circular_reference.severity,
            Category::LargeCodeBlock => self.checkers.large_code_block.severity,
            Category::DuplicateSection => self.checkers.duplicate_section.severity,
            Category::AbsolutePath => self.checkers.absolute_path.severity,
            Category::GenericInstruction => self.checkers.generic_instruction.severity,
            Category::MisorderedSteps => self.checkers.misordered_steps.severity,
            Category::SectionLengthImbalance => self.checkers.section_length_imbalance.severity,
            Category::UnclosedFence => self.checkers.unclosed_fence.severity,
            Category::UntaggedCodeBlock => self.checkers.untagged_code_block.severity,
            Category::DuplicateInstructionFile => self.checkers.duplicate_instruction_file.severity,
            Category::OutdatedModelReference => self.checkers.outdated_model_reference.severity,
            Category::BrokenTable => self.checkers.broken_table.severity,
            Category::PlaceholderUrl => self.checkers.placeholder_url.severity,
            Category::EmphasisOveruse => self.checkers.emphasis_overuse.severity,
            Category::BoilerplateTemplate => self.checkers.boilerplate_template.severity,
            Category::GeneratedAttribution => self.checkers.generated_attribution.severity,
            Category::OrphanedSection => self.checkers.orphaned_section.severity,
            Category::ExcessiveNesting => self.checkers.excessive_nesting.severity,
            Category::ContextWindowWaste => self.checkers.context_window_waste.severity,
            Category::AmbiguousScopeReference => self.checkers.ambiguous_scope_reference.severity,
            Category::InstructionWithoutContext => {
                self.checkers.instruction_without_context.severity
            }
            Category::CrossFileContradiction => self.checkers.cross_file_contradiction.severity,
            Category::StaleStyleRule => self.checkers.stale_style_rule.severity,
            Category::HardcodedWindowsPath => self.checkers.hardcoded_windows_path.severity,
            Category::HardcodedFileStructure => self.checkers.hardcoded_file_structure.severity,
            Category::UnversionedStackReference => {
                self.checkers.unversioned_stack_reference.severity
            }
            Category::MissingStandardFile => self.checkers.missing_standard_file.severity,
            Category::BareUrl => self.checkers.bare_url.severity,
            Category::RepeatedWord => self.checkers.repeated_word.severity,
            Category::UndocumentedEnvVar => self.checkers.undocumented_env_var.severity,
            Category::EmptyCodeBlock => self.checkers.empty_code_block.severity,
            Category::ClickHereLink => self.checkers.click_here_link.severity,
            Category::DoubleNegation => self.checkers.double_negation.severity,
            Category::ImperativeHeading => self.checkers.imperative_heading.severity,
            Category::InconsistentCommandPrefix => {
                self.checkers.inconsistent_command_prefix.severity
            }
            Category::EmptyHeading => self.checkers.empty_heading.severity,
            Category::CopiedMetaInstructions => self.checkers.copied_meta_instructions.severity,
            Category::CommandWithoutCodeblock => self.checkers.command_without_codeblock.severity,
            Category::MissingVerificationStep => self.checkers.missing_verification_step.severity,
            Category::BrokenAnchorLink => self.checkers.broken_anchor_link.severity,
            Category::LongParagraph => self.checkers.long_paragraph.severity,
            Category::XmlDocumentWrapper => self.checkers.xml_document_wrapper.severity,
            Category::InvalidSuppression | Category::UnusedSuppression => None,
            Category::CustomPattern(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.checkers.dead_reference.enabled);
        assert!(config.checkers.vague_directive.enabled);
        assert!(config.checkers.vague_directive.extra_patterns.is_empty());
        assert!(config.checkers.vague_directive.scope.is_empty());
        assert!(config.checkers.naming_inconsistency.enabled);
        assert!(config.checkers.naming_inconsistency.scope.is_empty());
        assert!(
            !config.checkers.enum_drift.enabled,
            "enum_drift should be disabled by default (strict-only)"
        );
        assert!(config.checkers.enum_drift.scope.is_empty());
        assert_eq!(config.ignore.len(), 3);
        assert_eq!(config.include.len(), 4);
        assert!(config.include.contains(&"CLAUDE.md".to_string()));
        assert!(config.include.contains(&"AGENTS.md".to_string()));
    }

    #[test]
    fn test_parse_toml() {
        let toml_str = r#"
ignore = [".git"]

[checkers.dead_reference]
enabled = false

[checkers.vague_directive]
enabled = true
extra_patterns = ["(?i)\\bmaybe\\b"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.checkers.dead_reference.enabled);
        assert!(config.checkers.vague_directive.enabled);
        assert_eq!(
            config.checkers.vague_directive.extra_patterns,
            vec![r"(?i)\bmaybe\b"]
        );
        assert_eq!(config.ignore, vec![".git"]);
    }

    #[test]
    fn test_parse_custom_patterns() {
        let toml_str = r#"
[[checkers.custom_patterns]]
name = "todo-comment"
pattern = "(?i)\\bTODO\\b"
severity = "warning"
message = "TODO comment found"

[[checkers.custom_patterns]]
name = "fixme"
pattern = "(?i)\\bFIXME\\b"
severity = "error"
message = "FIXME found"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.checkers.custom_patterns.len(), 2);
        assert_eq!(config.checkers.custom_patterns[0].name, "todo-comment");
        assert_eq!(config.checkers.custom_patterns[1].name, "fixme");
    }

    #[test]
    fn test_parse_scope_on_enum_drift() {
        let toml_str = r#"
[checkers.enum_drift]
enabled = true
scope = ["CLAUDE.md", "AGENTS.md"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.checkers.enum_drift.enabled);
        assert_eq!(
            config.checkers.enum_drift.scope,
            vec!["CLAUDE.md", "AGENTS.md"]
        );
    }

    #[test]
    fn test_parse_scope_on_naming_inconsistency() {
        let toml_str = r#"
[checkers.naming_inconsistency]
enabled = true
scope = [".claude/**"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.checkers.naming_inconsistency.enabled);
        assert_eq!(
            config.checkers.naming_inconsistency.scope,
            vec![".claude/**"]
        );
    }

    #[test]
    fn test_parse_strict_vague_directive() {
        let toml_str = r#"
[checkers.vague_directive]
enabled = true
strict = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.checkers.vague_directive.strict);
    }

    #[test]
    fn test_default_strict_is_false() {
        let config = Config::default();
        assert!(!config.checkers.vague_directive.strict);
    }

    #[test]
    fn test_parse_scope_on_vague_directive() {
        let toml_str = r#"
[checkers.vague_directive]
enabled = true
scope = ["CLAUDE.md"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.checkers.vague_directive.enabled);
        assert_eq!(config.checkers.vague_directive.scope, vec!["CLAUDE.md"]);
    }

    #[test]
    fn test_empty_scope_defaults() {
        let toml_str = r#"
[checkers.enum_drift]
enabled = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.checkers.enum_drift.scope.is_empty());
    }

    #[test]
    fn test_parse_include() {
        let toml_str = r#"
include = ["**/*.md"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.include, vec!["**/*.md"]);
    }

    #[test]
    fn test_omitted_include_gets_defaults() {
        let toml_str = r#"
[checkers.dead_reference]
enabled = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.include, Config::default().include);
        assert_eq!(config.include.len(), 4);
    }

    #[test]
    fn test_config_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".spectralintrc.toml");
        std::fs::write(&path, "invalid toml [[[").unwrap();

        let result = Config::load(Some(&path), dir.path());
        assert!(result.is_err(), "Invalid TOML should return Err");
        assert!(
            result.unwrap_err().to_string().contains("parse error"),
            "Error should mention parse error"
        );
    }

    #[test]
    fn test_config_load_nonexistent_file() {
        let result = Config::load(
            Some(Path::new("/nonexistent/config.toml")),
            Path::new("/tmp"),
        );
        assert!(
            result.is_err(),
            "Non-existent config path should return Err"
        );
    }

    #[test]
    fn test_config_load_no_config_uses_default() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(None, dir.path()).unwrap();
        assert_eq!(config.include, Config::default().include);
        assert!(config.checkers.dead_reference.enabled);
    }

    #[test]
    fn test_default_toml_template_is_parseable() {
        let _config: Config = toml::from_str(Config::default_toml()).unwrap();
    }

    #[test]
    fn test_custom_pattern_default_severity() {
        let toml_str = r#"
[[checkers.custom_patterns]]
name = "todo"
pattern = "TODO"
message = "TODO found"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.checkers.custom_patterns[0].severity,
            Severity::Warning,
            "Default severity for custom patterns should be Warning"
        );
    }

    #[test]
    fn test_empty_custom_patterns_array() {
        let config: Config = toml::from_str("").unwrap();
        assert!(
            config.checkers.custom_patterns.is_empty(),
            "Default config should have no custom patterns"
        );
    }
}
