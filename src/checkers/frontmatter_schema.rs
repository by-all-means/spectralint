//! Validates the frontmatter of tool-specific instruction files against what
//! each tool actually requires. Every check here corresponds to a documented
//! silent failure: a Claude Code subagent without `name` is treated as
//! documentation, a rule with an unreadable `paths` glob matches nothing, a
//! Copilot instruction file without `applyTo` applies to nothing.
//!
//! Unknown keys are reported only when they are a near-miss of a documented
//! key, since vendors add fields faster than any list can follow.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::file_kind::FileKind;
use crate::parser::frontmatter::{FmValue, Frontmatter};
use crate::parser::types::ParsedFile;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct FrontmatterSchemaChecker {
    scope: ScopeFilter,
}

impl FrontmatterSchemaChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

/// Top-level keys each tool documents. Used for near-miss detection only;
/// a key that matches nothing here is left alone.
fn known_keys(kind: FileKind) -> Option<&'static [&'static str]> {
    Some(match kind {
        FileKind::ClaudeSubagent => &[
            "name",
            "description",
            "tools",
            "disallowedTools",
            "model",
            "permissionMode",
            "maxTurns",
            "skills",
            "mcpServers",
            "hooks",
            "memory",
            "background",
            "effort",
            "isolation",
            "color",
            "initialPrompt",
            "experimental",
        ],
        FileKind::Skill => &[
            "name",
            "description",
            "when_to_use",
            "license",
            "compatibility",
            "metadata",
            "allowed-tools",
            "disallowed-tools",
            "argument-hint",
            "arguments",
            "disable-model-invocation",
            "user-invocable",
            "model",
            "effort",
            "context",
            "agent",
            "background",
            "hooks",
            "paths",
            "shell",
        ],
        FileKind::ClaudeCommand => &[
            "description",
            "allowed-tools",
            "argument-hint",
            "model",
            "disable-model-invocation",
        ],
        FileKind::ClaudeRule => &["paths"],
        FileKind::CursorRule => &["description", "globs", "alwaysApply"],
        FileKind::CopilotInstruction => &["applyTo", "description", "name", "excludeAgent"],
        FileKind::CopilotAgent => &[
            "name",
            "description",
            "target",
            "tools",
            "model",
            "disable-model-invocation",
            "user-invocable",
            "mcp-servers",
            "metadata",
        ],
        FileKind::CopilotPrompt => &[
            "name",
            "description",
            "argument-hint",
            "agent",
            "model",
            "tools",
            "mode",
        ],
        FileKind::KiroSteering => &["inclusion", "fileMatchPattern"],
        FileKind::DevinRules => &["trigger", "globs", "description"],
        _ => return None,
    })
}

/// Line to anchor a diagnostic about `key`: the key's own line, else the
/// opening delimiter (line 1 when there is no frontmatter at all).
fn anchor(fm: &Frontmatter, key: &str) -> usize {
    fm.line_of(key).unwrap_or(fm.open + 1)
}

fn non_empty_str<'a>(fm: &'a Frontmatter, key: &str) -> Option<&'a str> {
    fm.get_str(key).filter(|s| !s.trim().is_empty())
}

