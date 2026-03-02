# spectralint

[![CI](https://github.com/by-all-means/spectralint/actions/workflows/ci.yml/badge.svg)](https://github.com/by-all-means/spectralint/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/spectralint.svg)](https://crates.io/crates/spectralint)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<!-- spectralint-disable dead-reference -->
**Static analysis for AI agent instruction files.**
<!-- spectralint-enable dead-reference -->

Catches bugs that creep in when agent instructions are spread across multiple markdown files — dead references, naming drift, divergent tables, and vague directives.

## Why?

AI agent setups (CLAUDE.md, AGENTS.md, .cursorrules, Copilot instructions) tend to grow into multiple interconnected files. Over time:

- Files get renamed or deleted, but references to them stay behind
- The same field is called `api_key` in one file and `apiKey` in another
- Routing tables define different status values in different files
- Instructions accumulate vague language like "try to" and "when possible"

These are silent bugs — the agent won't tell you it's confused. spectralint catches them deterministically, with no AI/LLM dependency, in milliseconds.

## Security

**No tokens. No API keys. No data leakage. 100% local.**

spectralint is a standalone Rust binary that runs entirely on your machine. Your instruction files — which often contain project architecture, internal tooling names, and operational secrets — never leave your environment. There is no network access, no telemetry, no cloud dependency. Safe for air-gapped environments and enterprise security policies.

## What It Finds (Real-World Results)

We cloned the top 100 public GitHub repos containing `CLAUDE.md` (sorted by star count, from React at 243k stars to small projects) and ran spectralint on each. Dead references verified against the actual repo filesystem — every flagged file genuinely does not exist.

Methodology: GitHub code search for `filename:CLAUDE.md`, ranked by `stargazers_count`, top 100 results, February 2026. Full shallow clones, all instruction files scanned (CLAUDE.md, AGENTS.md, .claude/\*\*, .github/copilot-instructions.md). See [`benchmarks/`](benchmarks/) for the repo list and reproduction script.

```
100 repos scanned → 531 findings (56% of repos)

  absolute-path                139   warning   (hardcoded /Users/... paths)
  hardcoded-file-structure     107   info      (source paths that don't exist on disk)
  large-code-block              73   info      (inline code >40 lines)
  dead-reference                45   error     (files that genuinely don't exist)
  orphaned-section              25   info      (sections with no actionable content)
  file-size                     21   info/warn (files exceeding 400/500 lines)
  duplicate-instruction-file    18   warning   (near-duplicate files)
  placeholder-text              17   warning   ([TODO], [TBD] left in)
  placeholder-url               12   info      (example.com/localhost URLs left in)
  instruction-without-context   12   info      (directives with no code examples)
  vague-directive               12   info      ("try to", "when possible")
  stale-style-rule               7   info      (formatter-enforceable rules)
  dangerous-command              6   warning   (rm -rf, DROP TABLE in code blocks)
```

**40% of repos had errors or warnings** — dead references to files that genuinely don't exist, duplicate instruction files, dangerous commands, placeholder text, and hardcoded paths. Every finding manually verified against the actual repo.

## 66 Built-in Rules

| Rule | Severity | What it catches |
|------|----------|-----------------|
| `dead-reference` | error | `.md` references to files that don't exist |
| `credential-exposure` | error | Hardcoded API keys, tokens, passwords |
| `absolute-path` | warning | Hardcoded `/Users/...`, `C:\...` paths |
| `naming-inconsistency` | warning | `api_key` in one file vs `apiKey` in another |
| `duplicate-instruction-file` | warning | Near-duplicate instruction files |
| `placeholder-text` | warning | `[TODO]`, `[TBD]`, unfinished content |
| `dangerous-command` | warning | `rm -rf`, `DROP TABLE` in code blocks |
| `session-journal` | warning | Session logs masquerading as instructions |
| `circular-reference` | warning | A→B→C→A file reference cycles |
| `broken-table` | warning | Malformed markdown tables |
| `duplicate-section` | warning | Repeated section headings in same file |
| `broken-anchor-link` | warning | In-file `[text](#anchor)` links that don't match any heading |
| `hardcoded-windows-path` | warning | Backslash paths (`scripts\helper.py`) that break on non-Windows |
| `unclosed-fence` | warning | Code blocks missing closing ` ``` ` |
| `stale-reference` | warning | "After March 2025, use the new API" time bombs |
| `file-size` | info/warn | Files exceeding 400/500 lines |
| `hardcoded-file-structure` | info | Source paths (`src/auth/handler.ts`) that don't exist on disk |
| `large-code-block` | info | Inline code blocks exceeding 40 lines |
| `orphaned-section` | info | Sections with no actionable content |
| `placeholder-url` | info | `example.com` URLs left in |
| `vague-directive` | info | "try to", "when possible", "use your judgment" |
| `generic-instruction` | info | "follow best practices", "write clean code" |
| `instruction-without-context` | info | Directive-heavy files with no code examples |
| `context-window-waste` | info | 3+ consecutive blank lines wasting tokens |
| `stale-style-rule` | info | Formatter-enforceable rules (indentation, quotes, semicolons) |
| `ambiguous-scope-reference` | info | Unclear "this file", "the config" references |
| `generated-attribution` | info | AI-tool attribution lines ("Generated with Claude Code") |
| `boilerplate-template` | info | Unchanged template content |
| `outdated-model-reference` | info | References to deprecated AI model names |
| `missing-essential-sections` | info | No build/test commands for agents to verify work |
| `misordered-steps` | info | Numbered steps out of sequence |
| `prompt-injection-vector` | warn/info | "Ignore previous instructions", hidden Unicode, base64 payloads |
| `conflicting-directives` | warning | Contradictory instructions in the same file *(strict)* |
| `cross-file-contradiction` | warning | Contradictory instructions across files *(strict)* |
| `enum-drift` | warning | Tables with matching columns but divergent values *(strict)* |
| `agent-guidelines` | info | Missing boundaries, multi-responsibility, no output format *(strict)* |
| `heading-hierarchy` | info | Skipped heading levels (h1 → h3) *(strict)* |
| `emoji-density` | info | 20+ decorative emoji wasting tokens *(strict)* |
| `missing-verification` | info | Action sections without success criteria *(strict)* |
| `negative-only-framing` | info | 75%+ of directives are "Don't/Never/Avoid" *(strict)* |
| `missing-role-definition` | info | No "You are..." or Role section *(strict)* |
| `redundant-directive` | info | Near-duplicate directive lines *(strict)* |
| `instruction-density` | info | Sections with 15+ consecutive bullet points *(strict)* |
| `missing-examples` | info | Format specs without code examples *(strict)* |
| `unbounded-scope` | info | Capability grants without boundary constraints *(strict)* |
| `section-length-imbalance` | info | Wildly uneven section sizes *(strict)* |
| `untagged-code-block` | info | Code blocks without language tags *(strict)* |
| `emphasis-overuse` | info | Excessive bold/italic/caps formatting *(strict)* |
| `excessive-nesting` | info | Deeply nested list structures *(strict)* |
| `unversioned-stack-reference` | info | "Built with React" without version pinning *(strict)* |
| `missing-standard-file` | info | Projects missing CLAUDE.md or settings.json *(strict)* |
| `bare-url` | info | Raw URLs not wrapped in `[text](url)` syntax *(strict)* |
| `repeated-word` | info | Accidental consecutive duplicate words ("the the") *(strict)* |
| `undocumented-env-var` | info | `$ENV_VAR` references without nearby explanation *(strict)* |
| `empty-code-block` | info | Code blocks with no content *(strict)* |
| `click-here-link` | info | Opaque link text like "[click here](url)" *(strict)* |
| `double-negation` | info | "never don't", "not fail to" — confusing phrasing *(strict)* |
| `imperative-heading` | info | Headings that are instructions, not topics *(strict)* |
| `inconsistent-command-prefix` | info | Mixed `$` prefix styles in shell code blocks *(strict)* |
| `command-without-codeblock` | info | Bare shell commands not in code blocks or backticks *(strict)* |
| `missing-verification-step` | info | Files with workflow steps but no test/verify command *(strict)* |
| `long-paragraph` | info | Dense text blocks (8+ consecutive prose lines) *(strict)* |
| `empty-heading` | info | Headings with no title text (`## `) *(strict)* |
| `copied-meta-instructions` | warning | AI boilerplate like "You are a helpful assistant" *(strict)* |
| `xml-document-wrapper` | warning | XML declarations and wrapper tags in markdown *(strict)* |
| `invalid-suppression` | warning | Unrecognized rule names in suppress comments |
| `unused-suppression` | info | Suppress comments that didn't suppress anything |
| `custom` | configurable | Your own regex patterns |

## Features

- **66 built-in rules** covering security, consistency, content quality, and agent best practices
<!-- spectralint-disable-next-line vague-directive -->
- **Vague directive detection** — finds non-deterministic language ("try to", "when possible")
- **Cross-file analysis** — naming inconsistency and enum drift across multiple files
- **Prompt injection detection** — social engineering, invisible Unicode, base64 payloads
- **Custom regex patterns** — define your own lint rules in config
- **Inline suppression** — disable rules with `<!-- spectralint-disable -->` comments; validates rule names and flags unused suppressions
- **Multiple output formats** — text (colored), JSON, SARIF, and GitHub Actions annotations
- **Fast** — parallel parsing via rayon, scans hundreds of files in milliseconds

## Install

### Homebrew (macOS/Linux)

```sh
brew install by-all-means/tap/spectralint
```

### Shell installer (macOS/Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/by-all-means/spectralint/releases/latest/download/spectralint-installer.sh | sh
```

### Cargo

```sh
cargo install spectralint
```

## Quick Start

```sh
# Initialize config
spectralint init
spectralint init --preset minimal    # dead-reference + credential-exposure only
spectralint init --preset strict     # all checkers enabled

# Lint your project
spectralint check .

# Enable strict mode (opinionated checks)
spectralint check . --strict

# Output formats
spectralint check . --format json    # structured JSON
spectralint check . --format sarif   # SARIF for IDE/CI integration
spectralint check . --format github  # GitHub Actions annotations

# Filter and control output
spectralint check . --rule dead-reference  # only show specific rules
spectralint check . --count               # summary counts only
spectralint check . --quiet               # exit code only, no output
spectralint check . --no-color            # disable colored output (also respects NO_COLOR env var)

# List all available rules
spectralint explain

# Learn why a rule matters
spectralint explain naming-inconsistency
```

## Example Output

```
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  2 errors, 8 warnings across 3 files
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  ✗ dead-reference (2)
    CLAUDE.md
      L5    "agent_definitions/scout.md" does not exist
      L6    "agent_definitions/followup_drafter.md" does not exist

  ⚠ naming-inconsistency (4)
    AGENTS.md
      L5    Inconsistent naming: "apiKey" vs "api_key" refer to the same concept
    CLAUDE.md
      L5    Inconsistent naming: "apiKey" vs "api_key" refer to the same concept

  ⚠ enum-drift (4)
    AGENTS.md
      L5    Column "Status" has values "archived" not found in CLAUDE.md
    CLAUDE.md
      L5    Column "Status" has values "pending" not found in AGENTS.md
```

## Configuration

Create `.spectralintrc.toml` in your project root (or run `spectralint init`):

```toml
# Which files to scan (glob patterns, case-insensitive)
# Default: known AI instruction file patterns
# Set to ["**/*.md"] to scan all markdown files
include = ["CLAUDE.md", "AGENTS.md", ".claude/**", ".github/copilot-instructions.md"]

# Directories to ignore when scanning (supports glob patterns)
ignore = ["node_modules", ".git", "target", "build_*"]

# Files to skip entirely
# ignore_files = ["changelog.md"]

# Files treated as historical (dead refs and enum drift are skipped)
# Matched case-insensitively.
# historical_files = ["changelog*", "retro*", "history*", "archive*", "restart*"]

[checkers.dead_reference]
enabled = true

[checkers.vague_directive]
enabled = true
# strict = true  # also flag "when possible", "when needed", "as needed", "consider"
# Add your own vague language patterns (extends built-in list)
# extra_patterns = ["(?i)\\bmaybe\\b", "(?i)\\bprobably\\b"]

[checkers.naming_inconsistency]
enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

[checkers.missing_essential_sections]
enabled = true
# min_lines = 10  # skip files shorter than this

[checkers.prompt_injection_vector]
enabled = true

# Per-checker severity override (promote info to warning, etc.)
# [checkers.vague_directive]
# severity = "error"

# Strict-only checkers (disabled by default, enabled by --strict or strict = true):
# enum_drift, agent_guidelines, heading_hierarchy, emoji_density,
# missing_verification, negative_only_framing, conflicting_directives,
# missing_role_definition, redundant_directive, instruction_density,
# missing_examples, unbounded_scope, bare_url, repeated_word,
# undocumented_env_var, missing_standard_file, empty_code_block,
# click_here_link, double_negation, imperative_heading,
# inconsistent_command_prefix, empty_heading,
# copied_meta_instructions, xml_document_wrapper

# Custom regex patterns
[[checkers.custom_patterns]]
name = "todo-comment"
pattern = "(?i)\\bTODO\\b"
severity = "warning"
message = "TODO comment found"
```

### Scope Boundaries

Cross-file checkers (enum-drift, naming-inconsistency, vague-directive) compare all files by default. In projects with output reports or generated files, this can produce noise. Use `scope` to limit which files each checker examines:

```toml
[checkers.enum_drift]
enabled = true
scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

[checkers.naming_inconsistency]
enabled = true
scope = ["CLAUDE.md", "AGENTS.md"]

[checkers.vague_directive]
enabled = true
scope = ["CLAUDE.md", "AGENTS.md"]
```

- **Per-checker** — each checker can have its own scope
- **Positive match** — list files to include (glob patterns, case-insensitive)
- **Empty = all files** — omitting `scope` preserves default behavior (all files compared)

### Include Filter

By default, spectralint only scans known AI instruction files (`CLAUDE.md`, `AGENTS.md`, `.claude/**`, `.github/copilot-instructions.md`). This prevents false positives from documentation, reports, and other markdown files that aren't agent instructions.

To scan all markdown files (old behavior):

```toml
include = ["**/*.md"]
```

- **Top-level config** — controls which files are parsed, independent of per-checker `scope`
- **Case-insensitive** — `CLAUDE.md` matches `claude.md`
- **`include = []`** — scans nothing (empty set)
- **`include` + `ignore_files`** — file must match `include` AND not match `ignore_files`

## Inline Suppression

Suppress diagnostics with HTML comments:

```markdown
<!-- spectralint-disable dead-reference -->
Load `some/nonexistent/file.md` here.
<!-- spectralint-enable dead-reference -->

<!-- spectralint-disable-next-line vague-directive -->
Try to be helpful when possible.

<!-- spectralint-disable -->
Everything in this block is ignored.
<!-- spectralint-enable -->
```

Suppression comments are validated automatically:
- **`invalid-suppression`** — warns if you reference a rule name that doesn't exist (catches typos)
- **`unused-suppression`** — flags suppress comments that didn't actually suppress any diagnostic

## CI Integration

### GitHub Actions

Add to your workflow:

```yaml
- name: Install spectralint
  run: curl --proto '=https' --tlsv1.2 -LsSf https://github.com/by-all-means/spectralint/releases/latest/download/spectralint-installer.sh | sh
- name: Lint agent instructions
  run: spectralint check . --format github
```

<!-- Bundled GitHub Action coming in a future release:
- uses: by-all-means/spectralint@v1
-->

## Strict Mode

Enable 29 additional opinionated checkers (enum-drift, agent-guidelines, heading-hierarchy, emoji-density, missing-verification, negative-only-framing, cross-file-contradiction, missing-role-definition, redundant-directive, instruction-density, missing-examples, unbounded-scope, section-length-imbalance, untagged-code-block, emphasis-overuse, excessive-nesting, unversioned-stack-reference, missing-standard-file, bare-url, repeated-word, undocumented-env-var, empty-code-block, click-here-link, double-negation, imperative-heading, inconsistent-command-prefix, empty-heading, copied-meta-instructions, xml-document-wrapper):

```sh
# Via CLI flag
spectralint check . --strict

# Or in .spectralintrc.toml
# strict = true
```

## Fail Threshold

Control which severity level causes a non-zero exit code:

```sh
# Default: only errors fail (exit 1)
spectralint check .

# Fail on warnings too
spectralint check . --fail-on warning

# Fail on anything (including info)
spectralint check . --fail-on info
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | No diagnostics at or above the `--fail-on` threshold |
| 1 | One or more diagnostics at or above the threshold |

## License

MIT
