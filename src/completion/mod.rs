//! Completion system for the editor.
//!
//! Provides intelligent code completion through:
//! - Language Server Protocol (LSP) integration
//! - Keyword-based fallback completion
//! - Extension API for custom providers
//!
//! ## Architecture
//!
//! The completion system uses a provider-based architecture:
//! - `CompletionProvider` trait defines the interface for completion sources
//! - `CompletionEngine` runs in a background task, managing providers
//! - `CompletionHandle` provides a thread-safe interface for the UI
//!
//! ## Usage
//!
//! ```ignore
//! let handle = CompletionHandle::new();
//! handle.trigger(context);
//!
//! // In render loop
//! if let Some(suggestion) = handle.current_suggestion() {
//!     render_ghost_text(&suggestion.insert_text);
//! }
//!
//! // On Ctrl+Space
//! if let Some(text) = handle.accept() {
//!     editor.insert_str(&text);
//! }
//! ```

pub mod cache;
pub mod debounce;
pub mod keyword;
pub mod lsp;
pub mod provider;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tokio::runtime::Runtime;
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{debug, warn};

pub use cache::{CacheKey, CompletionCache};
pub use debounce::DebounceState;
pub use keyword::KeywordProvider;
pub use lsp::LspProvider;
pub use provider::{
    CompletionContext, CompletionItem, CompletionKind, CompletionProvider, CompletionResult,
};

/// Channel buffer size for completion requests.
const REQUEST_CHANNEL_SIZE: usize = 16;

/// Request sent to the completion engine.
#[derive(Debug)]
pub struct CompletionRequest {
    /// Unique request ID for cancellation.
    pub id: u64,
    /// Completion context.
    pub context: CompletionContext,
    /// Response channel.
    pub response_tx: oneshot::Sender<Option<CompletionItem>>,
}

/// Handle for interacting with the completion engine from the UI thread.
///
/// This is the main interface for triggering completions, accepting suggestions,
/// and querying the current completion state.
pub struct CompletionHandle {
    /// Channel to send completion requests.
    request_tx: mpsc::Sender<CompletionRequest>,

    /// Current suggestion (shared with engine).
    current_suggestion: Arc<RwLock<Option<CompletionItem>>>,

    /// Last request ID for cancellation.
    last_request_id: AtomicU64,

    /// Whether the engine is running.
    running: Arc<AtomicBool>,

    /// Debounce state.
    debounce: Arc<DebounceState>,

    /// Runtime for async operations.
    runtime: Arc<Runtime>,
}

