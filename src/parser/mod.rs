pub(crate) mod types;

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena, Options};
use regex::Regex;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use types::{Directive, FileRef, InlineSuppress, ParsedFile, Section, SuppressKind, Table};

/// Build a pre-computed mask of which lines are inside fenced code blocks
/// or YAML frontmatter. Fence markers and frontmatter delimiters are marked
/// `true` (excluded from non-code iteration).
pub(crate) fn build_code_block_mask(lines: &[String]) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];

    // Mask YAML frontmatter (must start at line 0 with "---")
    let mut content_start = 0;
    if lines.first().is_some_and(|l| l.trim() == "---") {
        mask[0] = true;
        for (i, line) in lines.iter().enumerate().skip(1) {
            mask[i] = true;
            if line.trim() == "---" {
                content_start = i + 1;
                break;
            }
        }
    }

    // Mask fenced code blocks
    let mut in_code_block = false;
    for (i, line) in lines.iter().enumerate().skip(content_start) {
        if line.trim_start().starts_with("```") {
            mask[i] = true; // fence marker — excluded from both code and non-code
            in_code_block = !in_code_block;
        } else {
            mask[i] = in_code_block;
        }
    }
    mask
}

/// Iterate non-code lines using a pre-computed mask (O(1) per line, no fence tracking).
pub(crate) fn non_code_lines_masked<'a>(
    lines: &'a [String],
    mask: &'a [bool],
) -> impl Iterator<Item = (usize, &'a str)> {
    lines
        .iter()
        .zip(mask.iter())
        .enumerate()
        .filter_map(|(i, (line, &in_code))| (!in_code).then_some((i, line.as_str())))
}

/// Iterate non-code lines, computing fence state on the fly.
/// Used during parsing before the pre-computed mask is available.
/// Skips YAML frontmatter (lines between leading `---` delimiters).
fn non_code_lines(lines: &[String]) -> impl Iterator<Item = (usize, &str)> {
    // Pre-compute frontmatter end index
    let fm_end = if lines.first().is_some_and(|l| l.trim() == "---") {
        lines
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, l)| l.trim() == "---")
            .map(|(i, _)| i + 1)
            .unwrap_or(0)
    } else {
        0
    };

    let mut in_code_block = false;
    lines.iter().enumerate().filter_map(move |(i, line)| {
        if i < fm_end {
            return None;
        }
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            return None;
        }
        (!in_code_block).then_some((i, line.as_str()))
    })
}

/// Returns true if the line should be scanned for directives.
/// Skips indented code blocks, blockquotes, and table rows.
pub(crate) fn is_directive_line(line: &str) -> bool {
    let trimmed = line.trim();
    if line.starts_with("    ") && !trimmed.starts_with('-') && !trimmed.starts_with('*') {
        return false;
    }
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
pub(crate) static NON_DIRECTIVE_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:",
        r"\bcan\s+be\b",                          // capability: "Can be helpful"
        r"|\bmay\s+be\b",                         // possibility: "May be useful"
        r"|\bwe\s+(?:need|should|could|might)\b", // first person: "we need to consider"
        r"|^You\s+are\b",                         // identity: "You are X who..."
        r"|^You're\b",                            // identity: "You're X who..."
        r")",
    ))
    .unwrap()
});

static VAGUE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:",
        r"try to",
        r"|use your judgm?ent",
        r"|if appropriate",
        r"|be helpful",
        r"|as appropriate",
        r"|use best practices",
        r"|handle errors? properly",
        r"|handle errors? appropriately",
        r"|ensure quality",
        r")\b",
    ))
    .unwrap()
});

/// "Do not try to ..." and "try to avoid ..." are prohibitions, not vague guidance.
static NEGATED_TRY_TO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:\b(?:do\s+not|don't|never|avoid)\s+try\s+to\b|\btry\s+to\s+avoid\b)")
        .unwrap()
});

