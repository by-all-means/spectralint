use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::config::OutdatedModelReferenceConfig;
use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::engine::date;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::model_catalog::{ModelCatalog, ModelStatus};
use super::utils::{is_heading, ScopeFilter};
use super::Checker;

/// Lines that document a model's age themselves are not flagged.
static EXCLUSION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:histor(?:y|ical(?:ly)?)|changelog|deprecated|retired|formerly|previously|legacy|(?:migrat|upgrad)(?:ed|ing) from)\b",
    )
    .unwrap()
});

/// Where a model name was found; drives severity.
#[derive(Clone, Copy)]
enum Location {
    /// Free text: a stale name is informational.
    Prose,
    /// A field the runtime reads (`model:` frontmatter, settings.json): a
    /// retired model here fails at request time.
    Config,
}

fn severity(location: Location, status: ModelStatus) -> Severity {
    match (location, status) {
        (Location::Config, ModelStatus::Retired | ModelStatus::Deprecated) => Severity::Warning,
        _ => Severity::Info,
    }
}

pub(crate) struct OutdatedModelReferenceChecker {
    scope: ScopeFilter,
    catalog: ModelCatalog,
    /// User-supplied names to flag, as one case-insensitive whole-word alternation.
    extra: Option<Regex>,
    /// `0` disables the snapshot-age check.
    max_snapshot_age_days: i64,
    today_days: i64,
}

impl OutdatedModelReferenceChecker {
    pub(crate) fn new(config: &OutdatedModelReferenceConfig) -> Self {
        let alternatives: Vec<String> = config
            .extra_models
            .iter()
            .map(|m| regex::escape(m.trim()))
            .filter(|m| !m.is_empty())
            .collect();
        let extra = (!alternatives.is_empty()).then(|| {
            Regex::new(&format!(r"(?i)\b(?:{})\b", alternatives.join("|")))
                .expect("escaped literals form a valid regex")
        });
        Self {
            scope: ScopeFilter::new(&config.scope),
            catalog: ModelCatalog::new(&config.current_models),
            extra,
            max_snapshot_age_days: i64::from(config.max_snapshot_age_days),
            today_days: date::today_days(),
        }
    }

    #[cfg(test)]
    fn with_today(mut self, year: i64, month: u32, day: u32) -> Self {
        self.today_days = date::days_from_civil(year, month, day);
        self
    }

    fn scan(
        &self,
        result: &mut CheckResult,
        path: &Arc<PathBuf>,
        line: usize,
        text: &str,
        location: Location,
    ) {
        let extra_spans: Vec<(usize, usize)> = self.extra.as_ref().map_or_else(Vec::new, |re| {
            re.find_iter(text).map(|m| (m.start(), m.end())).collect()
        });
        for &(start, end) in &extra_spans {
            emit!(
                result,
                path,
                line,
                severity(location, ModelStatus::Retired),
                Category::OutdatedModelReference,
                suggest: "Listed in extra_models; update to a current model",
                "Outdated model reference: {}",
                &text[start..end]
            );
        }

        for (start, end, mention) in self.catalog.mentions(text) {
            if extra_spans.iter().any(|&(s, e)| start < e && s < end) {
                continue;
            }
            let Some(class) = self.catalog.classify(mention) else {
                continue;
            };
            match class.status {
                Some(ModelStatus::Current) => {}
                Some(status) => {
                    let name = class.name.map_or(class.key.as_str(), |n| n);
                    let successor = class.successor.unwrap_or("a current model");
                    let (label, suggestion) = match status {
                        ModelStatus::Retired => (
                            "Retired",
                            format!("{name} is no longer served; migrate to {successor}"),
                        ),
                        ModelStatus::Deprecated => (
                            "Deprecated",
                            format!("{name} is scheduled for retirement; migrate to {successor}"),
                        ),
                        ModelStatus::Superseded | ModelStatus::Current => (
                            "Superseded",
                            format!(
                                "A newer generation than {name} is available; consider {successor}"
                            ),
                        ),
                    };
                    emit!(
                        result,
                        path,
                        line,
                        severity(location, status),
                        Category::OutdatedModelReference,
                        suggest: suggestion,
                        "{label} model reference: {mention}"
                    );
                }
                None => {
                    let Some((y, m, d)) = class.snapshot else {
                        continue;
                    };
                    if self.max_snapshot_age_days == 0 {
                        continue;
                    }
                    let age = self.today_days - date::days_from_civil(y, m, d);
                    if age > self.max_snapshot_age_days {
                        emit!(
                            result,
                            path,
                            line,
                            Severity::Info,
                            Category::OutdatedModelReference,
                            suggest: "Check whether a newer release of this model exists",
                            "Model snapshot {mention} is {age} days old"
                        );
                    }
                }
            }
        }
    }
}

