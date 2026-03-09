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
        "Same concept named differently within or across files",
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
        "Detects leftover placeholders like TODO, TBD, FIXME, etc.",
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
        "Flags files where 65%+ of directives are negative",
    ),
    (
        "conflicting-directives",
        "Detects contradictory instructions in the same file",
    ),
    (
        "missing-role-definition",
        "Flags files without a \"You are...\" or Role section",
    ),
    (
        "redundant-directive",
        "Detects near-duplicate directive lines via similarity",
    ),
    (
        "instruction-density",
        "Flags sections with excessive consecutive bullet points",
    ),
    (
        "missing-examples",
        "Flags format specs without accompanying code examples",
    ),
    (
        "unbounded-scope",
        "Detects capability grants without boundary constraints",
    ),
    (
        "circular-reference",
        "Detects circular file reference chains between instruction files",
    ),
    (
        "large-code-block",
        "Flags inline code blocks exceeding a configurable threshold",
    ),
    (
        "duplicate-section",
        "Flags repeated section headings within a file",
    ),
    (
        "absolute-path",
        "Flags hardcoded personal paths that break portability",
    ),
    (
        "generic-instruction",
        "Flags meaningless instructions the model already knows",
    ),
    ("misordered-steps", "Flags out-of-order numbered steps"),
    (
        "section-length-imbalance",
        "Flags disproportionately long sections relative to siblings",
    ),
    ("unclosed-fence", "Flags code fences that are never closed"),
    (
        "untagged-code-block",
        "Flags code fences without a language tag",
    ),
    (
        "duplicate-instruction-file",
        "Flags near-duplicate instruction files",
    ),
    (
        "outdated-model-reference",
        "Flags references to deprecated or old model names",
    ),
    ("broken-table", "Flags malformed markdown tables"),
    ("placeholder-url", "Flags placeholder/example URLs in prose"),
    (
        "emphasis-overuse",
        "Flags files with excessive emphasis markers creating alert fatigue",
    ),
    (
        "boilerplate-template",
        "Flags unchanged default template instruction files",
    ),
    (
        "generated-attribution",
        "Flags AI-tool attribution lines that waste context tokens",
    ),
    (
        "command-without-codeblock",
        "Flags bare shell commands not wrapped in code blocks or backticks",
    ),
    (
        "missing-verification-step",
        "Flags files with workflow steps but no verification or test command",
    ),
    (
        "broken-anchor-link",
        "Flags in-file anchor links that don't match any heading",
    ),
    (
        "long-paragraph",
        "Flags dense text blocks that are hard for agents to parse",
    ),
    (
        "hardcoded-windows-path",
        "Flags backslash file paths that break on non-Windows systems",
    ),
    (
        "orphaned-section",
        "Flags headings with no content before the next heading",
    ),
    (
        "excessive-nesting",
        "Flags lists nested too deeply for agents to parse",
    ),
    (
        "context-window-waste",
        "Flags decorative elements that waste context window tokens",
    ),
    (
        "ambiguous-scope-reference",
        "Flags vague scope references like 'the relevant files'",
    ),
    (
        "instruction-without-context",
        "Flags instruction files with no code blocks, file refs, or inline code",
    ),
    (
        "cross-file-contradiction",
        "Detects contradictory instructions across ancestor-descendant files",
    ),
    (
        "stale-style-rule",
        "Flags formatter-enforceable style prescriptions that waste context tokens",
    ),
    (
        "hardcoded-file-structure",
        "Flags references to non-.md source files that don't exist on disk",
    ),
    (
        "unversioned-stack-reference",
        "Flags tech stack mentions without version numbers",
    ),
    (
        "missing-standard-file",
        "Flags projects missing common instruction files like CLAUDE.md",
    ),
    (
        "bare-url",
        "Flags raw URLs not wrapped in markdown link syntax",
    ),
    (
        "repeated-word",
        "Flags accidental consecutive duplicate words",
    ),
    (
        "undocumented-env-var",
        "Flags env var references without nearby explanation",
    ),
    ("empty-code-block", "Flags code blocks with no content"),
    (
        "click-here-link",
        "Flags opaque link text like [click here](url)",
    ),
    (
        "double-negation",
        "Flags double negatives that confuse agents",
    ),
    (
        "imperative-heading",
        "Flags headings that contain instructions instead of topics",
    ),
    (
        "inconsistent-command-prefix",
        "Flags mixed $ prefix styles in shell code blocks",
    ),
    ("empty-heading", "Flags headings with no title text"),
    (
        "copied-meta-instructions",
        "Flags AI boilerplate like 'You are a helpful assistant'",
    ),
    (
        "xml-document-wrapper",
        "Flags XML declarations and wrapper tags in markdown",
    ),
    (
        "stale-file-tree",
        "Flags ASCII directory trees containing paths that don't exist on disk",
    ),
    (
        "command-validation",
        "Flags build/test commands whose toolchain prerequisites are missing",
    ),
    (
        "token-budget",
        "Estimates token cost of instruction files and flags context window overuse",
    ),
    (
        "invalid-suppression",
        "Warns on unrecognized rule names in suppress comments",
    ),
    (
        "unused-suppression",
        "Reports suppress comments that didn't suppress any diagnostic",
    ),
    ("custom", "User-defined regex patterns from config"),
];

