//! YAML frontmatter: detection, parsing into a small value model, and
//! key-to-line lookup for diagnostics.
//!
//! The YAML crate never leaves this module. A strict parse runs first; when
//! it fails (Cursor writes unquoted `globs: *.ts`, which is alias syntax in
//! YAML), the error is recorded and a lenient flat parse of `key: value`
//! lines keeps the fields usable.

use regex::Regex;
use std::sync::LazyLock;
use yaml_rust2::{Yaml, YamlLoader};

/// A frontmatter value, decoupled from the YAML library's own type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FmValue {
    Str(String),
    Bool(bool),
    Int(i64),
    List(Vec<FmValue>),
    Map(Vec<(String, FmValue)>),
    Null,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Frontmatter {
    /// 0-based index of the opening `---`.
    pub open: usize,
    /// 0-based index of the closing `---` or `...`.
    pub close: usize,
    /// Top-level fields in document order.
    pub fields: Vec<(String, FmValue)>,
    /// Strict YAML parse failure, with the scanner's message. When set, the
    /// fields come from the lenient line-based parse instead.
    pub parse_error: Option<String>,
    /// 1-based line of each top-level key.
    key_lines: Vec<(String, usize)>,
}

/// A top-level `key:` line. The colon must be followed by whitespace or the
/// end of the line so `description:Use` (a plain scalar) is not a key.
static KEY_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z0-9_.-]+):(?:\s+(.*))?\s*$").unwrap());
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+-\s*(.*?)\s*$").unwrap());

/// Whether a line is a top-level `key:` line, as the frontmatter parser reads it.
pub(crate) fn is_key_line(line: &str) -> bool {
    KEY_LINE.is_match(line)
}

/// The frontmatter delimiter lines, if the file opens with a closed block.
/// An unclosed block is ordinary content, as it is for Claude Code.
pub(crate) fn detect_frontmatter(lines: &[String]) -> Option<(usize, usize)> {
    if lines.first()?.trim() != "---" {
        return None;
    }
    let close = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| matches!(l.trim(), "---" | "..."))?
        .0;
    // Two rules with only prose between them are horizontal rules, not a
    // frontmatter block: require at least one `key:` line.
    lines[1..close]
        .iter()
        .any(|l| KEY_LINE.is_match(l))
        .then_some((0, close))
}

/// Parse the frontmatter block, if any.
pub(crate) fn parse(lines: &[String]) -> Option<Frontmatter> {
    let (open, close) = detect_frontmatter(lines)?;
    let body = &lines[open + 1..close];
    let key_lines = body
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            KEY_LINE
                .captures(l)
                .map(|c| (c[1].to_string(), open + i + 2))
        })
        .collect();
    let (fields, parse_error) = match YamlLoader::load_from_str(&body.join("\n")) {
        Ok(docs) => (
            docs.into_iter().next().map(top_level).unwrap_or_default(),
            None,
        ),
        Err(e) => (lenient(body), Some(e.to_string())),
    };
    Some(Frontmatter {
        open,
        close,
        fields,
        parse_error,
        key_lines,
    })
}

fn top_level(doc: Yaml) -> Vec<(String, FmValue)> {
    match doc {
        Yaml::Hash(map) => map
            .into_iter()
            .filter_map(|(k, v)| key_name(&k).map(|k| (k, convert(v))))
            .collect(),
        _ => Vec::new(),
    }
}

