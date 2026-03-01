# Changelog

All notable changes to spectralint will be documented in this file.

## 0.3.0 (2026-03-01)

### New Rules (12 added, 60 total)

- **missing-standard-file** — flags projects with instruction files but no CLAUDE.md *(strict-only)*
- **bare-url** — flags raw URLs not wrapped in markdown link syntax *(strict-only)*
- **repeated-word** — flags accidental consecutive duplicate words like "the the" *(strict-only)*
- **undocumented-env-var** — flags `$ENV_VAR` references without nearby explanation *(strict-only)*
- **empty-code-block** — flags code blocks with no content *(strict-only)*
- **click-here-link** — flags opaque link text like "[click here](url)" *(strict-only)*
- **double-negation** — flags confusing double negatives like "never don't" *(strict-only)*
- **imperative-heading** — flags headings that are instructions rather than topics *(strict-only)*
- **inconsistent-command-prefix** — flags mixed `$` prefix styles in shell blocks *(strict-only)*
- **empty-heading** — flags headings with no title text *(strict-only)*
- **copied-meta-instructions** — flags AI boilerplate like "You are a helpful assistant" *(strict-only)*
- **xml-document-wrapper** — flags XML declarations and wrapper tags in markdown *(strict-only)*

### CLI

- **SARIF output format** — `--format sarif` for GitHub Code Scanning integration
- **`--rule` filter** — show only specific rules (e.g., `--rule dead-reference`)
- **`--quiet` flag** — suppress output, exit code only
- **`--no-color` flag** — disable colored output; also respects `NO_COLOR` env var
- **`--count` flag** — print summary counts only (e.g., "3 errors, 12 warnings, 5 info")
- Colors now auto-suppress when stdout is piped (proper TTY detection)

### Config

- **Per-checker severity override** — e.g., `[checkers.vague_directive]\nseverity = "error"`
- **`init --preset`** — `--preset minimal`, `--preset standard`, `--preset strict`
- **Default format in config** — set `format = "json"` in `.spectralintrc.toml`

### Suppression Validation

- **invalid-suppression** — warns on typos in `<!-- spectralint-disable bad-rule-name -->`
- **unused-suppression** — reports suppress comments that didn't suppress anything

### Performance

- Pre-compute `HashSet` per file in `duplicate-instruction-file` (was O(n\*m), now O(min(n,m)))
- Cap name extraction at 500 in `naming-inconsistency` (prevents quadratic blowup)
- Bitmask pre-indexing in `cross-file-contradiction` (skip file pairs with no overlapping patterns, short-circuit when both sides found)
- Lazy `category.to_string()` in suppression hot path (avoids allocation when no rule-specific ranges match)
- Cache `current_year_month()` in `LazyLock` (was calling `std::env::var()` per regex match)
- Single-pass severity counting for `--count` mode
- `HashSet` lookup for `--rule` filter (was linear scan)
- Eliminated per-line `Vec` allocation in `repeated-word` checker
- Hoisted per-line regex check outside inner loop in `undocumented-env-var`

### Code Quality

- Extracted shared `inside_inline_code()` utility (was duplicated in 5 checkers)
- Unified all checker visibility to `pub(crate)`
- Derived suppression rule names from `AVAILABLE_RULES` (eliminates hand-maintained duplicate list)

### Correctness

- **MediaWiki detection** — skip files with `{{template}}`, `[[links]]`, `<ref>` markup (eliminates false positives)
- **Date-aware `stale-reference`** — only flags dates in the past (with 1-month grace period)

---

## 0.2.0 (2026-03-01)

### New Rules (30 added, 48 total)

- **absolute-path** — hardcoded `/Users/...`, `C:\...` paths
- **ambiguous-scope-reference** — unclear "this file", "the above" references
- **boilerplate-template** — unmodified template content left in
- **broken-table** — malformed markdown tables
- **circular-reference** — A→B→C→A file reference cycles
- **conflicting-directives** — contradictory instructions across files
- **context-window-waste** — content that wastes agent context window tokens
- **cross-file-contradiction** — opposing instructions in different files
- **duplicate-instruction-file** — near-duplicate instruction files
- **duplicate-section** — repeated section headings in the same file
- **emphasis-overuse** — excessive bold/caps/emphasis reducing signal *(strict-only)*
- **excessive-nesting** — deeply nested heading or list structures *(strict-only)*
- **generic-instruction** — "follow best practices", "write clean code"
- **hardcoded-file-structure** — source paths (`src/auth/handler.ts`) that don't exist on disk
- **instruction-density** — files with high instruction-to-context ratio *(strict-only)*
- **instruction-without-context** — directive-heavy sections with no code examples
- **large-code-block** — inline code blocks exceeding 40 lines
- **misordered-steps** — numbered steps that appear out of logical order
- **missing-examples** — rule descriptions without concrete examples *(strict-only)*
- **missing-role-definition** — agent files with no role/identity definition *(strict-only)*
- **orphaned-section** — sections with no actionable content
- **outdated-model-reference** — references to deprecated AI model names
- **placeholder-url** — `example.com`/`localhost` URLs left in
- **redundant-directive** — near-identical instructions repeated *(strict-only)*
- **section-length-imbalance** — extreme section length variation *(strict-only)*
- **stale-style-rule** — formatting rules enforceable by linters/formatters
- **unbounded-scope** — open-ended instructions with no clear boundary *(strict-only)*
- **unclosed-fence** — code blocks missing closing ` ``` `
- **untagged-code-block** — fenced code blocks with no language tag *(strict-only)*
- **unversioned-stack-reference** — tech stack mentions without version pinning *(strict-only)*

### Improvements

- Hardened existing checkers to reduce false positives
- Expanded scope filtering and config options for all checkers

### Benchmark

100 repos scanned — 531 findings, 56% of repos affected. 40% had errors or warnings. Every finding manually verified.

---

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
