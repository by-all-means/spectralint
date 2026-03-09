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
            "python ",
            "python3 ",
            "pip install",
            "pip3 install",
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
        prefixes: &["make"],
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
    let lower = line.to_lowercase();
    lower.contains("docker run") || lower.contains("docker exec")
}

/// Extract commands from inline backticks in a non-code line.
static INLINE_COMMAND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());

fn toolchain_present(
    rule: &ToolchainRule,
    project_root: &std::path::Path,
    filename_index: &HashSet<String>,
) -> bool {
    // Check specific required files at project root
    for req in rule.required_files {
        if project_root.join(req).exists() {
            return true;
        }
        // Also check if file exists anywhere in tree
        if filename_index.contains(*req) {
            return true;
        }
    }

    // For .NET: check extensions in filename_index
    if rule.check_extensions {
        for ext in DOTNET_EXTENSIONS {
            if filename_index.iter().any(|f| f.ends_with(ext)) {
                return true;
            }
        }
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
    file_path: &std::path::Path,
    flagged: &mut HashSet<&'static str>,
    result: &mut CheckResult,
) {
    if is_docker_command(full_line) {
        return;
    }

    for rule in TOOLCHAIN_RULES {
        let matches = rule.prefixes.iter().any(|prefix| {
            cmd.starts_with(prefix) || cmd == prefix.trim() // exact match for "make", "pytest", etc.
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
                Arc::new(file_path.to_path_buf()),
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
}
