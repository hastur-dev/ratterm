//! Docker manager operations.

use tracing::{info, warn};

use crate::docker::{DockerDiscovery, DockerHost, DockerHostManager};
use crate::ui::docker_manager::{DockerItemDisplay, DockerManagerMode, DockerManagerSelector};
use crate::ui::popup::PopupKind;

use super::{App, AppMode};

impl App {
    /// Shows the Docker manager popup.
    pub fn show_docker_manager(&mut self) {
        // Initialize Docker manager if not already
        if self.docker_manager.is_none() {
            self.docker_manager = Some(DockerManagerSelector::new());
        }

        // Load Docker items if not loaded
        if !self.docker_storage.is_initialized() {
            match self.docker_storage.load() {
                Ok(items) => {
                    self.docker_items = items;
                }
                Err(e) => {
                    self.set_status(format!("Failed to load Docker settings: {}", e));
                }
            }
        }

        // Start discovery
        self.refresh_docker_discovery();

        // Show popup
        self.popup.set_kind(PopupKind::DockerManager);
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Hides the Docker manager popup.
    pub fn hide_docker_manager(&mut self) {
        self.popup.hide();
        self.mode = AppMode::Normal;
        self.request_redraw();
    }

    /// Toggles the hotkey overlay for the Docker manager.
    pub fn toggle_hotkey_overlay_docker(&mut self) {
        use crate::app::dashboard_hotkeys::docker_manager_list_hotkeys;
        use crate::ui::hotkey_overlay::HotkeyOverlay;

        if self.hotkey_overlay.as_ref().is_some_and(|o| o.is_visible()) {
            self.hotkey_overlay = None;
        } else {
            self.hotkey_overlay = Some(HotkeyOverlay::new(docker_manager_list_hotkeys()));
        }
    }

    /// Refreshes Docker container/image discovery.
    pub fn refresh_docker_discovery(&mut self) {
        info!("refresh_docker_discovery: starting");

        if let Some(ref mut manager) = self.docker_manager {
            manager.set_mode(DockerManagerMode::Discovering);
        }

        // Get the selected host
        let host = self.docker_items.selected_host.clone();

        // Debug: show host details
        let host_debug = match &host {
            DockerHost::Local => "Local".to_string(),
            DockerHost::Remote {
                hostname,
                username,
                password,
                ..
            } => {
                format!(
                    "Remote({}@{}, has_pwd={})",
                    username,
                    hostname,
                    password.is_some()
                )
            }
        };
        info!("refresh_docker_discovery: host={}", host_debug);
        self.set_status(format!("Discovery: host={}", host_debug));

        // Perform discovery based on host
        let result = DockerDiscovery::discover_all_for_host(&host);

        // Show discovery result for remote hosts
        if host.is_remote() {
            if let Some(ref err) = result.error {
                self.set_status(format!("Remote Docker error: {}", err));
            } else if result.docker_available {
                self.set_status(format!(
                    "Found {} containers, {} images on {}",
                    result.running_containers.len() + result.stopped_containers.len(),
                    result.images.len(),
                    host.display_name()
                ));
            }
        }

        if let Some(ref mut manager) = self.docker_manager {
            manager.update_from_discovery(result);
            manager.set_mode(DockerManagerMode::List);
        }
    }

    /// Starts host selection mode, loading available SSH hosts.
    pub fn docker_start_host_selection(&mut self) {
        info!("docker_start_host_selection: starting");

        // Ensure SSH hosts are loaded (they might not be if SSH manager wasn't opened)
        if self.ssh_hosts.is_empty() {
            info!("docker_start_host_selection: ssh_hosts is empty, loading...");
            self.load_ssh_hosts();
        }

        info!(
            "docker_start_host_selection: ssh_hosts count = {}",
            self.ssh_hosts.hosts().count()
        );

        // Collect SSH hosts info
        let ssh_hosts: Vec<_> = self
            .ssh_hosts
            .hosts()
            .map(|h| {
                let has_creds = self.ssh_hosts.get_credentials(h.id).is_some();
                let cred_info = if has_creds {
                    let cred = self.ssh_hosts.get_credentials(h.id);
                    if let Some(c) = cred {
                        format!(
                            "user='{}', has_pwd={}",
                            c.username,
                            c.password.is_some()
                        )
                    } else {
                        "has_creds=true but None?".to_string()
                    }
                } else {
                    "no credentials".to_string()
                };
                info!(
                    "docker_start_host_selection: SSH host id={}, hostname={}, port={}, display={:?}, {}",
                    h.id, h.hostname, h.port, h.display_name, cred_info
                );
                (h.id, h.hostname.clone(), h.port, h.display_name.clone(), has_creds)
            })
            .collect();

        // Debug: show number of hosts found
        let host_count = ssh_hosts.len();
        self.set_status(format!(
            "Host selection: {} SSH hosts available",
            host_count
        ));

        if let Some(ref mut manager) = self.docker_manager {
            info!(
                "docker_start_host_selection: loading {} hosts into manager",
                ssh_hosts.len()
            );
            manager.load_available_hosts(&ssh_hosts);
            manager.start_host_selection();
            info!(
                "docker_start_host_selection: mode is now {:?}",
                manager.mode()
            );
        } else {
            warn!("docker_start_host_selection: docker_manager is None!");
        }
    }

    /// Returns the currently selected Docker host.
    #[must_use]
    pub fn docker_selected_host(&self) -> &DockerHost {
        &self.docker_items.selected_host
    }

    /// Returns display name for the currently selected host.
    #[must_use]
    pub fn docker_host_display_name(&self) -> String {
        match &self.docker_items.selected_host {
            DockerHost::Local => "Local".to_string(),
            DockerHost::Remote {
                display_name,
                hostname,
                ..
            } => display_name.clone().unwrap_or_else(|| hostname.clone()),
        }
    }

    /// Saves Docker items to storage.
    pub fn save_docker_items(&mut self) {
        if let Err(e) = self.docker_storage.save(&self.docker_items) {
            self.set_status(format!("Failed to save Docker settings: {}", e));
        }
    }

    /// Assigns a quick-connect slot to the selected Docker item.
    pub fn assign_docker_quick_connect(&mut self, slot: usize) {
        assert!(slot < 9, "slot must be 0-8");

        let Some(ref manager) = self.docker_manager else {
            return;
        };

        let Some(item) = manager.selected_item() else {
            self.set_status("No item selected".to_string());
            return;
        };

        use crate::docker::DockerQuickConnectItem;

        let qc_item = match item {
            DockerItemDisplay::Container(c) => DockerQuickConnectItem::from_container(&c),
            DockerItemDisplay::Image(i) => DockerQuickConnectItem::from_image(&i),
        };

        self.docker_items.set_quick_connect(slot, qc_item.clone());
        self.save_docker_items();
        self.set_status(format!(
            "Assigned {} to Ctrl+Alt+{}",
            qc_item.name,
            slot + 1
        ));
    }

    /// Connects to Docker quick-connect slot by index (0-8).
    pub fn docker_connect_by_index(&mut self, index: usize) {
        assert!(index < 9, "index must be 0-8");

        // Load Docker items if not loaded
        if !self.docker_storage.is_initialized() {
            match self.docker_storage.load() {
                Ok(items) => {
                    self.docker_items = items;
                }
                Err(e) => {
                    self.set_status(format!("Failed to load Docker settings: {}", e));
                    return;
                }
            }
        }

        let Some(qc_item) = self.docker_items.get_quick_connect(index) else {
            self.set_status(format!("No Docker item assigned to Ctrl+Alt+{}", index + 1));
            return;
        };

        let item_id = qc_item.id.clone();
        let item_name = qc_item.name.clone();
        let item_type = qc_item.item_type;

        use crate::docker::DockerItemType;

        match item_type {
            DockerItemType::RunningContainer => {
                self.exec_into_container(&item_id, &item_name);
            }
            DockerItemType::StoppedContainer => {
                // Start the container first, then exec
                self.start_and_exec_container(&item_id, &item_name);
            }
            DockerItemType::Image => {
                // Run the image as a new container
                self.run_image_interactive(&item_id, &item_name);
            }
        }
    }

    /// Returns the Docker manager selector.
    #[must_use]
    pub fn docker_manager(&self) -> Option<&DockerManagerSelector> {
        self.docker_manager.as_ref()
    }

    /// Returns mutable Docker manager selector.
    pub fn docker_manager_mut(&mut self) -> Option<&mut DockerManagerSelector> {
        self.docker_manager.as_mut()
    }

    /// Returns the default shell for Docker exec.
    #[must_use]
    pub fn docker_default_shell(&self) -> &str {
        &self.docker_items.default_shell
    }

    // === Direct Host Management API ===

    /// Sets the Docker host directly to a remote host with password.
    ///
    /// This bypasses the UI and sets the host directly, useful for testing.
    /// After setting, call `refresh_docker_discovery()` to refresh the container list.
    pub fn docker_set_remote_host(
        &mut self,
        host_id: u32,
        hostname: &str,
        port: u16,
        username: &str,
        password: &str,
        display_name: Option<&str>,
    ) {
        let mut manager = DockerHostManager::new(&mut self.docker_items);
        manager.set_remote_with_password(host_id, hostname, port, username, password, display_name);

        let host_display = display_name.unwrap_or(hostname);
        self.set_status(format!(
            "Set Docker host: {}@{} (pwd=true)",
            username, host_display
        ));
    }

    /// Sets the Docker host to local.
    pub fn docker_set_local_host(&mut self) {
        let mut manager = DockerHostManager::new(&mut self.docker_items);
        manager.set_local();
        self.set_status("Set Docker host: Local".to_string());
    }

    /// Returns debug info about the current Docker host configuration.
    #[must_use]
    pub fn docker_host_debug_info(&self) -> String {
        let host = &self.docker_items.selected_host;
        match host {
            DockerHost::Local => "DockerHost::Local".to_string(),
            DockerHost::Remote {
                host_id,
                hostname,
                port,
                username,
                display_name,
                password,
            } => {
                format!(
                    "DockerHost::Remote {{ id: {}, host: {}, port: {}, user: {}, name: {:?}, has_pwd: {} }}",
                    host_id,
                    hostname,
                    port,
                    username,
                    display_name,
                    password.is_some()
                )
            }
        }
    }

    /// Tests the current Docker host and returns diagnostic information.
    pub fn docker_test_current_host(&mut self) -> Vec<String> {
        let manager = DockerHostManager::new(&mut self.docker_items);
        manager.test_current_host()
    }

    /// Convenience method to set a remote host and immediately discover containers.
    ///
    /// This is the recommended way to programmatically switch to a remote host.
    pub fn docker_switch_to_remote(
        &mut self,
        host_id: u32,
        hostname: &str,
        port: u16,
        username: &str,
        password: &str,
        display_name: Option<&str>,
    ) {
        self.docker_set_remote_host(host_id, hostname, port, username, password, display_name);
        self.refresh_docker_discovery();
    }

    /// Convenience method to switch to local Docker and refresh.
    pub fn docker_switch_to_local(&mut self) {
        self.docker_set_local_host();
        self.refresh_docker_discovery();
    }

    /// Shows debug info about the current Docker host in the status bar.
    pub fn docker_show_host_debug(&mut self) {
        let info = self.docker_host_debug_info();
        self.set_status(format!("DEBUG: {}", info));
    }

    // =========================================================================
    // Background Image Pull Operations
    // =========================================================================

    /// Spawns a background task to pull a Docker image.
    ///
    /// The result will be available via `check_docker_background_tasks()`.
    pub fn spawn_background_image_pull(&mut self, host: DockerHost, image_name: String) {
        use std::sync::mpsc::channel;
        use std::thread;

        info!(
            "Spawning background pull for image '{}' on {:?}",
            image_name, host
        );

        let (tx, rx) = channel();
        self.docker_background_rx = Some(rx);

        let image_clone = image_name.clone();
        thread::spawn(move || {
            let result = DockerDiscovery::pull_image_on_host(&host, &image_clone);
            let msg = super::DockerBackgroundResult::ImagePulled {
                image: image_clone,
                success: result.is_ok(),
                error: result.err(),
            };
            let _ = tx.send(msg);
        });

        self.set_status(format!("Downloading image '{}'...", image_name));
    }

    /// Checks for completed background Docker operations.
    ///
    /// Call this periodically (e.g., in the event loop) to handle results.
    /// Returns `true` if a result was processed.
    pub fn check_docker_background_tasks(&mut self) -> bool {
        let result = if let Some(ref rx) = self.docker_background_rx {
            match rx.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => return false,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.docker_background_rx = None;
                    return false;
                }
            }
        } else {
            return false;
        };

