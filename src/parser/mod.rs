pub(crate) mod types;

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena, Options};
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

use types::{Directive, FileRef, InlineSuppress, ParsedFile, Section, SuppressKind, Table};

/// Iterate over non-code-block lines, yielding `(zero_based_index, line)` pairs.
/// Fenced code blocks (lines starting with ```) are skipped entirely.
pub(crate) fn non_code_lines(lines: &[String]) -> impl Iterator<Item = (usize, &str)> {
    let mut in_code_block = false;
    lines.iter().enumerate().filter_map(move |(i, line)| {
        if line.trim().starts_with("```") {
            in_code_block = !in_code_block;
            return None;
        }
        if in_code_block {
            return None;
        }
        Some((i, line.as_str()))
    })
}

/// Iterate over lines inside fenced code blocks, yielding `(zero_based_index, line)` pairs.
/// Only lines within ``` fences are yielded (fence markers themselves are excluded).
pub(crate) fn code_block_lines(lines: &[String]) -> impl Iterator<Item = (usize, &str)> {
    let mut in_code_block = false;
    lines.iter().enumerate().filter_map(move |(i, line)| {
        if line.trim().starts_with("```") {
            in_code_block = !in_code_block;
            return None;
        }
        if in_code_block {
            return Some((i, line.as_str()));
        }
        None
    })
}

/// Returns true if a line (already outside fenced code blocks) should be
/// scanned for directives. Skips indented code blocks, blockquotes, and
/// table rows, which are content rather than instructions.
pub(crate) fn is_directive_line(line: &str) -> bool {
    let trimmed = line.trim();
    // Skip indented code blocks (but not list items)
    if line.starts_with("    ") && !trimmed.starts_with('-') && !trimmed.starts_with('*') {
        return false;
    }
    // Skip blockquotes and table rows
    if trimmed.starts_with('>') || trimmed.starts_with('|') {
        return false;
    }
    true
}

