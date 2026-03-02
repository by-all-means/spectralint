use std::path::Path;
use std::sync::LazyLock;

use globset::GlobSet;
use regex::Regex;

use crate::engine::cross_ref::build_glob_set;
use crate::engine::scanner::matches_glob;
use crate::parser::{is_directive_line, non_code_lines_masked};

pub(crate) const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "build",
    "dist",
    ".next",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
];

/// Returns `true` if `resolved` is within `project_root` (catches `..` traversals).
/// When `canonical_root` is provided, avoids re-canonicalizing the project root.
pub(crate) fn is_within_project(
    resolved: &Path,
    canonical_root: Option<&Path>,
    project_root: &Path,
) -> bool {
    let Ok(canonical) = resolved.canonicalize() else {
        return false;
    };
    match canonical_root {
        Some(root) => canonical.starts_with(root),
        None => project_root
            .canonicalize()
            .is_ok_and(|root| canonical.starts_with(root)),
    }
}

/// A pair of contradictory directive patterns shared by `conflicting_directives`
/// and `cross_file_contradiction`.
pub(crate) struct ConflictPair {
    pub a: Regex,
    pub b: Regex,
    pub description: &'static str,
}

pub(crate) static CONFLICT_PAIRS: LazyLock<Vec<ConflictPair>> = LazyLock::new(|| {
    vec![
        // Tone
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+use\s+formal|formal\s+tone|be\s+formal)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:keep\s+it\s+casual|casual\s+tone|be\s+casual|conversational\s+tone|informal\s+tone|be\s+informal)\b").unwrap(),
            description: "tone: formal vs casual",
        },
        // API usage
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:never\s+use\s+external\s+APIs?|no\s+external\s+(?:API|service)\s+calls?|do\s+not\s+(?:call|use)\s+external)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:fetch\s+from\s+the\s+API|call\s+the\s+(?:external\s+)?API|use\s+the\s+(?:external\s+)?API)\b").unwrap(),
            description: "API usage: no external APIs vs use the API",
        },
        // File creation
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:never\s+create\s+(?:new\s+)?files?|do\s+not\s+create\s+(?:new\s+)?files?|don'?t\s+create\s+(?:new\s+)?files?)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:create\s+(?:new\s+)?files?\s+(?:as\s+needed|when\s+needed|freely)|feel\s+free\s+to\s+create)\b").unwrap(),
            description: "file creation: never create files vs create files freely",
        },
        // Confirmation behavior
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+ask\s+(?:for\s+)?confirm|require\s+(?:user\s+)?confirm|ask\s+before\s+(?:every|each|any))\w*\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:never\s+ask\s+(?:for\s+)?confirm|don'?t\s+ask\s+(?:for\s+)?confirm|proceed\s+without\s+(?:asking|confirm)|skip\s+confirm)\w*\b").unwrap(),
            description: "confirmation: always ask vs never ask",
        },
        // Verbosity
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:be\s+(?:as\s+)?(?:brief|concise|short|terse|succinct)|keep\s+(?:responses?\s+)?(?:short|concise|brief)|minimal\s+(?:output|response))\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:be\s+(?:very\s+)?(?:detailed|verbose|thorough|comprehensive|elaborate)|provide\s+(?:detailed|comprehensive|extensive|thorough)\s+(?:explanations?|responses?))\b").unwrap(),
            description: "verbosity: be concise vs be detailed",
        },
        // Resource modification
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:never\s+(?:modify|edit|change)\s+(?:existing|production)|read[\s-]only\s+(?:mode|access)|do\s+not\s+(?:modify|change)\s+existing)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:freely\s+(?:modify|edit|update)|modify\s+(?:any|all)\s+files?|full\s+write\s+access)\b").unwrap(),
            description: "resource modification: read-only vs full write access",
        },
        // Testing
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+write\s+tests?|must\s+(?:include|write|add)\s+tests?|require\s+tests?)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:skip\s+tests?|no\s+tests?\s+needed|don'?t\s+(?:write|add)\s+tests?|tests?\s+are\s+not\s+(?:needed|required|necessary))\b").unwrap(),
            description: "testing: always write tests vs skip tests",
        },
        // Comments
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:comment\s+everything|document\s+everything|add\s+comments?\s+to\s+(?:every|all)|always\s+(?:add|include)\s+comments?)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:no\s+comments?|avoid\s+comments?|code\s+should\s+be\s+self[- ]documenting|self[- ]documenting\s+code|don'?t\s+(?:add|write)\s+comments?)\b").unwrap(),
            description: "comments: comment everything vs self-documenting code",
        },
        // Dependencies
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:minimize\s+dependencies|fewer\s+dependencies|avoid\s+(?:external\s+)?dependencies|reduce\s+dependencies)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:use\s+(?:existing\s+)?libraries|don'?t\s+reinvent|prefer\s+(?:existing\s+)?(?:libraries|packages)|leverage\s+(?:existing\s+)?(?:libraries|packages))\b").unwrap(),
            description: "dependencies: minimize dependencies vs use libraries",
        },
        // Error handling
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:fail\s+fast|crash\s+on\s+error|let\s+it\s+crash|panic\s+on\s+error)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:handle\s+(?:errors?\s+)?gracefully|never\s+(?:crash|panic)|recover\s+from\s+errors?|don'?t\s+(?:crash|panic))\b").unwrap(),
            description: "error handling: fail fast vs handle gracefully",
        },
        // Autonomy
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:ask\s+before|confirm\s+with\s+(?:the\s+)?user|check\s+with\s+(?:the\s+)?user|get\s+(?:user\s+)?approval)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:work\s+autonomously|don'?t\s+ask|act\s+independently|without\s+(?:asking|confirmation)|proceed\s+independently)\b").unwrap(),
            description: "autonomy: ask before acting vs work autonomously",
        },
        // Commits
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:small\s+commits?|atomic\s+commits?|frequent\s+commits?|commit\s+(?:each|every)\s+change)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:squash\s+(?:all\s+)?commits?|single\s+commit|one\s+(?:big\s+)?commit|combine\s+(?:all\s+)?commits?)\b").unwrap(),
            description: "commits: small/atomic commits vs squash into one",
        },
        // Complexity
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:keep\s+it\s+simple|KISS|simplicity\s+first|simple\s+(?:solutions?|code)|avoid\s+(?:over[- ]?engineering|complexity))\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:optimize\s+for\s+performance|maximize\s+(?:efficiency|performance)|performance\s+(?:is\s+)?(?:critical|paramount|top\s+priority))\b").unwrap(),
            description: "complexity: keep it simple vs optimize for performance",
        },
        // Git workflow
        ConflictPair {
            a: Regex::new(r"(?i)\b(?:always\s+create\s+(?:a\s+)?(?:new\s+)?branch|work\s+on\s+(?:a\s+)?(?:feature\s+)?branch|never\s+commit\s+(?:directly\s+)?to\s+main)\b").unwrap(),
            b: Regex::new(r"(?i)\b(?:commit\s+directly\s+to\s+main|push\s+(?:directly\s+)?to\s+main|no\s+(?:feature\s+)?branch(?:es)?(?:\s+needed)?)\b").unwrap(),
            description: "git workflow: always branch vs commit to main",
        },
    ]
});

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
        || path.contains("path/to/")
        || path.starts_with('@')
        || path.starts_with("example/")
        // Placeholder segments like xxx, your_, my_, filename
        || path.split('/').any(|seg| {
            seg.starts_with("xxx")
                || seg.starts_with("your_")
                || seg.starts_with("my_")
                || seg.starts_with("filename.")
        })
        // Extension-list patterns like .ts/.tsx (not real paths)
        || path.split('/').all(|seg| seg.starts_with('.'))
}

