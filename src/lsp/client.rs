//! LSP client for JSON-RPC communication with language servers.
//!
//! Implements the Language Server Protocol for completion, hover,
//! definition, references, rename, diagnostics, code actions,
//! signature help, symbols, and formatting requests.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::debug;

use super::actions::{CodeActionResult, parse_code_actions};
use super::config::LspConfig;
use super::definition::{LocationResult, parse_location_response, path_to_uri};
use super::diagnostics::DiagnosticInfo;
use super::formatting::{TextEditResult, parse_formatting_response};
use super::hover::{HoverResult, parse_hover_response};
use super::rename::{RenameRange, WorkspaceEditResult, parse_prepare_rename, parse_workspace_edit};
use super::signature::{SignatureHelpResult, parse_signature_help};
use super::symbols::{
    DocumentSymbolResult, SymbolInfoResult, parse_document_symbols, parse_workspace_symbols,
};
use crate::completion::provider::{CompletionItem, CompletionKind};

/// LSP request timeout in milliseconds.
const REQUEST_TIMEOUT_MS: u64 = 5000;

/// Maximum pending requests.
const MAX_PENDING_REQUESTS: usize = 50;

/// LSP client error types.
#[derive(Debug, Error)]
pub enum LspError {
    #[error("Failed to spawn server: {0}")]
    SpawnError(#[from] std::io::Error),

    #[error("Server process not running")]
    NotRunning,

    #[error("Request timeout")]
    Timeout,

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Server error: {code} - {message}")]
    ServerError { code: i64, message: String },

    #[error("Channel closed")]
    ChannelClosed,

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// A notification message received from the LSP server.
#[derive(Debug, Clone)]
pub struct LspNotificationMessage {
    /// Notification method name.
    pub method: String,
    /// Notification parameters.
    pub params: JsonValue,
}

/// LSP request message.
#[derive(Debug, Serialize)]
struct LspRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: JsonValue,
}

/// LSP response message.
#[derive(Debug, Deserialize)]
struct LspResponse {
    id: Option<u64>,
    result: Option<JsonValue>,
    error: Option<LspResponseError>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<JsonValue>,
}

/// LSP error response.
#[derive(Debug, Deserialize)]
struct LspResponseError {
    code: i64,
    message: String,
}

/// LSP notification message (outgoing).
#[derive(Debug, Serialize)]
struct LspNotification {
    jsonrpc: &'static str,
    method: String,
    params: JsonValue,
}

/// Pending request with response channel.
struct PendingRequest {
    response_tx: oneshot::Sender<Result<JsonValue, LspError>>,
}

/// LSP client connection to a language server.
pub struct LspClient {
    /// Language ID.
    language_id: String,

    /// Server process.
    process: Option<Child>,

    /// Next request ID.
    next_id: AtomicU64,

    /// Pending requests awaiting response.
    pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,

    /// Channel to send messages to the server.
    writer_tx: Option<mpsc::Sender<String>>,

    /// Server capabilities.
    capabilities: Option<lsp_types::ServerCapabilities>,

    /// Document versions.
    doc_versions: HashMap<PathBuf, i32>,

    /// Root path.
    root_path: PathBuf,

    /// Whether the server is initialized.
    initialized: bool,

    /// Notification receiver (taken by caller via `take_notification_rx`).
    notification_rx: Option<mpsc::Receiver<LspNotificationMessage>>,
}

impl LspClient {
    /// Spawns a new LSP client.
    pub async fn spawn(config: &LspConfig, root_path: &Path) -> Result<Self, LspError> {
        let command = config.platform_command();

        debug!("Spawning LSP server: {} {:?}", command, config.args);

        let mut child = Command::new(&command)
            .args(&config.args)
            .current_dir(root_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::SpawnError(std::io::Error::other("Failed to get stdin")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::SpawnError(std::io::Error::other("Failed to get stdout")))?;

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = Arc::clone(&pending);

        // Channel for writing to stdin
        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(64);

        // Channel for notifications from server
        let (notification_tx, notification_rx) = mpsc::channel::<LspNotificationMessage>(128);

        // Spawn writer task
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = writer_rx.recv().await {
                let header = format!("Content-Length: {}\r\n\r\n", msg.len());
                if stdin.write_all(header.as_bytes()).is_err() {
                    break;
                }
                if stdin.write_all(msg.as_bytes()).is_err() {
                    break;
                }
                if stdin.flush().is_err() {
                    break;
                }
            }
        });

