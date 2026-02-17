pub const AVAILABLE_RULES: &[(&str, &str)] = &[
    (
        "dead-reference",
        "Flags .md references to files that don't exist",
    ),
    (
        "vague-directive",
        "Detects non-deterministic language in instructions",
    ),
    (
        "naming-inconsistency",
        "Same concept named differently across files",
    ),
    (
        "enum-drift",
        "Tables with matching columns but divergent values",
    ),
    (
        "agent-guidelines",
        "Best-practice violations in agent instructions",
    ),
    (
        "placeholder-text",
        "Detects leftover placeholders like [TODO], [TBD], etc.",
    ),
    (
        "file-size",
        "Warns when instruction files exceed recommended length",
    ),
    (
        "credential-exposure",
        "Detects hardcoded secrets and API keys",
    ),
    (
        "heading-hierarchy",
        "Detects skipped heading levels in markdown",
    ),
    (
        "dangerous-command",
        "Flags dangerous shell/SQL commands in code blocks",
    ),
    (
        "stale-reference",
        "Detects time-sensitive conditional logic that becomes stale",
    ),
    (
        "emoji-density",
        "Flags excessive emoji usage that adds noise for agents",
    ),
    (
        "session-journal",
        "Detects session logs masquerading as instruction files",
    ),
    (
        "missing-essential-sections",
        "Flags files lacking build/test commands or setup sections",
    ),
    (
        "prompt-injection-vector",
        "Detects patterns that could be prompt injection attacks",
    ),
    (
        "missing-verification",
        "Flags action sections without verification or success criteria",
    ),
    (
        "negative-only-framing",
        "Flags files where 75%+ of directives are negative",
    ),
    ("custom", "User-defined regex patterns from config"),
];

pub fn list_rules() -> String {
    use std::fmt::Write;
    let mut out = String::from("Available rules:\n\n");
    for (name, desc) in AVAILABLE_RULES {
        let _ = writeln!(out, "  {name:<24} {desc}");
    }
    out.push_str("\nRun `spectralint explain <rule>` for details.");
    out
}

