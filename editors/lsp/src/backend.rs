//! LSP Backend implementation for Rhythm
//!
//! Implements the Language Server Protocol for the Rhythm workflow language.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::completions::{get_completions, get_signature_help, CompletionContext};
use crate::hover::get_hover_from_ast;
use crate::parser::{parse_workflow, ParseError, WorkflowDef};
use crate::validation;

/// Document state stored for each open file
#[derive(Debug, Clone)]
pub struct DocumentState {
    pub content: String,
    pub version: i32,
    pub workflow: Option<WorkflowDef>,
    pub parse_error: Option<ParseError>,
}

impl DocumentState {
    pub fn new(content: String, version: i32) -> Self {
        let (workflow, parse_error) = match parse_workflow(&content) {
            Ok(w) => (Some(w), None),
            Err(e) => (None, Some(e)),
        };

        Self {
            content,
            version,
            workflow,
            parse_error,
        }
    }

    pub fn update(&mut self, content: String, version: i32) {
        self.content = content;
        self.version = version;

        match parse_workflow(&self.content) {
            Ok(w) => {
                self.workflow = Some(w);
                self.parse_error = None;
            }
            Err(e) => {
                self.workflow = None;
                self.parse_error = Some(e);
            }
        }
    }
}

/// The Rhythm Language Server backend
pub struct RhythmBackend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
}

impl RhythmBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Publish diagnostics for a document
    async fn publish_diagnostics(&self, uri: Url) {
        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return;
        };

        let diagnostics = if let Some(err) = &doc.parse_error {
            // Parse error - report that first
            let range = if let Some(span) = &err.span {
                Range {
                    start: Position {
                        line: span.start_line as u32,
                        character: span.start_col as u32,
                    },
                    end: Position {
                        line: span.end_line as u32,
                        character: span.end_col as u32,
                    },
                }
            } else {
                Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                }
            };

            vec![Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("rhythm".to_string()),
                message: err.message.clone(),
                related_information: None,
                tags: None,
                data: None,
            }]
        } else if let Some(workflow) = &doc.workflow {
            // Parse succeeded - run semantic validation
            validation::validate_workflow(workflow, &doc.content)
        } else {
            vec![]
        };

        self.client
            .publish_diagnostics(uri, diagnostics, Some(doc.version))
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for RhythmBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: None,
                        will_save_wait_until: None,
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), "(".to_string()]),
                    resolve_provider: Some(false),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "rhythm-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Rhythm language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let version = params.text_document.version;

        let doc = DocumentState::new(content, version);
        self.documents.write().await.insert(uri.clone(), doc);
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // We use FULL sync, so there's only one change with the full content
        if let Some(change) = params.content_changes.into_iter().next() {
            let mut docs = self.documents.write().await;
            if let Some(doc) = docs.get_mut(&uri) {
                doc.update(change.text, version);
            }
        }

        self.publish_diagnostics(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        // Re-parse on save if text is included
        if let Some(text) = params.text {
            let uri = params.text_document.uri;
            let mut docs = self.documents.write().await;
            if let Some(doc) = docs.get_mut(&uri) {
                doc.update(text, doc.version);
            }
            drop(docs);
            self.publish_diagnostics(uri).await;
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let ctx = CompletionContext::from_position(&doc.content, position.line, position.character);
        let items = get_completions(&ctx);

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let hover = if let Some(workflow) = &doc.workflow {
            get_hover_from_ast(workflow, &doc.content, position.line, position.character)
        } else {
            crate::hover::get_hover(&doc.content, position.line, position.character)
        };

        Ok(hover)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let help = get_signature_help(&doc.content, position.line, position.character);
        Ok(help)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        // Get the word at position
        let lines: Vec<&str> = doc.content.lines().collect();
        let Some(line) = lines.get(position.line as usize) else {
            return Ok(None);
        };

        let char_idx = position.character as usize;
        if char_idx > line.len() {
            return Ok(None);
        }

        // Find word boundaries
        let before = &line[..char_idx];
        let after = &line[char_idx..];

        let start = before
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);

        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());

        let word = format!("{}{}", &before[start..], &after[..end]);

        if word.is_empty() {
            return Ok(None);
        }

        // Look for variable declarations
        if let Some(workflow) = &doc.workflow {
            let vars = crate::completions::collect_variables(&workflow.body);
            for (name, span) in vars {
                if name == word {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: span.start_line as u32,
                                character: span.start_col as u32,
                            },
                            end: Position {
                                line: span.end_line as u32,
                                character: span.end_col as u32,
                            },
                        },
                    })));
                }
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        // Get the word at position
        let lines: Vec<&str> = doc.content.lines().collect();
        let Some(line) = lines.get(position.line as usize) else {
            return Ok(None);
        };

        let char_idx = position.character as usize;
        if char_idx > line.len() {
            return Ok(None);
        }

        let before = &line[..char_idx];
        let after = &line[char_idx..];

        let start = before
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);

        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());

        let word = format!("{}{}", &before[start..], &after[..end]);

        if word.is_empty() {
            return Ok(None);
        }

        // Find all occurrences of the word
        let mut locations = Vec::new();
        for (line_num, line_text) in lines.iter().enumerate() {
            let mut search_start = 0;
            while let Some(idx) = line_text[search_start..].find(&word) {
                let actual_idx = search_start + idx;

                // Verify it's a word boundary
                let before_ok = actual_idx == 0
                    || !line_text[..actual_idx]
                        .chars()
                        .last()
                        .map(|c| c.is_alphanumeric() || c == '_')
                        .unwrap_or(false);

                let after_idx = actual_idx + word.len();
                let after_ok = after_idx >= line_text.len()
                    || !line_text[after_idx..]
                        .chars()
                        .next()
                        .map(|c| c.is_alphanumeric() || c == '_')
                        .unwrap_or(false);

                if before_ok && after_ok {
                    locations.push(Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: line_num as u32,
                                character: actual_idx as u32,
                            },
                            end: Position {
                                line: line_num as u32,
                                character: (actual_idx + word.len()) as u32,
                            },
                        },
                    });
                }

                search_start = actual_idx + word.len();
            }
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let Some(workflow) = &doc.workflow else {
            return Ok(None);
        };

        let vars = crate::completions::collect_variables(&workflow.body);
        let symbols: Vec<SymbolInformation> = vars
            .into_iter()
            .map(|(name, span)| {
                #[allow(deprecated)]
                SymbolInformation {
                    name,
                    kind: SymbolKind::VARIABLE,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: span.start_line as u32,
                                character: span.start_col as u32,
                            },
                            end: Position {
                                line: span.end_line as u32,
                                character: span.end_col as u32,
                            },
                        },
                    },
                    container_name: None,
                }
            })
            .collect();

        Ok(Some(DocumentSymbolResponse::Flat(symbols)))
    }
}
