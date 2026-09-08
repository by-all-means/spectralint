# Contributing to spectralint

Thanks for your interest in contributing! Here's how to get started.

## Development Setup

```sh
git clone https://github.com/by-all-means/spectralint.git
cd spectralint
cargo build
```

Requires Rust 1.80+ (uses `std::sync::LazyLock`).

## Running Tests

```sh
cargo test          # all tests (unit + integration)
cargo clippy        # lint check
cargo fmt --check   # format check
```

All three must pass before merging. CI enforces this.

## Adding a New Checker

1. Create `src/checkers/your_checker.rs` implementing the `Checker` trait
2. Add a `Category` variant in `src/types.rs` (including the `as_str` and `FromStr` arms)
3. Add a config struct entry in `src/config/mod.rs` (default value + commented block in the TOML template)
4. Register it in `src/checkers/mod.rs` `all_checkers()`
5. Add an explanation in `src/cli/explain.rs` (both `AVAILABLE_RULES` and the `explain()` match)
6. Add unit tests in the checker file, integration tests in `tests/cli_tests.rs`, and a fixture in `tests/fixtures/` if needed
7. Battle-test against a real corpus to catch false positives before opening a PR

Look at an existing checker like `placeholder_text.rs` for the pattern.

Format-specific rules read `file.kind` (`src/file_kind.rs`) and `file.frontmatter` (`src/parser/frontmatter.rs`). To support a new tool, add its paths to the table in `src/file_kind.rs`, to `DEFAULT_INCLUDE` in `src/config/mod.rs`, and to the scanned-formats table in the README.

## Code Style

- Run `cargo fmt` before committing
- No clippy warnings (`cargo clippy -- -D warnings`)
- Prefer `anyhow::Result` for error handling
- Use `emit!` macro for creating diagnostics
- Keep checkers self-contained (one file per checker)

## Pull Requests

- Keep PRs focused on a single change
- Include tests for new functionality
- Update the CHANGELOG if adding user-facing features
- Refresh the model catalog (`src/checkers/model_catalog.rs`) against the vendors' deprecation pages when cutting a release, and bump its "as of" date
- PRs require passing CI before merge

## Reporting Issues

Open an issue on GitHub with:
- What you expected vs what happened
- The markdown file that triggered the issue (or a minimal reproduction)
- `spectralint --version` output

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