pub fn explain(rule: &str) -> Option<&'static str> {
    match rule {
        "dead-reference" => Some(
            "dead-reference: Flags .md file references that point to files not on disk.\n\
             \n\
             When an agent instruction file says `load agent_definitions/scout.md` but that file\n\
             has been renamed or deleted, the agent silently skips it. There's no error — the agent\n\
             just operates with incomplete instructions. This checker catches those broken links\n\
             before they reach the agent.\n\
             \n\
             Severity: error\n\
             Skipped for: historical files (changelogs, retros)\n\
             Config: [checkers.dead_reference]",
        ),
        "vague-directive" => Some(
            "vague-directive: Detects non-deterministic language in agent instructions.\n\
             \n\
             Phrases like \"try to\", \"use your judgment\", \"if appropriate\", and \"as appropriate\"\n\
             give agents wiggle room that produces inconsistent behavior across runs. An instruction\n\
             that says \"try to cache results\" will sometimes cache and sometimes not,\n\
             depending on the model's interpretation. Replace vague language with deterministic\n\
             rules: \"cache all GET responses for 60 seconds.\"\n\
             \n\
             Strict mode (strict = true) additionally flags borderline hedging phrases:\n\
             \"when possible\", \"when needed\", \"as needed\", \"when necessary\", \"consider\".\n\
             These are normal in English prose but can introduce ambiguity for agents\n\
             that interpret instructions literally.\n\
             \n\
             Severity: info\n\
             Config: [checkers.vague_directive] (strict, extra_patterns)",
        ),
        "naming-inconsistency" => Some(
            "naming-inconsistency: Detects the same concept named differently across files.\n\
             \n\
             LLMs treat `api_key` and `apiKey` as two different concepts. When one instruction file\n\
             uses snake_case and another uses camelCase for the same field, the agent builds a\n\
             fragmented mental model — it may read the value from one file but fail to apply it\n\
             where the other name is used. This checker normalizes identifiers and flags mismatches\n\
             using Jaro-Winkler similarity (0.92 threshold).\n\
             \n\
             Severity: warning\n\
             Config: [checkers.naming_inconsistency]",
        ),
        "enum-drift" => Some(
            "enum-drift: Finds tables with matching columns but divergent value sets.\n\
             \n\
             When CLAUDE.md defines a Status column with {active, pending} and AGENTS.md defines\n\
             the same column with {active, archived}, the agent sees two conflicting sources of\n\
             truth. It may invent a merged set, drop values, or flip-flop between definitions.\n\
             This checker compares table columns across files and flags value-set mismatches.\n\
             \n\
             Severity: warning\n\
             Skipped for: historical files (changelogs, retros)\n\
             Config: [checkers.enum_drift]",
        ),
        "agent-guidelines" => Some(
            "agent-guidelines: Flags best-practice violations in agent instruction files.\n\
             \n\
             Four sub-checks based on common failure patterns:\n\
             \n\
             1. Missing negative constraints — File has \"Always\" and \"Must\" but no \"Never\" or\n\
                \"Do not\". Agents without boundaries over-apply rules or take unintended actions.\n\
                Good instructions define what NOT to do, not just what to do.\n\
             \n\
             2. Multi-responsibility — File covers 4+ distinct areas (build, test, deploy,\n\
                security, etc.) in section headings. Mixed responsibilities produce muddy feedback.\n\
                Split into focused single-responsibility agent files.\n\
             \n\
             3. Unconstrained delegation — Phrases like \"do whatever\", \"figure it out\", \"use\n\
                your best judgment\" give agents open-ended autonomy without guardrails. Agents\n\
                with unconstrained delegation make unpredictable choices.\n\
             \n\
             4. Missing output format — No mention of output, format, return, or response\n\
                structure. Without format constraints, agents choose their own structure, making\n\
                downstream parsing unreliable.\n\
             \n\
             Severity: info\n\
             Config: [checkers.agent_guidelines]",
        ),
        "placeholder-text" => Some(
            "placeholder-text: Detects leftover placeholders in instruction files.\n\
             \n\
             Patterns like [TODO], [TBD], [FIXME], [insert here], \"etc.\", \"and so on\", and\n\
             trailing ellipsis (...) indicate unfinished content. Agents interpret placeholders\n\
             literally or skip them entirely, leading to incomplete behavior. Replace every\n\
             placeholder with actual, specific content before the file reaches an agent.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.placeholder_text]",
        ),
        "file-size" => Some(
            "file-size: Warns when instruction files exceed recommended length.\n\
             \n\
             LLMs suffer from \"lost in the middle\" degradation — instructions buried in the\n\
             middle of a long file are more likely to be ignored or misapplied. At 300+ lines\n\
             this checker emits an info-level notice; at 500+ lines it emits a warning.\n\
             Split large files into focused sub-files and use file references for progressive\n\
             disclosure.\n\
             \n\
             Severity: info at 300 lines, warning at 500 lines (configurable)\n\
             Config: [checkers.file_size] (max_lines, warn_lines)",
        ),
        "credential-exposure" => Some(
            "credential-exposure: Detects hardcoded secrets in instruction files.\n\
             \n\
             API keys, tokens, passwords, and other credentials should never appear in agent\n\
             instruction files. These files are often committed to version control, shared\n\
             across teams, and read by AI agents that may echo them in output. This checker\n\
             scans all lines (including code blocks) for common credential patterns: API keys,\n\
             AWS access keys, GitHub tokens, Slack tokens, JWTs, and Bearer tokens.\n\
             \n\
             Severity: error\n\
             Config: [checkers.credential_exposure]",
        ),
        "heading-hierarchy" => Some(
            "heading-hierarchy: Detects skipped heading levels in markdown.\n\
             \n\
             Jumping from # (h1) directly to ### (h3) without an intermediate ## (h2) breaks\n\
             the document's logical structure. LLMs use heading hierarchy to understand section\n\
             relationships and scope. Skipped levels can cause agents to misinterpret which\n\
             instructions belong to which section.\n\
             \n\
             Severity: info\n\
             Config: [checkers.heading_hierarchy]",
        ),
        "dangerous-command" => Some(
            "dangerous-command: Flags dangerous shell/SQL commands in code blocks.\n\
             \n\
             Commands like `rm -rf`, `git push --force`, `git reset --hard`, `DROP TABLE`,\n\
             `TRUNCATE TABLE`, and `--no-verify` in code blocks may be executed by agents\n\
             without human confirmation. This checker only scans inside fenced code blocks\n\
             (where executable commands live) and ignores prose mentions. Add confirmation\n\
             steps or restrict when these commands may be used.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.dangerous_command]",
        ),
        "stale-reference" => Some(
            "stale-reference: Detects time-sensitive conditional logic that becomes stale.\n\
             \n\
             Instructions like \"After March 2025, use the new API\" or \"deprecated since v3\"\n\
             create time bombs — they were correct when written but become confusing or wrong\n\
             as time passes. Agents follow stale conditional logic literally, sometimes using\n\
             outdated approaches. Replace time-sensitive instructions with permanent ones:\n\
             instead of \"After March 2025, use v2\" just say \"Use v2\".\n\
             \n\
             Severity: warning\n\
             Config: [checkers.stale_reference]",
        ),
        "emoji-density" => Some(
            "emoji-density: Flags excessive emoji usage in instruction files.\n\
             \n\
             Emoji like 🚀, ✅, 📊 are visual decorations designed for human readers. Agents\n\
             process them as tokens but gain no instruction value. A file with 10+ emoji is\n\
             likely styled for human presentation rather than optimized for agent consumption.\n\
             Each emoji wastes context window tokens that could carry actual instructions.\n\
             \n\
             Severity: info\n\
             Config: [checkers.emoji_density] (max_emoji, default: 10)",
        ),
        "session-journal" => Some(
            "session-journal: Detects session logs masquerading as instruction files.\n\
             \n\
             A common antipattern: Claude writes a summary of what it accomplished during a\n\
             session, and that summary becomes the permanent CLAUDE.md. The result is a file\n\
             full of \"What We Accomplished\", \"Files Modified\", and ✅ checkmarks — a changelog,\n\
             not instructions. Agents reading this file get historical context instead of\n\
             actionable directives.\n\
             \n\
             The checker requires 3+ co-occurring markers (retrospective headings, file\n\
             changelogs, session references, PR status, high checkmark density) to avoid\n\
             false positives on files that legitimately mention one of these patterns.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.session_journal]",
        ),
        "missing-essential-sections" => Some(
            "missing-essential-sections: Flags files lacking build/test commands.\n\
             \n\
             The #1 most impactful content in agent instruction files is concrete build/test\n\
             commands. Without them, agents cannot verify their own work. This checker looks for\n\
             three signals (any one is enough to pass): code blocks with command patterns (cargo,\n\
             npm, pytest, make, etc.), section headings matching Commands/Build/Test/Setup, or\n\
             inline backtick commands. If none are found, it emits at line 1.\n\
             \n\
             Severity: info\n\
             Config: [checkers.missing_essential_sections] (min_lines, default: 10)",
        ),
        "prompt-injection-vector" => Some(
            "prompt-injection-vector: Detects patterns that could be prompt injection attacks.\n\
             \n\
             Four sub-checks:\n\
             \n\
             1. Social engineering (Warning) — Phrases like \"ignore previous instructions\",\n\
                \"you are now\", \"forget everything\" that attempt to override agent behavior.\n\
             \n\
             2. Base64 payloads (Info) — Base64 strings > 50 chars that could contain hidden\n\
                instructions. Skips lines mentioning sha/hash/checksum.\n\
             \n\
             3. Invisible Unicode (Warning) — Zero-width characters and other invisible control\n\
                characters that could hide injected text.\n\
             \n\
             4. Hidden HTML instructions (Info) — HTML comments containing suspicious keywords\n\
                (ignore, override, forget, system, prompt). Excludes spectralint comments.\n\
             \n\
             Severity: warning for social engineering and invisible unicode, info for others\n\
             Config: [checkers.prompt_injection_vector]",
        ),
        "missing-verification" => Some(
            "missing-verification: Flags action sections without verification criteria.\n\
             \n\
             Sections with 2+ action directives (run, execute, create, build, deploy, etc.)\n\
             but no verification signals (verify, test, assert, expected output, \"should see\")\n\
             leave agents with no way to confirm success. Adding verification steps — expected\n\
             output, test commands, or success criteria — makes instructions self-validating.\n\
             \n\
             Severity: info\n\
             Config: [checkers.missing_verification] (min_action_verbs, default: 2)",
        ),
        "negative-only-framing" => Some(
            "negative-only-framing: Flags files where 75%+ of directives are negative.\n\
             \n\
             Files dominated by \"Don't\", \"Never\", and \"Avoid\" tell agents what NOT to do\n\
             but give no clear path forward. Agents without positive guidance (Always/Use/Run/\n\
             Follow) tend to become paralyzed or overly conservative. A healthy instruction\n\
             file balances constraints with actionable directives.\n\
             \n\
             Fires when: negative_count >= 5 AND negative/(positive+negative) >= 0.75\n\
             \n\
             Severity: info\n\
             Config: [checkers.negative_only_framing] (threshold, min_negative_count)",
        ),
        "custom" => Some(
            "custom:<name>: User-defined regex patterns from config.\n\
             \n\
             Define your own lint rules in .spectralintrc.toml without writing Rust:\n\
             \n\
             [[checkers.custom_patterns]]\n\
             name = \"todo-comment\"\n\
             pattern = \"(?i)\\\\bTODO\\\\b\"\n\
             severity = \"warning\"\n\
             message = \"TODO comment found\"\n\
             \n\
             Each pattern is scanned against non-code-block lines. Useful for project-specific\n\
             conventions, banned terms, or required markers.\n\
             \n\
             Severity: configurable (default: warning)\n\
             Config: [[checkers.custom_patterns]]",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_rules_have_explanations() {
        for (rule, _) in AVAILABLE_RULES {
            assert!(
                explain(rule).is_some(),
                "Rule '{rule}' should have an explanation"
            );
        }
    }

    #[test]
    fn test_unknown_rule_returns_none() {
        assert!(explain("nonexistent-rule").is_none());
    }

    #[test]
    fn test_explanations_contain_severity() {
        for (rule, _) in AVAILABLE_RULES {
            let text = explain(rule).unwrap();
            assert!(
                text.contains("Severity:"),
                "Explanation for '{rule}' should mention severity"
            );
        }
    }

    #[test]
    fn test_list_rules_contains_all() {
        let listing = list_rules();
        for (rule, desc) in AVAILABLE_RULES {
            assert!(listing.contains(rule), "Listing should contain {rule}");
            assert!(
                listing.contains(desc),
                "Listing should contain description for {rule}"
            );
        }
    }
}