/// The top-level `model` string from a Claude Code settings file, with its 1-based line.
fn settings_model(path: &Path) -> Option<(usize, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let model = json.get("model")?.as_str()?.to_string();
    let line = content
        .lines()
        .position(|l| l.contains("\"model\"") && l.contains(&model))
        .map_or(1, |i| i + 1);
    Some((line, model))
}

impl Checker for OutdatedModelReferenceChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "outdated-model-reference",
            description: "Flags retired, deprecated, or superseded AI model names",
            default_severity: Severity::Info,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for (file_idx, file) in ctx.files.iter().enumerate() {
            if ctx.historical_indices.contains(&file_idx)
                || !self.scope.includes(&file.path, &ctx.project_root)
            {
                continue;
            }

            if let Some(fm) = &file.frontmatter {
                if let Some(value) = fm.get_str("model") {
                    let line = fm.line_of("model").unwrap_or(fm.open + 1);
                    self.scan(&mut result, &file.path, line, value, Location::Config);
                }
            }

            for (idx, line) in file.non_code_lines() {
                if is_heading(line) || EXCLUSION_PATTERN.is_match(line) {
                    continue;
                }
                self.scan(&mut result, &file.path, idx + 1, line, Location::Prose);
            }
        }

        for path in &ctx.settings_files {
            if !self.scope.includes(path, &ctx.project_root) {
                continue;
            }
            if let Some((line, value)) = settings_model(path) {
                self.scan(
                    &mut result,
                    &Arc::new(path.clone()),
                    line,
                    &value,
                    Location::Config,
                );
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn checker_with(config: OutdatedModelReferenceConfig) -> OutdatedModelReferenceChecker {
        OutdatedModelReferenceChecker::new(&config).with_today(2026, 9, 7)
    }

    fn run_with(config: OutdatedModelReferenceConfig, lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        checker_with(config).check(&ctx)
    }

    fn run_check(lines: &[&str]) -> CheckResult {
        run_with(OutdatedModelReferenceConfig::default(), lines)
    }

    fn messages(result: &CheckResult) -> Vec<&str> {
        result
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect()
    }

    // ── Prose: catalog verdicts ─────────────────────────────────────────

    #[test]
    fn retired_models_flag_in_prose_and_id_forms() {
        for line in [
            "Use GPT-3.5 for cheap tasks",
            "Use claude-3-sonnet for summaries",
            "We recommend claude-2 for this task",
            "Use text-davinci-003 as fallback",
            "Try claude-instant for speed",
            "Switch to gpt-4-turbo for faster responses",
            "claude-v1 was the first model",
            "Use Claude 3.5 Sonnet for summaries",
            "Use Claude Sonnet 3.5 for summaries",
            "model = claude-3-5-sonnet-20241022",
            "Prefer claude-3-5-sonnet-latest",
            "Bedrock: anthropic.claude-3-5-sonnet-20241022-v2:0",
        ] {
            let result = run_check(&[line]);
            assert_eq!(result.diagnostics.len(), 1, "{line}");
            assert_eq!(result.diagnostics[0].severity, Severity::Info, "{line}");
        }
    }

    #[test]
    fn message_names_the_status_and_mention() {
        let result = run_check(&["Use claude-3-5-sonnet-20241022 for summaries"]);
        assert_eq!(
            messages(&result),
            ["Retired model reference: claude-3-5-sonnet-20241022"]
        );
        assert_eq!(
            result.diagnostics[0].suggestion.as_deref(),
            Some("Claude 3.5 Sonnet is no longer served; migrate to claude-sonnet-5")
        );

        let result = run_check(&["Use claude-sonnet-4-20250514"]);
        assert_eq!(
            messages(&result),
            ["Deprecated model reference: claude-sonnet-4-20250514"]
        );

        let result = run_check(&["Use Claude Sonnet 4.5 for reviews"]);
        assert_eq!(
            messages(&result),
            ["Superseded model reference: Claude Sonnet 4.5"]
        );
        assert!(result.diagnostics[0]
            .suggestion
            .as_deref()
            .unwrap()
            .contains("claude-sonnet-5"));
    }

    #[test]
    fn current_models_do_not_flag() {
        for line in [
            "Use claude-sonnet-4-6 for reviews",
            "Use claude-opus-5 for planning",
            "Use claude-haiku-4-5-20251001 for triage",
            "Use gpt-5 for reasoning",
            "Use gemini-2.5-pro for long context",
            "Delegate to sonnet or haiku",
        ] {
            assert!(run_check(&[line]).diagnostics.is_empty(), "{line}");
        }
    }

    #[test]
    fn unknown_versions_and_bare_names_do_not_flag() {
        for line in [
            "Use claude-opus-4-9 when it ships",
            "GPT-4 class models are fine",
            "Claude Code reads CLAUDE.md",
            "Claude's memory is file based",
            "o1 is a variable name here",
            "See the sonnet in chapter 3",
        ] {
            assert!(run_check(&[line]).diagnostics.is_empty(), "{line}");
        }
    }

    #[test]
    fn superseded_gpt4o_flags() {
        let result = run_check(&["Use gpt-4o for reasoning"]);
        assert_eq!(messages(&result), ["Superseded model reference: gpt-4o"]);
    }

    #[test]
    fn multiple_mentions_on_one_line_each_flag() {
        let result = run_check(&["Route claude-3-opus and gpt-4o-mini traffic"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    // ── Prose: exclusions ───────────────────────────────────────────────

    #[test]
    fn in_code_block_no_flag() {
        let result = run_check(&["```", "model = 'gpt-3.5-turbo'", "```"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn heading_no_flag() {
        let result = run_check(&["## GPT-3.5 Migration"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn self_documenting_lines_no_flag() {
        for line in [
            "Historically we used GPT-3.5",
            "GPT-3.5 is deprecated, use gpt-5 instead",
            "changelog: migrated from claude-2 to claude-sonnet-4",
            "Migrated from claude-3-5-sonnet in March",
            "Legacy path still mentions claude-2",
        ] {
            assert!(run_check(&[line]).diagnostics.is_empty(), "{line}");
        }
    }

    #[test]
    fn historical_files_are_skipped() {
        let (_dir, mut ctx) = single_file_ctx(&["Switched away from claude-3-opus"]);
        ctx.historical_indices.insert(0);
        let result = checker_with(OutdatedModelReferenceConfig::default()).check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    // ── Config locations ────────────────────────────────────────────────

    #[test]
    fn retired_model_in_agent_frontmatter_is_a_warning() {
        let result = run_check(&[
            "---",
            "name: reviewer",
            "model: claude-3-5-sonnet-20241022",
            "---",
            "# Reviewer",
        ]);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.line, 3);
        assert_eq!(d.severity, Severity::Warning);
        assert!(d.message.starts_with("Retired model reference"));
    }

    #[test]
    fn superseded_model_in_frontmatter_stays_info() {
        let result = run_check(&["---", "model: \"claude-sonnet-4-5\"", "---", "# Agent"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn alias_in_frontmatter_no_flag() {
        for value in ["opus", "sonnet", "haiku", "inherit", "claude-opus-5"] {
            let result = run_check(&["---", &format!("model: {value}"), "---", "# Agent"]);
            assert!(result.diagnostics.is_empty(), "{value}");
        }
    }

    #[test]
    fn nested_model_key_in_frontmatter_is_ignored() {
        let result = run_check(&["---", "tools:", "  model: claude-2", "---", "# Agent"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn settings_json_model_is_checked_with_warning() {
        let (dir, mut ctx) = single_file_ctx(&["# Project"]);
        ctx.settings_files = vec![dir.path().join(".claude/settings.json")];
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(
            dir.path().join(".claude/settings.json"),
            "{\n  \"permissions\": {},\n  \"model\": \"claude-opus-4-1-20250805\"\n}\n",
        )
        .unwrap();
        let result = checker_with(OutdatedModelReferenceConfig::default()).check(&ctx);
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert!(d.file.ends_with("settings.json"), "{}", d.file.display());
        assert_eq!(d.line, 3);
        assert_eq!(d.severity, Severity::Warning);
    }

    #[test]
    fn settings_json_alias_or_missing_no_flag() {
        let (dir, mut ctx) = single_file_ctx(&["# Project"]);
        ctx.settings_files = vec![
            dir.path().join(".claude/settings.json"),
            dir.path().join(".claude/settings.local.json"),
        ];
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(
            dir.path().join(".claude/settings.json"),
            "{\"model\": \"opus\"}",
        )
        .unwrap();
        std::fs::write(dir.path().join(".claude/settings.local.json"), "not json").unwrap();
        let result = checker_with(OutdatedModelReferenceConfig::default()).check(&ctx);
        assert!(result.diagnostics.is_empty());
    }

    // ── Snapshot age ────────────────────────────────────────────────────

    #[test]
    fn old_snapshot_of_unknown_model_flags() {
        let result = run_check(&["Use claude-sonnet-4-9-20250101 for drafts"]);
        assert_eq!(
            messages(&result),
            ["Model snapshot claude-sonnet-4-9-20250101 is 614 days old"]
        );
    }

    #[test]
    fn recent_snapshot_of_unknown_model_no_flag() {
        let result = run_check(&["Use claude-sonnet-4-9-20260801 for drafts"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn catalog_verdict_wins_over_snapshot_age() {
        let result = run_check(&["Use gpt-4o-2024-08-06"]);
        assert_eq!(
            messages(&result),
            ["Superseded model reference: gpt-4o-2024-08-06"]
        );
    }

    #[test]
    fn current_model_snapshot_is_never_flagged_by_age() {
        let (_dir, ctx) = single_file_ctx(&["Use claude-haiku-4-5-20251001"]);
        let checker = OutdatedModelReferenceChecker::new(&OutdatedModelReferenceConfig::default())
            .with_today(2030, 1, 1);
        assert!(checker.check(&ctx).diagnostics.is_empty());
    }

    #[test]
    fn snapshot_age_check_can_be_disabled() {
        let config = OutdatedModelReferenceConfig {
            max_snapshot_age_days: 0,
            ..Default::default()
        };
        let result = run_with(config, &["Use claude-sonnet-4-9-20250101"]);
        assert!(result.diagnostics.is_empty());
    }

    // ── Regressions from review ─────────────────────────────────────────

    #[test]
    fn prose_counts_are_not_models() {
        for line in [
            "I asked Claude 3 times to rewrite it",
            "Claude 2 weeks ago was different",
            "Talk to GPT 4 people about it",
        ] {
            assert!(run_check(&[line]).diagnostics.is_empty(), "{line}");
        }
    }

    #[test]
    fn glued_and_revisioned_spellings_flag() {
        for (line, prefix) in [
            ("Use GPT3.5 here", "Superseded"),
            ("We recommend claude2", "Retired"),
            ("Use Claude3 Sonnet", "Retired"),
            ("Switch to GPT4 Turbo", "Superseded"),
            ("Use gpt-3.5-turbo-0125 for cheap tasks", "Superseded"),
            ("Use gemini-1.5-pro-002", "Retired"),
            ("Use claude-3-5-sonnet-v2", "Retired"),
        ] {
            let result = run_check(&[line]);
            assert_eq!(result.diagnostics.len(), 1, "{line}");
            let message = &result.diagnostics[0].message;
            assert!(message.starts_with(prefix), "{line}: {message}");
        }
    }

    #[test]
    fn dated_snapshot_of_unknown_bare_model_is_age_checked() {
        let result = run_check(&["Pin o3-2025-04-16 for evals"]);
        assert_eq!(
            messages(&result),
            ["Model snapshot o3-2025-04-16 is 509 days old"]
        );
    }

    #[test]
    fn blank_current_models_entry_does_not_panic() {
        let config = OutdatedModelReferenceConfig {
            current_models: vec![String::new(), " ".to_string(), "-".to_string()],
            ..Default::default()
        };
        let result = run_with(config, &["Use gpt-4o"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn unclosed_frontmatter_is_prose_not_config() {
        let result = run_check(&["---", "model: claude-2", "# Never closed"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, Severity::Info);
    }

    // ── User overrides ──────────────────────────────────────────────────

    #[test]
    fn extra_models_flag_as_whole_words() {
        let config = OutdatedModelReferenceConfig {
            extra_models: vec!["acme-llm-v1".to_string()],
            ..Default::default()
        };
        let result = run_with(config.clone(), &["Call ACME-LLM-v1 for routing"]);
        assert_eq!(messages(&result), ["Outdated model reference: ACME-LLM-v1"]);
        let result = run_with(config, &["Call acme-llm-v10 for routing"]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn extra_model_overlapping_catalog_reports_once() {
        let config = OutdatedModelReferenceConfig {
            extra_models: vec!["gpt-4o".to_string()],
            ..Default::default()
        };
        let result = run_with(config, &["Use gpt-4o here"]);
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn current_models_exempt_catalog_entries() {
        let config = OutdatedModelReferenceConfig {
            current_models: vec!["gpt-4o".to_string(), "Claude Sonnet 4".to_string()],
            ..Default::default()
        };
        let result = run_with(config, &["Use gpt-4o and claude-sonnet-4-20250514"]);
        assert!(result.diagnostics.is_empty());
    }
}
