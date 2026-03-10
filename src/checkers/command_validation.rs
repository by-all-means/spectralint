use regex::Regex;
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{Category, CheckResult, RuleMeta, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct CommandValidationChecker {
    scope: ScopeFilter,
}

impl CommandValidationChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

struct ToolchainRule {
    prefixes: &'static [&'static str],
    label: &'static str,
    /// Files to check for at project root. If ANY exists, the toolchain is present.
    required_files: &'static [&'static str],
    /// If true, search filename_index for files with these extensions instead.
    check_extensions: bool,
}

const TOOLCHAIN_RULES: &[ToolchainRule] = &[
    ToolchainRule {
        prefixes: &[
            "cargo build",
            "cargo test",
            "cargo run",
            "cargo check",
            "cargo clippy",
            "cargo fmt",
            "cargo bench",
            "cargo install",
        ],
        label: "Cargo",
        required_files: &["Cargo.toml"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &[
            "npm run",
            "npm install",
            "npm test",
            "npm start",
            "npm ci",
            "npm exec",
            "npx ",
        ],
        label: "npm",
        required_files: &["package.json"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &[
            "yarn ",
            "yarn install",
            "yarn add",
            "yarn build",
            "yarn test",
        ],
        label: "Yarn",
        required_files: &["package.json"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["pnpm ", "pnpm install", "pnpm run", "pnpm add"],
        label: "pnpm",
        required_files: &["package.json"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["bun ", "bun run", "bun install", "bun test", "bunx "],
        label: "Bun",
        required_files: &["package.json", "bun.lockb"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &[
            "go build", "go test", "go run", "go mod", "go get", "go vet",
        ],
        label: "Go",
        required_files: &["go.mod"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &[
            "pytest",
            "pip install",
            "pip3 install",
            "python -m ",
            "python3 -m ",
        ],
        label: "Python",
        required_files: &[
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "pyproject.toml",
            "Pipfile",
        ],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["make "],
        label: "Make",
        required_files: &["Makefile", "makefile", "GNUmakefile"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["mvn ", "mvn install", "mvn test", "mvn package"],
        label: "Maven",
        required_files: &["pom.xml"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["gradle ", "gradlew", "./gradlew"],
        label: "Gradle",
        required_files: &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["dotnet ", "dotnet build", "dotnet test", "dotnet run"],
        label: ".NET",
        required_files: &[],
        check_extensions: true, // check for .csproj, .sln, .fsproj
    },
    ToolchainRule {
        prefixes: &["bundle ", "bundle install", "bundle exec", "rake "],
        label: "Ruby/Bundler",
        required_files: &["Gemfile"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["mix ", "mix deps", "mix test", "mix compile"],
        label: "Elixir",
        required_files: &["mix.exs"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &["flutter ", "dart "],
        label: "Flutter/Dart",
        required_files: &["pubspec.yaml"],
        check_extensions: false,
    },
    ToolchainRule {
        prefixes: &[
            "docker-compose ",
            "docker compose ",
            "docker-compose up",
            "docker compose up",
        ],
        label: "Docker Compose",
        required_files: &[
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ],
        check_extensions: false,
    },
];

/// .NET file extensions to check in filename_index.
const DOTNET_EXTENSIONS: &[&str] = &[".csproj", ".sln", ".fsproj", ".vbproj"];

/// Conditional language that makes a command advisory rather than required.
static CONDITIONAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:if\s+(?:using|you\s+use|you're\s+using)|optionally?|alternatively|or\s+use|when\s+using|can\s+also)\b").unwrap()
});

/// Normalize a command line: strip leading $, >, whitespace.
fn normalize_command(line: &str) -> &str {
    let s = line.trim();
    let s = s.strip_prefix("$ ").unwrap_or(s);
    let s = s.strip_prefix("> ").unwrap_or(s);
    s.trim()
}

/// Check if a line contains a docker execution prefix (commands run inside containers).
fn is_docker_command(line: &str) -> bool {
    let haystack = line.as_bytes();
    if haystack.len() < 10 {
        return false;
    }

    let is_word_boundary =
        |pos: usize| -> bool { pos >= haystack.len() || !haystack[pos].is_ascii_alphanumeric() };
    let is_start_boundary =
        |pos: usize| -> bool { pos == 0 || !haystack[pos - 1].is_ascii_alphanumeric() };

    for i in 0..=haystack.len() - 10 {
        if haystack[i..i + 10].eq_ignore_ascii_case(b"docker run")
            && is_start_boundary(i)
            && is_word_boundary(i + 10)
        {
            return true;
        }
        if i + 11 <= haystack.len()
            && haystack[i..i + 11].eq_ignore_ascii_case(b"docker exec")
            && is_start_boundary(i)
            && is_word_boundary(i + 11)
        {
            return true;
        }
    }
    false
}

/// Extract commands from inline backticks in a non-code line.
static INLINE_COMMAND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());

fn toolchain_present(
    rule: &ToolchainRule,
    project_root: &std::path::Path,
    filename_index: &HashSet<String>,
) -> bool {
    // Check specific required files (O(1) HashSet lookup before filesystem syscall)
    for req in rule.required_files {
        if filename_index.contains(*req) {
            return true;
        }
        if project_root.join(req).exists() {
            return true;
        }
    }

    // For .NET: single pass over filename_index checking all extensions
    if rule.check_extensions
        && filename_index
            .iter()
            .any(|f| DOTNET_EXTENSIONS.iter().any(|ext| f.ends_with(ext)))
    {
        return true;
    }

    false
}

impl Checker for CommandValidationChecker {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            name: "command-validation",
            description: "Flags build/test commands whose toolchain prerequisites are missing",
            default_severity: Severity::Warning,
            strict_only: false,
        }
    }

    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            // Track which toolchains we've already flagged for this file (dedup)
            let mut flagged_toolchains: HashSet<&str> = HashSet::new();

            // Scan code block lines
            for (idx, line) in file.code_block_lines() {
                let cmd = normalize_command(line);
                if cmd.is_empty() {
                    continue;
                }

                check_command(
                    cmd,
                    line,
                    idx + 1,
                    &ctx.project_root,
                    &ctx.filename_index,
                    &file.path,
                    &mut flagged_toolchains,
                    &mut result,
                );
            }

            // Scan non-code lines for inline backtick commands
            for (idx, line) in file.non_code_lines() {
                // Skip conditional lines
                if CONDITIONAL.is_match(line) {
                    continue;
                }

                for caps in INLINE_COMMAND.captures_iter(line) {
                    let cmd = normalize_command(&caps[1]);
                    check_command(
                        cmd,
                        line,
                        idx + 1,
                        &ctx.project_root,
                        &ctx.filename_index,
                        &file.path,
                        &mut flagged_toolchains,
                        &mut result,
                    );
                }
            }
        }

        result
    }
}

#[allow(clippy::too_many_arguments)]
fn check_command(
    cmd: &str,
    full_line: &str,
    line_num: usize,
    project_root: &std::path::Path,
    filename_index: &HashSet<String>,
    file_path: &Arc<std::path::PathBuf>,
    flagged: &mut HashSet<&'static str>,
    result: &mut CheckResult,
) {
    if is_docker_command(full_line) {
        return;
    }

    // Skip global npm installs (no package.json needed)
    if cmd.starts_with("npm install -g")
        || cmd.starts_with("npm install --global")
        || cmd.starts_with("npm i -g")
    {
        return;
    }

    for rule in TOOLCHAIN_RULES {
        let matches = rule.prefixes.iter().any(|prefix| {
            cmd.starts_with(prefix) || cmd == prefix.trim() // exact match for "pytest", etc.
        });

        if !matches {
            continue;
        }

        // Already flagged this toolchain for this file
        if flagged.contains(rule.label) {
            return;
        }

        if !toolchain_present(rule, project_root, filename_index) {
            flagged.insert(rule.label);
            let required_desc = if rule.required_files.is_empty() {
                "project files".to_string()
            } else {
                rule.required_files.join(" or ")
            };
            emit!(
                result,
                file_path,
                line_num,
                Severity::Warning,
                Category::CommandValidation,
                suggest: "Verify the toolchain is set up correctly or update the documented commands",
                "{} commands referenced but no {} found in project",
                rule.label,
                required_desc
            );
        }

        return; // Only match first matching rule
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn run_check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        CommandValidationChecker::new(&[]).check(&ctx)
    }

    fn run_check_with_file(lines: &[&str], create_file: &str) -> CheckResult {
        let (dir, ctx) = single_file_ctx(lines);
        let path = dir.path().join(create_file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "").unwrap();
        CommandValidationChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_cargo_without_cargo_toml() {
        let result = run_check(&["```bash", "cargo test", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Cargo"));
    }

    #[test]
    fn test_cargo_with_cargo_toml() {
        let result = run_check_with_file(&["```bash", "cargo test", "```"], "Cargo.toml");
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_npm_without_package_json() {
        let result = run_check(&["```bash", "npm install", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("npm"));
    }

    #[test]
    fn test_npm_with_package_json() {
        let result = run_check_with_file(&["```bash", "npm install", "```"], "package.json");
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_go_without_go_mod() {
        let result = run_check(&["```bash", "go test ./...", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Go"));
    }

    #[test]
    fn test_make_without_makefile() {
        let result = run_check(&["```bash", "make build", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Make"));
    }

    #[test]
    fn test_make_with_makefile() {
        let result = run_check_with_file(&["```bash", "make build", "```"], "Makefile");
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_inline_backtick_command() {
        let result = run_check(&["Run `cargo test` before committing"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Cargo"));
    }

    #[test]
    fn test_conditional_command_no_flag() {
        let result = run_check(&["If you're using Go, run `go test ./...`"]);
        assert!(
            result.diagnostics.is_empty(),
            "Conditional commands should not flag"
        );
    }

    #[test]
    fn test_docker_context_no_flag() {
        let result = run_check(&["```bash", "docker run myimage cargo test", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Commands inside docker context should not flag"
        );
    }

    #[test]
    fn test_dedup_same_toolchain() {
        let result = run_check(&[
            "```bash",
            "cargo build",
            "cargo test",
            "cargo clippy",
            "```",
        ]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Should only flag once per toolchain per file"
        );
    }

    #[test]
    fn test_no_commands_no_flag() {
        let result = run_check(&["# Build Instructions", "Follow the README for setup."]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_dollar_prefix_stripped() {
        let result = run_check(&["```bash", "$ cargo test", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Cargo"));
    }

    #[test]
    fn test_pytest_without_python_project() {
        let result = run_check(&["```bash", "pytest", "```"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert!(result.diagnostics[0].message.contains("Python"));
    }

    #[test]
    fn test_pytest_with_pyproject() {
        let result = run_check_with_file(&["```bash", "pytest", "```"], "pyproject.toml");
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_npm_global_install_no_flag() {
        let result = run_check(&["```bash", "npm install -g firebase-tools", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Global npm installs should not require package.json"
        );
    }

    #[test]
    fn test_npm_global_install_long_flag_no_flag() {
        let result = run_check(&["```bash", "npm install --global typescript", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "Global npm installs (--global) should not require package.json"
        );
    }

    #[test]
    fn test_make_colon_colon_no_flag() {
        // Rust module path `make::something` should not trigger Make rule
        let result = run_check(&[
            "```rust",
            "make::js_identifier_binding(make::ident(\"x\"))",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Rust module paths like make:: should not trigger Make rule"
        );
    }

    #[test]
    fn test_python_script_no_flag() {
        // Bare `python script.py` should not require dependency manifest
        let result = run_check(&["```bash", "python scripts/update-docs.py", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "python script.py should not require requirements.txt"
        );
    }

    #[test]
    fn test_python_dash_c_no_flag() {
        let result = run_check(&[
            "```bash",
            "python -c \"import sys; print(sys.version)\"",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "python -c one-liners should not require requirements.txt"
        );
    }

    #[test]
    fn test_pip_install_still_flags() {
        let result = run_check(&["```bash", "pip install requests", "```"]);
        assert_eq!(result.diagnostics.len(), 1, "pip install should still flag");
    }

    #[test]
    fn test_python_dash_m_still_flags() {
        let result = run_check(&["```bash", "python -m pytest", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "python -m should still flag as it implies project tooling"
        );
    }

    #[test]
    fn test_command_with_pipe() {
        // Piped commands: the first segment is what matters for toolchain detection
        let result = run_check(&["```bash", "cargo test 2>&1 | grep FAILED", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Piped cargo command should still flag without Cargo.toml"
        );
        assert!(result.diagnostics[0].message.contains("Cargo"));
    }

    #[test]
    fn test_command_with_and_chaining() {
        // && chained commands: first command in the chain should still trigger
        let result = run_check(&["```bash", "cargo build && cargo test", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Chained cargo commands should flag once (dedup)"
        );
        assert!(result.diagnostics[0].message.contains("Cargo"));
    }

    #[test]
    fn test_command_in_non_shell_code_block() {
        // Commands inside a python code block — these are code block lines,
        // but `make build` in a python block should still be detected if it matches
        let result = run_check(&[
            "```python",
            "import subprocess",
            "subprocess.run(['make', 'build'])",
            "```",
        ]);
        // "make build" won't match because the line is "subprocess.run(['make', 'build'])"
        // which doesn't start with "make " — this should not flag
        assert!(
            result.diagnostics.is_empty(),
            "Python code referencing make indirectly should not flag: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_npm_install_g_short_no_flag() {
        let result = run_check(&["```bash", "npm i -g typescript", "```"]);
        assert!(
            result.diagnostics.is_empty(),
            "npm i -g should not require package.json"
        );
    }

    #[test]
    fn test_make_double_colon_rust_path_no_flag() {
        // Additional test: bare `make::` at start of line in a Rust code block
        let result = run_check(&[
            "```rust",
            "let node = make::js_string_literal(\"hello\");",
            "```",
        ]);
        assert!(
            result.diagnostics.is_empty(),
            "Rust paths starting with make:: should not trigger Make checker"
        );
    }

    #[test]
    fn test_pipe_with_npm_no_package_json() {
        let result = run_check(&["```bash", "npm run build | tee build.log", "```"]);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "Piped npm command should still flag without package.json"
        );
        assert!(result.diagnostics[0].message.contains("npm"));
    }
}
