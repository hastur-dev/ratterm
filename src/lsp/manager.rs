//! LSP manager for handling multiple language server connections.
//!
//! Manages lazy startup of language servers and routes requests
//! to the appropriate server based on file type.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::actions::CodeActionResult;
use super::client::{LspClient, LspError, LspNotificationMessage};
use super::config::{LspConfigRegistry, detect_language};
use super::definition::LocationResult;
use super::diagnostics::{DiagnosticInfo, DiagnosticStore};
use super::formatting::TextEditResult;
use super::hover::HoverResult;
use super::rename::{RenameRange, WorkspaceEditResult};
use super::signature::SignatureHelpResult;
use super::symbols::{DocumentSymbolResult, SymbolInfoResult};
use crate::completion::provider::{
    CompletionContext, CompletionFuture, CompletionItem, CompletionProvider, CompletionResult,
};

/// Manager for multiple LSP clients.
pub struct LspManager {
    /// Active LSP clients by language ID.
    clients: HashMap<String, LspClient>,

    /// Configuration registry.
    configs: LspConfigRegistry,

    /// Current working directory.
    cwd: PathBuf,

    /// Whether manager is active.
    active: bool,

    /// Shared diagnostic store.
    diagnostics: DiagnosticStore,
}

impl LspManager {
    /// Creates a new LSP manager.
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            clients: HashMap::new(),
            configs: LspConfigRegistry::new(),
            cwd,
            active: true,
            diagnostics: DiagnosticStore::new(),
        }
    }

    /// Returns a reference to the diagnostic store.
    #[must_use]
    pub fn diagnostics(&self) -> &DiagnosticStore {
        &self.diagnostics
    }

    /// Starts a language server for the given language.
    pub async fn start_server(&mut self, language_id: &str) -> Result<(), LspError> {
        if self.clients.contains_key(language_id) {
            return Ok(());
        }

        let config = self
            .configs
            .get(language_id)
            .ok_or_else(|| LspError::InvalidResponse(format!("No config for {language_id}")))?
            .clone();

        info!("Starting LSP server for {}", language_id);

        match LspClient::spawn(&config, &self.cwd).await {
            Ok(mut client) => {
                // Start notification processing if receiver is available
                if let Some(rx) = client.take_notification_rx() {
                    let diag_store = self.diagnostics.clone();
                    tokio::spawn(async move {
                        Self::process_notifications(rx, diag_store).await;
                    });
                }
                self.clients.insert(language_id.to_string(), client);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to start LSP server for {}: {}", language_id, e);
                Err(e)
            }
        }
    }

    /// Processes notifications from a language server.
    async fn process_notifications(
        mut rx: tokio::sync::mpsc::Receiver<LspNotificationMessage>,
        diagnostics: DiagnosticStore,
    ) {
        while let Some(notification) = rx.recv().await {
            if notification.method == "textDocument/publishDiagnostics" {
                diagnostics.update_from_notification(&notification.params);
            }
            // Other notifications can be handled here in the future
        }
    }

    /// Returns a client for the given language, starting if needed.
    pub async fn get_client(&mut self, language_id: &str) -> Option<&mut LspClient> {
        if !self.clients.contains_key(language_id) {
            if let Err(e) = self.start_server(language_id).await {
                debug!("Could not start LSP server for {}: {}", language_id, e);
                return None;
            }
        }
        self.clients.get_mut(language_id)
    }

    /// Gets the language ID for a file path.
    #[must_use]
    pub fn language_for_file(&self, path: &Path) -> Option<String> {
        detect_language(path)
    }

    // ── Document synchronization ───────────────────────────────────

    /// Notifies the server of a document open.
    pub async fn did_open(&mut self, path: &Path, content: &str) -> Result<(), LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(()),
        };

        if let Some(client) = self.get_client(&lang).await {
            client.did_open(path, content).await?;
        }
        Ok(())
    }

    /// Notifies the server of a document change.
    pub async fn did_change(&mut self, path: &Path, content: &str) -> Result<(), LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(()),
        };

        if let Some(client) = self.clients.get_mut(&lang) {
            client.did_change(path, content).await?;
        }
        Ok(())
    }

    /// Notifies the server of a document close.
    pub async fn did_close(&mut self, path: &Path) -> Result<(), LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(()),
        };

        if let Some(client) = self.clients.get_mut(&lang) {
            client.did_close(path).await?;
        }
        Ok(())
    }

    /// Notifies the server of a document save.
    pub async fn did_save(&mut self, path: &Path) -> Result<(), LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(()),
        };

        if let Some(client) = self.clients.get_mut(&lang) {
            client.did_save(path).await?;
        }
        Ok(())
    }

    // ── Completion ─────────────────────────────────────────────────

    /// Requests completions for a file.
    pub async fn completion(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
        trigger_char: Option<char>,
    ) -> Result<Vec<CompletionItem>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => client.completion(path, line, character, trigger_char).await,
            None => Ok(Vec::new()),
        }
    }

    // ── Hover ──────────────────────────────────────────────────────

    /// Requests hover info for a file.
    pub async fn hover(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<HoverResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(None),
        };

        match self.get_client(&lang).await {
            Some(client) => client.hover(path, line, character).await,
            None => Ok(None),
        }
    }

    // ── Definition ─────────────────────────────────────────────────

    /// Go to definition.
    pub async fn goto_definition(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LocationResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => client.goto_definition(path, line, character).await,
            None => Ok(Vec::new()),
        }
    }

    /// Go to type definition.
    pub async fn goto_type_definition(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LocationResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => client.goto_type_definition(path, line, character).await,
            None => Ok(Vec::new()),
        }
    }

    /// Go to implementation.
    pub async fn goto_implementation(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LocationResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => client.goto_implementation(path, line, character).await,
            None => Ok(Vec::new()),
        }
    }

    // ── References ─────────────────────────────────────────────────

    /// Find references.
    pub async fn references(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<LocationResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => {
                client
                    .references(path, line, character, include_declaration)
                    .await
            }
            None => Ok(Vec::new()),
        }
    }

    // ── Rename ─────────────────────────────────────────────────────

    /// Prepare rename.
    pub async fn prepare_rename(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<RenameRange>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(None),
        };

        match self.get_client(&lang).await {
            Some(client) => client.prepare_rename(path, line, character).await,
            None => Ok(None),
        }
    }

    /// Rename symbol.
    pub async fn rename(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<Option<WorkspaceEditResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(None),
        };

        match self.get_client(&lang).await {
            Some(client) => client.rename(path, line, character, new_name).await,
            None => Ok(None),
        }
    }

    // ── Code Actions ───────────────────────────────────────────────

    /// Code actions.
    pub async fn code_action(
        &mut self,
        path: &Path,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        diagnostics: &[DiagnosticInfo],
    ) -> Result<Vec<CodeActionResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => {
                client
                    .code_action(path, start_line, start_char, end_line, end_char, diagnostics)
                    .await
            }
            None => Ok(Vec::new()),
        }
    }

    // ── Signature Help ─────────────────────────────────────────────

    /// Signature help.
    pub async fn signature_help(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<SignatureHelpResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(None),
        };

        match self.get_client(&lang).await {
            Some(client) => client.signature_help(path, line, character).await,
            None => Ok(None),
        }
    }

    // ── Symbols ────────────────────────────────────────────────────

    /// Document symbols.
    pub async fn document_symbols(
        &mut self,
        path: &Path,
    ) -> Result<Vec<DocumentSymbolResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => client.document_symbols(path).await,
            None => Ok(Vec::new()),
        }
    }

    /// Workspace symbols.
    pub async fn workspace_symbols(
        &mut self,
        language_id: &str,
        query: &str,
    ) -> Result<Vec<SymbolInfoResult>, LspError> {
        match self.get_client(language_id).await {
            Some(client) => client.workspace_symbols(query).await,
            None => Ok(Vec::new()),
        }
    }

    // ── Formatting ─────────────────────────────────────────────────

    /// Format document.
    pub async fn formatting(
        &mut self,
        path: &Path,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<Vec<TextEditResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => client.formatting(path, tab_size, insert_spaces).await,
            None => Ok(Vec::new()),
        }
    }

    /// Format range.
    pub async fn range_formatting(
        &mut self,
        path: &Path,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<Vec<TextEditResult>, LspError> {
        let lang = match self.language_for_file(path) {
            Some(l) => l,
            None => return Ok(Vec::new()),
        };

        match self.get_client(&lang).await {
            Some(client) => {
                client
                    .range_formatting(
                        path, start_line, start_char, end_line, end_char, tab_size, insert_spaces,
                    )
                    .await
            }
            None => Ok(Vec::new()),
        }
    }

    // ── Lifecycle ──────────────────────────────────────────────────

    /// Shuts down all language servers.
    pub async fn shutdown_all(&mut self) {
        self.active = false;

        for (lang, mut client) in self.clients.drain() {
            info!("Shutting down LSP server for {}", lang);
            if let Err(e) = client.shutdown().await {
                error!("Error shutting down LSP server for {}: {}", lang, e);
            }
        }
    }

    /// Returns whether any servers are running.
    #[must_use]
    pub fn has_active_servers(&self) -> bool {
        !self.clients.is_empty()
    }

    /// Returns the list of active language IDs.
    #[must_use]
    pub fn active_languages(&self) -> Vec<&str> {
        self.clients.keys().map(String::as_str).collect()
    }
}

