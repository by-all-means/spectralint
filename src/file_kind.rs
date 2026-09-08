//! Classification of scanned files by the tool and role they serve.
//!
//! Every path the scanner accepts is tagged with a [`FileKind`] so checkers
//! can apply format-specific knowledge: a Claude Code subagent has a required
//! `name`, a Cursor rule has `globs`, a Copilot instruction file has
//! `applyTo`. Classification is by relative path against an ordered table;
//! the first matching row wins, and anything unmatched is [`FileKind::Generic`],
//! which behaves exactly as every file did before kinds existed.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum FileKind {
    /// Any markdown file not recognised as a tool-specific format.
    #[default]
    Generic,
    /// `CLAUDE.md`, `CLAUDE.local.md`, `.claude/CLAUDE.md`.
    ClaudeMd,
    /// `AGENTS.md` / `AGENT.md`, the cross-tool standard.
    AgentsMd,
    /// `GEMINI.md`.
    GeminiMd,
    /// `.claude/rules/**/*.md`, path-scoped via `paths:`.
    ClaudeRule,
    /// `.claude/agents/**/*.md`.
    ClaudeSubagent,
    /// `.claude/commands/**/*.md` (legacy; merged into skills).
    ClaudeCommand,
    /// `SKILL.md` anywhere (Agent Skills spec).
    Skill,
    /// `.cursor/rules/**/*.mdc`.
    CursorRule,
    /// Legacy single-file `.cursorrules`.
    Cursorrules,
    /// `.clinerules` file or `.clinerules/` directory.
    Clinerules,
    /// `.windsurfrules` or `.windsurf/rules/**`.
    WindsurfRules,
    /// `.devin/rules/**` (Windsurf's successor).
    DevinRules,
    /// `.kiro/steering/**/*.md`.
    KiroSteering,
    /// `.roo/rules*/**`.
    RooRules,
    /// `.junie/**/*.md`.
    Junie,
    /// `.github/copilot-instructions.md`, repository-wide.
    CopilotInstructions,
    /// `.github/instructions/**/*.instructions.md`, scoped via `applyTo`.
    CopilotInstruction,
    /// `.github/agents/**/*.agent.md`.
    CopilotAgent,
    /// `.github/prompts/**/*.prompt.md`.
    CopilotPrompt,
}

impl FileKind {
    /// Short, single-purpose files (rules, subagents, skills, prompts) as
    /// opposed to project-wide instruction files. Checkers that expect build
    /// commands, role definitions, or long prose skip these.
    #[must_use]
    pub fn is_component(self) -> bool {
        matches!(
            self,
            Self::ClaudeRule
                | Self::ClaudeSubagent
                | Self::ClaudeCommand
                | Self::Skill
                | Self::CursorRule
                | Self::WindsurfRules
                | Self::DevinRules
                | Self::KiroSteering
                | Self::RooRules
                | Self::Junie
                | Self::CopilotInstruction
                | Self::CopilotAgent
                | Self::CopilotPrompt
        )
    }

    /// Kinds whose tool reads `@path` imports at load time (Claude Code
    /// memory, rules, and commands; Gemini CLI context files).
    #[must_use]
    pub fn supports_imports(self) -> bool {
        matches!(
            self,
            Self::ClaudeMd | Self::ClaudeRule | Self::ClaudeCommand | Self::GeminiMd
        )
    }

    /// Stable lowercase identifier for messages and structured output.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Generic => "markdown",
            Self::ClaudeMd => "claude-md",
            Self::AgentsMd => "agents-md",
            Self::GeminiMd => "gemini-md",
            Self::ClaudeRule => "claude-rule",
            Self::ClaudeSubagent => "claude-subagent",
            Self::ClaudeCommand => "claude-command",
            Self::Skill => "skill",
            Self::CursorRule => "cursor-rule",
            Self::Cursorrules => "cursorrules",
            Self::Clinerules => "clinerules",
            Self::WindsurfRules => "windsurf-rule",
            Self::DevinRules => "devin-rule",
            Self::KiroSteering => "kiro-steering",
            Self::RooRules => "roo-rule",
            Self::Junie => "junie-rule",
            Self::CopilotInstructions => "copilot-instructions",
            Self::CopilotInstruction => "copilot-instruction",
            Self::CopilotAgent => "copilot-agent",
            Self::CopilotPrompt => "copilot-prompt",
        }
    }
}

/// One classification rule. `by_name` rows match the bare file name, the
/// others match the path relative to the project root (any depth, so nested
/// projects in a monorepo classify too).
struct Rule {
    pattern: &'static str,
    by_name: bool,
    kind: FileKind,
}