        // Spawn reader task
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            Self::read_loop(reader, pending_clone, notification_tx).await;
        });

        let mut client = Self {
            language_id: config.language_id.clone(),
            process: Some(child),
            next_id: AtomicU64::new(1),
            pending,
            writer_tx: Some(writer_tx),
            capabilities: None,
            doc_versions: HashMap::new(),
            root_path: root_path.to_path_buf(),
            initialized: false,
            notification_rx: Some(notification_rx),
        };

        // Initialize the server
        client.initialize().await?;

        Ok(client)
    }

    /// Takes the notification receiver. Can only be called once.
    pub fn take_notification_rx(&mut self) -> Option<mpsc::Receiver<LspNotificationMessage>> {
        self.notification_rx.take()
    }

    /// Reads messages from the server.
    async fn read_loop(
        mut reader: BufReader<std::process::ChildStdout>,
        pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,
        notification_tx: mpsc::Sender<LspNotificationMessage>,
    ) {
        let mut content_length: usize;

        loop {
            content_length = 0;

            // Read headers
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => return, // EOF
                    Ok(_) => {
                        if line == "\r\n" || line == "\n" {
                            break;
                        }
                        if line.to_lowercase().starts_with("content-length:") {
                            if let Some(len_str) = line.split(':').nth(1) {
                                content_length = len_str.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                    Err(_) => return,
                }
            }

            if content_length == 0 {
                continue;
            }

            // Read content
            let mut content = vec![0u8; content_length];
            if reader.read_exact(&mut content).is_err() {
                return;
            }

            // Parse response
            let content_str = match String::from_utf8(content) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let response: LspResponse = match serde_json::from_str(&content_str) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Handle response with id (request response)
            if let Some(id) = response.id {
                let mut pending_guard = pending.lock().await;
                if let Some(request) = pending_guard.remove(&id) {
                    let result = if let Some(error) = response.error {
                        Err(LspError::ServerError {
                            code: error.code,
                            message: error.message,
                        })
                    } else {
                        Ok(response.result.unwrap_or(JsonValue::Null))
                    };
                    let _ = request.response_tx.send(result);
                }
            } else if let Some(method) = response.method {
                // No id = notification from server
                let params = response.params.unwrap_or(JsonValue::Null);
                let _ = notification_tx
                    .send(LspNotificationMessage { method, params })
                    .await;
            }
        }
    }

    /// Initializes the LSP connection.
    async fn initialize(&mut self) -> Result<(), LspError> {
        let root_uri = path_to_uri(&self.root_path);
        let workspace_name = self
            .root_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");

        let params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "completion": {
                        "completionItem": {
                            "snippetSupport": false,
                            "documentationFormat": ["plaintext"],
                            "resolveSupport": {
                                "properties": ["documentation", "detail"]
                            }
                        },
                        "contextSupport": true
                    },
                    "hover": {
                        "contentFormat": ["markdown", "plaintext"]
                    },
                    "definition": {
                        "linkSupport": true
                    },
                    "typeDefinition": {
                        "linkSupport": true
                    },
                    "implementation": {
                        "linkSupport": true
                    },
                    "references": {},
                    "rename": {
                        "prepareSupport": true
                    },
                    "codeAction": {
                        "codeActionLiteralSupport": {
                            "codeActionKind": {
                                "valueSet": [
                                    "quickfix",
                                    "refactor",
                                    "refactor.extract",
                                    "refactor.inline",
                                    "refactor.rewrite",
                                    "source",
                                    "source.organizeImports"
                                ]
                            }
                        },
                        "isPreferredSupport": true
                    },
                    "signatureHelp": {
                        "signatureInformation": {
                            "parameterInformation": {
                                "labelOffsetSupport": true
                            },
                            "documentationFormat": ["markdown", "plaintext"]
                        }
                    },
                    "documentSymbol": {
                        "hierarchicalDocumentSymbolSupport": true
                    },
                    "formatting": {},
                    "rangeFormatting": {},
                    "publishDiagnostics": {
                        "relatedInformation": true,
                        "codeDescriptionSupport": true
                    },
                    "synchronization": {
                        "didSave": true,
                        "willSave": false,
                        "willSaveWaitUntil": false
                    }
                },
                "workspace": {
                    "workspaceFolders": true,
                    "symbol": {
                        "symbolKind": {
                            "valueSet": [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26]
                        }
                    }
                }
            },
            "workspaceFolders": [{
                "uri": root_uri,
                "name": workspace_name
            }]
        });

        let result = self.request("initialize", params).await?;

        // Parse capabilities
        if let Ok(init_result) = serde_json::from_value::<lsp_types::InitializeResult>(result) {
            self.capabilities = Some(init_result.capabilities);
        }

        // Send initialized notification
        self.notify("initialized", json!({})).await?;
        self.initialized = true;

        debug!("LSP server {} initialized", self.language_id);
        Ok(())
    }

    /// Sends a request to the server.
    async fn request(&self, method: &str, params: JsonValue) -> Result<JsonValue, LspError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let request = LspRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let msg = serde_json::to_string(&request)?;

        // Create response channel
        let (response_tx, response_rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending.lock().await;
            if pending.len() >= MAX_PENDING_REQUESTS {
                return Err(LspError::InvalidResponse(
                    "Too many pending requests".into(),
                ));
            }
            pending.insert(id, PendingRequest { response_tx });
        }

        // Send request
        let writer = self.writer_tx.as_ref().ok_or(LspError::NotRunning)?;
        writer
            .send(msg)
            .await
            .map_err(|_| LspError::ChannelClosed)?;

        // Wait for response with timeout
        match tokio::time::timeout(
            std::time::Duration::from_millis(REQUEST_TIMEOUT_MS),
            response_rx,
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LspError::ChannelClosed),
            Err(_) => {
                // Remove from pending
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err(LspError::Timeout)
            }
        }
    }

    /// Sends a notification to the server.
    async fn notify(&self, method: &str, params: JsonValue) -> Result<(), LspError> {
        let notification = LspNotification {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
        };

        let msg = serde_json::to_string(&notification)?;
        let writer = self.writer_tx.as_ref().ok_or(LspError::NotRunning)?;
        writer
            .send(msg)
            .await
            .map_err(|_| LspError::ChannelClosed)?;
        Ok(())
    }

    // ── Document synchronization ───────────────────────────────────

    /// Notifies the server of a document open.
    pub async fn did_open(&mut self, path: &Path, content: &str) -> Result<(), LspError> {
        if !self.initialized {
            return Ok(());
        }

        let uri = path_to_uri(path);
        self.doc_versions.insert(path.to_path_buf(), 1);

        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": self.language_id,
                    "version": 1,
                    "text": content
                }
            }),
        )
        .await
    }

    /// Notifies the server of a document change.
    pub async fn did_change(&mut self, path: &Path, content: &str) -> Result<(), LspError> {
        if !self.initialized {
            return Ok(());
        }

        let uri = path_to_uri(path);
        let version = {
            let version_ref = self.doc_versions.entry(path.to_path_buf()).or_insert(0);
            *version_ref += 1;
            *version_ref
        };

        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": uri,
                    "version": version
                },
                "contentChanges": [{
                    "text": content
                }]
            }),
        )
        .await
    }

    /// Notifies the server of a document close.
    pub async fn did_close(&mut self, path: &Path) -> Result<(), LspError> {
        if !self.initialized {
            return Ok(());
        }

        let uri = path_to_uri(path);
        self.doc_versions.remove(path);

        self.notify(
            "textDocument/didClose",
            json!({
                "textDocument": {
                    "uri": uri
                }
            }),
        )
        .await
    }

    /// Notifies the server of a document save.
    pub async fn did_save(&self, path: &Path) -> Result<(), LspError> {
        if !self.initialized {
            return Ok(());
        }

        let uri = path_to_uri(path);
        self.notify(
            "textDocument/didSave",
            json!({
                "textDocument": { "uri": uri }
            }),
        )
        .await
    }

    // ── Completion ─────────────────────────────────────────────────

    /// Requests completions at the given position.
    pub async fn completion(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        trigger_char: Option<char>,
    ) -> Result<Vec<CompletionItem>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let uri = path_to_uri(path);

        let mut params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });

        if let Some(ch) = trigger_char {
            params["context"] = json!({
                "triggerKind": 2,
                "triggerCharacter": ch.to_string()
            });
        }

        let result = self.request("textDocument/completion", params).await?;
        self.parse_completion_response(result)
    }

    /// Parses the completion response into our item format.
    fn parse_completion_response(
        &self,
        result: JsonValue,
    ) -> Result<Vec<CompletionItem>, LspError> {
        let lsp_items: Vec<lsp_types::CompletionItem> = if result.is_array() {
            serde_json::from_value(result)?
        } else if let Some(list) = result.get("items") {
            serde_json::from_value(list.clone())?
        } else {
            Vec::new()
        };

        let items = lsp_items
            .into_iter()
            .map(|item| {
                let kind = item
                    .kind
                    .map(Self::lsp_kind_to_completion_kind)
                    .unwrap_or(CompletionKind::Text);

                let insert_text = item
                    .insert_text
                    .or_else(|| {
                        item.text_edit.as_ref().map(|te| match te {
                            lsp_types::CompletionTextEdit::Edit(edit) => edit.new_text.clone(),
                            lsp_types::CompletionTextEdit::InsertAndReplace(edit) => {
                                edit.new_text.clone()
                            }
                        })
                    })
                    .unwrap_or_else(|| item.label.clone());

                let detail = item.detail.or_else(|| {
                    item.documentation.as_ref().map(|doc| match doc {
                        lsp_types::Documentation::String(s) => s.clone(),
                        lsp_types::Documentation::MarkupContent(mc) => mc.value.clone(),
                    })
                });

                CompletionItem::new(insert_text, item.label, kind, "lsp".to_string())
                    .with_priority(100)
                    .with_detail(detail.unwrap_or_default())
            })
            .collect();

        Ok(items)
    }

    /// Converts LSP completion kind to our kind.
    fn lsp_kind_to_completion_kind(kind: lsp_types::CompletionItemKind) -> CompletionKind {
        use lsp_types::CompletionItemKind;
        match kind {
            CompletionItemKind::TEXT => CompletionKind::Text,
            CompletionItemKind::METHOD => CompletionKind::Method,
            CompletionItemKind::FUNCTION => CompletionKind::Function,
            CompletionItemKind::CONSTRUCTOR => CompletionKind::Constructor,
            CompletionItemKind::FIELD => CompletionKind::Field,
            CompletionItemKind::VARIABLE => CompletionKind::Variable,
            CompletionItemKind::CLASS => CompletionKind::Class,
            CompletionItemKind::INTERFACE => CompletionKind::Interface,
            CompletionItemKind::MODULE => CompletionKind::Module,
            CompletionItemKind::PROPERTY => CompletionKind::Property,
            CompletionItemKind::UNIT => CompletionKind::Unit,
            CompletionItemKind::VALUE => CompletionKind::Value,
            CompletionItemKind::ENUM => CompletionKind::Enum,
            CompletionItemKind::KEYWORD => CompletionKind::Keyword,
            CompletionItemKind::SNIPPET => CompletionKind::Snippet,
            CompletionItemKind::COLOR => CompletionKind::Color,
            CompletionItemKind::FILE => CompletionKind::File,
            CompletionItemKind::REFERENCE => CompletionKind::Reference,
            CompletionItemKind::FOLDER => CompletionKind::Folder,
            CompletionItemKind::ENUM_MEMBER => CompletionKind::EnumMember,
            CompletionItemKind::CONSTANT => CompletionKind::Constant,
            CompletionItemKind::STRUCT => CompletionKind::Struct,
            CompletionItemKind::EVENT => CompletionKind::Event,
            CompletionItemKind::OPERATOR => CompletionKind::Operator,
            CompletionItemKind::TYPE_PARAMETER => CompletionKind::TypeParameter,
            _ => CompletionKind::Text,
        }
    }

    // ── Hover ──────────────────────────────────────────────────────

    /// Requests hover info at position.
    pub async fn hover(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<HoverResult>, LspError> {
        if !self.initialized {
            return Ok(None);
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character }
        });

        let result = self.request("textDocument/hover", params).await?;
        Ok(parse_hover_response(result))
    }

    // ── Definition / Type Definition / Implementation ──────────────

    /// Go to definition.
    pub async fn goto_definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LocationResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character }
        });

        let result = self.request("textDocument/definition", params).await?;
        Ok(parse_location_response(result))
    }

    /// Go to type definition.
    pub async fn goto_type_definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LocationResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character }
        });

        let result = self.request("textDocument/typeDefinition", params).await?;
        Ok(parse_location_response(result))
    }

    /// Go to implementation.
    pub async fn goto_implementation(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LocationResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character }
        });

        let result = self.request("textDocument/implementation", params).await?;
        Ok(parse_location_response(result))
    }

    // ── References ─────────────────────────────────────────────────

    /// Find references.
    pub async fn references(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<LocationResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character },
            "context": { "includeDeclaration": include_declaration }
        });

        let result = self.request("textDocument/references", params).await?;
        Ok(parse_location_response(result))
    }

    // ── Rename ─────────────────────────────────────────────────────

    /// Prepare rename.
    pub async fn prepare_rename(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<RenameRange>, LspError> {
        if !self.initialized {
            return Ok(None);
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character }
        });

        let result = self.request("textDocument/prepareRename", params).await?;
        Ok(parse_prepare_rename(result))
    }

    /// Rename symbol.
    pub async fn rename(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<Option<WorkspaceEditResult>, LspError> {
        if !self.initialized {
            return Ok(None);
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character },
            "newName": new_name
        });

        let result = self.request("textDocument/rename", params).await?;
        Ok(parse_workspace_edit(result))
    }

    // ── Code Actions ───────────────────────────────────────────────

    /// Code actions.
    pub async fn code_action(
        &self,
        path: &Path,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        diagnostics: &[DiagnosticInfo],
    ) -> Result<Vec<CodeActionResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let diag_json: Vec<JsonValue> = diagnostics
            .iter()
            .map(|d| {
                let mut diag = json!({
                    "range": {
                        "start": { "line": d.range.start_line, "character": d.range.start_char },
                        "end": { "line": d.range.end_line, "character": d.range.end_char }
                    },
                    "message": d.message
                });
                if let Some(ref code) = d.code {
                    diag["code"] = json!(code);
                }
                if let Some(ref source) = d.source {
                    diag["source"] = json!(source);
                }
                diag
            })
            .collect();

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "range": {
                "start": { "line": start_line, "character": start_char },
                "end": { "line": end_line, "character": end_char }
            },
            "context": {
                "diagnostics": diag_json
            }
        });

        let result = self.request("textDocument/codeAction", params).await?;
        Ok(parse_code_actions(result))
    }

    // ── Signature Help ─────────────────────────────────────────────

    /// Signature help.
    pub async fn signature_help(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<SignatureHelpResult>, LspError> {
        if !self.initialized {
            return Ok(None);
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "position": { "line": line, "character": character }
        });

        let result = self.request("textDocument/signatureHelp", params).await?;
        Ok(parse_signature_help(result))
    }

    // ── Symbols ────────────────────────────────────────────────────

    /// Document symbols.
    pub async fn document_symbols(
        &self,
        path: &Path,
    ) -> Result<Vec<DocumentSymbolResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) }
        });

        let result = self.request("textDocument/documentSymbol", params).await?;
        Ok(parse_document_symbols(result))
    }

    /// Workspace symbols.
    pub async fn workspace_symbols(&self, query: &str) -> Result<Vec<SymbolInfoResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({ "query": query });

        let result = self.request("workspace/symbol", params).await?;
        Ok(parse_workspace_symbols(result))
    }

    // ── Formatting ─────────────────────────────────────────────────

    /// Format document.
    pub async fn formatting(
        &self,
        path: &Path,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<Vec<TextEditResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "options": {
                "tabSize": tab_size,
                "insertSpaces": insert_spaces
            }
        });

        let result = self.request("textDocument/formatting", params).await?;
        Ok(parse_formatting_response(result))
    }

    /// Format range.
    #[allow(clippy::too_many_arguments)]
    pub async fn range_formatting(
        &self,
        path: &Path,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<Vec<TextEditResult>, LspError> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let params = json!({
            "textDocument": { "uri": path_to_uri(path) },
            "range": {
                "start": { "line": start_line, "character": start_char },
                "end": { "line": end_line, "character": end_char }
            },
            "options": {
                "tabSize": tab_size,
                "insertSpaces": insert_spaces
            }
        });

        let result = self.request("textDocument/rangeFormatting", params).await?;
        Ok(parse_formatting_response(result))
    }

    // ── Lifecycle ──────────────────────────────────────────────────

    /// Shuts down the language server.
    pub async fn shutdown(&mut self) -> Result<(), LspError> {
        if !self.initialized {
            return Ok(());
        }

        // Send shutdown request
        let _ = self.request("shutdown", JsonValue::Null).await;

        // Send exit notification
        let _ = self.notify("exit", JsonValue::Null).await;

        // Close writer channel
        self.writer_tx.take();

        // Kill process if still running
        if let Some(ref mut process) = self.process {
            let _ = process.kill();
            let _ = process.wait();
        }

        self.initialized = false;
        debug!("LSP server {} shut down", self.language_id);
        Ok(())
    }

    /// Returns whether the server is initialized.
    #[must_use]
    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Returns the language ID.
    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Attempt graceful shutdown
        if let Some(ref mut process) = self.process {
            let _ = process.kill();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_error_display() {
        let error = LspError::Timeout;
        assert_eq!(error.to_string(), "Request timeout");

        let error = LspError::ServerError {
            code: -32600,
            message: "Invalid request".into(),
        };
        assert!(error.to_string().contains("Invalid request"));
    }

    #[test]
    fn test_lsp_kind_conversion() {
        use lsp_types::CompletionItemKind;

        assert_eq!(
            LspClient::lsp_kind_to_completion_kind(CompletionItemKind::FUNCTION),
            CompletionKind::Function
        );
        assert_eq!(
            LspClient::lsp_kind_to_completion_kind(CompletionItemKind::VARIABLE),
            CompletionKind::Variable
        );
        assert_eq!(
            LspClient::lsp_kind_to_completion_kind(CompletionItemKind::KEYWORD),
            CompletionKind::Keyword
        );
    }

    #[test]
    fn test_notification_message_fields() {
        let msg = LspNotificationMessage {
            method: "textDocument/publishDiagnostics".to_string(),
            params: json!({"uri": "file:///test.rs"}),
        };
        assert_eq!(msg.method, "textDocument/publishDiagnostics");
        assert!(msg.params.get("uri").is_some());
    }
}
