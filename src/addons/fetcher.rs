//! Background fetcher for non-blocking GitHub API operations.
//!
//! Runs HTTP requests in a separate thread to avoid blocking the UI.

use super::github::AddonGitHubClient;
use super::types::{Addon, AddonError, ScriptType};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tracing::{debug, info, warn};

/// Result of a background fetch operation.
#[derive(Debug)]
pub enum FetchResult {
    /// Addon list fetched successfully.
    AddonList(Vec<Addon>),
    /// Script content fetched successfully.
    Script {
        addon_id: String,
        script_type: ScriptType,
        content: String,
    },
    /// Fetch operation failed.
    Error(AddonError),
}

/// Request for a background fetch operation.
#[derive(Debug, Clone)]
pub enum FetchRequest {
    /// Fetch the list of available addons.
    FetchAddonList { force_refresh: bool },
    /// Fetch a script for an addon.
    FetchScript {
        addon_id: String,
        script_type: ScriptType,
    },
}

/// Status of the background fetcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetcherStatus {
    /// Fetcher is idle, no operation in progress.
    Idle,
    /// Currently fetching addon list.
    FetchingList,
    /// Currently fetching a script.
    FetchingScript,
}

/// Background fetcher for GitHub API operations.
///
/// Runs HTTP requests in a dedicated thread to avoid blocking the main UI thread.
pub struct BackgroundFetcher {
    /// Sender for requests to the background thread.
    request_tx: Sender<FetchRequest>,
    /// Receiver for results from the background thread.
    result_rx: Receiver<FetchResult>,
    /// Current status.
    status: Arc<Mutex<FetcherStatus>>,
    /// Handle to the background thread.
    _thread_handle: JoinHandle<()>,
}

impl BackgroundFetcher {
    /// Creates a new background fetcher.
    ///
    /// # Arguments
    /// * `repository` - GitHub repository in "owner/repo" format.
    /// * `branch` - Branch to fetch from.
    #[must_use]
    pub fn new(repository: &str, branch: &str) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<FetchRequest>();
        let (result_tx, result_rx) = mpsc::channel::<FetchResult>();
        let status = Arc::new(Mutex::new(FetcherStatus::Idle));
        let status_clone = Arc::clone(&status);

        let repo = repository.to_string();
        let br = branch.to_string();

        let thread_handle = thread::spawn(move || {
            info!("[ADDON-FETCHER] Background thread started");
            let client = AddonGitHubClient::new(&repo, &br);

            Self::run_fetch_loop(request_rx, result_tx, status_clone, client);

            info!("[ADDON-FETCHER] Background thread exiting");
        });