/// Size limit (in bytes) for compiled regexes built from user-supplied patterns.
/// Prevents ReDoS via pathologically large NFA construction.
pub(crate) const REGEX_SIZE_LIMIT: usize = 1 << 20; // 1 MiB

/// Minimum number of non-empty directive lines for a file to be considered
/// a substantive instruction file (used by multiple checkers).
pub(crate) const MIN_DIRECTIVE_LINES: usize = 5;

/// Count non-empty directive lines outside code blocks.
pub(crate) fn count_directive_lines(raw_lines: &[String], mask: &[bool]) -> usize {
    non_code_lines_masked(raw_lines, mask)
        .filter(|(_, line)| is_directive_line(line) && !line.trim().is_empty())
        .count()
}

static IMPERATIVE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:must|always|never|do\s+not|don't|ensure|avoid|use|run|follow|make\s+sure|shall|need\s+to|check|verify)\b").unwrap()
});

const MIN_IMPERATIVE_LINES: usize = 3;

/// Returns `true` if the file has enough imperative content to be considered
/// an instruction file (as opposed to a context dump, activity log, or curated list).
pub(crate) fn is_instruction_file(raw_lines: &[String], mask: &[bool]) -> bool {
    let count = non_code_lines_masked(raw_lines, mask)
        .filter(|(_, line)| is_directive_line(line) && IMPERATIVE_PATTERN.is_match(line))
        .count();
    count >= MIN_IMPERATIVE_LINES
}