        if let Some(result) = result {
            self.docker_background_rx = None;
            self.handle_docker_background_result(result);
            return true;
        }

        false
    }

    /// Handles a completed background Docker operation.
    fn handle_docker_background_result(&mut self, result: super::DockerBackgroundResult) {
        match result {
            super::DockerBackgroundResult::ImagePulled {
                image,
                success,
                error,
            } => {
                if success {
                    info!("Background pull completed for '{}'", image);
                    self.set_status(format!("Downloaded '{}' successfully", image));

                    // Update creation state
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.on_image_pull_complete(true, None);
                    }
                } else {
                    let err_msg = error.unwrap_or_else(|| "Unknown error".to_string());
                    warn!("Background pull failed for '{}': {}", image, err_msg);
                    self.set_status(format!("Failed to download '{}': {}", image, err_msg));

                    // Update creation state with error
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.on_image_pull_complete(false, Some(err_msg));
                    }
                }
                self.request_redraw();
            }
        }
    }

    // =========================================================================
    // Container Creation Operations
    // =========================================================================

    /// Starts the container creation workflow.
    pub fn docker_start_container_creation(&mut self) {
        if let Some(ref mut manager) = self.docker_manager {
            manager.start_container_creation();
        }
    }

    /// Starts container creation from an existing image.
    pub fn docker_start_creation_from_image(&mut self, image_name: &str) {
        if let Some(ref mut manager) = self.docker_manager {
            manager.start_creation_from_image(image_name);
        }
    }

    /// Handles file browser selection for volume mount.
    pub fn handle_docker_volume_path_selected(&mut self, path: &std::path::Path) {
        let path_str = path.display().to_string();
        if let Some(ref mut manager) = self.docker_manager {
            manager.set_volume_host_path(&path_str);
        }
    }
}