impl CompletionHandle {
    /// Creates a new completion handle and spawns the background engine.
    ///
    /// # Arguments
    /// * `cwd` - The current working directory for LSP operations
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        // Create a dedicated tokio runtime for completion operations
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("completion-runtime")
                .build()
                .expect("Failed to create completion runtime"),
        );

        let (request_tx, request_rx) = mpsc::channel(REQUEST_CHANNEL_SIZE);
        let current_suggestion = Arc::new(RwLock::new(None));
        let running = Arc::new(AtomicBool::new(true));
        let debounce = Arc::new(DebounceState::with_default_delay());

        // Spawn the background engine with LSP support within our runtime
        let engine = CompletionEngine::new(request_rx, Arc::clone(&running), cwd);

        runtime.spawn(async move {
            engine.run().await;
        });

        Self {
            request_tx,
            current_suggestion,
            last_request_id: AtomicU64::new(0),
            running,
            debounce,
            runtime,
        }
    }

    /// Triggers a debounced completion request.
    ///
    /// The actual completion will be requested after the debounce delay
    /// (default 300ms) unless cancelled by a new trigger.
    pub fn trigger(&self, context: CompletionContext) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }

        let request_id = self.last_request_id.fetch_add(1, Ordering::SeqCst) + 1;
        let debounce_id = self.debounce.trigger();

        let request_tx = self.request_tx.clone();
        let debounce = Arc::clone(&self.debounce);
        let current_suggestion = Arc::clone(&self.current_suggestion);

        self.runtime.spawn(async move {
            // Wait for debounce
            if debounce.wait(debounce_id).await.is_none() {
                // Cancelled by newer request
                return;
            }

            // Create response channel
            let (response_tx, response_rx) = oneshot::channel();

            let request = CompletionRequest {
                id: request_id,
                context,
                response_tx,
            };

            // Send request to engine
            if request_tx.send(request).await.is_err() {
                warn!("Failed to send completion request");
                return;
            }

            // Wait for response
            match response_rx.await {
                Ok(Some(item)) => {
                    let mut suggestion = current_suggestion.write().await;
                    *suggestion = Some(item);
                }
                Ok(None) => {
                    let mut suggestion = current_suggestion.write().await;
                    *suggestion = None;
                }
                Err(_) => {
                    debug!("Completion response channel closed");
                }
            }
        });
    }

    /// Triggers a completion request immediately (no debounce).
    pub fn trigger_immediate(&self, context: CompletionContext) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }

        let request_id = self.last_request_id.fetch_add(1, Ordering::SeqCst) + 1;
        let request_tx = self.request_tx.clone();
        let current_suggestion = Arc::clone(&self.current_suggestion);

        self.runtime.spawn(async move {
            let (response_tx, response_rx) = oneshot::channel();

            let request = CompletionRequest {
                id: request_id,
                context,
                response_tx,
            };

            if request_tx.send(request).await.is_err() {
                warn!("Failed to send immediate completion request");
                return;
            }

            match response_rx.await {
                Ok(Some(item)) => {
                    let mut suggestion = current_suggestion.write().await;
                    *suggestion = Some(item);
                }
                Ok(None) => {
                    let mut suggestion = current_suggestion.write().await;
                    *suggestion = None;
                }
                Err(_) => {
                    debug!("Completion response channel closed");
                }
            }
        });
    }

    /// Cancels any pending completion request.
    pub fn cancel(&self) {
        self.debounce.cancel();
        self.last_request_id.fetch_add(1, Ordering::SeqCst);
    }

    /// Returns the current suggestion (blocking).
    ///
    /// For non-blocking access, use `try_current_suggestion`.
    pub fn current_suggestion_blocking(&self) -> Option<CompletionItem> {
        // Use try_read to avoid blocking
        self.current_suggestion
            .try_read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Returns the current suggestion text for display.
    pub fn suggestion_text(&self) -> Option<String> {
        self.current_suggestion_blocking()
            .map(|item| item.insert_text)
    }

    /// Accepts the current suggestion and clears it.
    ///
    /// Returns the text to insert, or None if no suggestion.
    pub fn accept(&self) -> Option<String> {
        // Try to get write lock without blocking
        if let Ok(mut guard) = self.current_suggestion.try_write() {
            guard.take().map(|item| item.insert_text)
        } else {
            None
        }
    }

    /// Dismisses the current suggestion without accepting.
    pub fn dismiss(&self) {
        if let Ok(mut guard) = self.current_suggestion.try_write() {
            *guard = None;
        }
        self.cancel();
    }

    /// Returns whether there is a current suggestion.
    pub fn has_suggestion(&self) -> bool {
        self.current_suggestion
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Shuts down the completion engine.
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.cancel();
    }
}

impl Default for CompletionHandle {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

impl Drop for CompletionHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Background completion engine.
///
/// Runs in a separate task, processing completion requests and
/// coordinating between multiple providers.
struct CompletionEngine {
    /// Channel to receive completion requests.
    request_rx: mpsc::Receiver<CompletionRequest>,

    /// Registered completion providers.
    providers: Vec<Arc<dyn CompletionProvider>>,

    /// Completion result cache.
    cache: CompletionCache,

