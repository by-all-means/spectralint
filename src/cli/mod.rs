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
    },
    /// Create a default .spectralintrc.toml
    Init,
    /// Explain what a checker does and why it matters (omit rule to list all)
    Explain {
        /// Checker name (e.g., dead-reference, naming-inconsistency, agent-guidelines)
        rule: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Github,
}
