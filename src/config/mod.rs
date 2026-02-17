use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use crate::cli::OutputFormat;
use crate::types::Severity;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(skip)]
    pub format: OutputFormat,
    pub strict: bool,
    pub include: Vec<String>,
    pub ignore: Vec<String>,
    pub ignore_files: Vec<String>,
    pub historical_files: Vec<String>,
    pub checkers: CheckersConfig,
}

#[derive(Debug, Deserialize)]
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
    pub custom_patterns: Vec<CustomPattern>,
}

impl Default for CheckersConfig {
    fn default() -> Self {
        Self {
            dead_reference: ScopedCheckerConfig::default(),
            vague_directive: VagueDirectiveConfig::default(),
            naming_inconsistency: ScopedCheckerConfig::default(),
            // Strict-only checkers: disabled by default, enabled by strict = true
            enum_drift: ScopedCheckerConfig {
                enabled: false,
                ..Default::default()
            },
            agent_guidelines: ScopedCheckerConfig {
                enabled: false,
                ..Default::default()
            },
            placeholder_text: ScopedCheckerConfig::default(),
            file_size: FileSizeConfig::default(),
            credential_exposure: ScopedCheckerConfig::default(),
            heading_hierarchy: ScopedCheckerConfig {
                enabled: false,
                ..Default::default()
            },
            dangerous_command: ScopedCheckerConfig::default(),
            stale_reference: ScopedCheckerConfig::default(),
            emoji_density: EmojiDensityConfig::default(),
            session_journal: ScopedCheckerConfig::default(),
            missing_essential_sections: MissingEssentialSectionsConfig::default(),
            prompt_injection_vector: ScopedCheckerConfig::default(),
            missing_verification: MissingVerificationConfig::default(),
            negative_only_framing: NegativeOnlyFramingConfig::default(),
            custom_patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct EmojiDensityConfig {
    pub enabled: bool,
    pub max_emoji: usize,
}

impl Default for EmojiDensityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_emoji: 10,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct MissingEssentialSectionsConfig {
    pub enabled: bool,
    pub min_lines: usize,
    pub scope: Vec<String>,
}

impl Default for MissingEssentialSectionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_lines: 10,
            scope: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct MissingVerificationConfig {
    pub enabled: bool,
    pub min_action_verbs: usize,
    pub scope: Vec<String>,
}

impl Default for MissingVerificationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_action_verbs: 2,
            scope: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct NegativeOnlyFramingConfig {
    pub enabled: bool,
    pub threshold: f64,
    pub min_negative_count: usize,
    pub scope: Vec<String>,
}

impl Default for NegativeOnlyFramingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.75,
            min_negative_count: 5,
            scope: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct FileSizeConfig {
    pub enabled: bool,
    pub max_lines: usize,
    pub warn_lines: usize,
}

impl Default for FileSizeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 500,
            warn_lines: 300,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct VagueDirectiveConfig {
    pub enabled: bool,
    pub strict: bool,
    pub extra_patterns: Vec<String>,
    pub scope: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ScopedCheckerConfig {
    pub enabled: bool,
    pub scope: Vec<String>,
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
        }
    }
}

impl Default for ScopedCheckerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scope: Vec::new(),
        }
    }
}

impl Config {
    pub fn load(config_path: Option<&Path>, project_root: &Path) -> Result<Self> {
        let path = config_path.map(Path::to_path_buf).or_else(|| {
            let default = project_root.join(".spectralintrc.toml");
            default.exists().then_some(default)
        });

        match path {
            Some(path) => {
                let content = std::fs::read_to_string(&path)?;
                toml::from_str(&content).map_err(|e| anyhow::anyhow!("Config parse error: {e}"))
            }
            None => Ok(Config::default()),
        }
    }

    pub const fn default_toml() -> &'static str {
        r#"# spectralint configuration

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
max_lines = 500
warn_lines = 300

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
# max_emoji = 10

# [checkers.missing_verification]
# enabled = true
# min_action_verbs = 2

# [checkers.negative_only_framing]
# enabled = true
# threshold = 0.75
# min_negative_count = 5

# Custom regex patterns:
# [[checkers.custom_patterns]]
# name = "todo-comment"
# pattern = "(?i)\\bTODO\\b"
# severity = "warning"
# message = "TODO comment found"
"#
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

    // ── Item 2: Config loading with malformed TOML ───────────────────────

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

    // ── Item 24: Default template validity ───────────────────────────────

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