pub(crate) fn is_heading(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// Returns `true` if byte offset `pos` falls inside an inline backtick span.
/// Used by checkers that scan prose lines and need to skip inline code.
pub(crate) fn inside_inline_code(line: &str, pos: usize) -> bool {
    line[..pos].chars().filter(|&c| c == '`').count() % 2 == 1
}

/// Returns true if text after a regex match contains elaboration (colon, em dash, etc.),
/// indicating the matched phrase is followed by a concrete explanation.
/// Used by `generic_instruction` and `ambiguous_scope_reference`.
pub(crate) fn has_elaboration_after(line: &str, match_end: usize) -> bool {
    let rest = line[match_end..].trim_start();
    rest.starts_with(':')
        || rest.starts_with("—")
        || rest.starts_with("- ")
        || rest.starts_with("– ")
}

pub(crate) static LIST_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:[-*+]|\d+\.)\s+").unwrap());

pub(crate) fn is_bullet_line(line: &str) -> bool {
    LIST_MARKER.is_match(line)
}

/// Strip list markers, collapse whitespace, and lowercase a directive line
/// for fuzzy comparison. Used by redundant_directive and duplicate_instruction_file.
pub(crate) fn normalize_directive(line: &str) -> String {
    LIST_MARKER
        .replace(line, "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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
        let raw_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        let in_code_block = crate::parser::build_code_block_mask(&raw_lines);
        let file = ParsedFile {
            path: root.join("CLAUDE.md"),
            sections,
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines,
            in_code_block,
        };
        let canonical_root = root.canonicalize().ok();
        let filename_index = crate::engine::cross_ref::build_filename_index(root);
        let ctx = CheckerContext {
            files: vec![file],
            project_root: root.to_path_buf(),
            canonical_root,
            filename_index,
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

    /// Build a `Section` for tests. `end_line` defaults to 0 (assign manually if needed).
    pub fn section(title: &str, level: u8, line: usize) -> crate::parser::types::Section {
        crate::parser::types::Section {
            level,
            title: title.to_string(),
            line,
            end_line: 0,
        }
    }

    /// Build a `Section` with an explicit `end_line` for tests that need section ranges.
    pub fn section_with_end(
        title: &str,
        level: u8,
        line: usize,
        end_line: usize,
    ) -> crate::parser::types::Section {
        crate::parser::types::Section {
            level,
            title: title.to_string(),
            line,
            end_line,
        }
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
