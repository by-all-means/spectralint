# Changelog

All notable changes to spectralint will be documented in this file.

## 0.5.0 (2026-03-09)

### New Rules (3 added, 71 total)

- **token-budget** — estimates context window cost per file; warns when approaching/exceeding configurable thresholds
- **stale-file-tree** — validates ASCII directory trees in code blocks against the actual filesystem *(strict-only)*
- **command-validation** — flags shell commands in code blocks that reference tools not found in PATH

### New Features

- **Autofix engine** — `--fix` flag applies structured text replacements (e.g., removing repeated words). Overlap detection prevents conflicting fixes.
- **Watch mode** — `--watch` re-scans on file changes with 2-second polling
- **Result caching** — automatic whole-project cache using FNV-1a hash with mtime+size-based invalidation. `--no-cache` to bypass.
- **GitHub Action** — bundled `action.yml` for CI integration

### Architecture

- **RuleMeta** — self-describing checkers via `meta()` trait method. Each checker declares its name, description, default severity, and strict-only status, eliminating the 5-file-edit problem when adding new rules.
- **Fix/Replacement types** — structured autofix data model with line/column ranges and replacement text
- **Arc\<PathBuf\>** — shared path allocation across diagnostics (eliminates per-diagnostic path cloning)
- **Category::as_str()** — zero-allocation string conversion for the hot path
- **Category::FromStr** — typed rule filtering via `--rule` flag
- **CustomPattern(Box\<str\>)** — reduced Category enum size from 32 to 16 bytes

### Performance

- **RegexSet pre-filter** — batch regex matching for conflict pair detection (conflicting-directives, cross-file-contradiction)
- **Single-pass normalize_directive()** — 4 allocations reduced to 1
- **normalize() without Vec\<char\>** — identifier normalization using char_indices iterator, no heap allocation
- **Jaro-Winkler length pre-filter** — mathematically-derived bound skips pairs that can never match in redundant-directive O(n²) loop
- **Cache path interning** — shared Arc\<PathBuf\> for diagnostics referencing the same file during cache load
- **Redundant sort removed** — compute_files_hash no longer re-sorts already-sorted input
- **serialize_arc_pathbuf** — format directly into serializer via collect_str, no intermediate String

### Correctness

- **YAML frontmatter support** — parser now recognizes `---` delimited frontmatter; prevents YAML comments from being mis-parsed as Markdown headings (eliminated 12 FPs across orphaned-section, hardcoded-windows-path, and other checkers)
- **broken-anchor-link** — `heading_to_anchor()` no longer trims leading/trailing hyphens, matching GitHub's actual `github-slugger` behavior (fixes FP on emoji-prefixed headings like `🚀 Contributing` → `#-contributing`)
- **command-validation** — `make` now requires word boundary (prevents FP on Rust `make::` module paths); `npm install -g` skipped (global installs don't need `package.json`); `python` prefix narrowed to `python -m` (bare `python script.py` doesn't require dependency manifest)
- **hardcoded-windows-path** — recognizes Markdown escaped underscores (`\_`) and YAML `\n` escapes as non-path backslash sequences; handles multi-backslash matches correctly
- **Reasoning prompt heuristic** — files with zero code blocks, zero file references, and zero shell commands are skipped by vague-directive, generic-instruction, and missing-essential-sections (eliminates false positives on FlowKit/workflow agent prompts)
- **Shared is_reasoning_prompt()** — extracted to utils.rs from 3 duplicate implementations
- **Serde default fix** — strict-only checkers now use `#[serde(default = "ScopedCheckerConfig::disabled")]` to prevent re-enabling when deserializing partial config
- **inside_inline_code soundness** — fixed byte/char boundary issue by operating on bytes instead of chars
- **orphaned-section** — intentionally-empty detection for numbered outlines and question headings (prevents FPs on LLM prompt templates)
- **missing-standard-file** — only flags in project roots (directories with .git or package manifests)
- **stale-file-tree** — moved to strict-only with Info severity after benchmark showed 580 false positives across 32 repos

### Tests

- **1,266 tests** (up from ~800 in v0.4.0) — 1,115 unit tests + 151 integration tests
- New FP regression tests for YAML frontmatter, emoji anchors, `make::` Rust paths, `npm install -g`, `python -c`, markdown escaped underscores
- Edge case tests covering cache invalidation, fix engine error paths, Category serde roundtrip, CLI flag combinations, SARIF validation, and reasoning prompt edge cases
- 7 FP regression tests for reasoning agent scenarios

### Noise Reduction

- **hardcoded-file-structure** — skips generic task templates (plans, reports, prompts, BMad tasks, agent definitions) where paths are illustrative, not references to actual files (-49 findings)
- **large-code-block** — skips `references/` and `agents/` directories where large code blocks are intentional output templates (-36 findings)
- **placeholder-url** — no longer flags `localhost` or `127.0.0.1` URLs; these are intentional dev server addresses in instruction files (-7 findings)

### Benchmark

100 repos scanned — 297 findings (43% of repos affected). 44% are errors or warnings. 117 noise findings eliminated via FP fixes (25) and noise reduction (92). Dead references increased from 13 to 47 as repos grew their instruction files.

---

## 0.4.0 (2026-03-05)

### New Rules (6 added, 68 total)

- **generated-attribution** — flags AI-tool attribution lines wasting context tokens
- **broken-anchor-link** — validates in-file `#anchor` links against heading slugs
- **hardcoded-windows-path** — flags backslash paths that break on non-Windows
- **command-without-codeblock** — flags bare shell commands not in code blocks *(strict-only)*
- **missing-verification-step** — flags files with workflow steps but no verification *(strict-only)*
- **long-paragraph** — flags dense text blocks (8+ consecutive prose lines) *(strict-only)*

### Editor Integration

- **LSP server** — real-time diagnostics via Language Server Protocol (`spectralint lsp`)
- **VS Code extension** — minimal client that launches the LSP server automatically
- LSP is now a **default feature** — included in all release binaries

### Correctness

- Hardened 8 checkers to reduce false positives (absolute-path, broken-anchor-link, credential-exposure, dangerous-command, dead-reference, missing-standard-file, placeholder-text, vague-directive)
- **placeholder-text**: case-sensitive TODO (only ALL CAPS), skip inline code, file references (`TODO.md`), and noun usage (`TODO items`)
- **absolute-path**: skip tilde paths to hidden config dirs (`~/.config/`, `~/.claude/`)
- **dangerous-command**: skip SQL with inline comments (educational examples)
- **placeholder-url**: skip template URLs on well-known real domains (github.com, etc.)
- **orphaned-section**: skip document titles, MediaWiki list items, separators, slash commands, and mis-parsed comment lines
- Parser improvements for more accurate non-code-block line extraction

### Benchmark

100 repos scanned — 139 findings (down from 531), 42% of repos affected. 17% had errors or warnings. 74% fewer findings than v0.3.0 through aggressive false positive reduction.

---

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