fn key_name(key: &Yaml) -> Option<String> {
    match key {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn convert(value: Yaml) -> FmValue {
    match value {
        Yaml::String(s) | Yaml::Real(s) => FmValue::Str(s),
        Yaml::Integer(i) => FmValue::Int(i),
        Yaml::Boolean(b) => FmValue::Bool(b),
        Yaml::Array(items) => FmValue::List(items.into_iter().map(convert).collect()),
        Yaml::Hash(map) => FmValue::Map(top_level(Yaml::Hash(map))),
        Yaml::Alias(_) | Yaml::Null | Yaml::BadValue => FmValue::Null,
    }
}

/// Line-based fallback for blocks strict YAML rejects: `key: value` scalars
/// (quotes stripped, `true`/`false` as booleans) and `key:` followed by
/// indented `- item` lines as lists. Anything else is skipped.
fn lenient(body: &[String]) -> Vec<(String, FmValue)> {
    let mut fields: Vec<(String, FmValue)> = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let Some(caps) = KEY_LINE.captures(&body[i]) else {
            i += 1;
            continue;
        };
        let key = caps[1].to_string();
        let value = caps.get(2).map_or("", |m| m.as_str().trim());
        i += 1;
        if value.is_empty() {
            let mut items = Vec::new();
            while i < body.len() {
                let Some(item) = LIST_ITEM.captures(&body[i]) else {
                    break;
                };
                items.push(scalar(&item[1]));
                i += 1;
            }
            fields.push((
                key,
                if items.is_empty() {
                    FmValue::Null
                } else {
                    FmValue::List(items)
                },
            ));
        } else {
            fields.push((key, scalar(value)));
        }
    }
    fields
}

fn scalar(text: &str) -> FmValue {
    let text = text.trim();
    let unquoted = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')));
    match unquoted {
        Some(inner) => FmValue::Str(inner.to_string()),
        None => match text {
            "true" => FmValue::Bool(true),
            "false" => FmValue::Bool(false),
            _ => FmValue::Str(text.to_string()),
        },
    }
}

/// Split a comma-separated list, leaving commas inside `{a,b}` glob groups alone.
fn split_commas_outside_braces(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in s.chars() {
        match c {
            '{' => {
                depth += 1;
                current.push(c);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    out.push(current);
    out.into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

impl Frontmatter {
    pub(crate) fn get(&self, key: &str) -> Option<&FmValue> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub(crate) fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// The value if it is a string scalar.
    pub(crate) fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            FmValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The value as a boolean; the strings `true`/`false` count, since the
    /// lenient parse keeps everything textual.
    pub(crate) fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            FmValue::Bool(b) => Some(*b),
            FmValue::Str(s) if s == "true" => Some(true),
            FmValue::Str(s) if s == "false" => Some(false),
            _ => None,
        }
    }

    /// A list of strings from either a YAML list or a comma-separated
    /// scalar, which Claude Code accepts for `tools`, `paths`, and friends.
    pub(crate) fn get_str_list(&self, key: &str) -> Option<Vec<String>> {
        match self.get(key)? {
            FmValue::List(items) => Some(
                items
                    .iter()
                    .filter_map(|v| match v {
                        FmValue::Str(s) => Some(s.clone()),
                        FmValue::Int(i) => Some(i.to_string()),
                        _ => None,
                    })
                    .collect(),
            ),
            FmValue::Str(s) => Some(split_commas_outside_braces(s)),
            _ => None,
        }
    }

    /// 1-based line of a top-level key, for anchoring diagnostics.
    pub(crate) fn line_of(&self, key: &str) -> Option<usize> {
        self.key_lines
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, l)| *l)
    }

    /// Every top-level key, in document order.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|(k, _)| k.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(text: &str) -> Option<Frontmatter> {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        parse(&lines)
    }

    #[test]
    fn detects_closed_block_only() {
        assert_eq!(
            fm("---\nname: x\n---\n# Body").map(|f| (f.open, f.close)),
            Some((0, 2))
        );
        assert_eq!(
            fm("---\nname: x\n...\n# Body").map(|f| (f.open, f.close)),
            Some((0, 2))
        );
        assert!(fm("---\nname: x\n# never closed").is_none());
        assert!(fm("# Body\n---\nname: x\n---").is_none());
        assert!(fm("").is_none());
    }

    #[test]
    fn strict_parse_exposes_scalars_lists_and_maps() {
        let f = fm("---\nname: reviewer\nmodel: \"opus\"\nalwaysApply: true\nmaxTurns: 5\ntools:\n  - Read\n  - Grep\nhooks:\n  pre: check\n---")
            .unwrap();
        assert!(f.parse_error.is_none());
        assert_eq!(f.get_str("name"), Some("reviewer"));
        assert_eq!(f.get_str("model"), Some("opus"));
        assert_eq!(f.get_bool("alwaysApply"), Some(true));
        assert_eq!(f.get("maxTurns"), Some(&FmValue::Int(5)));
        assert_eq!(f.get_str_list("tools").unwrap(), vec!["Read", "Grep"]);
        assert!(matches!(f.get("hooks"), Some(FmValue::Map(_))));
        assert_eq!(f.get_str("hooks"), None);
        assert_eq!(
            f.keys().collect::<Vec<_>>(),
            vec!["name", "model", "alwaysApply", "maxTurns", "tools", "hooks"]
        );
    }

    #[test]
    fn comma_separated_scalars_are_lists_too() {
        let f = fm("---\ntools: Read, Grep,Bash\npaths: src/**\n---").unwrap();
        assert_eq!(
            f.get_str_list("tools").unwrap(),
            vec!["Read", "Grep", "Bash"]
        );
        assert_eq!(f.get_str_list("paths").unwrap(), vec!["src/**"]);
        assert_eq!(f.get_str_list("missing"), None);
    }

    #[test]
    fn block_scalars_fold() {
        let f = fm("---\ndescription: >\n  Use this skill when\n  the user asks.\n---").unwrap();
        assert!(f.parse_error.is_none());
        assert!(f
            .get_str("description")
            .unwrap()
            .starts_with("Use this skill when the user asks."));
    }

    #[test]
    fn unquoted_glob_falls_back_to_lenient_fields() {
        let f = fm("---\ndescription: React components\nglobs: *.tsx\nalwaysApply: false\n---")
            .unwrap();
        assert!(
            f.parse_error.is_some(),
            "YAML alias syntax must fail strict parsing"
        );
        assert_eq!(f.get_str("description"), Some("React components"));
        assert_eq!(f.get_str("globs"), Some("*.tsx"));
        assert_eq!(f.get_bool("alwaysApply"), Some(false));
    }

    #[test]
    fn colon_in_value_falls_back_to_lenient_fields() {
        let f = fm("---\nname: deploy\ndescription: Use when: the user says deploy\n---").unwrap();
        assert!(f.parse_error.is_some());
        assert_eq!(f.get_str("name"), Some("deploy"));
        assert_eq!(
            f.get_str("description"),
            Some("Use when: the user says deploy")
        );
    }

    #[test]
    fn lenient_parse_reads_indented_lists_and_quotes() {
        let f = fm("---\ntools:\n  - Read\n  - 'Grep'\nglobs: *.ts\nname: \"x\"\n---").unwrap();
        assert!(f.parse_error.is_some());
        assert_eq!(f.get_str_list("tools").unwrap(), vec!["Read", "Grep"]);
        assert_eq!(f.get_str("name"), Some("x"));
    }

    #[test]
    fn key_lines_are_one_based_file_lines() {
        let f = fm("---\nname: x\ndescription: y\n---\n# Body").unwrap();
        assert_eq!(f.line_of("name"), Some(2));
        assert_eq!(f.line_of("description"), Some(3));
        assert_eq!(f.line_of("nope"), None);
    }

    #[test]
    fn nested_keys_are_not_top_level() {
        let f = fm("---\ntools:\n  model: claude-2\n---").unwrap();
        assert_eq!(f.get_str("model"), None);
        assert_eq!(f.line_of("model"), None);
        assert!(f.has("tools"));
    }

    #[test]
    fn blocks_without_a_key_line_are_content() {
        assert!(fm("---\n---\n# Body").is_none());
        assert!(fm("---\n# Codacy Rules\nSome prose\n---\nMore").is_none());
    }

    #[test]
    fn brace_groups_survive_comma_splitting() {
        let f = fm("---\nglobs: \"packages/{rlp,vm}/test/**/*.ts, docs/**\"\n---").unwrap();
        assert_eq!(
            f.get_str_list("globs").unwrap(),
            vec!["packages/{rlp,vm}/test/**/*.ts", "docs/**"]
        );
    }

    #[test]
    fn crlf_input_parses() {
        let lines: Vec<String> = "---\r\nname: x\r\n---\r\n# Body\r\n"
            .lines()
            .map(String::from)
            .collect();
        let f = parse(&lines).unwrap();
        assert_eq!(f.get_str("name"), Some("x"));
        assert_eq!(f.close, 2);
    }
}
