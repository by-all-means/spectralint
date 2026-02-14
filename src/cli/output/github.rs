use std::path::Path;

use crate::types::{CheckResult, Severity};

pub fn render(result: &CheckResult, project_root: &Path) {
    for d in &result.diagnostics {
        let rel = super::relative_path(&d.file, project_root);

        let level = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "notice",
        };

        print!(
            "::{level} file={rel},line={line},title={category}::{msg}",
            line = d.line,
            category = d.category,
            msg = d.message,
        );
        if let Some(suggestion) = &d.suggestion {
            println!("%0Ahelp: {suggestion}");
        } else {
            println!();
        }
    }
}
