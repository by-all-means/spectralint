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

## Features

- **Dead reference detection** — flags `.md` references to files that don't exist
<!-- spectralint-disable-next-line vague-directive -->
- **Vague directive detection** — finds non-deterministic language ("try to", "when possible")
- **Naming inconsistency** — detects `api_key` in one file vs `apiKey` in another
- **Enum drift** — finds tables with matching columns but divergent value sets across files
- **Custom regex patterns** — define your own lint rules in config
- **Inline suppression** — disable rules with `<!-- spectralint-disable -->` comments
- **Multiple output formats** — text (colored), JSON, and GitHub Actions annotations
- **Fast** — parallel parsing via rayon, scans hundreds of files in milliseconds

## Install

```sh
cargo install spectralint
```

## Quick Start

```sh
# Initialize config
spectralint init

# Lint your project
spectralint check .

# JSON output
spectralint check . --format json

# GitHub Actions annotations
spectralint check . --format github
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
# Add your own vague language patterns (extends built-in list)
# extra_patterns = ["(?i)\\bmaybe\\b", "(?i)\\bprobably\\b"]

[checkers.naming_inconsistency]
enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

[checkers.enum_drift]
enabled = true
# scope = ["CLAUDE.md", "AGENTS.md", ".claude/**"]

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
  run: cargo install spectralint
- name: Lint agent instructions
  run: spectralint check . --format github
```

Or use the bundled action:

```yaml
- uses: by-all-means/spectralint@v1
```

## Strictness

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
