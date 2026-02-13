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
    pub include: Vec<String>,
    pub ignore: Vec<String>,
    pub ignore_files: Vec<String>,
    pub historical_files: Vec<String>,
    pub checkers: CheckersConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CheckersConfig {
    pub dead_reference: ScopedCheckerConfig,
    pub vague_directive: VagueDirectiveConfig,
    pub naming_inconsistency: ScopedCheckerConfig,
    pub enum_drift: ScopedCheckerConfig,
    pub custom_patterns: Vec<CustomPattern>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct VagueDirectiveConfig {
    pub enabled: bool,
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

[checkers.dead_reference]
enabled = true

[checkers.vague_directive]
enabled = true
# extra_patterns = ["(?i)\\bmaybe\\b", "(?i)\\bprobably\\b"]
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

[checkers.naming_inconsistency]
enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

[checkers.enum_drift]
enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

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
        assert!(config.checkers.enum_drift.enabled);
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
