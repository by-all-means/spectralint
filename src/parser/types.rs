use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    pub tables: Vec<Table>,
    pub file_refs: Vec<FileRef>,
    pub directives: Vec<Directive>,
    pub suppress_comments: Vec<InlineSuppress>,
    pub raw_lines: Vec<String>,
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

#[derive(Debug, Clone, PartialEq)]
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
