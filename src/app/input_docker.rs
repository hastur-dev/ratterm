//! Docker manager input handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::docker::DockerAvailability;
use crate::ui::docker_manager::{DockerListSection, DockerManagerMode};

use super::App;

impl App {
    /// Handles key events for the Docker manager popup.
    pub fn handle_docker_manager_key(&mut self, key: KeyEvent) {
        let Some(ref mut manager) = self.docker_manager else {
            return;
        };

        match manager.mode() {
            DockerManagerMode::List | DockerManagerMode::Discovering => {
                self.handle_docker_list_key(key);
            }
            DockerManagerMode::RunOptions => {
                self.handle_docker_run_options_key(key);
            }
            DockerManagerMode::Confirming => {
                self.handle_docker_confirm_key(key);
            }
            DockerManagerMode::Connecting => {
                // No input during connection
            }
        }
    }

    /// Handles key events in Docker list mode.
    fn handle_docker_list_key(&mut self, key: KeyEvent) {
        // Check if Docker is unavailable - handle special keys
        let availability = self
            .docker_manager
            .as_ref()
            .map(|m| m.availability())
            .unwrap_or(DockerAvailability::Unknown);

        if !availability.is_available() {
            match (key.modifiers, key.code) {
                // Close manager
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    self.hide_docker_manager();
                }
                // Retry discovery
                (KeyModifiers::NONE, KeyCode::Char('r')) => {
                    self.set_status("Checking Docker availability...".to_string());
                    self.refresh_docker_discovery();
                }
                // Start/Restart Docker (for NotRunning and DaemonError states)
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    // Check if we can start Docker (NotRunning or DaemonError)
                    let can_start = matches!(
                        availability,
                        DockerAvailability::NotRunning | DockerAvailability::DaemonError(_)
                    );
                    if can_start {
                        self.start_docker_desktop();
                    }
                }
                _ => {}
            }
            return;
        }

        match (key.modifiers, key.code) {
            // Close manager
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.hide_docker_manager();
            }

            // Navigation
            (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.select_prev();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.select_next();
                }
            }
            (KeyModifiers::NONE, KeyCode::Home | KeyCode::Char('g')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.select_first();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Char('G')) | (KeyModifiers::NONE, KeyCode::End) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.select_last();
                }
            }

            // Section switching
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.next_section();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.prev_section();
                }
            }

            // Quick section jump
            (KeyModifiers::NONE, KeyCode::Char('r')) => {
                // Use lowercase 'r' for refresh
                self.refresh_docker_discovery();
            }
            (KeyModifiers::SHIFT, KeyCode::Char('R')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_section(DockerListSection::RunningContainers);
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Char('S')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_section(DockerListSection::StoppedContainers);
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Char('I')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.set_section(DockerListSection::Images);
                }
            }

            // Select/connect
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.docker_select_item();
            }

            // Run with options (Ctrl+O)
            (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                self.docker_show_run_options();
            }

            // Quick connect assignment (1-9)
            (KeyModifiers::NONE, KeyCode::Char(c)) if c.is_ascii_digit() && c != '0' => {
                let slot = (c as u8 - b'1') as usize;
                self.assign_docker_quick_connect(slot);
            }

            // Delete/remove
            (KeyModifiers::NONE, KeyCode::Char('d') | KeyCode::Delete) => {
                self.docker_remove_selected();
            }

            _ => {}
        }
    }

    /// Starts Docker Desktop and shows status updates.
    fn start_docker_desktop(&mut self) {
        use crate::docker::DockerDiscovery;

        self.set_status("Starting Docker Desktop...".to_string());

        match DockerDiscovery::start_docker_desktop() {
            Ok(()) => {
                self.set_status(
                    "Docker Desktop is starting. Press 'r' to refresh when ready.".to_string(),
                );
            }
            Err(e) => {
                self.set_status(format!("Failed to start Docker: {}", e));
            }
        }
    }

    /// Handles key events in run options mode.
    fn handle_docker_run_options_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Cancel
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.cancel_run_options();
                }
            }

            // Submit
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.docker_submit_run_options();
            }

            // Field navigation
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.next_run_options_field();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.prev_run_options_field();
                }
            }

            // Text input
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.insert_char(c);
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.backspace();
                }
            }

            _ => {}
        }
    }

    /// Handles key events in confirm mode.
    fn handle_docker_confirm_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Cancel
            (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Char('n')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.cancel_confirm();
                }
            }

            // Confirm
            (KeyModifiers::NONE, KeyCode::Enter) | (KeyModifiers::NONE, KeyCode::Char('y')) => {
                self.docker_confirm_run();
            }

            // Run with options
            (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                // Switch to run options mode
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(target) = manager.confirm_target().map(String::from) {
                        manager.cancel_confirm();
                        manager.start_run_options(target);
                    }
                }
            }

            _ => {}
        }
    }

    /// Selects the current Docker item (connect or show confirm).
    fn docker_select_item(&mut self) {
        let Some(ref manager) = self.docker_manager else {
            return;
        };

        let Some(item) = manager.selected_item() else {
            self.set_status("No item selected".to_string());
            return;
        };

        match manager.section() {
            DockerListSection::RunningContainers => {
                // Exec into running container
                if let Some(container) = item.as_container() {
                    let id = container.id.clone();
                    let name = container.display().to_string();
                    self.exec_into_container(&id, &name);
                }
            }
            DockerListSection::StoppedContainers => {
                // Start and exec into stopped container
                if let Some(container) = item.as_container() {
                    let id = container.id.clone();
                    let name = container.display().to_string();
                    self.start_and_exec_container(&id, &name);
                }
            }
            DockerListSection::Images => {
                // Show confirm dialog for images
                if let Some(image) = item.as_image() {
                    let name = image.full_name();
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.start_confirm(name);
                    }
                }
            }
        }
    }

    /// Shows run options for the selected image.
    fn docker_show_run_options(&mut self) {
        let Some(ref manager) = self.docker_manager else {
            return;
        };

        // Only works on images
        if manager.section() != DockerListSection::Images {
            self.set_status("Run options only available for images".to_string());
            return;
        }

        let Some(item) = manager.selected_item() else {
            return;
        };

        if let Some(image) = item.as_image() {
            let name = image.full_name();
            if let Some(ref mut manager) = self.docker_manager {
                manager.start_run_options(name);
            }
        }
    }

    /// Submits run options and starts the container.
    fn docker_submit_run_options(&mut self) {
        let Some(ref mut manager) = self.docker_manager else {
            return;
        };

        let target = manager.run_target().map(String::from);

        match manager.finish_run_options() {
            Ok(options) => {
                if let Some(image_name) = target {
                    let display_name = image_name.clone();
                    self.run_image_with_options(&image_name, &display_name, &options);
                }
            }
            Err(e) => {
                self.set_status(format!("Invalid options: {}", e));
            }
        }
    }

    /// Confirms running an image.
    fn docker_confirm_run(&mut self) {
        let Some(ref mut manager) = self.docker_manager else {
            return;
        };

        let target = manager.confirm_target().map(String::from);
        manager.cancel_confirm();

        if let Some(image_name) = target {
            let display_name = image_name.clone();
            self.run_image_interactive(&image_name, &display_name);
        }
    }

    /// Removes the selected container or image.
    fn docker_remove_selected(&mut self) {
        let Some(ref manager) = self.docker_manager else {
            return;
        };

        let Some(item) = manager.selected_item() else {
            self.set_status("No item selected".to_string());
            return;
        };

        use crate::docker::DockerDiscovery;

        match manager.section() {
            DockerListSection::RunningContainers => {
                self.set_status("Cannot remove running container. Stop it first.".to_string());
            }
            DockerListSection::StoppedContainers => {
                if let Some(container) = item.as_container() {
                    match DockerDiscovery::remove_container(&container.id, false) {
                        Ok(()) => {
                            self.set_status(format!("Removed container {}", container.name));
                            self.refresh_docker_discovery();
                        }
                        Err(e) => {
                            self.set_status(format!("Failed to remove container: {}", e));
                        }
                    }
                }
            }
            DockerListSection::Images => {
                if let Some(image) = item.as_image() {
                    match DockerDiscovery::remove_image(&image.id, false) {
                        Ok(()) => {
                            self.set_status(format!("Removed image {}", image.full_name()));
                            self.refresh_docker_discovery();
                        }
                        Err(e) => {
                            self.set_status(format!("Failed to remove image: {}", e));
                        }
                    }
                }
            }
        }
    }
}