        Self {
            request_tx,
            result_rx,
            status,
            _thread_handle: thread_handle,
        }
    }

    /// Runs the fetch loop in the background thread.
    fn run_fetch_loop(
        request_rx: Receiver<FetchRequest>,
        result_tx: Sender<FetchResult>,
        status: Arc<Mutex<FetcherStatus>>,
        client: AddonGitHubClient,
    ) {
        // Process requests until the channel is closed
        while let Ok(request) = request_rx.recv() {
            debug!("[ADDON-FETCHER] Received request: {:?}", request);

            let result = match request {
                FetchRequest::FetchAddonList { force_refresh } => {
                    // Update status
                    if let Ok(mut s) = status.lock() {
                        *s = FetcherStatus::FetchingList;
                    }

                    info!("[ADDON-FETCHER] Fetching addon list (force={})", force_refresh);

                    match client.fetch_addons(force_refresh) {
                        Ok(addons) => {
                            info!("[ADDON-FETCHER] Fetched {} addons", addons.len());
                            FetchResult::AddonList(addons)
                        }
                        Err(e) => {
                            warn!("[ADDON-FETCHER] Failed to fetch addons: {}", e);
                            FetchResult::Error(e)
                        }
                    }
                }
                FetchRequest::FetchScript {
                    addon_id,
                    script_type,
                } => {
                    // Update status
                    if let Ok(mut s) = status.lock() {
                        *s = FetcherStatus::FetchingScript;
                    }

                    info!(
                        "[ADDON-FETCHER] Fetching script: {} {:?}",
                        addon_id, script_type
                    );

                    match client.fetch_script(&addon_id, script_type) {
                        Ok(content) => {
                            info!(
                                "[ADDON-FETCHER] Fetched script: {} bytes",
                                content.len()
                            );
                            FetchResult::Script {
                                addon_id,
                                script_type,
                                content,
                            }
                        }
                        Err(e) => {
                            warn!("[ADDON-FETCHER] Failed to fetch script: {}", e);
                            FetchResult::Error(e)
                        }
                    }
                }
            };

            // Reset status to idle
            if let Ok(mut s) = status.lock() {
                *s = FetcherStatus::Idle;
            }

            // Send result back
            if result_tx.send(result).is_err() {
                warn!("[ADDON-FETCHER] Result channel closed, exiting");
                break;
            }
        }
    }

    /// Requests fetching the addon list.
    ///
    /// This is non-blocking. Call `poll_result()` to check for completion.
    pub fn request_addon_list(&self, force_refresh: bool) {
        info!("[ADDON-FETCHER] Requesting addon list fetch");

        if let Err(e) = self.request_tx.send(FetchRequest::FetchAddonList { force_refresh }) {
            warn!("[ADDON-FETCHER] Failed to send request: {}", e);
        }
    }

    /// Requests fetching a script.
    ///
    /// This is non-blocking. Call `poll_result()` to check for completion.
    pub fn request_script(&self, addon_id: &str, script_type: ScriptType) {
        info!(
            "[ADDON-FETCHER] Requesting script fetch: {} {:?}",
            addon_id, script_type
        );

        if let Err(e) = self.request_tx.send(FetchRequest::FetchScript {
            addon_id: addon_id.to_string(),
            script_type,
        }) {
            warn!("[ADDON-FETCHER] Failed to send request: {}", e);
        }
    }

    /// Polls for a result from the background thread.
    ///
    /// Returns `Some(result)` if a result is available, `None` otherwise.
    /// This is non-blocking.
    pub fn poll_result(&self) -> Option<FetchResult> {
        match self.result_rx.try_recv() {
            Ok(result) => {
                debug!("[ADDON-FETCHER] Received result");
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                warn!("[ADDON-FETCHER] Result channel disconnected");
                None
            }
        }
    }

    /// Returns the current status of the fetcher.
    #[must_use]
    pub fn status(&self) -> FetcherStatus {
        self.status.lock().map(|s| *s).unwrap_or(FetcherStatus::Idle)
    }

    /// Returns true if the fetcher is currently busy.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.status() != FetcherStatus::Idle
    }

    /// Updates the repository and branch configuration.
    ///
    /// Note: This creates a new background thread with the new configuration.
    pub fn set_repository(&mut self, repository: &str, branch: &str) {
        info!(
            "[ADDON-FETCHER] Updating repository to {}/{}",
            repository, branch
        );

        // Create new channels and thread
        let (request_tx, request_rx) = mpsc::channel::<FetchRequest>();
        let (result_tx, result_rx) = mpsc::channel::<FetchResult>();
        let status = Arc::new(Mutex::new(FetcherStatus::Idle));
        let status_clone = Arc::clone(&status);

        let repo = repository.to_string();
        let br = branch.to_string();

        let thread_handle = thread::spawn(move || {
            info!("[ADDON-FETCHER] New background thread started");
            let client = AddonGitHubClient::new(&repo, &br);

            Self::run_fetch_loop(request_rx, result_tx, status_clone, client);

            info!("[ADDON-FETCHER] Background thread exiting");
        });

        self.request_tx = request_tx;
        self.result_rx = result_rx;
        self.status = status;
        self._thread_handle = thread_handle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_status() {
        assert_eq!(FetcherStatus::Idle, FetcherStatus::Idle);
        assert_ne!(FetcherStatus::Idle, FetcherStatus::FetchingList);
    }

    #[test]
    fn test_fetch_request_debug() {
        let req = FetchRequest::FetchAddonList { force_refresh: true };
        let debug_str = format!("{:?}", req);
        assert!(debug_str.contains("FetchAddonList"));
    }
}
