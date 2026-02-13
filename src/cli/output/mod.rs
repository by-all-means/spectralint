pub mod github;
pub mod json;
pub mod text;

use std::path::Path;

use crate::cli::OutputFormat;
use crate::types::CheckResult;

pub fn render(result: &CheckResult, project_root: &Path, format: OutputFormat) {
    match format {
        OutputFormat::Text => text::render(result, project_root),
        OutputFormat::Json => json::render(result, project_root),
        OutputFormat::Github => github::render(result, project_root),
    }
}

fn relative_path(file: &Path, project_root: &Path) -> String {
    file.strip_prefix(project_root)
        .unwrap_or(file)
        .display()
        .to_string()
}
