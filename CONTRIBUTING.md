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
2. Register it in `src/checkers/mod.rs`
3. Add an explanation in `src/cli/explain.rs`
4. Add a test fixture in `tests/fixtures/` if needed
5. Add unit tests in the checker file and integration tests in `tests/cli_tests.rs`

Look at an existing checker like `placeholder_text.rs` for the pattern.

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
- PRs require passing CI before merge

## Reporting Issues

Open an issue on GitHub with:
- What you expected vs what happened
- The markdown file that triggered the issue (or a minimal reproduction)
- `spectralint --version` output

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
