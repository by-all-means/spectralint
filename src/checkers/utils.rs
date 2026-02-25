use std::path::Path;
use std::sync::LazyLock;

use globset::GlobSet;
use regex::Regex;

use crate::engine::cross_ref::build_glob_set;
use crate::engine::scanner::matches_glob;
use crate::parser::{is_directive_line, non_code_lines};

/// Returns `true` if the file-ref path looks like a template, glob, shell
/// variable, or placeholder path that should not be resolved against disk.
///
/// Shared by `dead_reference` and `circular_reference` so the skip logic
/// stays in sync.
pub(crate) fn is_template_ref(path: &str) -> bool {
    path.contains(['*', '[', '{', '<', '>'])
        || path.starts_with('~')
        || path.starts_with('/')
        || path.contains('$')
        || path.starts_with("path/to/")
        || path.starts_with('@')
        || path.starts_with("example/")
}

/// Minimum number of non-empty directive lines for a file to be considered
/// a substantive instruction file (used by multiple checkers).
pub(crate) const MIN_DIRECTIVE_LINES: usize = 5;

/// Count non-empty directive lines outside code blocks.
pub(crate) fn count_directive_lines(raw_lines: &[String]) -> usize {
    non_code_lines(raw_lines)
        .filter(|(_, line)| is_directive_line(line) && !line.trim().is_empty())
        .count()
}

/// Imperative verbs that signal a file is giving instructions, not just describing context.
static IMPERATIVE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:must|always|never|do\s+not|don't|ensure|avoid|use|run|follow|make\s+sure|shall|need\s+to|check|verify)\b").unwrap()
});

/// Minimum imperative lines for a file to be considered an instruction file.
const MIN_IMPERATIVE_LINES: usize = 3;

/// Returns `true` if the file has enough imperative content to be considered
/// an instruction file (as opposed to a context dump, activity log, or curated list).
pub(crate) fn is_instruction_file(raw_lines: &[String]) -> bool {
    let count = non_code_lines(raw_lines)
        .filter(|(_, line)| is_directive_line(line) && IMPERATIVE_PATTERN.is_match(line))
        .count();
    count >= MIN_IMPERATIVE_LINES
}

pub(crate) struct ScopeFilter(Option<GlobSet>);

impl ScopeFilter {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        let glob_set = (!scope_patterns.is_empty()).then(|| build_glob_set(scope_patterns));
        Self(glob_set)
    }

    pub(crate) fn includes(&self, path: &Path, root: &Path) -> bool {
        self.0
            .as_ref()
            .map_or(true, |set| matches_glob(path, root, set))
    }
}

/// Normalize an identifier by splitting on `_`, `-`, space, and camelCase boundaries,
/// lowercasing all parts, and joining with `_`.
///
/// Handles all-caps acronyms: `HTTPRequest` -> `http_request`, `APIKey` -> `api_key`.
pub(crate) fn normalize(name: &str) -> String {
    let mut parts = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = name.chars().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        let c = chars[i];

        if c == '_' || c == '-' || c == ' ' {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            i += 1;
            continue;
        }

        if c.is_uppercase() {
            let mut j = i;
            while j < len && chars[j].is_uppercase() {
                j += 1;
            }
            let upper_len = j - i;

            if upper_len > 1 {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
                if j < len && chars[j].is_lowercase() {
                    let acronym: String = chars[i..j - 1].iter().collect();
                    parts.push(acronym.to_lowercase());
                    current.push(chars[j - 1].to_ascii_lowercase());
                    i = j;
                } else {
                    let acronym: String = chars[i..j].iter().collect();
                    parts.push(acronym.to_lowercase());
                    i = j;
                }
            } else {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
                current.push(c.to_ascii_lowercase());
                i += 1;
            }
        } else {
            current.push(c.to_ascii_lowercase());
            i += 1;
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts.join("_")
}

#[cfg(test)]
pub mod test_helpers {
    use crate::engine::cross_ref::CheckerContext;
    use crate::parser::types::ParsedFile;
    use crate::types::CheckResult;
    use std::collections::HashSet;

    /// Build a `CheckerContext` containing a single file with the given raw lines,
    /// located at `<tempdir>/CLAUDE.md`. Returns `(tempdir, context)` — the caller
    /// must keep `tempdir` alive for the duration of the test.
    pub fn single_file_ctx(lines: &[&str]) -> (tempfile::TempDir, CheckerContext) {
        single_file_ctx_with_sections(lines, vec![])
    }

    /// Like [`single_file_ctx`] but also attaches parsed sections.
    pub fn single_file_ctx_with_sections(
        lines: &[&str],
        sections: Vec<crate::parser::types::Section>,
    ) -> (tempfile::TempDir, CheckerContext) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections,
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines: lines.iter().map(|s| s.to_string()).collect(),
        };
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            historical_indices: HashSet::new(),
        };
        (dir, ctx)
    }

    /// Count diagnostics whose message contains the given substring.
    pub fn count_matching(result: &CheckResult, substring: &str) -> usize {
        result
            .diagnostics
            .iter()
            .filter(|d| d.message.contains(substring))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_snake_case() {
        assert_eq!(normalize("api_key"), "api_key");
    }

    #[test]
    fn test_normalize_camel_case() {
        assert_eq!(normalize("apiKey"), "api_key");
    }

    #[test]
    fn test_normalize_kebab_case() {
        assert_eq!(normalize("api-key"), "api_key");
    }

    #[test]
    fn test_normalize_spaces() {
        assert_eq!(normalize("api key"), "api_key");
    }

    #[test]
    fn test_normalize_acronym() {
        assert_eq!(normalize("HTTPRequest"), "http_request");
    }

    #[test]
    fn test_normalize_trailing_acronym() {
        assert_eq!(normalize("requestAPI"), "request_api");
    }

    #[test]
    fn test_normalize_pascal_case() {
        assert_eq!(normalize("ApiKey"), "api_key");
    }

    #[test]
    fn test_normalize_mixed() {
        assert_eq!(normalize("myAPI_key"), "my_api_key");
    }

    // ── Item 20: Normalize edge cases ────────────────────────────────────

    #[test]
    fn test_normalize_mixed_delimiters() {
        assert_eq!(normalize("api_key-name"), "api_key_name");
    }

    #[test]
    fn test_normalize_leading_trailing_delimiters() {
        assert_eq!(normalize("_api_key_"), "api_key");
    }

    #[test]
    fn test_normalize_all_lowercase() {
        assert_eq!(normalize("simple"), "simple");
    }

    #[test]
    fn test_normalize_numbers_in_identifier() {
        assert_eq!(normalize("apiV2"), "api_v2");
    }

    #[test]
    fn test_normalize_empty_string() {
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn test_normalize_single_char() {
        assert_eq!(normalize("A"), "a");
        assert_eq!(normalize("a"), "a");
    }

    #[test]
    fn test_normalize_all_caps() {
        assert_eq!(normalize("API"), "api");
    }

    #[test]
    fn test_normalize_number_after_lowercase() {
        assert_eq!(normalize("api2"), "api2");
    }

    #[test]
    fn test_normalize_all_caps_with_number() {
        assert_eq!(normalize("API2"), "api_2");
    }

    #[test]
    fn test_normalize_camel_with_number_prefix() {
        // Number followed by uppercase letter
        assert_eq!(normalize("api2Key"), "api2_key");
    }
}
