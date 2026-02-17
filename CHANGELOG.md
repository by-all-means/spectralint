# Changelog

All notable changes to spectralint will be documented in this file.

## 0.1.0 (2026-02-17)

Initial release.

### Rules (18 built-in)

- **dead-reference** — flags `.md` references to files that don't exist
- **credential-exposure** — detects hardcoded API keys, tokens, passwords
- **naming-inconsistency** — catches `api_key` vs `apiKey` across files
- **enum-drift** — tables with matching columns but divergent values *(strict-only)*
- **stale-reference** — time-sensitive conditional logic that becomes stale
- **placeholder-text** — leftover `[TODO]`, `[TBD]`, unfinished content
- **dangerous-command** — `rm -rf`, `DROP TABLE` in code blocks
- **session-journal** — session logs masquerading as instructions
- **file-size** — files exceeding 300/500 lines
- **vague-directive** — non-deterministic language ("try to", "when possible")
- **agent-guidelines** — missing boundaries, multi-responsibility, no output format
- **heading-hierarchy** — skipped heading levels (h1 → h3)
- **emoji-density** — 10+ decorative emoji wasting tokens
- **missing-essential-sections** — no build/test commands for agents
- **prompt-injection-vector** — social engineering, invisible Unicode, base64 payloads
- **missing-verification** — action sections without success criteria
- **negative-only-framing** — 75%+ of directives are negative
- **custom** — user-defined regex patterns from config

### Features

- Cross-file analysis (naming inconsistency, enum drift)
- Inline suppression via `<!-- spectralint-disable -->` comments
- Multiple output formats: text (colored), JSON, GitHub Actions annotations
- Custom regex patterns in `.spectralintrc.toml`
- `--fail-on` threshold (error, warning, info)
- `--strict` mode for opinionated checks
- `spectralint explain` command with per-rule documentation
- Parallel parsing via rayon
- Include filter for targeted scanning (default: AI instruction files only)
- Scope boundaries per checker
- Historical file support (skip dead-ref/enum-drift for changelogs)

### Benchmark

100 repos scanned — 286 findings, 47 dead references verified against actual filesystems, 21% of repos had errors or warnings.
