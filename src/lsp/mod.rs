use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::config::Config;
use crate::types::{Diagnostic as SpectralDiag, Severity};

struct SpectralintServer {
    client: Client,
    workspace_root: Arc<Mutex<Option<PathBuf>>>,
    config: Arc<Mutex<Config>>,
    published_uris: Arc<Mutex<HashSet<Url>>>,
}

impl SpectralintServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            workspace_root: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(Config::default())),
            published_uris: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    async fn run_and_publish_diagnostics(&self) {
        let Some(root) = self.workspace_root.lock().await.clone() else {
            return;
        };
        let cfg = self.config.lock().await.clone();

        let run_root = root.clone();
        let result =
            tokio::task::spawn_blocking(move || crate::engine::run(&run_root, &cfg, false, None))
                .await;

        let check_result = match result {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                self.client
                    .log_message(MessageType::WARNING, format!("spectralint: {e}"))
                    .await;
                return;
            }
            Err(e) => {
                self.client
                    .log_message(MessageType::ERROR, format!("spectralint panic: {e}"))
                    .await;
                return;
            }
        };

        // Group diagnostics by file
        let mut by_file: HashMap<PathBuf, Vec<Diagnostic>> = HashMap::new();
        for d in &check_result.diagnostics {
            let abs_path = if d.file.is_absolute() {
                (*d.file).clone()
            } else {
                root.join(d.file.as_ref() as &std::path::Path)
            };
            by_file
                .entry(abs_path)
                .or_default()
                .push(to_lsp_diagnostic(d));
        }

        // Publish diagnostics for files with issues
        let mut new_uris = HashSet::new();
        for (path, diags) in by_file {
            if let Ok(uri) = Url::from_file_path(&path) {
                self.client
                    .publish_diagnostics(uri.clone(), diags, None)
                    .await;
                new_uris.insert(uri);
            }
        }

        // Clear diagnostics for files that no longer have issues
        let mut published = self.published_uris.lock().await;
        for stale_uri in published.difference(&new_uris) {
            self.client
                .publish_diagnostics(stale_uri.clone(), vec![], None)
                .await;
        }
        *published = new_uris;
    }
}

fn to_lsp_diagnostic(d: &SpectralDiag) -> Diagnostic {
    let severity = Some(match d.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
    });

    // LSP lines are 0-based; spectralint lines are 1-based
    let line = d.line.saturating_sub(1);

    let mut message = d.message.clone();
    if let Some(ref suggestion) = d.suggestion {
        message.push_str("\n\nSuggestion: ");
        message.push_str(suggestion);
    }

    Diagnostic {
        range: Range {
            start: Position::new(line as u32, 0),
            end: Position::new(line as u32, u32::MAX),
        },
        severity,
        code: Some(NumberOrString::String(d.category.to_string())),
        code_description: None,
        source: Some("spectralint".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SpectralintServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract workspace root
        if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                let cfg = Config::load(None, &path).unwrap_or_default();
                *self.workspace_root.lock().await = Some(path);
                *self.config.lock().await = cfg;
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::NONE),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(false),
                        })),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "spectralint LSP initialized")
            .await;
        self.run_and_publish_diagnostics().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, _params: DidOpenTextDocumentParams) {
        self.run_and_publish_diagnostics().await;
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        self.run_and_publish_diagnostics().await;
    }

    async fn did_change_watched_files(&self, _params: DidChangeWatchedFilesParams) {
        if let Some(root) = self.workspace_root.lock().await.clone() {
            let cfg = Config::load(None, &root).unwrap_or_default();
            *self.config.lock().await = cfg;
        }
        self.run_and_publish_diagnostics().await;
    }
}

pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(SpectralintServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Category, Severity};
    use std::path::PathBuf;

    fn make_diag(severity: Severity, line: usize, suggestion: Option<&str>) -> SpectralDiag {
        SpectralDiag {
            file: Arc::new(PathBuf::from("test.md")),
            line,
            column: None,
            end_line: None,
            end_column: None,
            severity,
            category: Category::DeadReference,
            message: "broken ref to `foo.md`".to_string(),
            suggestion: suggestion.map(String::from),
            fix: None,
        }
    }

    #[test]
    fn test_severity_mapping() {
        let d = to_lsp_diagnostic(&make_diag(Severity::Error, 5, None));
        assert_eq!(d.severity, Some(DiagnosticSeverity::ERROR));

        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 5, None));
        assert_eq!(d.severity, Some(DiagnosticSeverity::WARNING));

        let d = to_lsp_diagnostic(&make_diag(Severity::Info, 5, None));
        assert_eq!(d.severity, Some(DiagnosticSeverity::INFORMATION));
    }

    #[test]
    fn test_line_conversion() {
        // 1-based line 5 → 0-based line 4
        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 5, None));
        assert_eq!(d.range.start.line, 4);

        // line 0 edge case → stays 0
        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 0, None));
        assert_eq!(d.range.start.line, 0);

        // line 1 → 0
        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 1, None));
        assert_eq!(d.range.start.line, 0);
    }

    #[test]
    fn test_category_as_code() {
        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 1, None));
        assert_eq!(
            d.code,
            Some(NumberOrString::String("dead-reference".to_string()))
        );
    }

    #[test]
    fn test_suggestion_appended() {
        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 1, Some("use bar.md instead")));
        assert!(d.message.contains("broken ref to `foo.md`"));
        assert!(d.message.contains("Suggestion: use bar.md instead"));
    }

    #[test]
    fn test_no_suggestion() {
        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 1, None));
        assert!(!d.message.contains("Suggestion"));
    }

    #[test]
    fn test_source_is_spectralint() {
        let d = to_lsp_diagnostic(&make_diag(Severity::Warning, 1, None));
        assert_eq!(d.source, Some("spectralint".to_string()));
    }
}
