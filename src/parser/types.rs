use std::path::PathBuf;
use std::sync::Arc;

use crate::file_kind::FileKind;

#[derive(Debug, Clone, Default)]
pub struct ParsedFile {
    pub path: Arc<PathBuf>,
    /// What kind of instruction file this is, from its path relative to the
    /// project root. `Generic` for anything not tool-specific.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "read by kind-aware checkers once the scanner sets it"
        )
    )]
    pub kind: FileKind,
    pub sections: Vec<Section>,
    pub tables: Vec<Table>,
    pub file_refs: Vec<FileRef>,
    pub directives: Vec<Directive>,
    pub suppress_comments: Vec<InlineSuppress>,
    pub raw_lines: Vec<String>,
    /// Pre-computed code block mask: `true` if line is inside a fenced code block.
    /// Fence markers themselves are marked `true` (excluded from non-code iteration).
    pub in_code_block: Vec<bool>,
}

impl ParsedFile {
    /// Build a file from in-memory lines: classifies `path` relative to `root`
    /// and computes the code-block mask. Structural fields (sections, tables,
    /// references) stay empty; callers that need them set them afterwards.
    #[cfg(test)]
    pub(crate) fn from_lines(
        path: &std::path::Path,
        root: &std::path::Path,
        lines: &[String],
    ) -> Self {
        let relative = path.strip_prefix(root).unwrap_or(path);
        Self {
            path: Arc::new(path.to_path_buf()),
            kind: crate::file_kind::classify(relative),
            in_code_block: crate::parser::build_code_block_mask(lines),
            raw_lines: lines.to_vec(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Section {
    pub level: u8,
    pub title: String,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub line: usize,
    pub parent_section: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileRef {
    pub path: String,
    pub line: usize,
    pub source_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Directive {
    pub line: usize,
    pub pattern_matched: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuppressKind {
    Disable,
    Enable,
    DisableNextLine,
}

#[derive(Debug, Clone)]
pub struct InlineSuppress {
    pub line: usize,
    pub kind: SuppressKind,
    pub rule: Option<String>,
}

impl ParsedFile {
    /// Whether line `i` is inside a fenced code block.
    /// Returns `false` when the mask is empty (test convenience).
    pub fn is_code(&self, i: usize) -> bool {
        self.in_code_block.get(i).copied().unwrap_or(false)
    }

    /// Iterate non-code lines using the pre-computed mask (O(1) per line, no fence tracking).
    pub fn non_code_lines(&self) -> impl Iterator<Item = (usize, &str)> + '_ {
        self.raw_lines
            .iter()
            .enumerate()
            .filter_map(move |(i, line)| (!self.is_code(i)).then_some((i, line.as_str())))
    }

    /// Iterate code-block lines using the pre-computed mask (excludes fence markers).
    pub fn code_block_lines(&self) -> impl Iterator<Item = (usize, &str)> + '_ {
        self.raw_lines
            .iter()
            .enumerate()
            .filter_map(move |(i, line)| {
                (self.is_code(i) && !line.trim_start().starts_with("```"))
                    .then_some((i, line.as_str()))
            })
    }
}