const fn path(pattern: &'static str, kind: FileKind) -> Rule {
    Rule {
        pattern,
        by_name: false,
        kind,
    }
}

const fn name(pattern: &'static str, kind: FileKind) -> Rule {
    Rule {
        pattern,
        by_name: true,
        kind,
    }
}

/// Ordered: path-anchored rows before name-only rows, specific before generic.
static TABLE: &[Rule] = &[
    path(
        "**/.github/instructions/**/*.instructions.md",
        FileKind::CopilotInstruction,
    ),
    path("**/.github/agents/**/*.agent.md", FileKind::CopilotAgent),
    path("**/.github/prompts/**/*.prompt.md", FileKind::CopilotPrompt),
    path(
        "**/.github/copilot-instructions.md",
        FileKind::CopilotInstructions,
    ),
    path("**/.claude/rules/**/*.md", FileKind::ClaudeRule),
    path("**/.claude/agents/**/*.md", FileKind::ClaudeSubagent),
    path("**/.claude/commands/**/*.md", FileKind::ClaudeCommand),
    path("**/.claude/CLAUDE.md", FileKind::ClaudeMd),
    path("**/.cursor/rules/**/*.mdc", FileKind::CursorRule),
    name(".cursorrules", FileKind::Cursorrules),
    name(".clinerules", FileKind::Clinerules),
    path("**/.clinerules/**", FileKind::Clinerules),
    name(".windsurfrules", FileKind::WindsurfRules),
    path("**/.windsurf/rules/**", FileKind::WindsurfRules),
    path("**/.devin/rules/**", FileKind::DevinRules),
    path("**/.kiro/steering/**/*.md", FileKind::KiroSteering),
    path("**/.roo/rules*/**", FileKind::RooRules),
    path("**/.junie/**/*.md", FileKind::Junie),
    name("SKILL.md", FileKind::Skill),
    name("CLAUDE.md", FileKind::ClaudeMd),
    name("CLAUDE.local.md", FileKind::ClaudeMd),
    name("AGENTS.md", FileKind::AgentsMd),
    name("AGENT.md", FileKind::AgentsMd),
    name("GEMINI.md", FileKind::GeminiMd),
];

/// A compiled glob set plus the `TABLE` index of each glob in it.
struct Matcher {
    set: GlobSet,
    rows: Vec<usize>,
}

fn build(by_name: bool) -> Matcher {
    let mut builder = GlobSetBuilder::new();
    let mut rows = Vec::new();
    for (i, rule) in TABLE.iter().enumerate() {
        if rule.by_name != by_name {
            continue;
        }
        let glob = GlobBuilder::new(rule.pattern)
            .case_insensitive(true)
            .literal_separator(true)
            .build()
            .expect("file-kind table patterns are valid globs");
        builder.add(glob);
        rows.push(i);
    }
    Matcher {
        set: builder.build().expect("file-kind glob set builds"),
        rows,
    }
}

static BY_PATH: LazyLock<Matcher> = LazyLock::new(|| build(false));
static BY_NAME: LazyLock<Matcher> = LazyLock::new(|| build(true));