impl Drop for LspManager {
    fn drop(&mut self) {
        // Clients will be dropped, which kills processes
    }
}

/// Thread-safe LSP provider wrapping the manager.
pub struct LspProvider {
    /// Shared manager.
    manager: Arc<RwLock<LspManager>>,

    /// Provider ID.
    id: String,
}

impl LspProvider {
    /// Creates a new LSP provider.
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            manager: Arc::new(RwLock::new(LspManager::new(cwd))),
            id: "lsp".to_string(),
        }
    }

    /// Returns a reference to the manager for direct operations.
    #[must_use]
    pub fn manager(&self) -> &Arc<RwLock<LspManager>> {
        &self.manager
    }
}

impl CompletionProvider for LspProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn priority(&self) -> u32 {
        100 // High priority
    }

    fn supports_language(&self, language_id: &str) -> bool {
        // LSP supports all languages that have configs
        let configs = LspConfigRegistry::new();
        configs.get(language_id).is_some()
    }

    fn complete(&self, context: &CompletionContext) -> CompletionFuture {
        let manager = Arc::clone(&self.manager);
        let file_path = context.file_path.clone();
        let line = context.line as u32;
        let col = context.col as u32;
        let trigger_char = context.trigger_char;

        Box::pin(async move {
            let path = file_path?;

            let items = {
                let mut manager = manager.write().await;
                manager
                    .completion(&path, line, col, trigger_char)
                    .await
                    .ok()?
            };

            if items.is_empty() {
                None
            } else {
                Some(CompletionResult::new("lsp", items))
            }
        })
    }

    fn shutdown(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>> {
        let manager = Arc::clone(&self.manager);
        Box::pin(async move {
            let mut manager = manager.write().await;
            manager.shutdown_all().await;
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        let manager = LspManager::new(PathBuf::from("."));

        assert_eq!(
            manager.language_for_file(Path::new("main.rs")),
            Some("rust".to_string())
        );
        assert_eq!(
            manager.language_for_file(Path::new("app.py")),
            Some("python".to_string())
        );
        assert_eq!(
            manager.language_for_file(Path::new("index.ts")),
            Some("typescript".to_string())
        );
    }

    #[test]
    fn test_lsp_provider_supports_language() {
        let provider = LspProvider::new(PathBuf::from("."));

        assert!(provider.supports_language("rust"));
        assert!(provider.supports_language("python"));
        assert!(provider.supports_language("javascript"));
        assert!(!provider.supports_language("unknown_lang"));
    }

    #[test]
    fn test_lsp_provider_priority() {
        let provider = LspProvider::new(PathBuf::from("."));
        assert_eq!(provider.priority(), 100);
    }

    #[test]
    fn test_diagnostic_store_accessible() {
        let manager = LspManager::new(PathBuf::from("."));
        assert_eq!(manager.diagnostics().total_count(), 0);
    }

    #[test]
    fn test_has_no_active_servers_initially() {
        let manager = LspManager::new(PathBuf::from("."));
        assert!(!manager.has_active_servers());
        assert!(manager.active_languages().is_empty());
    }
}