#[must_use]
pub fn list_rules() -> String {
    use std::fmt::Write;
    let mut out = String::from("Available rules:\n\n");
    for (name, desc) in AVAILABLE_RULES {
        let _ = writeln!(out, "  {name:<24} {desc}");
    }
    out.push_str("\nRun `spectralint explain <rule>` for details.");
    out
}

#[must_use]
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
            "naming-inconsistency: Detects the same concept named differently within or across files.\n\
             \n\
             LLMs treat `api_key` and `apiKey` as two different concepts. When one instruction file\n\
             uses snake_case and another uses camelCase for the same field, the agent builds a\n\
             fragmented mental model — it may read the value from one file but fail to apply it\n\
             where the other name is used. This checker normalizes identifiers and flags mismatches\n\
             using Jaro-Winkler similarity (0.95 threshold).\n\
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
             2. Multi-responsibility — File covers 6+ distinct areas (build, test, deploy,\n\
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
             Patterns like TODO, TBD, FIXME, [insert here], \"etc.\", \"and so on\", and\n\
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
             middle of a long file are more likely to be ignored or misapplied. At 500+ lines\n\
             this checker emits an info-level notice; at 750+ lines it emits a warning.\n\
             Split large files into focused sub-files and use file references for progressive\n\
             disclosure.\n\
             \n\
             Severity: info at 500 lines, warning at 750 lines (configurable)\n\
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
             process them as tokens but gain no instruction value. A file with 20+ emoji is\n\
             likely styled for human presentation rather than optimized for agent consumption.\n\
             Each emoji wastes context window tokens that could carry actual instructions.\n\
             \n\
             Severity: info\n\
             Config: [checkers.emoji_density] (max_emoji, default: 20)",
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
             Sections with 4+ action directives (run, execute, create, build, deploy, etc.)\n\
             but no verification signals (verify, test, assert, expected output, \"should see\")\n\
             leave agents with no way to confirm success. Adding verification steps — expected\n\
             output, test commands, or success criteria — makes instructions self-validating.\n\
             \n\
             Severity: info\n\
             Config: [checkers.missing_verification] (min_action_verbs, default: 4)",
        ),
        "negative-only-framing" => Some(
            "negative-only-framing: Flags files where 65%+ of directives are negative.\n\
             \n\
             Files dominated by \"Don't\", \"Never\", and \"Avoid\" tell agents what NOT to do\n\
             but give no clear path forward. Agents without positive guidance (Always/Use/Run/\n\
             Follow) tend to become paralyzed or overly conservative. A healthy instruction\n\
             file balances constraints with actionable directives.\n\
             \n\
             Fires when: negative_count >= 3 AND negative/(positive+negative) >= 0.65\n\
             \n\
             Severity: info\n\
             Config: [checkers.negative_only_framing] (threshold, min_negative_count)",
        ),
        "conflicting-directives" => Some(
            "conflicting-directives: Detects contradictory instructions in the same file.\n\
             \n\
             When a file says \"always use formal tone\" and also \"keep it casual\", the agent\n\
             receives mutually exclusive instructions. It may follow one, alternate between both,\n\
             or produce confused output. This checker defines ~14 contradiction pairs covering tone,\n\
             API usage, file creation, confirmation, verbosity, resource modification, testing,\n\
             comments, dependencies, error handling, autonomy, commits, complexity, and git workflow.\n\
             When both members of a pair match on different lines, it emits.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.conflicting_directives]",
        ),
        "missing-role-definition" => Some(
            "missing-role-definition: Flags files without a role/identity definition.\n\
             \n\
             Files with 15+ directive lines benefit from an explicit identity statement like\n\
             \"You are a senior Rust developer\" or a dedicated ## Role / Mission section.\n\
             Without one, the agent has no persona to anchor its behavior — it may adopt\n\
             different voices across interactions. Files in commands/, skills/, or tasks/\n\
             subdirectories are excluded, as are deeply nested files (depth > 2 from root)\n\
             since they're typically context injections, not standalone agent definitions.\n\
             \n\
             Severity: info\n\
             Config: [checkers.missing_role_definition]",
        ),
        "redundant-directive" => Some(
            "redundant-directive: Detects near-duplicate directive lines via Jaro-Winkler similarity.\n\
             \n\
             Copy-paste or incremental editing can leave two lines that say almost the same thing:\n\
             \"Always run the test suite before committing code\" and \"Always run the test suite\n\
             before committing changes.\" The agent processes both, wasting context window tokens\n\
             on redundant information. This checker normalizes lines (strips list markers, lowercases,\n\
             collapses whitespace) and flags pairs with >=95% Jaro-Winkler similarity.\n\
             Lines shorter than 15 characters and headings are skipped. Capped at 200 directive\n\
             lines per file for performance.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.redundant_directive] (similarity_threshold, min_line_length)",
        ),
        "instruction-density" => Some(
            "instruction-density: Flags sections with excessive consecutive bullet points.\n\
             \n\
             A wall of 15+ bullet points without structural breaks (blank lines, subheadings,\n\
             code examples) overwhelms agents. Studies on LLM instruction following show that\n\
             compliance drops in dense, undifferentiated lists. Breaking long lists into smaller\n\
             groups with headings or whitespace improves adherence.\n\
             \n\
             Only fires on files with 2+ sections to avoid flagging simple list-only files.\n\
             \n\
             Severity: info\n\
             Config: [checkers.instruction_density] (max_consecutive_bullets, default: 15)",
        ),
        "missing-examples" => Some(
            "missing-examples: Flags format specifications without accompanying code examples.\n\
             \n\
             When a section says \"format as JSON\" or \"output must be YAML\" but provides no\n\
             concrete example, the agent must guess the exact shape. Different models may produce\n\
             different structures. A single code block showing the target format eliminates\n\
             ambiguity. The checker also accepts \"e.g.\", \"for example\" inline signals, and\n\
             sibling sections titled \"Example\" or \"Sample\".\n\
             \n\
             Severity: info\n\
             Config: [checkers.missing_examples]",
        ),
        "unbounded-scope" => Some(
            "unbounded-scope: Detects capability grants without boundary constraints.\n\
             \n\
             Files that say \"you can modify any files\" or \"full write access\" without any\n\
             refusal conditions (\"never modify...\", \"out of scope\", \"ask for confirmation\")\n\
             create agents with unlimited autonomy. Unbounded agents are unpredictable — they\n\
             may delete critical files, modify production configs, or execute destructive\n\
             commands. Every capability grant should be paired with explicit boundaries.\n\
             \n\
             Only fires on files with 5+ directive lines to avoid flagging minimal configs.\n\
             \n\
             Severity: info\n\
             Config: [checkers.unbounded_scope]",
        ),
        "circular-reference" => Some(
            "circular-reference: Detects circular file reference chains between instruction files.\n\
             \n\
             When file A references file B, and file B references file A, the agent encounters\n\
             an infinite loop in its instruction graph. More complex cycles (A → B → C → A) are\n\
             equally problematic. This checker builds a directed graph from all file_refs, resolves\n\
             paths the same way dead-reference does, and runs DFS cycle detection.\n\
             \n\
             Template/glob refs (*, [, {, <, ~, $, path/to/) are skipped, and references that\n\
             don't resolve to any scanned file are ignored (those are dead-reference's job).\n\
             \n\
             Severity: warning\n\
             Config: [checkers.circular_reference]",
        ),
        "large-code-block" => Some(
            "large-code-block: Flags inline code blocks exceeding a line threshold.\n\
             \n\
             Code blocks longer than 40 lines in instruction files waste context window tokens\n\
             and make the file harder to maintain. Long code examples should be extracted into\n\
             separate files and referenced instead, keeping instruction files focused on directives.\n\
             \n\
             The checker counts lines between ``` fences and emits when the count exceeds the\n\
             configurable threshold (default: 40).\n\
             \n\
             Severity: info\n\
             Config: [checkers.large_code_block] (max_lines, default: 40)",
        ),
        "duplicate-section" => Some(
            "duplicate-section: Flags repeated section headings within a file.\n\
             \n\
             When the same heading appears twice at the same level (e.g., two `## Testing`\n\
             sections), agents may only process one or conflate their contents. This typically\n\
             happens from copy-paste or incremental editing. The checker case-normalizes titles\n\
             and includes heading level in the comparison, so `## Testing` and `### Testing`\n\
             are treated as distinct.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.duplicate_section]",
        ),
        "absolute-path" => Some(
            "absolute-path: Flags hardcoded personal paths that break portability.\n\
             \n\
             Paths like `/home/john/project`, `/Users/alice/dev`, `C:\\Users\\Bob\\`, and\n\
             `~/Documents` are tied to a specific machine. When instruction files contain these\n\
             paths, they break for every other developer or CI environment. Agents following\n\
             hardcoded paths will fail silently or create files in non-existent directories.\n\
             \n\
             System paths (`/etc/`, `/usr/`, `/tmp/`, `/var/`, `/opt/`, etc.) are excluded\n\
             since they're legitimate cross-machine references.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.absolute_path]",
        ),
        "generic-instruction" => Some(
            "generic-instruction: Flags meaningless instructions the model already knows.\n\
             \n\
             Phrases like \"follow best practices\", \"write clean code\", and \"think step by step\"\n\
             waste context window tokens without adding actionable information. The model already\n\
             knows to write clean code — what it needs is *your* project's specific definition of\n\
             clean: naming conventions, error handling patterns, test requirements.\n\
             \n\
             Lines with elaboration (followed by `:` or `—`) are excluded, since the generic\n\
             phrase is being used as a lead-in to specific guidance.\n\
             \n\
             Severity: info\n\
             Config: [checkers.generic_instruction]",
        ),
        "misordered-steps" => Some(
            "misordered-steps: Flags out-of-order numbered steps.\n\
             \n\
             When instructions say \"Step 1... Step 3... Step 2\", agents execute in document\n\
             order regardless of numbering. The mismatch between numbered order and document\n\
             order creates confusion — a human reader expects step 2 before step 3, but the\n\
             agent processes them as written. The checker tracks step numbers per section and\n\
             flags when a lower number follows a higher one.\n\
             \n\
             Step 1 resets the sequence (allows re-enumeration within a section).\n\
             Each heading starts a fresh tracking context.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.misordered_steps]",
        ),
        "section-length-imbalance" => Some(
            "section-length-imbalance: Flags disproportionately long sections.\n\
             \n\
             When one `##` section is 80 lines and its siblings are 10 lines each, the long\n\
             section dominates the file and likely covers too much ground. Agents may lose focus\n\
             on instructions buried deep in an oversized section. The checker compares sibling\n\
             sections at the same heading level and flags outliers that exceed the median by a\n\
             configurable ratio (default: 4x) and minimum line count (default: 50).\n\
             \n\
             Requires at least 3 sibling sections to compute meaningful statistics.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.section_length_imbalance] (min_section_lines, imbalance_ratio)",
        ),
        "unclosed-fence" => Some(
            "unclosed-fence: Flags code fences that are never closed.\n\
             \n\
             An opening ``` fence without a matching closing fence causes everything below\n\
             it to be treated as code. Agents parsing the file will miss all instructions\n\
             after the unclosed fence, silently dropping critical directives.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.unclosed_fence]",
        ),
        "untagged-code-block" => Some(
            "untagged-code-block: Flags code fences without a language tag.\n\
             \n\
             Bare ``` fences without a language tag (e.g., ```bash, ```json) make it harder\n\
             for agents to parse and interpret the block correctly. Adding a language tag\n\
             provides semantic context that improves code extraction and execution.\n\
             \n\
             Only flags blocks with 2+ content lines (one-liners often don't need a tag).\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.untagged_code_block]",
        ),
        "duplicate-instruction-file" => Some(
            "duplicate-instruction-file: Flags near-duplicate instruction files.\n\
             \n\
             When two instruction files share 70%+ of their directive lines, they likely\n\
             represent the same instructions maintained in two places. This creates a\n\
             consistency burden — edits to one file may not be reflected in the other,\n\
             leading to divergent agent behavior depending on which file is loaded.\n\
             \n\
             Consolidate into one file or split responsibilities clearly.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.duplicate_instruction_file]",
        ),
        "outdated-model-reference" => Some(
            "outdated-model-reference: Flags references to deprecated or old model names.\n\
             \n\
             References to GPT-3.5, GPT-4 Turbo, Claude 2, Claude Instant, Claude 3 Haiku/\n\
             Sonnet/Opus, text-davinci, or code-davinci point to models that have been\n\
             superseded. Agents following these references may use deprecated API endpoints\n\
             or make incorrect capability assumptions.\n\
             \n\
             Lines containing \"history\", \"changelog\", or \"deprecated\" are excluded, as are\n\
             headings and lines inside code blocks.\n\
             \n\
             Severity: info\n\
             Config: [checkers.outdated_model_reference]",
        ),
        "broken-table" => Some(
            "broken-table: Flags malformed markdown tables.\n\
             \n\
             Two sub-checks:\n\
             \n\
             1. Column count mismatch — A data row has a different number of columns than\n\
                the header row. Agents parsing the table may misalign values or skip rows.\n\
             \n\
             2. Missing separator row — A table header is followed by data rows without a\n\
                |---|---| separator. Without the separator, markdown parsers don't recognize\n\
                the block as a table at all.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.broken_table]",
        ),
        "placeholder-url" => Some(
            "placeholder-url: Flags placeholder/example URLs in prose.\n\
             \n\
             URLs like https://example.com, http://localhost:3000, https://your-api.com, and\n\
             template URLs with {placeholders} in non-code prose indicate unfinished content.\n\
             Agents may attempt to call these endpoints, resulting in failures or unintended\n\
             network requests. Replace with actual endpoints or remove.\n\
             \n\
             URLs inside code blocks are excluded (code examples legitimately use example.com).\n\
             \n\
             Severity: info\n\
             Config: [checkers.placeholder_url]",
        ),
        "emphasis-overuse" => Some(
            "emphasis-overuse: Flags files with excessive emphasis markers.\n\
             \n\
             Files with 10+ IMPORTANT/CRITICAL/WARNING/CAUTION/NOTE markers create alert\n\
             fatigue — agents can't prioritize when everything screams for attention. Bold\n\
             markers (**IMPORTANT**) and standalone all-caps markers (IMPORTANT:) are both\n\
             counted. Markers inside code blocks and headings are excluded.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.emphasis_overuse] (max_emphasis, default: 10)",
        ),
        "boilerplate-template" => Some(
            "boilerplate-template: Flags unchanged default template instruction files.\n\
             \n\
             54% of Claude Code repos use the unchanged default template (\"This file provides\n\
             guidance to Claude Code\"). These provide minimal agent value and create a false\n\
             sense of having project-specific instructions. Only flags files with < 20 non-empty\n\
             lines — if substantial content was added on top of the template, that's fine.\n\
             \n\
             Severity: info\n\
             Config: [checkers.boilerplate_template]",
        ),
        "broken-anchor-link" => Some(
            "broken-anchor-link: Flags in-file anchor links that don't match any heading.\n\
             \n\
             Links like `[see setup](#setup-guide)` resolve to a heading anchor within\n\
             the same file. If no heading generates the anchor `setup-guide`, the link\n\
             is broken — readers and agents following it will land nowhere.\n\
             \n\
             Anchor slugs are generated using GitHub-flavored markdown rules: lowercase,\n\
             spaces become hyphens, punctuation is stripped. The checker converts all\n\
             headings to slugs and validates every `[text](#anchor)` link against them.\n\
             \n\
             Links to external URLs and other files (`guide.md#section`) are ignored.\n\
             Links inside code blocks and inline code are also excluded.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.broken_anchor_link]",
        ),
        "long-paragraph" => Some(
            "long-paragraph: Flags dense text blocks that are hard for agents to parse.\n\
             \n\
             A paragraph of 8+ consecutive prose lines (outside code blocks, lists, and\n\
             headings) creates a wall of text that agents struggle to extract structured\n\
             information from. Breaking long paragraphs into shorter ones or using bullet\n\
             points improves parseability.\n\
             \n\
             The threshold is configurable via `max_lines` (default: 8). Blank lines,\n\
             headings, list items, blockquotes, tables, and code blocks all break\n\
             paragraph tracking.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.long_paragraph]\n\
             Options: max_lines = 8",
        ),
        "command-without-codeblock" => Some(
            "command-without-codeblock: Flags bare shell commands not wrapped in code blocks.\n\
             \n\
             When a command like `cargo test --release` or `npm install express` appears as\n\
             plain text instead of inside a fenced code block (```) or inline backticks (`),\n\
             it's harder for both humans and agents to identify and copy-paste.\n\
             \n\
             This checker detects lines that look like executable commands — starting with a\n\
             known binary (cargo, npm, git, docker, kubectl, pip, etc.) followed by arguments —\n\
             and are not inside code blocks or backticks. Prose sentences that mention commands\n\
             in passing are excluded via heuristics (sentence structure, word count, etc.).\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.command_without_codeblock]",
        ),
        "missing-verification-step" => Some(
            "missing-verification-step: Flags files with workflow steps but no verification.\n\
             \n\
             Instruction files that describe build, deploy, or setup workflows should include\n\
             at least one verification step — a test command, expected output, or success\n\
             criteria. Without verification, agents complete workflows without confirming\n\
             they succeeded.\n\
             \n\
             This is a file-level check (complementing the section-level missing-verification).\n\
             It fires when a file has 5+ workflow verbs (run, build, deploy, install, etc.)\n\
             but zero verification signals anywhere in the file — no test commands in code\n\
             blocks, no verify/check/ensure keywords, no \"should see\" phrases.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.missing_verification_step]",
        ),
        "generated-attribution" => Some(
            "generated-attribution: Flags AI-tool attribution lines that waste context tokens.\n\
             \n\
             Lines like \"Generated with Claude Code\", \"Built with Cursor\", or\n\
             \"Co-Authored-By: Claude\" are tool-attribution boilerplate. They commonly\n\
             appear when AI-generated content is pasted into instruction files, or when\n\
             commit-message footers leak into documentation.\n\
             \n\
             These lines provide zero value to agents parsing the file — they waste context\n\
             tokens and add noise. Remove them.\n\
             \n\
             Detected patterns include: \"Generated/Created/Built/Made/Written with/by/using\"\n\
             followed by known AI tools (Claude, Copilot, ChatGPT, GPT-4, Cursor, Windsurf,\n\
             Aider, Cody), plus \"Co-Authored-By\" lines referencing AI tools.\n\
             \n\
             Lines inside code blocks and inline code are excluded.\n\
             \n\
             Severity: info\n\
             Config: [checkers.generated_attribution]",
        ),
        "hardcoded-windows-path" => Some(
            "hardcoded-windows-path: Flags backslash file paths that break on non-Windows systems.\n\
             \n\
             Paths like scripts\\helper.py or src\\utils\\config.py use Windows-style backslash\n\
             separators that fail on macOS and Linux. Agent instructions should use forward slashes\n\
             for cross-platform compatibility.\n\
             \n\
             The checker excludes: absolute Windows paths (C:\\...) which are caught by\n\
             absolute-path, regex escape sequences (\\d, \\n, \\s, etc.), content inside\n\
             inline code or code blocks, and table rows.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.hardcoded_windows_path]",
        ),
        "orphaned-section" => Some(
            "orphaned-section: Flags headings with no content before the next heading.\n\
             \n\
             A heading followed by another heading of equal or higher level with no content\n\
             between them indicates unfinished structure. Agents see the heading but get no\n\
             instructions. Parent-to-child transitions (## → ###) are not flagged since that's\n\
             normal document hierarchy.\n\
             \n\
             Severity: info\n\
             Config: [checkers.orphaned_section]",
        ),
        "excessive-nesting" => Some(
            "excessive-nesting: Flags lists nested too deeply for agents to parse.\n\
             \n\
             Lists nested 4+ levels deep are hard for agents to parse correctly. Instructions\n\
             buried at deep indentation levels get lost. The checker counts indent depth of\n\
             list items and flags when nesting exceeds the configurable threshold.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.excessive_nesting] (max_depth, default: 4)",
        ),
        "context-window-waste" => Some(
            "context-window-waste: Flags decorative elements that waste context window tokens.\n\
             \n\
             Three sub-checks:\n\
             \n\
             1. Excessive blank lines — 3+ consecutive blank lines waste tokens. Reduce to one.\n\
             \n\
             2. Decorative dividers — Lines of repeated decoration characters (====, ****,\n\
                ~~~~, ════, ────, etc.) outside code blocks. Standard markdown HRs (---)\n\
                are excluded.\n\
             \n\
             3. Decorative HTML comments — Comments like <!-- ----- --> that are just\n\
                visual decoration, not suppress directives.\n\
             \n\
             Severity: info\n\
             Config: [checkers.context_window_waste]",
        ),
        "ambiguous-scope-reference" => Some(
            "ambiguous-scope-reference: Flags vague scope references in directives.\n\
             \n\
             Phrases like \"the relevant files\", \"appropriate tests\", \"necessary configuration\",\n\
             and \"related modules\" tell the agent to act on something without specifying what.\n\
             The agent must guess which files, tests, or modules are \"relevant\" — and different\n\
             models will guess differently. Replace with concrete references: specific file paths,\n\
             test commands, or module names.\n\
             \n\
             Lines containing inline code (backticks), file extensions, or elaboration after a\n\
             colon are excluded, since the ambiguous phrase is being disambiguated.\n\
             \n\
             Severity: info\n\
             Config: [checkers.ambiguous_scope_reference]",
        ),
        "instruction-without-context" => Some(
            "instruction-without-context: Flags instruction files with no concrete context.\n\
             \n\
             Files with 10+ directive lines but zero code blocks, zero file path references,\n\
             and zero inline code spans are entirely abstract — all prose, no specifics. Agents\n\
             following these files get general guidance but no anchoring in the actual codebase.\n\
             Add concrete examples: code blocks with commands, file paths, or inline code\n\
             references to make instructions actionable.\n\
             \n\
             Severity: info\n\
             Config: [checkers.instruction_without_context]",
        ),
        "cross-file-contradiction" => Some(
            "cross-file-contradiction: Detects contradictory instructions across files.\n\
             \n\
             When a root CLAUDE.md says \"always write tests\" and backend/CLAUDE.md says \"skip\n\
             tests\", the agent in backend/ sees conflicting inherited instructions. This checker\n\
             reuses the same conflict pairs as conflicting-directives but compares files in\n\
             ancestor-descendant directory relationships.\n\
             \n\
             Only ancestor-descendant pairs are compared (CLAUDE.md vs backend/CLAUDE.md).\n\
             Sibling directories (frontend/ vs backend/) are skipped — they represent\n\
             intentionally different contexts.\n\
             \n\
             Severity: warning (strict-only)\n\
             Config: [checkers.cross_file_contradiction]",
        ),
        "stale-style-rule" => Some(
            "stale-style-rule: Flags formatter-enforceable style prescriptions.\n\
             \n\
             Rules like \"use 2 spaces for indentation\", \"always use semicolons\", or\n\
             \"prefer single quotes\" waste context window tokens. Your formatter (Prettier,\n\
             Black, rustfmt, etc.) already enforces these — the agent doesn't need to be told.\n\
             \n\
             Three sub-patterns:\n\
             1. Imperative formatting rules — indentation, semicolons, quotes, trailing commas,\n\
                brace style, import sorting\n\
             2. Line length limits — \"max line length of 80\" etc.\n\
             3. Naming style prescriptions — \"use camelCase for variables\" etc.\n\
             \n\
             Lines with backticks are excluded (project-specific tool references).\n\
             Headings and code blocks are excluded.\n\
             \n\
             Severity: info\n\
             Config: [checkers.stale_style_rule]",
        ),
        "hardcoded-file-structure" => Some(
            "hardcoded-file-structure: Flags references to non-.md source files that don't exist.\n\
             \n\
             When an instruction file says \"auth logic lives in `src/auth/handler.ts`\" but that\n\
             file was renamed or deleted, the agent operates with a false mental model of the\n\
             codebase. Unlike dead-reference (which handles .md files), this checker targets\n\
             source code paths (.ts, .py, .rs, .go, etc.).\n\
             \n\
             Path resolution: source-relative → root-relative → tree search by basename.\n\
             \n\
             Excluded: creation verbs (\"create `src/foo.ts`\"), example context, headings,\n\
             code blocks, markdown links, template/glob refs.\n\
             \n\
             Severity: info\n\
             Config: [checkers.hardcoded_file_structure]",
        ),
        "unversioned-stack-reference" => Some(
            "unversioned-stack-reference: Flags tech stack mentions without version numbers.\n\
             \n\
             \"Built with React\" (flagged) vs \"Built with React 18\" (clean). When instruction\n\
             files declare a tech stack without versions, agents can't make version-specific\n\
             decisions. \"Use the React hooks API\" is fine for React 16.8+ but wrong for\n\
             React 15. Pinning versions prevents drift.\n\
             \n\
             Dual-pattern approach: only flags when BOTH a well-known framework name (~40\n\
             frameworks) AND a stack-description context (\"built with\", \"written in\",\n\
             \"stack:\", etc.) are present on the same line, AND no version number is found.\n\
             \n\
             Prose mentions without stack context (\"check the React docs\") are not flagged.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.unversioned_stack_reference]",
        ),
        "missing-standard-file" => Some(
            "missing-standard-file: Flags projects missing common instruction files.\n\
             \n\
             If a project has instruction files but is missing a CLAUDE.md, this checker\n\
             suggests creating one. If CLAUDE.md exists but .claude/settings.json is missing,\n\
             it notes the absence at info severity.\n\
             \n\
             Helps ensure projects follow the standard instruction file layout.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.missing_standard_file]",
        ),
        "bare-url" => Some(
            "bare-url: Flags raw URLs not wrapped in markdown link syntax.\n\
             \n\
             Raw URLs like https://example.com/docs in prose are harder for agents to parse\n\
             and don't carry descriptive context. Wrapping in markdown link syntax\n\
             [descriptive text](url) gives agents both the URL and its purpose.\n\
             \n\
             Skips: URLs inside code blocks, inline backticks, headings, markdown links,\n\
             and angle-bracket URLs (<https://...>).\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.bare_url]",
        ),
        "repeated-word" => Some(
            "repeated-word: Flags accidental consecutive duplicate words.\n\
             \n\
             Typos like \"the the\", \"is is\", and \"to to\" are common copy-paste artifacts.\n\
             While harmless to agents, they signal unproofed content and can confuse human\n\
             reviewers. Grammatically valid constructs like \"that that\" and \"had had\" are\n\
             allowlisted.\n\
             \n\
             Skips: table rows, inline backticks, code blocks.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.repeated_word]",
        ),
        "undocumented-env-var" => Some(
            "undocumented-env-var: Flags env var references without nearby explanation.\n\
             \n\
             References like $DATABASE_URL or process.env.API_KEY without context on the\n\
             same or adjacent line leave agents guessing what the variable holds and how\n\
             to set it. Adding a brief explanation (\"set\", \"configure\", \"defaults to\",\n\
             or a colon/equals sign) provides the needed context.\n\
             \n\
             Skips: code blocks, inline backticks, headings, lines with explanation keywords.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.undocumented_env_var]",
        ),
        "empty-code-block" => Some(
            "empty-code-block: Flags code blocks with no content.\n\
             \n\
             An empty code block (two consecutive fence lines with nothing between them)\n\
             is typically a template artifact or editing mistake. Agents encountering an\n\
             empty code block expect a command or example and find nothing, which may cause\n\
             them to skip the section or hallucinate content.\n\
             \n\
             Whitespace-only code blocks are also flagged.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.empty_code_block]",
        ),
        "click-here-link" => Some(
            "click-here-link: Flags opaque link text like [click here](url).\n\
             \n\
             Agents typically cannot follow URLs. The link text is their only context for\n\
             understanding what a link points to. Text like \"click here\", \"here\",\n\
             \"this link\", or \"this\" provides zero information about the destination.\n\
             Replace with descriptive text: [API documentation](url).\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.click_here_link]",
        ),
        "double-negation" => Some(
            "double-negation: Flags double negatives that confuse agents.\n\
             \n\
             Phrases like \"never don't validate\", \"do not fail to run tests\", and\n\
             \"don't avoid error handling\" create logical ambiguity. LLMs sometimes\n\
             interpret double negatives as single negatives, flipping the intended\n\
             meaning. Rephrase as positive directives: \"always validate\",\n\
             \"always run tests\", \"use error handling\".\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.double_negation]",
        ),
        "imperative-heading" => Some(
            "imperative-heading: Flags headings that contain instructions instead of topics.\n\
             \n\
             Headings like \"## Always Run Tests Before Committing\" or \"## Never Use\n\
             Global State\" bury instructions in the document's navigation structure.\n\
             Agents use headings to navigate and scope — a heading should be a topic\n\
             (\"## Testing\", \"## State Management\"), with the imperative rules in the\n\
             section body. Legitimate patterns like \"## How to...\" are excluded.\n\
             \n\
             Only flags headings with 3+ words to avoid false positives on short titles.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.imperative_heading]",
        ),
        "inconsistent-command-prefix" => Some(
            "inconsistent-command-prefix: Flags mixed $ prefix styles in shell code blocks.\n\
             \n\
             When some commands in a bash/sh code block start with `$ ` and others don't,\n\
             agents may include the `$` literally when copying commands, breaking execution.\n\
             Pick one style: either all commands with `$ ` prefix (showing prompt context)\n\
             or none (bare commands ready to copy-paste).\n\
             \n\
             Only checks code blocks tagged bash/sh/shell/zsh or untagged blocks.\n\
             Requires 2+ command lines with at least one of each style to flag.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.inconsistent_command_prefix]",
        ),
        "empty-heading" => Some(
            "empty-heading: Flags headings with no title text.\n\
             \n\
             A heading line like `## ` or `###` with no text after the hash marks is always\n\
             a mistake — either an editing artifact or incomplete template. Agents use headings\n\
             for navigation and scoping, so an empty heading creates a broken section with no\n\
             way to reference it.\n\
             \n\
             Severity: info (strict-only)\n\
             Config: [checkers.empty_heading]",
        ),
        "copied-meta-instructions" => Some(
            "copied-meta-instructions: Flags AI boilerplate left in instruction files.\n\
             \n\
             Patterns like \"You are a helpful assistant\", \"As an AI language model\",\n\
             \"I cannot browse the internet\", and \"my training data\" are meta-instructions\n\
             from AI systems, not project-specific directives. They typically appear when\n\
             AI-generated content is pasted into instruction files without cleanup.\n\
             \n\
             These phrases waste context tokens and can confuse agents about their actual\n\
             role and capabilities. Replace with project-specific role definitions.\n\
             \n\
             Severity: warning (strict-only)\n\
             Config: [checkers.copied_meta_instructions]",
        ),
        "xml-document-wrapper" => Some(
            "xml-document-wrapper: Flags XML declarations and wrapper tags in markdown.\n\
             \n\
             XML declarations (`<?xml ...?>`) and wrapper tags like `<Document>`, `<Content>`,\n\
             `<Instructions>`, and `<Response>` in markdown files are AI output artifacts.\n\
             They appear when AI-generated content is copied verbatim without removing the\n\
             structural wrapper. These tags have no meaning in markdown and add noise.\n\
             \n\
             Tags inside code blocks are excluded (legitimate XML examples).\n\
             \n\
             Severity: warning (strict-only)\n\
             Config: [checkers.xml_document_wrapper]",
        ),
        "invalid-suppression" => Some(
            "invalid-suppression: Warns on unrecognized rule names in suppress comments.\n\
             \n\
             When you write <!-- spectralint-disable bad-rule-name -->, this checker verifies\n\
             that \"bad-rule-name\" is a known rule. Catches typos in suppress comments that\n\
             would otherwise silently fail to suppress anything.\n\
             \n\
             Severity: warning\n\
             Config: always enabled (cannot be disabled)",
        ),
        "unused-suppression" => Some(
            "unused-suppression: Reports suppress comments that didn't suppress any diagnostic.\n\
             \n\
             After all checkers run and suppressions are applied, any suppress comment that\n\
             didn't actually suppress a diagnostic is flagged. This keeps suppress comments\n\
             from accumulating as dead code when the underlying issue is fixed.\n\
             \n\
             Severity: info\n\
             Config: always enabled (cannot be disabled)",
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
        "stale-file-tree" => Some(
            "stale-file-tree: Flags ASCII directory trees containing paths that don't exist on disk.\n\
             \n\
             Instruction files often include directory tree diagrams to document project structure.\n\
             When files get renamed or deleted, these trees become stale — agents trust the documented\n\
             structure and make incorrect assumptions about where code lives.\n\
             \n\
             The checker parses Unicode box-drawing trees (├── └──) and ASCII variants (|-- +--)\n\
             inside fenced code blocks, then verifies each path exists relative to the project root.\n\
             \n\
             Excluded: trees preceded by example/creation context, trees containing ellipsis (...),\n\
             placeholder paths (<name>, {template}, xxx).\n\
             \n\
             Severity: warning (files), info (directories)\n\
             Config: [checkers.stale_file_tree]",
        ),
        "command-validation" => Some(
            "command-validation: Flags build/test commands whose toolchain prerequisites are missing.\n\
             \n\
             When a CLAUDE.md says `cargo test` but there's no Cargo.toml, or `npm install` but no\n\
             package.json, the documented commands won't work. This checker maps referenced commands\n\
             to their required manifest files and verifies they exist.\n\
             \n\
             Supported toolchains: Cargo, npm/Yarn/pnpm/Bun, Go, Python, Make, Maven, Gradle,\n\
             .NET, Ruby/Bundler, Elixir, Flutter/Dart, Docker Compose.\n\
             \n\
             Excluded: commands inside docker run/exec context, conditional references\n\
             (\"if using Go...\"), and advisory mentions (\"optionally\", \"alternatively\").\n\
             One diagnostic per toolchain per file.\n\
             \n\
             Severity: warning\n\
             Config: [checkers.command_validation]",
        ),
        "token-budget" => Some(
            "token-budget: Estimates token cost of instruction files and flags context window overuse.\n\
             \n\
             Every instruction file loaded by an agent consumes context window tokens. Large files\n\
             crowd out space for actual work — code diffs, tool outputs, conversation history.\n\
             This checker estimates token count using a character-based heuristic (~4 chars per token)\n\
             and flags files that exceed configurable thresholds.\n\
             \n\
             Default thresholds:\n\
             - warn_tokens: 4000 (~16KB) — emits info-level advisory\n\
             - max_tokens: 8000 (~32KB) — emits warning-level diagnostic\n\
             \n\
             The estimate is intentionally approximate — exact tokenization varies by model.\n\
             The goal is catching obviously oversized files, not precise token accounting.\n\
             \n\
             Severity: info (warn threshold), warning (max threshold)\n\
             Config: [checkers.token_budget]\n\
             Options: warn_tokens, max_tokens, scope, severity",
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
