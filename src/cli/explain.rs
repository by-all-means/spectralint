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
             Phrases like \"try to\", \"when possible\", \"use your judgment\", and \"if appropriate\"\n\
             give agents wiggle room that produces inconsistent behavior across runs. An instruction\n\
             that says \"cache results when possible\" will sometimes cache and sometimes not,\n\
             depending on the model's interpretation. Replace vague language with deterministic\n\
             rules: \"cache all GET responses for 60 seconds.\"\n\
             \n\
             Severity: info\n\
             Config: [checkers.vague_directive]",
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