/// Detect MediaWiki markup that is not standard markdown.
/// Returns true if 3+ MediaWiki markers are found in the first 50 lines.
pub(crate) fn is_mediawiki_content(content: &str) -> bool {
    let mut count = 0;
    for (i, line) in content.lines().enumerate() {
        if i >= 50 {
            break;
        }
        // {{template}} syntax
        if line.contains("{{") && line.contains("}}") {
            count += 1;
        }
        // [[internal link]] syntax
        if line.contains("[[") && line.contains("]]") {
            count += 1;
        }
        // <ref> tags
        if line.contains("<ref>") || line.contains("<ref ") || line.contains("</ref>") {
            count += 1;
        }
        // <nowiki> tags
        if line.contains("<nowiki>") || line.contains("</nowiki>") {
            count += 1;
        }
        // {| table syntax
        if line.trim_start().starts_with("{|") {
            count += 1;
        }
        if count >= 3 {
            return true;
        }
    }
    false
}

/// Maximum file size (10 MiB) that the parser will accept.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum AST traversal depth to prevent stack overflow on crafted inputs.
const MAX_AST_DEPTH: usize = 128;

pub(crate) fn parse_file(path: &Path) -> anyhow::Result<ParsedFile> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_FILE_SIZE {
        anyhow::bail!(
            "{}: file too large ({:.1} MiB, limit is 10 MiB)",
            path.display(),
            meta.len() as f64 / (1024.0 * 1024.0)
        );
    }
    let content = std::fs::read_to_string(path)?;
    let raw_lines: Vec<String> = content.lines().map(String::from).collect();

    // Skip MediaWiki markup files (not standard markdown — causes false positives)
    if is_mediawiki_content(&content) {
        let in_code_block = build_code_block_mask(&raw_lines);
        return Ok(ParsedFile {
            path: Arc::new(path.to_path_buf()),
            sections: vec![],
            tables: vec![],
            file_refs: vec![],
            directives: vec![],
            suppress_comments: vec![],
            raw_lines,
            in_code_block,
        });
    }

    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.front_matter_delimiter = Some("---".to_owned());
    let root = parse_document(&arena, &content, &options);

    let mut sections = Vec::new();
    let mut tables = Vec::new();
    let mut file_refs = Vec::new();
    let mut directives = Vec::new();
    let mut suppress_comments = Vec::new();

    extract_sections(root, &mut sections, 0);
    assign_section_end_lines(&mut sections, raw_lines.len());
    extract_tables(root, &mut tables, &sections, 0);
    extract_file_refs(&raw_lines, path, &mut file_refs);
    extract_directives(&raw_lines, &mut directives);
    extract_suppress_comments(&raw_lines, &mut suppress_comments);

    let in_code_block = build_code_block_mask(&raw_lines);

    Ok(ParsedFile {
        path: Arc::new(path.to_path_buf()),
        sections,
        tables,
        file_refs,
        directives,
        suppress_comments,
        raw_lines,
        in_code_block,
    })
}

fn extract_sections<'a>(
    node: &'a comrak::nodes::AstNode<'a>,
    sections: &mut Vec<Section>,
    depth: usize,
) {
    if depth > MAX_AST_DEPTH {
        return;
    }
    for child in node.children() {
        let data = child.data.borrow();
        if let NodeValue::Heading(heading) = &data.value {
            let title = collect_text(child, depth + 1);
            sections.push(Section {
                level: heading.level,
                title,
                line: data.sourcepos.start.line,
                end_line: 0,
            });
        }
        drop(data);
        extract_sections(child, sections, depth + 1);
    }
}

fn assign_section_end_lines(sections: &mut [Section], total_lines: usize) {
    let next_starts: Vec<usize> = sections
        .iter()
        .skip(1)
        .map(|s| s.line.saturating_sub(1))
        .chain(std::iter::once(total_lines))
        .collect();
    for (section, end) in sections.iter_mut().zip(next_starts) {
        section.end_line = end;
    }
}