static FILE_REF_BACKTICK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+\.md)`").unwrap());
static FILE_REF_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)]+\.md)\)").unwrap());
static FILE_REF_BARE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s,|])([a-zA-Z0-9_/.:-]+\.md)(?:[\s,|]|$)").unwrap());

static SUPPRESS_COMMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<!--\s*spectralint-(disable|enable|disable-next-line)(?:\s+([\w-]+))?\s*-->")
        .unwrap()
});

/// Lines matching these patterns are descriptive or first-person discussion,
/// not directives to the agent. Skip vague-directive detection on them.
pub(crate) static NON_DIRECTIVE_CONTEXTS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\bcan\s+be\b",                         // capability: "Can be helpful"
        r"(?i)\bmay\s+be\b",                         // possibility: "May be useful"
        r"(?i)\bwe\s+(?:need|should|could|might)\b", // first person: "we need to consider"
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

static VAGUE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    // Only flag patterns that are genuinely non-deterministic for agents.
    // Removed: "when possible", "when needed", "as needed", "when necessary",
    // "consider" — these are normal conditionals and suggestions that agents
    // handle fine. No prompt engineering guide specifically calls these out.
    let patterns = [
        r"(?i)\btry to\b",
        r"(?i)\buse your judgm?ent\b",
        r"(?i)\bif appropriate\b",
        r"(?i)\bbe helpful\b",
        r"(?i)\bas appropriate\b",
    ];
    patterns.iter().map(|p| Regex::new(p).unwrap()).collect()
});

pub(crate) fn parse_file(path: &Path) -> anyhow::Result<ParsedFile> {
    let content = std::fs::read_to_string(path)?;
    let raw_lines: Vec<String> = content.lines().map(String::from).collect();

    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.table = true;
    let root = parse_document(&arena, &content, &options);

    let mut sections = Vec::new();
    let mut tables = Vec::new();
    let mut file_refs = Vec::new();
    let mut directives = Vec::new();
    let mut suppress_comments = Vec::new();

    extract_sections(root, &mut sections);
    assign_section_end_lines(&mut sections, raw_lines.len());
    extract_tables(root, &mut tables, &sections);
    extract_file_refs(&raw_lines, path, &mut file_refs);
    extract_directives(&raw_lines, &mut directives);
    extract_suppress_comments(&raw_lines, &mut suppress_comments);

    Ok(ParsedFile {
        path: path.to_path_buf(),
        sections,
        tables,
        file_refs,
        directives,
        suppress_comments,
        raw_lines,
    })
}

fn extract_sections<'a>(node: &'a comrak::nodes::AstNode<'a>, sections: &mut Vec<Section>) {
    for child in node.children() {
        {
            let data = child.data.borrow();
            if let NodeValue::Heading(heading) = &data.value {
                let title = collect_text(child);
                sections.push(Section {
                    level: heading.level,
                    title,
                    line: data.sourcepos.start.line,
                    end_line: 0,
                });
            }
        }
        extract_sections(child, sections);
    }
}

fn assign_section_end_lines(sections: &mut [Section], total_lines: usize) {
    for i in 0..sections.len() {
        sections[i].end_line = sections
            .get(i + 1)
            .map_or(total_lines, |next| next.line.saturating_sub(1));
    }
}

fn extract_tables<'a>(
    node: &'a comrak::nodes::AstNode<'a>,
    tables: &mut Vec<Table>,
    sections: &[Section],
) {
    for child in node.children() {
        {
            let data = child.data.borrow();
            if let NodeValue::Table(_) = &data.value {
                let line = data.sourcepos.start.line;
                let mut rows: Vec<Vec<String>> = Vec::new();

                for row_node in child.children() {
                    rows.push(row_node.children().map(collect_text).collect());
                }

                let mut rows_iter = rows.into_iter();
                let headers = rows_iter.next().unwrap_or_default();
                let data_rows: Vec<Vec<String>> = rows_iter.collect();

                let parent_section = sections
                    .iter()
                    .rev()
                    .find(|s| s.line < line)
                    .map(|s| s.title.clone());

                tables.push(Table {
                    headers,
                    rows: data_rows,
                    line,
                    parent_section,
                });
            }
        }
        extract_tables(child, tables, sections);
    }
}

fn collect_text<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    let mut buf = String::new();
    fn inner<'a>(node: &'a comrak::nodes::AstNode<'a>, buf: &mut String) {
        match &node.data.borrow().value {
            NodeValue::Text(t) => buf.push_str(t),
            NodeValue::Code(c) => buf.push_str(&c.literal),
            _ => {}
        }
        for child in node.children() {
            inner(child, buf);
        }
    }
    inner(node, &mut buf);
    buf
}

fn extract_file_refs(lines: &[String], source_path: &Path, refs: &mut Vec<FileRef>) {
    let push_unique = |refs: &mut Vec<FileRef>, path: String, line_num: usize| {
        if !refs.iter().any(|r| r.path == path && r.line == line_num) {
            refs.push(FileRef {
                path,
                line: line_num,
                source_file: source_path.to_path_buf(),
            });
        }
    };

    for (i, line) in non_code_lines(lines) {
        let line_num = i + 1;

        for cap in FILE_REF_BACKTICK.captures_iter(line) {
            let path = &cap[1];
            // Skip backtick captures containing whitespace (commands like `wc -c CLAUDE.md`)
            if path.contains(' ') {
                continue;
            }
            push_unique(refs, path.to_string(), line_num);
        }

        for cap in FILE_REF_LINK.captures_iter(line) {
            let path = &cap[2];
            // Skip URLs — they're not local file references
            if path.starts_with("http://") || path.starts_with("https://") {
                continue;
            }
            push_unique(refs, path.to_string(), line_num);
        }

        for cap in FILE_REF_BARE.captures_iter(line) {
            let path = &cap[1];
            // Skip URLs in bare references too
            if path.starts_with("http://") || path.starts_with("https://") {
                continue;
            }
            push_unique(refs, path.to_string(), line_num);
        }
    }
}

fn extract_directives(lines: &[String], directives: &mut Vec<Directive>) {
    for (i, line) in non_code_lines(lines) {
        if !is_directive_line(line) {
            continue;
        }

        // Skip lines that are descriptive/discussion rather than directives
        if NON_DIRECTIVE_CONTEXTS.iter().any(|p| p.is_match(line)) {
            continue;
        }

        for pattern in VAGUE_PATTERNS.iter() {
            if let Some(m) = pattern.find(line) {
                directives.push(Directive {
                    line: i + 1,
                    pattern_matched: m.as_str().to_string(),
                });
                break;
            }
        }
    }
}

fn extract_suppress_comments(lines: &[String], suppress: &mut Vec<InlineSuppress>) {
    for (i, line) in non_code_lines(lines) {
        if let Some(caps) = SUPPRESS_COMMENT.captures(line) {
            let kind = match &caps[1] {
                "disable" => SuppressKind::Disable,
                "enable" => SuppressKind::Enable,
                "disable-next-line" => SuppressKind::DisableNextLine,
                _ => continue,
            };
            let rule = caps.get(2).map(|m| m.as_str().to_string());
            suppress.push(InlineSuppress {
                line: i + 1,
                kind,
                rule,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_str(content: &str) -> ParsedFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        parse_file(f.path()).unwrap()
    }

    #[test]
    fn test_sections() {
        let parsed = parse_str("# Hello\n\nSome text\n\n## World\n\nMore text\n");
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].title, "Hello");
        assert_eq!(parsed.sections[0].level, 1);
        assert_eq!(parsed.sections[1].title, "World");
        assert_eq!(parsed.sections[1].level, 2);
    }

    #[test]
    fn test_file_refs_backtick() {
        let parsed = parse_str("Load `agent_definitions/outreach_drafter.md` for details.\n");
        assert_eq!(parsed.file_refs.len(), 1);
        assert_eq!(
            parsed.file_refs[0].path,
            "agent_definitions/outreach_drafter.md"
        );
    }

    #[test]
    fn test_file_refs_in_code_block_skipped() {
        let parsed = parse_str("```\n`some/file.md`\n```\n");
        assert_eq!(parsed.file_refs.len(), 0);
    }

    #[test]
    fn test_vague_directive() {
        let parsed = parse_str("You should try to be helpful.\n");
        assert_eq!(parsed.directives.len(), 1);
        assert_eq!(parsed.directives[0].pattern_matched, "try to");
    }

    #[test]
    fn test_vague_directive_skips_code_block() {
        let parsed = parse_str("```\ntry to do something\n```\n");
        assert_eq!(parsed.directives.len(), 0);
    }

    #[test]
    fn test_table_parsing() {
        let content =
            "## Routing\n\n| Input | Action |\n|-------|--------|\n| A | Do X |\n| B | Do Y |\n";
        let parsed = parse_str(content);
        assert_eq!(parsed.tables.len(), 1);
        assert_eq!(parsed.tables[0].headers, vec!["Input", "Action"]);
        assert_eq!(parsed.tables[0].rows.len(), 2);
        assert_eq!(parsed.tables[0].parent_section, Some("Routing".to_string()));
    }

    // ── Item 1: Parser I/O error handling ────────────────────────────────

    #[test]
    fn test_parse_nonexistent_file() {
        let result = parse_file(Path::new("/nonexistent/path/file.md"));
        assert!(
            result.is_err(),
            "Parsing a non-existent file should return Err"
        );
    }

    // ── Item 5: Vague directive skips blockquotes, tables, indented code ─

    #[test]
    fn test_vague_in_blockquote_skipped() {
        let parsed = parse_str("> Try to do something.\n");
        assert!(
            parsed.directives.is_empty(),
            "Vague patterns in blockquotes should not be flagged"
        );
    }

    #[test]
    fn test_vague_in_table_row_skipped() {
        let parsed = parse_str("| Step | Try to verify |\n");
        assert!(
            parsed.directives.is_empty(),
            "Vague patterns in table rows should not be flagged"
        );
    }

    #[test]
    fn test_vague_in_indented_code_skipped() {
        let parsed = parse_str("    Try to execute\n");
        assert!(
            parsed.directives.is_empty(),
            "Vague patterns in indented code blocks should not be flagged"
        );
    }

    #[test]
    fn test_vague_in_list_item_detected() {
        let parsed = parse_str("    - Try to complete this\n");
        assert_eq!(
            parsed.directives.len(),
            1,
            "Vague patterns in indented list items should still be flagged"
        );
    }

    #[test]
    fn test_vague_in_capability_description_skipped() {
        let parsed = parse_str("Can be helpful for system design questions.\n");
        assert!(
            parsed.directives.is_empty(),
            "\"can be\" context should suppress vague directive detection"
        );
    }

    #[test]
    fn test_vague_in_first_person_discussion_skipped() {
        let parsed = parse_str("We need to consider the best approach.\n");
        assert!(
            parsed.directives.is_empty(),
            "\"we need to\" context should suppress vague directive detection"
        );
    }

    #[test]
    fn test_vague_in_possibility_description_skipped() {
        let parsed = parse_str("This may be helpful when troubleshooting.\n");
        assert!(
            parsed.directives.is_empty(),
            "\"may be\" context should suppress vague directive detection"
        );
    }

    #[test]
    fn test_vague_in_directive_still_detected() {
        // Plain directive without non-directive context should still flag
        let parsed = parse_str("Try to follow the coding standards.\n");
        assert_eq!(
            parsed.directives.len(),
            1,
            "Plain vague directives should still be detected"
        );
    }

    // ── Item 6: File reference regex edge cases ──────────────────────────

    #[test]
    fn test_multiple_refs_same_line() {
        let parsed = parse_str("Load `a.md` and `b.md` now.\n");
        assert_eq!(parsed.file_refs.len(), 2);
    }

    #[test]
    fn test_ref_with_dots_in_path() {
        let parsed = parse_str("See `agent.definitions/scout.md`.\n");
        assert_eq!(parsed.file_refs.len(), 1);
        assert_eq!(parsed.file_refs[0].path, "agent.definitions/scout.md");
    }

    #[test]
    fn test_ref_dedup_same_line() {
        // Backtick ref and bare ref on same line should dedup
        let parsed = parse_str("Load `file.md` and also file.md here.\n");
        let unique: std::collections::HashSet<_> =
            parsed.file_refs.iter().map(|r| (&r.path, r.line)).collect();
        assert_eq!(
            unique.len(),
            1,
            "Duplicate refs on same line should be deduped"
        );
    }

    #[test]
    fn test_link_ref_detected() {
        let parsed = parse_str("See [guide](docs/guide.md) for details.\n");
        assert_eq!(parsed.file_refs.len(), 1);
        assert_eq!(parsed.file_refs[0].path, "docs/guide.md");
    }

    #[test]
    fn test_url_link_ref_skipped() {
        let parsed = parse_str("See [standard](https://code.claude.com/docs/en/skills.md) here.\n");
        assert!(
            parsed.file_refs.is_empty(),
            "URLs in markdown links should not be extracted as file refs"
        );
    }

    #[test]
    fn test_backtick_command_with_space_skipped() {
        let parsed = parse_str("Check size: `wc -c CLAUDE.md`\n");
        assert!(
            parsed.file_refs.is_empty(),
            "Backtick commands containing spaces should not be extracted as file refs"
        );
    }

    #[test]
    fn test_backtick_file_ref_without_space_still_works() {
        let parsed = parse_str("Load `docs/guide.md` for details.\n");
        assert_eq!(parsed.file_refs.len(), 1);
        assert_eq!(parsed.file_refs[0].path, "docs/guide.md");
    }

    // ── Item 16: Section end line edge cases ─────────────────────────────

    #[test]
    fn test_section_end_line_single_section() {
        let parsed = parse_str("# Only Section\n");
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].end_line, 1);
    }

    #[test]
    fn test_section_end_line_multiple_sections() {
        let parsed = parse_str("# First\n\nText\n\n# Second\n\nMore\n");
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].end_line, parsed.sections[1].line - 1);
        assert_eq!(parsed.sections[1].end_line, 7);
    }

    // ── Item 17: Table with no data rows ─────────────────────────────────

    #[test]
    fn test_table_no_data_rows() {
        let parsed = parse_str("| Header |\n|--------|\n");
        assert_eq!(parsed.tables.len(), 1);
        assert!(parsed.tables[0].rows.is_empty());
        assert_eq!(parsed.tables[0].headers, vec!["Header"]);
    }

    #[test]
    fn test_table_no_parent_section() {
        let parsed = parse_str("| Col |\n|-----|\n| val |\n");
        assert_eq!(parsed.tables.len(), 1);
        assert_eq!(parsed.tables[0].parent_section, None);
    }
}