fn valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl Checker for FrontmatterSchemaChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "frontmatter-schema",
            description: "Frontmatter fields a tool requires or cannot read",
            default_severity: Severity::Warning,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }
            let Some(keys) = known_keys(file.kind) else {
                continue;
            };
            let path = &file.path;

            // A leading `---` followed by a `key:` line is frontmatter someone forgot
            // to close; a leading `---` followed by prose is a horizontal rule.
            let opens_block = file.raw_lines.first().is_some_and(|l| l.trim() == "---")
                && file
                    .raw_lines
                    .get(1)
                    .is_some_and(|l| crate::parser::frontmatter::is_key_line(l));
            if file.frontmatter.is_none() && opens_block {
                emit!(
                    result,
                    path,
                    1,
                    Severity::Warning,
                    Category::FrontmatterSchema,
                    suggest: "Close the block with a second `---` line",
                    "Frontmatter opened on line 1 is never closed; the file is read as plain content"
                );
                continue;
            }

            let empty = Frontmatter::default();
            let fm = file.frontmatter.as_ref().unwrap_or(&empty);

            if let Some(err) = &fm.parse_error {
                // Tools read frontmatter with lenient line parsers, and the corpus
                // shows unquoted globs and colons in descriptions everywhere, so
                // this is a portability note. Cursor rules get nothing: unquoted
                // globs are Cursor's own style.
                if file.kind != FileKind::CursorRule {
                    emit!(
                        result,
                        path,
                        fm.open + 1,
                        Severity::Info,
                        Category::FrontmatterSchema,
                        suggest: "Quote values that contain `*`, `:`, `#`, or `[` so every tool's parser reads this block the same way",
                        "Frontmatter is not strict YAML: {err}"
                    );
                }
            }

            match file.kind {
                FileKind::ClaudeSubagent => {
                    let nested_fragment = file.frontmatter.is_none()
                        && file
                            .path
                            .parent()
                            .and_then(Path::file_name)
                            .is_some_and(|d| d != "agents");
                    if !nested_fragment {
                        check_subagent(&mut result, path, fm);
                    }
                }
                FileKind::Skill => check_skill(&mut result, path, fm, file, &ctx.project_root),
                FileKind::ClaudeRule => check_globs(&mut result, path, fm, "paths"),
                FileKind::CursorRule => check_cursor_rule(&mut result, path, fm),
                FileKind::CopilotInstruction => check_copilot_instruction(&mut result, path, fm),
                FileKind::CopilotAgent => check_copilot_agent(&mut result, path, fm, file),
                FileKind::KiroSteering => check_kiro(&mut result, path, fm),
                FileKind::DevinRules => check_devin(&mut result, path, fm),
                _ => {}
            }

            check_near_miss_keys(&mut result, path, fm, keys);
        }

        result
    }
}

fn check_subagent(result: &mut CheckResult, path: &Arc<PathBuf>, fm: &Frontmatter) {
    let Some(name) = non_empty_str(fm, "name") else {
        emit!(
            result,
            path,
            anchor(fm, "name"),
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Add `name: <lowercase-hyphenated>` to the frontmatter",
            "Subagent has no `name`; Claude Code treats this file as documentation and never loads it"
        );
        return;
    };
    if !valid_agent_name(name) {
        emit!(
            result,
            path,
            anchor(fm, "name"),
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Use lowercase letters, digits, and hyphens; no spaces, colons, or leading hyphen",
            "Subagent `name` `{name}` is invalid; Claude Code skips it"
        );
    }
    if non_empty_str(fm, "description").is_none() {
        emit!(
            result,
            path,
            anchor(fm, "description"),
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Describe when the agent should be used; Claude Code reads it to decide when to delegate",
            "Subagent `{name}` has no `description`; Claude Code skips it"
        );
    }
}

fn check_skill(
    result: &mut CheckResult,
    path: &Arc<PathBuf>,
    fm: &Frontmatter,
    file: &ParsedFile,
    project_root: &Path,
) {
    if let Some(name) = non_empty_str(fm, "name") {
        let dir = file
            .path
            .parent()
            .filter(|p| *p != project_root)
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned());
        if let Some(dir) = dir.filter(|d| d != name) {
            emit!(
                result,
                path,
                anchor(fm, "name"),
                Severity::Info,
                Category::FrontmatterSchema,
                suggest: "Rename one of them; the Agent Skills spec requires the name to equal the directory",
                "Skill `name` `{name}` does not match its directory `{dir}`"
            );
        }
        if !valid_skill_name(name) {
            emit!(
                result,
                path,
                anchor(fm, "name"),
                Severity::Info,
                Category::FrontmatterSchema,
                suggest: "The Agent Skills spec allows up to 64 lowercase letters, digits, and single hyphens",
                "Skill `name` `{name}` is not portable across tools"
            );
        }
    }

    let description = fm.get_str("description").map_or(0, |d| d.chars().count());
    let combined = description + fm.get_str("when_to_use").map_or(0, |d| d.chars().count());
    if combined > 1536 {
        emit!(
            result,
            path,
            anchor(fm, "description"),
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Move detail into the skill body; only the first 1,536 characters reach the model",
            "Skill description and when_to_use total {combined} characters; Claude Code truncates past 1,536"
        );
    } else if description > 1024 {
        emit!(
            result,
            path,
            anchor(fm, "description"),
            Severity::Info,
            Category::FrontmatterSchema,
            suggest: "Shorten it to stay portable; the Agent Skills spec allows 1,024 characters",
            "Skill description is {description} characters; the Agent Skills spec allows 1,024"
        );
    }
}