/// Classify a path relative to the project root. Separators are normalised,
/// so Windows paths classify the same as POSIX ones.
#[must_use]
pub fn classify(relative: &Path) -> FileKind {
    let rel: Vec<String> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let rel = rel.join("/");
    let name = relative
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut best: Option<usize> = None;
    for (matcher, text) in [(&*BY_PATH, rel.as_str()), (&*BY_NAME, name.as_str())] {
        for m in matcher.set.matches(text) {
            let row = matcher.rows[m];
            if best.is_none_or(|b| row < b) {
                best = Some(row);
            }
        }
    }
    best.map_or(FileKind::Generic, |i| TABLE[i].kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(p: &str) -> FileKind {
        classify(Path::new(p))
    }

    #[test]
    fn every_table_row_classifies() {
        use FileKind::*;
        let cases = [
            (
                ".github/instructions/api.instructions.md",
                CopilotInstruction,
            ),
            (
                ".github/instructions/backend/db.instructions.md",
                CopilotInstruction,
            ),
            (".github/agents/triage.agent.md", CopilotAgent),
            (".github/prompts/review.prompt.md", CopilotPrompt),
            (".github/copilot-instructions.md", CopilotInstructions),
            (".claude/rules/api.md", ClaudeRule),
            (".claude/rules/backend/db.md", ClaudeRule),
            (".claude/agents/reviewer.md", ClaudeSubagent),
            (".claude/commands/deploy.md", ClaudeCommand),
            (".claude/CLAUDE.md", ClaudeMd),
            (".cursor/rules/style.mdc", CursorRule),
            (".cursor/rules/frontend/react.mdc", CursorRule),
            (".cursorrules", Cursorrules),
            (".clinerules", Clinerules),
            (".clinerules/testing.md", Clinerules),
            (".windsurfrules", WindsurfRules),
            (".windsurf/rules/style.md", WindsurfRules),
            (".devin/rules/style.md", DevinRules),
            (".kiro/steering/product.md", KiroSteering),
            (".roo/rules/general.md", RooRules),
            (".roo/rules-code/style.md", RooRules),
            (".junie/rules/style.md", Junie),
            (".junie/AGENTS.md", Junie),
            (".claude/skills/deploy/SKILL.md", Skill),
            (".github/skills/deploy/SKILL.md", Skill),
            ("CLAUDE.md", ClaudeMd),
            ("CLAUDE.local.md", ClaudeMd),
            ("packages/api/CLAUDE.md", ClaudeMd),
            ("AGENTS.md", AgentsMd),
            ("AGENT.md", AgentsMd),
            ("GEMINI.md", GeminiMd),
        ];
        for (p, expected) in cases {
            assert_eq!(kind(p), expected, "{p}");
        }
    }

    #[test]
    fn unrecognised_markdown_is_generic() {
        for p in [
            "README.md",
            "docs/guide.md",
            ".claude/settings.json",
            ".github/workflows/ci.yml",
            "notes/skills.md",
            "src/main.rs",
        ] {
            assert_eq!(kind(p), FileKind::Generic, "{p}");
        }
    }

    #[test]
    fn nested_projects_classify() {
        assert_eq!(
            kind("packages/app/.claude/agents/reviewer.md"),
            FileKind::ClaudeSubagent
        );
        assert_eq!(
            kind("services/api/.cursor/rules/db.mdc"),
            FileKind::CursorRule
        );
        assert_eq!(
            kind("tools/.github/copilot-instructions.md"),
            FileKind::CopilotInstructions
        );
    }

    #[test]
    fn matching_is_case_insensitive_and_separator_agnostic() {
        assert_eq!(kind("claude.md"), FileKind::ClaudeMd);
        assert_eq!(kind(".Claude/Agents/Reviewer.MD"), FileKind::ClaudeSubagent);
        assert_eq!(
            classify(Path::new(".claude").join("agents").join("r.md").as_path()),
            FileKind::ClaudeSubagent
        );
    }

    #[test]
    fn path_rows_win_over_name_rows() {
        // A CLAUDE.md inside the agents directory is a subagent by location.
        assert_eq!(kind(".claude/agents/CLAUDE.md"), FileKind::ClaudeSubagent);
        // Copilot's per-path instruction beats the generic markdown extension.
        assert_eq!(
            kind(".github/instructions/AGENTS.instructions.md"),
            FileKind::CopilotInstruction
        );
    }

    #[test]
    fn component_kinds_are_the_short_scoped_files() {
        assert!(FileKind::ClaudeSubagent.is_component());
        assert!(FileKind::Skill.is_component());
        assert!(FileKind::CursorRule.is_component());
        assert!(!FileKind::ClaudeMd.is_component());
        assert!(!FileKind::AgentsMd.is_component());
        assert!(!FileKind::CopilotInstructions.is_component());
        assert!(!FileKind::Generic.is_component());
    }

    #[test]
    fn slugs_are_unique() {
        use FileKind::*;
        let all = [
            Generic,
            ClaudeMd,
            AgentsMd,
            GeminiMd,
            ClaudeRule,
            ClaudeSubagent,
            ClaudeCommand,
            Skill,
            CursorRule,
            Cursorrules,
            Clinerules,
            WindsurfRules,
            DevinRules,
            KiroSteering,
            RooRules,
            Junie,
            CopilotInstructions,
            CopilotInstruction,
            CopilotAgent,
            CopilotPrompt,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in all {
            assert!(seen.insert(k.slug()), "duplicate slug {}", k.slug());
        }
    }
}

#[cfg(test)]
mod helper_tests {
    use super::FileKind;
    use crate::checkers::utils::test_helpers::file_ctx_at;

    #[test]
    fn test_context_classifies_by_relative_path() {
        let (_dir, ctx) = file_ctx_at(".claude/agents/reviewer.md", &["# Reviewer"]);
        assert_eq!(ctx.files[0].kind, FileKind::ClaudeSubagent);
        let (_dir, ctx) = file_ctx_at("docs/notes.md", &["# Notes"]);
        assert_eq!(ctx.files[0].kind, FileKind::Generic);
    }
}
