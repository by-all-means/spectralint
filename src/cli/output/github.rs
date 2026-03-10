use std::path::Path;

use crate::types::{CheckResult, Severity};

/// Strip characters that could inject extra annotations or workflow commands.
fn sanitize_annotation(s: &str) -> String {
    s.replace(['\r', '\n'], " ").replace("::", ": :")
}

pub fn render(result: &CheckResult, project_root: &Path) {
    for d in &result.diagnostics {
        let rel = super::relative_path(&d.file, project_root);

        let level = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "notice",
        };

        let msg = sanitize_annotation(&d.message);
        let suffix = match d.suggestion.as_deref() {
            Some(s) => format!("%0Ahelp: {}", sanitize_annotation(s)),
            None => String::new(),
        };
        let rel = sanitize_annotation(&rel);
        let category = sanitize_annotation(&d.category.to_string());
        println!(
            "::{level} file={rel},line={line},title={category}::{msg}{suffix}",
            line = d.line,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_strips_newlines() {
        assert_eq!(sanitize_annotation("line1\nline2"), "line1 line2");
        assert_eq!(sanitize_annotation("line1\rline2"), "line1 line2");
    }

    #[test]
    fn test_sanitize_breaks_colons() {
        assert_eq!(sanitize_annotation("::warning"), ": :warning");
    }
}