fn extract_tables<'a>(
    node: &'a comrak::nodes::AstNode<'a>,
    tables: &mut Vec<Table>,
    sections: &[Section],
    depth: usize,
) {
    if depth > MAX_AST_DEPTH {
        return;
    }
    for child in node.children() {
        let data = child.data.borrow();
        if let NodeValue::Table(_) = &data.value {
            let line = data.sourcepos.start.line;
            let mut rows: Vec<Vec<String>> = Vec::new();

            for row_node in child.children() {
                rows.push(
                    row_node
                        .children()
                        .map(|c| collect_text(c, depth + 1))
                        .collect(),
                );
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
        drop(data);
        extract_tables(child, tables, sections, depth + 1);
    }
}

fn collect_text<'a>(node: &'a comrak::nodes::AstNode<'a>, depth: usize) -> String {
    let mut buf = String::new();
    fn inner<'a>(node: &'a comrak::nodes::AstNode<'a>, buf: &mut String, depth: usize) {
        if depth > MAX_AST_DEPTH {
            return;
        }
        match &node.data.borrow().value {
            NodeValue::Text(t) => buf.push_str(t),
            NodeValue::Code(c) => buf.push_str(&c.literal),
            _ => {}
        }
        for child in node.children() {
            inner(child, buf, depth + 1);
        }
    }
    inner(node, &mut buf, depth);
    buf
}

fn is_url(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
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
            if !path.contains(' ') {
                push_unique(refs, path.to_string(), line_num);
            }
        }

        for cap in FILE_REF_LINK.captures_iter(line) {
            let path = &cap[2];
            if !is_url(path) {
                push_unique(refs, path.to_string(), line_num);
            }
        }

        for cap in FILE_REF_BARE.captures_iter(line) {
            let path = &cap[1];
            if !is_url(path) {
                push_unique(refs, path.to_string(), line_num);
            }
        }
    }
}

fn extract_directives(lines: &[String], directives: &mut Vec<Directive>) {
    for (i, line) in non_code_lines(lines) {
        if !is_directive_line(line) {
            continue;
        }

        // Headings are structural labels, not directives.
        if line.starts_with('#') {
            continue;
        }

        if NON_DIRECTIVE_CONTEXT.is_match(line) {
            continue;
        }

        if let Some(m) = VAGUE_PATTERN.find(line) {
            if m.as_str().eq_ignore_ascii_case("try to") && NEGATED_TRY_TO.is_match(line) {
                continue;
            }
            directives.push(Directive {
                line: i + 1,
                pattern_matched: m.as_str().to_string(),
            });
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
    fn test_vague_directive_negated_try_to_skipped() {
        let parsed = parse_str("Do not try to auto-fix this.\n");
        assert_eq!(
            parsed.directives.len(),
            0,
            "Negated \"try to\" is a clear prohibition, not vague guidance"
        );
    }

    #[test]
    fn test_vague_directive_try_to_avoid_skipped() {
        let parsed = parse_str("BAD (try to avoid doing this):\n");
        assert_eq!(
            parsed.directives.len(),
            0,
            "\"try to avoid\" is a prohibition, not vague guidance"
        );
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

    // ── YAML frontmatter handling ───────────────────────────────────────

    #[test]
    fn test_frontmatter_not_parsed_as_sections() {
        let parsed =
            parse_str("---\ndescription: A helpful tool\n# examples:\n---\n\n# Real Section\n");
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].title, "Real Section");
    }

    #[test]
    fn test_frontmatter_masked_in_code_block_mask() {
        let lines: Vec<String> = vec![
            "---",
            "description: foo",
            "# not a heading",
            "---",
            "",
            "# Real",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let mask = build_code_block_mask(&lines);
        assert!(mask[0], "frontmatter delimiter should be masked");
        assert!(mask[1], "frontmatter content should be masked");
        assert!(mask[2], "frontmatter comment should be masked");
        assert!(mask[3], "closing delimiter should be masked");
        assert!(!mask[4], "content after frontmatter should not be masked");
        assert!(!mask[5], "heading after frontmatter should not be masked");
    }

    #[test]
    fn test_frontmatter_directives_skipped() {
        let parsed = parse_str("---\ndescription: Try to be helpful\n---\n\nTry to be helpful.\n");
        // Only the non-frontmatter occurrence should be detected
        assert_eq!(parsed.directives.len(), 1);
        assert_eq!(parsed.directives[0].line, 5);
    }
}
