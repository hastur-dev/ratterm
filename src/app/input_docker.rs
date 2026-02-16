//! Docker manager input handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::{debug, error, info, warn};

use crate::app::dashboard_nav::{NavResult, apply_dashboard_navigation};
use crate::docker::DockerAvailability;
use crate::ui::docker_manager::{DockerListSection, DockerManagerMode};

use super::App;

impl App {
    /// Handles key events for the Docker manager popup.
    pub fn handle_docker_manager_key(&mut self, key: KeyEvent) {
        // Handle hotkey overlay if visible
        if self.hotkey_overlay.as_ref().is_some_and(|o| o.is_visible()) {
            match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Char('?')) | (KeyModifiers::NONE, KeyCode::Esc) => {
                    self.hotkey_overlay = None;
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                    if let Some(ref mut overlay) = self.hotkey_overlay {
                        overlay.scroll_up();
                    }
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                    if let Some(ref mut overlay) = self.hotkey_overlay {
                        overlay.scroll_down();
                    }
                    return;
                }
                _ => {
                    self.hotkey_overlay = None;
                }
            }
        }

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
            DockerManagerMode::HostSelection => {
                self.handle_docker_host_selection_key(key);
            }
            DockerManagerMode::HostCredentials => {
                self.handle_docker_host_credentials_key(key);
            }
            // Container creation workflow modes
            DockerManagerMode::SearchingHub
            | DockerManagerMode::SearchResults
            | DockerManagerMode::CheckingImage
            | DockerManagerMode::DownloadingImage
            | DockerManagerMode::VolumeMountHostPath
            | DockerManagerMode::VolumeMountContainerPath
            | DockerManagerMode::VolumeMountConfirm
            | DockerManagerMode::StartupCommand
            | DockerManagerMode::CreateConfirm
            | DockerManagerMode::CreationError => {
                self.handle_docker_create_key(key);
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
                // Host selection - ALWAYS allow this even when local Docker unavailable
                (KeyModifiers::NONE, KeyCode::Char('h')) => {
                    self.docker_start_host_selection();
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

        // Unified dashboard navigation layer
        if let Some(ref mut manager) = self.docker_manager {
            match apply_dashboard_navigation(manager, &key) {
                NavResult::Handled => return,
                NavResult::ShowHelp => {
                    self.toggle_hotkey_overlay_docker();
                    return;
                }
                NavResult::Close => {
                    self.hide_docker_manager();
                    return;
                }
                NavResult::Activate => {
                    self.docker_select_item();
                    return;
                }
                NavResult::Unhandled => {}
            }
        }

        // Docker-specific keys layered on top
        match (key.modifiers, key.code) {
            // Vim-style first/last (g/G)
            (KeyModifiers::NONE, KeyCode::Char('g')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.select_first();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Char('G')) => {
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

            // Host selection
            (KeyModifiers::NONE, KeyCode::Char('h')) => {
                self.docker_start_host_selection();
            }

            // Create new container
            (KeyModifiers::NONE, KeyCode::Char('c')) => {
                self.docker_start_container_creation();
            }

            // Debug: Show current host info (Shift+D)
            (KeyModifiers::SHIFT, KeyCode::Char('D')) => {
                self.docker_show_host_debug();
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

        // Get the selected host for remote operations
        let host = self.docker_items.selected_host.clone();

        match manager.section() {
            DockerListSection::RunningContainers => {
                self.set_status("Cannot remove running container. Stop it first.".to_string());
            }
            DockerListSection::StoppedContainers => {
                if let Some(container) = item.as_container() {
                    match DockerDiscovery::remove_container_on_host(&container.id, false, &host) {
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
                    match DockerDiscovery::remove_image_on_host(&image.id, false, &host) {
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

    /// Handles key events in Docker host selection mode.
    fn handle_docker_host_selection_key(&mut self, key: KeyEvent) {
        debug!("Docker host selection key: {:?}", key.code);

        // Handle vim-style navigation for host list (j/k and arrows)
        if key.modifiers == KeyModifiers::NONE {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.select_next_host();
                        debug!(
                            "Docker host selection: moved down to index {}",
                            manager.host_selection_index()
                        );
                    }
                    return;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.select_prev_host();
                        debug!(
                            "Docker host selection: moved up to index {}",
                            manager.host_selection_index()
                        );
                    }
                    return;
                }
                _ => {}
            }
        }

        match (key.modifiers, key.code) {
            // Close/cancel
            (KeyModifiers::NONE, KeyCode::Esc) => {
                info!("Docker host selection cancelled");
                if let Some(ref mut manager) = self.docker_manager {
                    manager.cancel_host_selection();
                }
            }
            // Quick-select local
            (KeyModifiers::NONE, KeyCode::Char('l')) => {
                info!("Docker: quick-selecting local host");
                self.docker_select_local_host();
            }
            // Select host
            (KeyModifiers::NONE, KeyCode::Enter) => {
                // Log current selection state before confirming
                if let Some(ref manager) = self.docker_manager {
                    let idx = manager.host_selection_index();
                    let host_count = manager.available_hosts().len();
                    let selected = manager.selected_host_display();
                    info!(
                        "Docker: Enter pressed - selection_index={}, host_count={}, selected={:?}",
                        idx,
                        host_count,
                        selected.map(|h| (&h.display_name, h.host_id, h.has_credentials))
                    );
                }
                info!("Docker: confirming host selection");
                self.docker_confirm_host_selection();
            }
            _ => {}
        }
    }

    /// Handles key events in Docker host credentials entry mode.
    fn handle_docker_host_credentials_key(&mut self, key: KeyEvent) {
        debug!("Docker credentials key: {:?}", key.code);

        match (key.modifiers, key.code) {
            // Cancel
            (KeyModifiers::NONE, KeyCode::Esc) => {
                info!("Docker credential entry cancelled");
                if let Some(ref mut manager) = self.docker_manager {
                    manager.cancel_host_credentials();
                }
            }
            // Next field
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.next_cred_field();
                    debug!(
                        "Docker credentials: moved to field {:?}",
                        manager.cred_field()
                    );
                }
            }
            // Previous field
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.prev_cred_field();
                    debug!(
                        "Docker credentials: moved to field {:?}",
                        manager.cred_field()
                    );
                }
            }
            // Submit credentials
            (KeyModifiers::NONE, KeyCode::Enter) => {
                info!("Docker: submitting credentials");
                self.docker_submit_host_credentials();
            }
            // Backspace
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.cred_backspace();
                }
            }
            // Toggle checkbox (space)
            (KeyModifiers::NONE, KeyCode::Char(' ')) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.toggle_save_credentials();
                    debug!(
                        "Docker credentials: save checkbox toggled to {}",
                        manager.cred_save()
                    );
                }
            }
            // Character input
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if let Some(ref mut manager) = self.docker_manager {
                    manager.cred_insert_char(c);
                }
            }
            _ => {}
        }
    }

    /// Selects the local host for Docker management.
    fn docker_select_local_host(&mut self) {
        if let Some(ref mut manager) = self.docker_manager {
            manager.select_local_host();
        }
        self.docker_items
            .set_selected_host(crate::docker::DockerHost::Local);
        self.set_status("Switched to local Docker".to_string());
        self.refresh_docker_discovery();
    }

    /// Confirms the selected host and switches to it.
    fn docker_confirm_host_selection(&mut self) {
        info!("docker_confirm_host_selection: starting");

        // Extract display info first to avoid borrow issues
        let host_info = {
            let Some(ref manager) = self.docker_manager else {
                error!("docker_confirm_host_selection: docker_manager is None");
                return;
            };
            let Some(host_display) = manager.selected_host_display() else {
                error!("docker_confirm_host_selection: no host selected in manager");
                return;
            };
            info!(
                "docker_confirm_host_selection: selected host_display: name={}, is_local={}, has_creds={}, host_id={:?}",
                host_display.display_name,
                host_display.is_local(),
                host_display.has_credentials,
                host_display.host_id
            );
            (
                host_display.is_local(),
                host_display.has_credentials,
                host_display.host.clone(),
                host_display.display_name.clone(),
                host_display.host_id,
            )
        };

        let (is_local, has_creds, _host, display_name, host_id) = host_info;

        // Debug info
        self.set_status(format!(
            "Host: {} local={} has_creds={} host_id={:?}",
            display_name, is_local, has_creds, host_id
        ));

        if is_local {
            info!("docker_confirm_host_selection: selected local host");
            self.docker_select_local_host();
            return;
        }

        info!(
            "docker_confirm_host_selection: selected remote host, has_creds={}",
            has_creds
        );

        // Remote host - check if credentials are available
        if has_creds {
            // Use saved credentials - need to include password in DockerHost
            let Some(hid) = host_id else {
                // host_id is None but is_local is false - shouldn't happen
                error!("docker_confirm_host_selection: remote host with no host_id");
                self.set_status("Error: Remote host with no host_id".to_string());
                return;
            };

            info!(
                "docker_confirm_host_selection: looking up SSH host id={}",
                hid
            );

            // Extract all needed data from SSH hosts first (before mutable borrow)
            let host_data = {
                let ssh_host = self.ssh_hosts.get_by_id(hid);
                let creds = self.ssh_hosts.get_credentials(hid);

                info!(
                    "docker_confirm_host_selection: ssh_host found={}, creds found={}",
                    ssh_host.is_some(),
                    creds.is_some()
                );

                match (ssh_host, creds) {
                    (Some(ssh), Some(cred)) => {
                        info!(
                            "docker_confirm_host_selection: got credentials - username='{}', has_password={}",
                            cred.username,
                            cred.password.is_some()
                        );
                        Some((
                            ssh.hostname.clone(),
                            ssh.port,
                            cred.username.clone(),
                            cred.password.clone(),
                        ))
                    }
                    (None, _) => {
                        warn!(
                            "docker_confirm_host_selection: SSH host not found for id={}",
                            hid
                        );
                        None // SSH host not found
                    }
                    (_, None) => {
                        warn!(
                            "docker_confirm_host_selection: No credentials for host id={}",
                            hid
                        );
                        None // No credentials
                    }
                }
            };

            let Some((hostname, ssh_port, username, password)) = host_data else {
                info!(
                    "docker_confirm_host_selection: host_data is None, prompting for credentials"
                );
                self.set_status(format!("SSH host {} or credentials not found", hid));
                if let Some(ref mut manager) = self.docker_manager {
                    info!(
                        "docker_confirm_host_selection: calling start_host_credentials({})",
                        hid
                    );
                    manager.start_host_credentials(hid);
                    info!(
                        "docker_confirm_host_selection: mode is now {:?}",
                        manager.mode()
                    );
                }
                return;
            };

            if username.is_empty() {
                info!(
                    "docker_confirm_host_selection: username is empty, prompting for credentials"
                );
                self.set_status("Username is empty - prompting for credentials".to_string());
                if let Some(ref mut manager) = self.docker_manager {
                    info!(
                        "docker_confirm_host_selection: calling start_host_credentials({})",
                        hid
                    );
                    manager.start_host_credentials(hid);
                    info!(
                        "docker_confirm_host_selection: mode is now {:?}",
                        manager.mode()
                    );
                }
                return;
            }

            // Check if password is required but missing or empty
            // On Windows with plink, or with sshpass, we need a password
            // If no password or empty password, prompt for credentials
            let password_missing = password.as_ref().is_none_or(|p| p.is_empty());
            info!(
                "docker_confirm_host_selection: password check - is_none={}, is_empty={}, missing={}",
                password.is_none(),
                password.as_ref().is_some_and(|p| p.is_empty()),
                password_missing
            );
            if password_missing {
                info!(
                    "docker_confirm_host_selection: password is None or empty, prompting for credentials"
                );
                self.set_status("Password is required - prompting for credentials".to_string());
                if let Some(ref mut manager) = self.docker_manager {
                    info!(
                        "docker_confirm_host_selection: calling start_host_credentials({})",
                        hid
                    );
                    manager.start_host_credentials(hid);
                    info!(
                        "docker_confirm_host_selection: mode is now {:?}",
                        manager.mode()
                    );
                }
                return;
            }

            info!(
                "docker_confirm_host_selection: all credentials present, creating DockerHost for {}@{}",
                username, hostname
            );

            let docker_host = crate::docker::DockerHost::remote_with_password(
                hid,
                hostname.clone(),
                ssh_port,
                username.clone(),
                Some(display_name.clone()),
                password.clone(),
            );

            // Set the host in docker_items
            self.docker_items.set_selected_host(docker_host.clone());
            info!("docker_confirm_host_selection: set docker_items.selected_host");

            // Update the manager's selected host too for display consistency
            if let Some(ref mut manager) = self.docker_manager {
                manager.set_selected_host(docker_host.clone());
                manager.set_mode(DockerManagerMode::List);
                info!(
                    "docker_confirm_host_selection: updated manager, mode is now {:?}",
                    manager.mode()
                );
            }

            // Debug: show the host we're switching to
            self.set_status(format!(
                "Switching to: {}@{} (discovering...)",
                username, hostname
            ));

            info!("docker_confirm_host_selection: calling refresh_docker_discovery");
            self.refresh_docker_discovery();
        } else {
            // Need to prompt for credentials - no saved credentials for this host
            info!("docker_confirm_host_selection: has_creds=false, prompting for credentials");
            let Some(hid) = host_id else {
                error!("docker_confirm_host_selection: remote host with no host_id (else branch)");
                self.set_status("Error: Remote host with no host_id".to_string());
                return;
            };
            self.set_status(format!(
                "No credentials saved for {} - please enter credentials",
                display_name
            ));
            if let Some(ref mut manager) = self.docker_manager {
                info!(
                    "docker_confirm_host_selection: calling start_host_credentials({}) from else branch",
                    hid
                );
                manager.start_host_credentials(hid);
                info!(
                    "docker_confirm_host_selection: mode is now {:?}",
                    manager.mode()
                );
            }
        }
    }

    /// Submits the entered credentials and connects to host.
    fn docker_submit_host_credentials(&mut self) {
        // Extract all needed data first to avoid borrow issues
        let cred_info = {
            let Some(ref manager) = self.docker_manager else {
                return;
            };

            let (username, password, save) = manager.get_entered_credentials();
            if username.is_empty() {
                // Silent return - user hasn't entered username yet
                return;
            }
            if password.is_empty() {
                // Silent return - user hasn't entered password yet
                return;
            }

            let Some(host_id) = manager.cred_host_id() else {
                return;
            };

            let host_display = manager
                .available_hosts()
                .iter()
                .find(|h| h.host_id == Some(host_id))
                .cloned();

            let Some(hd) = host_display else {
                return;
            };

            (username, password, save, host_id, hd)
        };

        let (username, password, save, host_id, hd) = cred_info;

        // Get the SSH port from the SSH host
        let ssh_port = self
            .ssh_hosts
            .get_by_id(host_id)
            .map(|h| h.port)
            .unwrap_or(22);

        // Save credentials if requested
        if save {
            let creds = crate::ssh::SSHCredentials::new(username.clone(), Some(password.clone()));
            self.ssh_hosts.set_credentials(host_id, creds.clone());
            if let Err(e) = self.ssh_storage.save(&self.ssh_hosts) {
                self.set_status(format!("Warning: failed to save credentials: {}", e));
            }
        }

        // Create the docker host with the credentials including password
        let docker_host = crate::docker::DockerHost::remote_with_password(
            host_id,
            hd.hostname.clone(),
            ssh_port,
            username.clone(),
            Some(hd.display_name.clone()),
            Some(password),
        );

        // Update both docker_items AND manager selected host to stay in sync
        self.docker_items.set_selected_host(docker_host.clone());
        if let Some(ref mut manager) = self.docker_manager {
            manager.set_selected_host(docker_host);
            manager.set_mode(DockerManagerMode::List);
        }

        self.set_status(format!("Connecting to {}@{}...", username, hd.hostname));
        self.refresh_docker_discovery();
    }
}
