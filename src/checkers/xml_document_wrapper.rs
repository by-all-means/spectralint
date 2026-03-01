use crate::emit;
use crate::engine::cross_ref::CheckerContext;
use crate::parser::non_code_lines_masked;
use crate::types::{Category, CheckResult, Severity};

use super::utils::ScopeFilter;
use super::Checker;

pub(crate) struct XmlDocumentWrapperChecker {
    scope: ScopeFilter,
}

impl XmlDocumentWrapperChecker {
    pub(crate) fn new(scope_patterns: &[String]) -> Self {
        Self {
            scope: ScopeFilter::new(scope_patterns),
        }
    }
}

impl Checker for XmlDocumentWrapperChecker {
    fn check(&self, ctx: &CheckerContext) -> CheckResult {
        let mut result = CheckResult::default();

        for file in &ctx.files {
            if !self.scope.includes(&file.path, &ctx.project_root) {
                continue;
            }

            for (i, line) in non_code_lines_masked(&file.raw_lines, &file.in_code_block) {
                let line_num = i + 1;
                let trimmed = line.trim();

                // XML declaration
                if trimmed.starts_with("<?xml") {
                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Warning,
                        Category::XmlDocumentWrapper,
                        suggest: "Remove the XML wrapper — markdown files should not contain XML declarations",
                        "XML declaration in markdown — likely an AI output artifact"
                    );
                    continue;
                }

                // Document/Content wrapper tags (common AI output artifacts)
                if matches!(
                    trimmed,
                    "<Document>"
                        | "</Document>"
                        | "<Content>"
                        | "</Content>"
                        | "<Instructions>"
                        | "</Instructions>"
                        | "<Response>"
                        | "</Response>"
                ) {
                    let tag = trimmed;
                    emit!(
                        result,
                        file.path,
                        line_num,
                        Severity::Warning,
                        Category::XmlDocumentWrapper,
                        suggest: "Remove the XML wrapper tag — this is an AI output artifact",
                        "XML wrapper tag `{tag}` in markdown — likely copied from AI output"
                    );
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers::utils::test_helpers::single_file_ctx;

    fn check(lines: &[&str]) -> CheckResult {
        let (_dir, ctx) = single_file_ctx(lines);
        XmlDocumentWrapperChecker::new(&[]).check(&ctx)
    }

    #[test]
    fn test_xml_declaration_flagged() {
        let result = check(&["<?xml version=\"1.0\" encoding=\"UTF-8\"?>"]);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].category, Category::XmlDocumentWrapper);
    }

    #[test]
    fn test_document_tags_flagged() {
        let result = check(&["<Document>", "Some content", "</Document>"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_content_tags_flagged() {
        let result = check(&["<Content>", "Some text", "</Content>"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_instructions_tags_flagged() {
        let result = check(&["<Instructions>", "Do this", "</Instructions>"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_response_tags_flagged() {
        let result = check(&["<Response>", "Here is the answer", "</Response>"]);
        assert_eq!(result.diagnostics.len(), 2);
    }

    #[test]
    fn test_xml_in_code_block_not_flagged() {
        let result = check(&[
            "```xml",
            "<?xml version=\"1.0\"?>",
            "<Document>",
            "</Document>",
            "```",
        ]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_normal_html_not_flagged() {
        let result = check(&["<!-- comment -->", "<div>not flagged</div>"]);
        assert_eq!(result.diagnostics.len(), 0);
    }

    #[test]
    fn test_partial_tag_not_flagged() {
        // Tags with attributes or not exact matches shouldn't be flagged
        let result = check(&["<Document class=\"main\">"]);
        assert_eq!(result.diagnostics.len(), 0);
    }
}
