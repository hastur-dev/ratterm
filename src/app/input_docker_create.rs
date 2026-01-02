//! Docker container creation input handling.
//!
//! Handles keyboard input for the container creation workflow including:
//! - Docker Hub search
//! - Search result selection
//! - Volume mount configuration
//! - Startup command input
//! - Final confirmation

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::info;

use crate::docker::DockerDiscovery;
use crate::ui::docker_manager::DockerManagerMode;

use super::{App, FileBrowserContext};

impl App {
    /// Handles key events for Docker container creation workflow.
    pub fn handle_docker_create_key(&mut self, key: KeyEvent) {
        let Some(ref manager) = self.docker_manager else {
            return;
        };

        match manager.mode() {
            DockerManagerMode::SearchingHub => {
                self.handle_docker_search_hub_key(key);
            }
            DockerManagerMode::SearchResults => {
                self.handle_docker_search_results_key(key);
            }
            DockerManagerMode::CheckingImage => {
                // No input during image check (automatic)
            }
            DockerManagerMode::DownloadingImage => {
                self.handle_docker_downloading_key(key);
            }
            DockerManagerMode::VolumeMountHostPath => {
                self.handle_volume_host_path_key(key);
            }
            DockerManagerMode::VolumeMountContainerPath => {
                self.handle_volume_container_path_key(key);
            }
            DockerManagerMode::VolumeMountConfirm => {
                self.handle_volume_confirm_key(key);
            }
            DockerManagerMode::StartupCommand => {
                self.handle_startup_command_key(key);
            }
            DockerManagerMode::CreateConfirm => {
                self.handle_create_confirm_key(key);
            }
            DockerManagerMode::CreationError => {
                self.handle_creation_error_key(key);
            }
            _ => {}
        }
    }