/// `key` must hold globs as a list or a comma-separated string, and each glob
/// must compile: a glob the tool cannot read matches nothing, silently.
fn check_globs(result: &mut CheckResult, path: &Arc<PathBuf>, fm: &Frontmatter, key: &str) {
    // Absent or empty (`globs:` with nothing after it, which Cursor writes by
    // default) is not a type error.
    if matches!(fm.get(key), None | Some(FmValue::Null)) {
        return;
    }
    let Some(globs) = fm.get_str_list(key) else {
        emit!(
            result,
            path,
            anchor(fm, key),
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Write it as a YAML list or a comma-separated string",
            "`{key}` must be a list or comma-separated string of globs"
        );
        return;
    };
    for glob in globs {
        if let Err(err) = globset::Glob::new(&glob) {
            emit!(
                result,
                path,
                anchor(fm, key),
                Severity::Warning,
                Category::FrontmatterSchema,
                suggest: "Fix the pattern; an unreadable glob matches nothing",
                "`{key}` glob `{glob}` is invalid: {err}"
            );
        }
    }
}

fn check_cursor_rule(result: &mut CheckResult, path: &Arc<PathBuf>, fm: &Frontmatter) {
    check_globs(result, path, fm, "globs");
    if fm.has("alwaysApply") && fm.get_bool("alwaysApply").is_none() {
        emit!(
            result,
            path,
            anchor(fm, "alwaysApply"),
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Use `alwaysApply: true` or `alwaysApply: false`",
            "`alwaysApply` must be true or false"
        );
    }
    let has_description = non_empty_str(fm, "description").is_some();
    let has_globs = fm.get_str_list("globs").is_some_and(|g| !g.is_empty());
    if !has_description && !has_globs && fm.get_bool("alwaysApply") != Some(true) {
        emit!(
            result,
            path,
            fm.open + 1,
            Severity::Info,
            Category::FrontmatterSchema,
            suggest: "Add `alwaysApply: true`, `globs`, or a `description` unless manual invocation is intended",
            "Rule has no description, globs, or alwaysApply; Cursor applies it only when invoked as @rule-name"
        );
    }
}

fn check_copilot_instruction(result: &mut CheckResult, path: &Arc<PathBuf>, fm: &Frontmatter) {
    if !fm.has("applyTo") {
        emit!(
            result,
            path,
            fm.open + 1,
            Severity::Info,
            Category::FrontmatterSchema,
            suggest: "Add `applyTo: \"**\"` or a narrower glob unless manual attachment is intended",
            "Instruction file has no `applyTo`; Copilot applies it only when attached by hand"
        );
        return;
    }
    check_globs(result, path, fm, "applyTo");
}

fn check_copilot_agent(
    result: &mut CheckResult,
    path: &Arc<PathBuf>,
    fm: &Frontmatter,
    file: &ParsedFile,
) {
    if non_empty_str(fm, "description").is_none() {
        emit!(
            result,
            path,
            anchor(fm, "description"),
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Describe what the agent does; Copilot requires a description",
            "Custom agent has no `description`"
        );
    }
    let body_start = if file.frontmatter.is_some() {
        fm.close + 1
    } else {
        0
    };
    let body_chars: usize = file
        .raw_lines
        .get(body_start..)
        .unwrap_or_default()
        .iter()
        .map(|l| l.chars().count() + 1)
        .sum();
    if body_chars > 30_000 {
        emit!(
            result,
            path,
            body_start + 1,
            Severity::Warning,
            Category::FrontmatterSchema,
            suggest: "Trim the body; Copilot rejects custom agents over 30,000 characters",
            "Custom agent body is {body_chars} characters; the limit is 30,000"
        );
    }
}