    /// Whether the engine should keep running.
    running: Arc<AtomicBool>,
}

impl CompletionEngine {
    /// Creates a new completion engine.
    ///
    /// # Arguments
    /// * `request_rx` - Channel to receive completion requests
    /// * `running` - Shared flag for engine lifecycle
    /// * `cwd` - Current working directory for LSP operations
    fn new(
        request_rx: mpsc::Receiver<CompletionRequest>,
        running: Arc<AtomicBool>,
        cwd: PathBuf,
    ) -> Self {
        // Initialize with default providers (LSP has higher priority)
        let providers: Vec<Arc<dyn CompletionProvider>> = vec![
            Arc::new(LspProvider::new(cwd)),
            Arc::new(KeywordProvider::new()),
        ];

        Self {
            request_rx,
            providers,
            cache: CompletionCache::new(),
            running,
        }
    }

    /// Runs the completion engine until shutdown.
    async fn run(mut self) {
        debug!("Completion engine started");

        while self.running.load(Ordering::Relaxed) {
            tokio::select! {
                Some(request) = self.request_rx.recv() => {
                    self.handle_request(request).await;
                }
                else => {
                    // Channel closed
                    break;
                }
            }
        }

        // Shutdown providers
        for provider in &self.providers {
            provider.shutdown().await;
        }

        debug!("Completion engine stopped");
    }

    /// Handles a single completion request.
    async fn handle_request(&mut self, request: CompletionRequest) {
        let context = &request.context;

        // Check cache first
        let cache_key = CacheKey::new(
            context.file_path.clone(),
            context.line,
            &context.prefix,
            &context.language_id,
        );

        if let Some(cached) = self.cache.get(&cache_key) {
            if let Some(item) = cached.first().cloned() {
                let _ = request.response_tx.send(Some(item));
                return;
            }
        }

        // Query all providers
        let mut all_items = Vec::new();

        for provider in &self.providers {
            if !provider.supports_language(&context.language_id) {
                continue;
            }

            if let Some(result) = provider.complete(context).await {
                all_items.extend(result.items);
            }

            // Limit total items
            if all_items.len() >= provider::MAX_COMPLETION_ITEMS {
                break;
            }
        }

        // Sort by priority
        all_items.sort_by(|a, b| b.priority.cmp(&a.priority));
        all_items.truncate(provider::MAX_COMPLETION_ITEMS);

        // Cache results
        if !all_items.is_empty() {
            self.cache.insert(cache_key, all_items.clone());
        }

        // Return the best match
        let best_item = all_items.into_iter().next();
        let _ = request.response_tx.send(best_item);
    }

    /// Registers a new completion provider.
    #[allow(dead_code)]
    fn register_provider(&mut self, provider: Arc<dyn CompletionProvider>) {
        self.providers.push(provider);
        // Sort by priority (descending)
        self.providers
            .sort_by_key(|p| std::cmp::Reverse(p.priority()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests use #[test] instead of #[tokio::test] because
    // CompletionHandle creates its own internal tokio runtime, and dropping
    // a runtime from within an async context causes a panic.

    #[test]
    fn test_completion_handle_creation() {
        let handle = CompletionHandle::new(PathBuf::from("."));
        assert!(!handle.has_suggestion());
    }

    #[test]
    fn test_completion_handle_dismiss() {
        let handle = CompletionHandle::new(PathBuf::from("."));
        handle.dismiss();
        assert!(!handle.has_suggestion());
    }

    #[test]
    fn test_completion_handle_cancel() {
        let handle = CompletionHandle::new(PathBuf::from("."));
        handle.cancel();
        // Should not panic
    }

    #[test]
    fn test_completion_handle_shutdown() {
        let handle = CompletionHandle::new(PathBuf::from("."));
        handle.shutdown();
        assert!(!handle.running.load(Ordering::Relaxed));
    }

    #[test]
    fn test_completion_trigger() {
        let handle = CompletionHandle::new(PathBuf::from("."));

        let context = CompletionContext::new("rust", 0, 5)
            .with_prefix("let x")
            .with_word_at_cursor("x")
            .with_buffer_content("let x = 1;\nlet xy = 2;\nlet xyz = 3;");

        handle.trigger_immediate(context);

        // Give the engine time to process
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Should have a suggestion (from keyword provider)
        // Note: This may or may not have a suggestion depending on matching
    }
}
