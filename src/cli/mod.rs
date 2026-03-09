pub mod explain;
pub mod output;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use std::path::PathBuf;

use crate::types::Severity;

#[derive(Parser, Debug)]
#[command(
    name = "spectralint",
    version,
    about = "Static analysis for AI agent instruction files"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Lint markdown instruction files
    Check {
        /// Project root directory to scan
        path: PathBuf,

        /// Output format
        #[arg(short, long)]
        format: Option<OutputFormat>,

        /// Path to config file
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Minimum severity that causes a non-zero exit code
        #[arg(long, default_value = "error")]
        fail_on: Severity,

        /// Enable strict mode (activates opinionated checkers)
        #[arg(long)]
        strict: bool,

        /// Filter output to specific rules only (e.g., --rule dead-reference)
        #[arg(long)]
        rule: Vec<String>,

        /// Suppress normal output, only set exit code
        #[arg(short, long)]
        quiet: bool,

        /// Disable colored output (also respects NO_COLOR env var)
        #[arg(long)]
        no_color: bool,

        /// Print only a summary count (e.g., "3 errors, 12 warnings, 5 info")
        #[arg(long)]
        count: bool,

        /// Disable result caching (cache is enabled by default)
        #[arg(long)]
        no_cache: bool,

        /// Re-run on file changes (poll every 2 seconds)
        #[arg(long)]
        watch: bool,

        /// Automatically apply fixes for diagnostics that have structured fix data
        #[arg(long)]
        fix: bool,
    },
    /// Create a default .spectralintrc.toml
    Init {
        /// Configuration preset
        #[arg(long)]
        preset: Option<Preset>,
    },
    /// Explain what a checker does and why it matters (omit rule to list all)
    Explain {
        /// Checker name (e.g., dead-reference, naming-inconsistency, agent-guidelines)
        rule: Option<String>,
    },
    /// Start the Language Server Protocol server for editor integration
    #[cfg(feature = "lsp")]
    Lsp,
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Github,
    Sarif,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Preset {
    /// Dead-reference + credential-exposure only
    Minimal,
    /// Current defaults
    Standard,
    /// Standard + strict = true
    Strict,
}