fn check_kiro(result: &mut CheckResult, path: &Arc<PathBuf>, fm: &Frontmatter) {
    if let Some(inclusion) = fm.get_str("inclusion") {
        if !matches!(inclusion, "always" | "fileMatch" | "manual" | "auto") {
            emit!(
                result,
                path,
                anchor(fm, "inclusion"),
                Severity::Warning,
                Category::FrontmatterSchema,
                suggest: "Use one of always, fileMatch, manual, auto",
                "`inclusion` value `{inclusion}` is not one Kiro understands"
            );
        }
        if inclusion == "fileMatch" && !fm.has("fileMatchPattern") {
            emit!(
                result,
                path,
                anchor(fm, "inclusion"),
                Severity::Warning,
                Category::FrontmatterSchema,
                suggest: "Add `fileMatchPattern` with the glob to match",
                "`inclusion: fileMatch` without `fileMatchPattern` never applies"
            );
        }
    }
    check_globs(result, path, fm, "fileMatchPattern");
}

fn check_devin(result: &mut CheckResult, path: &Arc<PathBuf>, fm: &Frontmatter) {
    if let Some(trigger) = fm.get_str("trigger") {
        if !matches!(trigger, "always_on" | "glob" | "model_decision" | "manual") {
            emit!(
                result,
                path,
                anchor(fm, "trigger"),
                Severity::Warning,
                Category::FrontmatterSchema,
                suggest: "Use one of always_on, glob, model_decision, manual",
                "`trigger` value `{trigger}` is not one Devin understands"
            );
        }
        if trigger == "glob" && !fm.has("globs") {
            emit!(
                result,
                path,
                anchor(fm, "trigger"),
                Severity::Warning,
                Category::FrontmatterSchema,
                suggest: "Add `globs` with the patterns to match",
                "`trigger: glob` without `globs` never applies"
            );
        }
    }
    check_globs(result, path, fm, "globs");
}

