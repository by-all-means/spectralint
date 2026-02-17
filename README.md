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
100 repos scanned → 286 findings

  file-size                     100   info/warn (files exceeding 300/500 lines)
  naming-inconsistency           56   info      (similar names across files)
  dead-reference                 47   error     (files that genuinely don't exist)
  missing-essential-sections     35   info      (no build/test commands)
  dangerous-command              27   warning   (rm -rf, DROP TABLE in code blocks)
  vague-directive                15   info      ("try to", "when possible")
  placeholder-text                2   warning   ([TODO], [TBD] left in)
  stale-reference                 2   warning   (time-sensitive conditions)
  prompt-injection-vector         1   warning   (social engineering pattern)
  credential-exposure             1   error     (hardcoded secret)
```

**21% of repos had errors or warnings** — dead references to files that genuinely don't exist, dangerous commands in code blocks, stale time-sensitive logic, and one hardcoded credential. Every finding manually verified against the actual repo. With `--strict`, 6 additional opinionated checks (including enum-drift) bring total findings to 2,296 across 91% of repos.

## 18 Built-in Rules

| Rule | Severity | What it catches |
|------|----------|-----------------|
| `dead-reference` | error | `.md` references to files that don't exist |
| `credential-exposure` | error | Hardcoded API keys, tokens, passwords |
| `naming-inconsistency` | warning | `api_key` in one file vs `apiKey` in another |
| `enum-drift` | warning | Tables with matching columns but divergent values *(strict)* |
| `stale-reference` | warning | "After March 2025, use the new API" time bombs |
| `placeholder-text` | warning | `[TODO]`, `[TBD]`, unfinished content |
| `dangerous-command` | warning | `rm -rf`, `DROP TABLE` in code blocks |
| `session-journal` | warning | Session logs masquerading as instructions |
| `file-size` | info/warn | Files exceeding 300/500 lines |
| `vague-directive` | info | "try to", "when possible", "use your judgment" |
| `agent-guidelines` | info | Missing boundaries, multi-responsibility, no output format *(strict)* |
| `heading-hierarchy` | info | Skipped heading levels (h1 → h3) *(strict)* |
| `emoji-density` | info | 10+ decorative emoji wasting tokens *(strict)* |
| `missing-essential-sections` | info | No build/test commands for agents to verify work |
| `prompt-injection-vector` | warn/info | "Ignore previous instructions", hidden Unicode, base64 payloads |
| `missing-verification` | info | Action sections without success criteria *(strict)* |
| `negative-only-framing` | info | 75%+ of directives are "Don't/Never/Avoid" *(strict)* |
| `custom` | configurable | Your own regex patterns |

## Features

- **18 built-in rules** covering security, consistency, content quality, and agent best practices
<!-- spectralint-disable-next-line vague-directive -->
- **Vague directive detection** — finds non-deterministic language ("try to", "when possible")
- **Cross-file analysis** — naming inconsistency and enum drift across multiple files
- **Prompt injection detection** — social engineering, invisible Unicode, base64 payloads
- **Custom regex patterns** — define your own lint rules in config
- **Inline suppression** — disable rules with `<!-- spectralint-disable -->` comments
- **Multiple output formats** — text (colored), JSON, and GitHub Actions annotations
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

# Lint your project
spectralint check .

# Enable strict mode (opinionated checks)
spectralint check . --strict

# JSON output
spectralint check . --format json

# GitHub Actions annotations
spectralint check . --format github

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

# Strict-only checkers (disabled by default, enabled by --strict or strict = true):
# enum_drift, agent_guidelines, heading_hierarchy, emoji_density,
# missing_verification, negative_only_framing

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

Enable 6 additional opinionated checkers (enum-drift, agent-guidelines, heading-hierarchy, emoji-density, missing-verification, negative-only-framing):

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