    /// Handles key events in Docker Hub search mode.
    fn handle_docker_search_hub_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.cancel_container_creation();
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.execute_docker_hub_search();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.backspace_search();
                }
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.insert_char_search(c);
                }
            }
            _ => {}
        }
    }

    /// Executes the Docker Hub search.
    fn execute_docker_hub_search(&mut self) {
        let search_term = self
            .docker_manager
            .as_ref()
            .map(|m| m.creation_state().search_term.clone())
            .unwrap_or_default();

        if search_term.trim().is_empty() {
            self.set_status("Please enter a search term");
            return;
        }

        let host = self.docker_items.selected_host.clone();

        info!("Searching Docker Hub for '{}' on {:?}", search_term, host);
        self.set_status(format!("Searching for '{}'...", search_term));

        match DockerDiscovery::search_docker_hub(&host, &search_term, 25) {
            Ok(results) => {
                if results.is_empty() {
                    self.set_status(format!("No images found for '{}'", search_term));
                } else {
                    self.set_status(format!("Found {} images", results.len()));
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.set_search_results(results);
                    }
                }
            }
            Err(e) => {
                self.set_status(format!("Search failed: {}", e));
                if let Some(ref mut manager) = self.docker_manager {
                    manager.show_creation_error(e, true);
                }
            }
        }
    }

    /// Handles key events in search results mode.
    fn handle_docker_search_results_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Go back to search
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_mode(DockerManagerMode::SearchingHub);
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.confirm_search_and_check_image();
            }
            (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.select_prev_search_result();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.select_next_search_result();
                }
            }
            _ => {}
        }
    }

    /// Confirms search selection and checks if image exists.
    fn confirm_search_and_check_image(&mut self) {
        if let Some(ref mut manager) = self.docker_manager {
            manager.confirm_search_selection();
        }

        let image_name = self
            .docker_manager
            .as_ref()
            .and_then(|m| m.creation_state().selected_image.clone())
            .unwrap_or_default();

        if image_name.is_empty() {
            return;
        }

        let host = self.docker_items.selected_host.clone();

        info!("Checking if image '{}' exists on {:?}", image_name, host);
        self.set_status(format!("Checking image '{}'...", image_name));

        match DockerDiscovery::image_exists_on_host(&host, &image_name) {
            Ok(exists) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_image_exists(exists);
                }

                if exists {
                    self.set_status(format!("Image '{}' found", image_name));
                } else {
                    self.set_status(format!("Downloading '{}'...", image_name));
                    // Spawn background pull
                    self.spawn_background_image_pull(host, image_name);
                }
            }
            Err(e) => {
                self.set_status(format!("Image check failed: {}", e));
                if let Some(ref mut manager) = self.docker_manager {
                    manager.show_creation_error(e, true);
                }
            }
        }
    }

    /// Handles key events while downloading image.
    fn handle_docker_downloading_key(&mut self, key: KeyEvent) {
        if let (KeyModifiers::NONE, KeyCode::Esc) = (key.modifiers, key.code) {
            // Cancel and go back (note: download continues in background)
            if let Some(ref mut manager) = self.docker_manager {
                manager.cancel_container_creation();
            }
            self.set_status("Download cancelled (may continue in background)");
        }
    }

    /// Handles key events in volume host path mode.
    fn handle_volume_host_path_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Go back to search results or cancel
                if let Some(ref mut manager) = self.docker_manager {
                    if manager.creation_state().search_results.is_empty() {
                        // Came from existing image, cancel entirely
                        manager.cancel_container_creation();
                    } else {
                        manager.set_mode(DockerManagerMode::SearchResults);
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.confirm_host_path();
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('f')) => {
                // Open file browser for directory selection
                self.show_file_browser_for_volume_mount();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.backspace_host_path();
                }
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.insert_char_host_path(c);
                }
            }
            _ => {}
        }
    }

    /// Opens file browser for volume mount selection.
    fn show_file_browser_for_volume_mount(&mut self) {
        self.file_browser_context = FileBrowserContext::DockerVolumeMount;
        self.show_file_browser();
    }

    /// Handles key events in volume container path mode.
    fn handle_volume_container_path_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Go back to host path
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_mode(DockerManagerMode::VolumeMountHostPath);
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.confirm_container_path();
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.backspace_container_path();
                }
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.insert_char_container_path(c);
                }
            }
            _ => {}
        }
    }

    /// Handles key events in volume mount confirm mode.
    fn handle_volume_confirm_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Skip to startup command
                if let Some(ref mut manager) = self.docker_manager {
                    manager.confirm_add_another_volume(false);
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('y') | KeyCode::Char('Y')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.confirm_add_another_volume(true);
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.confirm_add_another_volume(false);
                }
            }
            _ => {}
        }
    }

    /// Handles key events in startup command mode.
    fn handle_startup_command_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Go back to volume confirm
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_mode(DockerManagerMode::VolumeMountConfirm);
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.confirm_startup_command();
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.backspace_startup_cmd();
                }
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.insert_char_startup_cmd(c);
                }
            }
            _ => {}
        }
    }

    /// Handles key events in create confirm mode.
    fn handle_create_confirm_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Go back to startup command
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_mode(DockerManagerMode::StartupCommand);
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                // Block creation while image is downloading
                let is_downloading = self
                    .docker_manager
                    .as_ref()
                    .map(|m| m.is_downloading())
                    .unwrap_or(false);

                if is_downloading {
                    self.set_status("Please wait for image download to complete");
                } else {
                    self.execute_container_creation();
                }
            }
            _ => {}
        }
    }

    /// Handles key events in creation error mode.
    fn handle_creation_error_key(&mut self, key: KeyEvent) {
        if let (KeyModifiers::NONE, KeyCode::Esc | KeyCode::Enter) = (key.modifiers, key.code) {
            if let Some(ref mut manager) = self.docker_manager {
                manager.dismiss_creation_error();
            }
        }
    }

    /// Executes the container creation command.
    fn execute_container_creation(&mut self) {
        let Some(run_command) = self
            .docker_manager
            .as_ref()
            .and_then(|m| m.get_creation_run_command())
        else {
            self.set_status("No image selected");
            return;
        };

        info!("Executing container creation: {}", run_command);

        // Get the host
        let host = self.docker_items.selected_host.clone();

        // Get the image name for the tab name
        let image_name = self
            .docker_manager
            .as_ref()
            .and_then(|m| m.creation_state().selected_image.clone())
            .unwrap_or_else(|| "container".to_string());

        // Close the Docker manager popup
        self.hide_docker_manager();

        // Focus terminal pane
        self.layout
            .set_focused(crate::ui::layout::FocusedPane::Terminal);

        match host {
            crate::docker::DockerHost::Local => {
                // Local: Create a new terminal tab and run the docker command
                self.run_local_docker_command(&run_command, &image_name);
            }
            crate::docker::DockerHost::Remote {
                hostname,
                port,
                username,
                password,
                ..
            } => {
                // Remote: Create an SSH terminal tab that runs the docker command
                self.run_remote_docker_command(
                    &run_command,
                    &image_name,
                    &hostname,
                    port,
                    &username,
                    password.as_deref(),
                );
            }
        }

        self.set_status("Starting container...");
    }

    /// Runs a docker command locally by creating a new terminal tab.
    fn run_local_docker_command(&mut self, command: &str, image_name: &str) {
        let Some(ref mut terminals) = self.terminals else {
            self.set_status("No terminal available");
            return;
        };

        // Add a new tab
        match terminals.add_tab() {
            Ok(tab_idx) => {
                // Set the tab name
                terminals.set_tab_name(tab_idx, format!("Docker: {}", image_name));

                // Send the command to the new terminal
                if let Some(terminal) = terminals.active_terminal_mut() {
                    let cmd_with_newline = format!("{}\n", command);
                    let _ = terminal.write(cmd_with_newline.as_bytes());
                }
            }
            Err(e) => {
                self.set_status(format!("Failed to create terminal tab: {}", e));
            }
        }
    }

    /// Runs a docker command on a remote host via SSH.
    fn run_remote_docker_command(
        &mut self,
        command: &str,
        image_name: &str,
        hostname: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
    ) {
        let Some(ref mut terminals) = self.terminals else {
            self.set_status("No terminal available");
            return;
        };

        let tab_name = format!("Docker: {}@{}", image_name, hostname);

        // Create an SSH terminal tab that runs the docker command
        match terminals.add_ssh_command_tab(
            hostname, port, username, command, &tab_name, password,
        ) {
            Ok(_) => {
                info!(
                    "Created SSH terminal for docker command on {}@{}",
                    username, hostname
                );
            }
            Err(e) => {
                self.set_status(format!("Failed to create SSH terminal: {}", e));
            }
        }
    }
}