/// A key that is not documented but sits within a typo of one that is.
fn check_near_miss_keys(
    result: &mut CheckResult,
    path: &Arc<PathBuf>,
    fm: &Frontmatter,
    known: &[&str],
) {
    for key in fm.keys() {
        if known.contains(&key) {
            continue;
        }
        let lower = key.to_ascii_lowercase();
        let best = known
            .iter()
            .map(|k| (strsim::jaro_winkler(&lower, &k.to_ascii_lowercase()), *k))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, suggestion)) = best.filter(|(score, _)| *score >= 0.9) {
            emit!(
                result,
                path,
                anchor(fm, key),
                Severity::Warning,
                Category::FrontmatterSchema,
                suggest: "Rename the field; the tool ignores keys it does not recognise",
                "Unknown frontmatter field `{key}`; did you mean `{suggestion}`?"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::file_ctx_at;

    fn run(rel: &str, lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = file_ctx_at(rel, lines);
        FrontmatterSchemaChecker::new(&[]).check(&ctx)
    }

    fn messages(rel: &str, lines: &[&str]) -> Vec<(Severity, String)> {
        run(rel, lines)
            .diagnostics
            .into_iter()
            .map(|d| (d.severity, d.message))
            .collect()
    }

    fn only(rel: &str, lines: &[&str]) -> (Severity, String) {
        let mut found = messages(rel, lines);
        assert_eq!(found.len(), 1, "{found:?}");
        found.pop().unwrap()
    }

    // ── Subagents ───────────────────────────────────────────────────────

    #[test]
    fn subagent_without_name_is_documentation() {
        let (sev, msg) = only(
            ".claude/agents/reviewer.md",
            &["---", "description: Reviews PRs", "---", "# Reviewer"],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("no `name`"), "{msg}");
        let (sev, msg) = only(
            ".claude/agents/reviewer.md",
            &["# Reviewer", "No frontmatter at all."],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("no `name`"), "{msg}");
    }

    #[test]
    fn subagent_with_name_but_no_description_is_skipped() {
        let (sev, msg) = only(
            ".claude/agents/reviewer.md",
            &["---", "name: reviewer", "---"],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("no `description`"), "{msg}");
    }

    #[test]
    fn subagent_name_must_be_lowercase_hyphenated() {
        for bad in ["Reviewer Bot", "review:er", "-reviewer"] {
            let found = messages(
                ".claude/agents/r.md",
                &["---", &format!("name: {bad}"), "description: x", "---"],
            );
            assert!(
                found.iter().any(|(_, m)| m.contains("is invalid")),
                "{bad}: {found:?}"
            );
        }
    }

    #[test]
    fn valid_subagent_is_clean() {
        let found = messages(
            ".claude/agents/reviewer.md",
            &[
                "---",
                "name: code-reviewer",
                "description: Reviews pull requests",
                "tools: Read, Grep",
                "model: sonnet",
                "---",
                "# Body",
            ],
        );
        assert!(found.is_empty(), "{found:?}");
    }

    // ── Skills ──────────────────────────────────────────────────────────

    #[test]
    fn skill_name_must_match_directory_and_spec() {
        let (sev, msg) = only(
            ".claude/skills/deploy/SKILL.md",
            &["---", "name: deployer", "description: Deploys", "---"],
        );
        assert_eq!(sev, Severity::Info);
        assert!(
            msg.contains("does not match its directory `deploy`"),
            "{msg}"
        );
        let found = messages(
            ".claude/skills/deploy/SKILL.md",
            &["---", "name: Deploy--Now", "description: Deploys", "---"],
        );
        assert!(
            found
                .iter()
                .any(|(s, m)| *s == Severity::Info && m.contains("not portable")),
            "{found:?}"
        );
    }

    #[test]
    fn skill_description_limits() {
        let long = "x".repeat(1100);
        let (sev, msg) = only(
            ".claude/skills/deploy/SKILL.md",
            &[
                "---",
                "name: deploy",
                &format!("description: {long}"),
                "---",
            ],
        );
        assert_eq!(sev, Severity::Info);
        assert!(msg.contains("1,024"), "{msg}");
        let longer = "x".repeat(1600);
        let (sev, msg) = only(
            ".claude/skills/deploy/SKILL.md",
            &[
                "---",
                "name: deploy",
                &format!("description: {longer}"),
                "---",
            ],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("1,536"), "{msg}");
    }

    #[test]
    fn spec_compliant_skill_is_clean() {
        let found = messages(
            ".claude/skills/deploy/SKILL.md",
            &[
                "---",
                "name: deploy",
                "description: >",
                "  Deploys the service when the user asks.",
                "allowed-tools: Bash, Read",
                "---",
                "# Deploy",
            ],
        );
        assert!(found.is_empty(), "{found:?}");
    }

    // ── Rules and globs ─────────────────────────────────────────────────

    #[test]
    fn claude_rule_paths_glob_must_compile() {
        let (sev, msg) = only(
            ".claude/rules/api.md",
            &["---", "paths:", "  - src/api/[", "---"],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("is invalid"), "{msg}");
        assert!(messages(
            ".claude/rules/api.md",
            &["---", "paths: src/**, tests/**", "---"]
        )
        .is_empty());
        let (sev, msg) = only(
            ".claude/rules/api.md",
            &["---", "paths:", "  key: value", "---"],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("must be a list"), "{msg}");
    }

    #[test]
    fn cursor_unquoted_globs_are_cursor_style() {
        let found = messages(
            ".cursor/rules/react.mdc",
            &[
                "---",
                "description: React rules",
                "globs: *.tsx",
                "alwaysApply: false",
                "---",
            ],
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn empty_globs_line_is_not_a_type_error() {
        let found = messages(
            ".cursor/rules/css.mdc",
            &[
                "---",
                "description: CSS",
                "globs:",
                "alwaysApply: false",
                "---",
            ],
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn cursor_rule_checks() {
        let (sev, msg) = only(
            ".cursor/rules/a.mdc",
            &["---", "description: x", "alwaysApply: yes", "---"],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("alwaysApply"), "{msg}");
        let (sev, msg) = only(".cursor/rules/a.mdc", &["---", "---", "Be terse."]);
        assert_eq!(sev, Severity::Info);
        assert!(msg.contains("@rule-name"), "{msg}");
        assert!(messages(
            ".cursor/rules/a.mdc",
            &[
                "---",
                "description: x",
                "globs:",
                "  - \"src/**/*.ts\"",
                "alwaysApply: false",
                "---"
            ]
        )
        .is_empty());
    }

    // ── Copilot ─────────────────────────────────────────────────────────

    #[test]
    fn copilot_instruction_needs_apply_to() {
        let (sev, msg) = only(
            ".github/instructions/api.instructions.md",
            &["---", "description: API rules", "---"],
        );
        assert_eq!(sev, Severity::Info);
        assert!(msg.contains("applyTo"), "{msg}");
        assert!(messages(
            ".github/instructions/api.instructions.md",
            &["---", "applyTo: \"src/**/*.ts, tests/**\"", "---"]
        )
        .is_empty());
    }

    #[test]
    fn copilot_agent_checks() {
        let (sev, msg) = only(
            ".github/agents/triage.agent.md",
            &["---", "name: triage", "---", "Body"],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("no `description`"), "{msg}");
        let big = "y".repeat(31_000);
        let (sev, msg) = only(
            ".github/agents/triage.agent.md",
            &["---", "description: Triage", "---", &big],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("30,000"), "{msg}");
    }

    // ── Kiro and Devin ──────────────────────────────────────────────────

    #[test]
    fn kiro_and_devin_trigger_fields() {
        let (_, msg) = only(
            ".kiro/steering/api.md",
            &["---", "inclusion: sometimes", "---"],
        );
        assert!(msg.contains("inclusion"), "{msg}");
        let (_, msg) = only(
            ".kiro/steering/api.md",
            &["---", "inclusion: fileMatch", "---"],
        );
        assert!(msg.contains("fileMatchPattern"), "{msg}");
        let (_, msg) = only(".devin/rules/api.md", &["---", "trigger: glob", "---"]);
        assert!(msg.contains("globs"), "{msg}");
        assert!(messages(".devin/rules/api.md", &["---", "trigger: always_on", "---"]).is_empty());
    }

    // ── Cross-cutting ───────────────────────────────────────────────────

    #[test]
    fn near_miss_keys_are_flagged_but_unknown_keys_are_not() {
        let (sev, msg) = only(
            ".claude/agents/r.md",
            &[
                "---",
                "name: r",
                "descripton: typo",
                "description: real",
                "---",
            ],
        );
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("did you mean `description`"), "{msg}");
        assert!(messages(
            ".claude/agents/r.md",
            &[
                "---",
                "name: r",
                "description: x",
                "owner: platform-team",
                "---"
            ]
        )
        .is_empty());
    }

    #[test]
    fn unclosed_frontmatter_is_reported_once() {
        let (sev, msg) = only(".claude/agents/r.md", &["---", "name: r", "# never closed"]);
        assert_eq!(sev, Severity::Warning);
        assert!(msg.contains("never closed"), "{msg}");
        // A horizontal rule at the top of a file is not an unclosed block.
        let found = messages(
            ".github/instructions/pr.instructions.md",
            &["---", "# Guidelines", "Prose"],
        );
        assert!(
            found.iter().all(|(_, m)| !m.contains("never closed")),
            "{found:?}"
        );
    }

    #[test]
    fn non_strict_yaml_is_a_portability_note() {
        let found = messages(
            ".claude/agents/r.md",
            &["---", "name: r", "description: Use when: asked", "---"],
        );
        assert!(
            found
                .iter()
                .any(|(s, m)| *s == Severity::Info && m.contains("not strict YAML")),
            "{found:?}"
        );
        assert!(
            found.iter().all(|(s, _)| *s != Severity::Error),
            "{found:?}"
        );
    }

    #[test]
    fn kinds_without_a_schema_are_ignored() {
        assert!(messages("CLAUDE.md", &["---", "globs: *.ts", "descripton: x", "---"]).is_empty());
        assert!(messages("docs/guide.md", &["---", "title: Guide", "---"]).is_empty());
    }
}

#[cfg(test)]
mod fragment_tests {
    use super::*;
    use crate::checkers::utils::test_helpers::file_ctx_at;

    #[test]
    fn nested_fragment_without_frontmatter_is_not_an_agent() {
        let (_dir, ctx) = file_ctx_at(
            ".claude/agents/roles/planner.md",
            &["# Planner", "You plan."],
        );
        assert!(FrontmatterSchemaChecker::new(&[])
            .check(&ctx)
            .diagnostics
            .is_empty());
        let (_dir, ctx) = file_ctx_at(
            ".claude/agents/roles/planner.md",
            &["---", "description: x", "---"],
        );
        assert_eq!(
            FrontmatterSchemaChecker::new(&[])
                .check(&ctx)
                .diagnostics
                .len(),
            1
        );
    }
}
