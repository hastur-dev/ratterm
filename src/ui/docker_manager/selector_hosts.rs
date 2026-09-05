//! Host selection, host credentials and the container-creation workflow.
//!
//! Split out of `selector.rs`, which had grown past this project's file-size
//! limit.

use tracing::info;

use crate::docker::{ContainerCreationState, DockerHost};

use super::selector::DockerManagerSelector;
use super::types::{DockerHostDisplay, DockerManagerMode, HostCredentialField, MAX_DISPLAY_HOSTS};

impl DockerManagerSelector {
    // --- Host Selection Mode ---

    /// Returns the currently selected Docker host.
    #[must_use]
    pub fn selected_host(&self) -> &DockerHost {
        &self.selected_host
    }

    /// Sets the selected Docker host.
    pub fn set_selected_host(&mut self, host: DockerHost) {
        self.selected_host = host;
    }

    /// Loads available hosts from SSH host list.
    pub fn load_available_hosts(&mut self, ssh_hosts: &[(u32, String, u16, Option<String>, bool)]) {
        self.available_hosts.clear();

        // Always add local first
        self.available_hosts.push(DockerHostDisplay::local());

        // Add SSH hosts
        for (id, hostname, port, display_name, has_creds) in ssh_hosts {
            // We need username from credentials - use hostname as placeholder if not available
            let username = "user".to_string(); // Will be filled in from SSH credentials
            let _ = port;
            self.available_hosts.push(DockerHostDisplay::remote(
                *id,
                hostname.clone(),
                username,
                display_name.clone(),
                *has_creds,
            ));
        }
    }

    /// Returns the available hosts.
    #[must_use]
    pub fn available_hosts(&self) -> &[DockerHostDisplay] {
        &self.available_hosts
    }

    /// Returns visible hosts for rendering (with scroll).
    #[must_use]
    pub fn visible_hosts(&self) -> Vec<(usize, &DockerHostDisplay)> {
        self.available_hosts
            .iter()
            .enumerate()
            .skip(self.host_scroll_offset)
            .take(MAX_DISPLAY_HOSTS)
            .collect()
    }

    /// Starts host selection mode.
    pub fn start_host_selection(&mut self) {
        self.mode = DockerManagerMode::HostSelection;
        self.host_selection_index = 0;
        self.host_scroll_offset = 0;
    }

    /// Cancels host selection and returns to list mode.
    pub fn cancel_host_selection(&mut self) {
        self.mode = DockerManagerMode::List;
    }

    /// Returns the currently selected host display in host selection mode.
    #[must_use]
    pub fn selected_host_display(&self) -> Option<&DockerHostDisplay> {
        self.available_hosts.get(self.host_selection_index)
    }

    /// Moves selection to previous host.
    pub fn select_prev_host(&mut self) {
        if !self.available_hosts.is_empty() {
            self.host_selection_index = self.host_selection_index.saturating_sub(1);
            self.update_host_scroll();
        }
    }

    /// Moves selection to next host.
    pub fn select_next_host(&mut self) {
        if !self.available_hosts.is_empty() {
            self.host_selection_index =
                (self.host_selection_index + 1).min(self.available_hosts.len() - 1);
            self.update_host_scroll();
        }
    }

    /// Updates host scroll offset to keep selection visible.
    pub(super) fn update_host_scroll(&mut self) {
        if self.host_selection_index < self.host_scroll_offset {
            self.host_scroll_offset = self.host_selection_index;
        } else if self.host_selection_index >= self.host_scroll_offset + MAX_DISPLAY_HOSTS {
            self.host_scroll_offset = self.host_selection_index - MAX_DISPLAY_HOSTS + 1;
        }
    }

    /// Selects local host directly.
    pub fn select_local_host(&mut self) {
        self.selected_host = DockerHost::Local;
        self.mode = DockerManagerMode::List;
    }

    /// Returns the host selection index.
    #[must_use]
    pub fn host_selection_index(&self) -> usize {
        self.host_selection_index
    }

    /// Returns the host scroll offset.
    #[must_use]
    pub fn host_scroll_offset(&self) -> usize {
        self.host_scroll_offset
    }

    // --- Host Credentials Mode ---

    /// Starts credential entry for a host.
    pub fn start_host_credentials(&mut self, host_id: u32) {
        info!(
            "DockerManagerSelector::start_host_credentials called with host_id={}",
            host_id
        );
        self.cred_host_id = Some(host_id);
        self.cred_username.clear();
        self.cred_password.clear();
        self.cred_save = false;
        self.cred_field = HostCredentialField::Username;
        self.mode = DockerManagerMode::HostCredentials;
        info!(
            "DockerManagerSelector::start_host_credentials: mode is now {:?}",
            self.mode
        );
    }

    /// Cancels credential entry and returns to host selection.
    pub fn cancel_host_credentials(&mut self) {
        self.cred_host_id = None;
        self.cred_username.clear();
        self.cred_password.clear();
        self.mode = DockerManagerMode::HostSelection;
    }

    /// Returns the host ID being configured.
    #[must_use]
    pub fn cred_host_id(&self) -> Option<u32> {
        self.cred_host_id
    }

    /// Returns the hostname of the host being configured.
    #[must_use]
    pub fn cred_host_name(&self) -> Option<&str> {
        self.cred_host_id.and_then(|id| {
            self.available_hosts
                .iter()
                .find(|h| h.host_id == Some(id))
                .map(|h| h.display_name.as_str())
        })
    }

    /// Moves to the next credential field.
    pub fn next_cred_field(&mut self) {
        self.cred_field = self.cred_field.next();
    }

    /// Moves to the previous credential field.
    pub fn prev_cred_field(&mut self) {
        self.cred_field = self.cred_field.prev();
    }

    /// Returns the current credential field.
    #[must_use]
    pub fn cred_field(&self) -> HostCredentialField {
        self.cred_field
    }

    /// Inserts a character into the current credential field.
    pub fn cred_insert_char(&mut self, c: char) {
        match self.cred_field {
            HostCredentialField::Username => self.cred_username.push(c),
            HostCredentialField::Password => self.cred_password.push(c),
            HostCredentialField::SaveCheckbox => {} // No text input for checkbox
        }
    }

    /// Removes the last character from the current credential field.
    pub fn cred_backspace(&mut self) {
        match self.cred_field {
            HostCredentialField::Username => {
                self.cred_username.pop();
            }
            HostCredentialField::Password => {
                self.cred_password.pop();
            }
            HostCredentialField::SaveCheckbox => {} // No text input for checkbox
        }
    }

    /// Toggles the save credentials checkbox.
    pub fn toggle_save_credentials(&mut self) {
        if self.cred_field == HostCredentialField::SaveCheckbox {
            self.cred_save = !self.cred_save;
        }
    }

    /// Returns the entered credentials.
    #[must_use]
    pub fn get_entered_credentials(&self) -> (String, String, bool) {
        (
            self.cred_username.clone(),
            self.cred_password.clone(),
            self.cred_save,
        )
    }

    /// Returns the username input.
    #[must_use]
    pub fn cred_username(&self) -> &str {
        &self.cred_username
    }

    /// Returns the password input (masked).
    #[must_use]
    pub fn cred_password(&self) -> &str {
        &self.cred_password
    }

    /// Returns whether save credentials is checked.
    #[must_use]
    pub fn cred_save(&self) -> bool {
        self.cred_save
    }

    // =========================================================================
    // Container Creation Workflow Methods
    // =========================================================================

    /// Starts the container creation workflow.
    pub fn start_container_creation(&mut self) {
        info!("Starting container creation workflow");
        self.creation_state.reset();
        self.mode = DockerManagerMode::SearchingHub;
        self.input_buffer.clear();
    }

    /// Starts container creation with a pre-selected image.
    pub fn start_creation_from_image(&mut self, image_name: &str) {
        info!("Starting container creation for image: {}", image_name);
        self.creation_state = ContainerCreationState::with_image(image_name.to_string());
        self.creation_state.image_exists = true; // Assume exists since we came from images list
        self.mode = DockerManagerMode::VolumeMountHostPath;
        self.input_buffer.clear();
    }

    /// Cancels container creation and returns to list mode.
    pub fn cancel_container_creation(&mut self) {
        info!("Canceling container creation");
        self.creation_state.reset();
        self.mode = DockerManagerMode::List;
        self.input_buffer.clear();
    }

    /// Returns a reference to the creation state.
    #[must_use]
    pub fn creation_state(&self) -> &ContainerCreationState {
        &self.creation_state
    }

    /// Returns a mutable reference to the creation state.
    pub fn creation_state_mut(&mut self) -> &mut ContainerCreationState {
        &mut self.creation_state
    }

    /// Sets the search term and updates state.
    pub fn set_search_term(&mut self, term: String) {
        self.creation_state.search_term = term;
    }

    /// Inserts a character into the search term.
    pub fn insert_char_search(&mut self, c: char) {
        self.creation_state.search_term.push(c);
    }

    /// Deletes the last character from the search term.
    pub fn backspace_search(&mut self) {
        self.creation_state.search_term.pop();
    }

    /// Sets the search results and transitions to results mode.
    pub fn set_search_results(&mut self, results: Vec<crate::docker::DockerSearchResult>) {
        self.creation_state.set_search_results(results);
        self.mode = DockerManagerMode::SearchResults;
    }

    /// Selects the next search result.
    pub fn select_next_search_result(&mut self) {
        self.creation_state.select_next_result();
    }

    /// Selects the previous search result.
    pub fn select_prev_search_result(&mut self) {
        self.creation_state.select_prev_result();
    }

    /// Confirms the selected search result.
    pub fn confirm_search_selection(&mut self) {
        self.creation_state.confirm_selection();
        self.mode = DockerManagerMode::CheckingImage;
    }

    /// Sets whether the image exists on the host.
    ///
    /// If the image doesn't exist, we mark it as downloading but still proceed
    /// to volume mount configuration. The download happens in the background
    /// and we check completion at the final confirmation step.
    pub fn set_image_exists(&mut self, exists: bool) {
        self.creation_state.image_exists = exists;
        if exists {
            // Image exists, proceed to volume mount
            self.mode = DockerManagerMode::VolumeMountHostPath;
            self.input_buffer.clear();
        } else {
            // Image needs to be downloaded - mark as downloading but proceed
            // The download will happen in the background
            self.creation_state.downloading = true;
            self.mode = DockerManagerMode::VolumeMountHostPath;
            self.input_buffer.clear();
        }
    }

    /// Called when image pull completes.
    ///
    /// Since the user may have progressed past the download step, we don't
    /// change the mode unless there was an error. The creation confirmation
    /// screen will check if the download is complete before allowing creation.
    pub fn on_image_pull_complete(&mut self, success: bool, error: Option<String>) {
        self.creation_state.downloading = false;
        if success {
            self.creation_state.image_exists = true;
            // Don't change mode - user may be configuring volumes/commands
            // The UI will update to reflect download is complete
        } else {
            // Only show error if we're still in the creation workflow
            if self.mode.is_creation_mode() {
                self.creation_state.set_error(
                    error.unwrap_or_else(|| "Image download failed".to_string()),
                    true,
                );
                self.mode = DockerManagerMode::CreationError;
            }
        }
    }

    /// Returns true if the image is ready (exists or download complete).
    #[must_use]
    pub fn is_image_ready(&self) -> bool {
        self.creation_state.image_exists && !self.creation_state.downloading
    }

    /// Returns true if an image download is in progress.
    #[must_use]
    pub fn is_downloading(&self) -> bool {
        self.creation_state.downloading
    }

    /// Inserts a character into the current host path.
    pub fn insert_char_host_path(&mut self, c: char) {
        self.creation_state.current_host_path.push(c);
    }

    /// Deletes the last character from the host path.
    pub fn backspace_host_path(&mut self) {
        self.creation_state.current_host_path.pop();
    }

    /// Sets the host path and transitions to container path input.
    pub fn set_host_path(&mut self, path: String) {
        self.creation_state.current_host_path = path;
        self.mode = DockerManagerMode::VolumeMountContainerPath;
        self.input_buffer.clear();
    }

    /// Sets the volume host path from file browser and transitions to container path.
    pub fn set_volume_host_path(&mut self, path: &str) {
        self.creation_state.current_host_path = path.to_string();
        self.mode = DockerManagerMode::VolumeMountContainerPath;
        self.input_buffer.clear();
    }

    /// Confirms the host path and moves to container path input.
    pub fn confirm_host_path(&mut self) {
        if self.creation_state.current_host_path.is_empty() {
            // Skip volume mount, go to startup command
            self.mode = DockerManagerMode::StartupCommand;
        } else {
            self.mode = DockerManagerMode::VolumeMountContainerPath;
        }
        self.input_buffer.clear();
    }

    /// Inserts a character into the current container path.
    pub fn insert_char_container_path(&mut self, c: char) {
        self.creation_state.current_container_path.push(c);
    }

    /// Deletes the last character from the container path.
    pub fn backspace_container_path(&mut self) {
        self.creation_state.current_container_path.pop();
    }

    /// Confirms the container path and adds the mount.
    pub fn confirm_container_path(&mut self) {
        if self.creation_state.add_current_volume_mount() {
            self.mode = DockerManagerMode::VolumeMountConfirm;
        } else {
            // Both paths were empty, skip to startup command
            self.mode = DockerManagerMode::StartupCommand;
        }
        self.input_buffer.clear();
    }

    /// Handles the "add another volume" confirmation.
    pub fn confirm_add_another_volume(&mut self, add_another: bool) {
        if add_another {
            self.mode = DockerManagerMode::VolumeMountHostPath;
        } else {
            self.mode = DockerManagerMode::StartupCommand;
        }
        self.input_buffer.clear();
    }

    /// Inserts a character into the startup command.
    pub fn insert_char_startup_cmd(&mut self, c: char) {
        self.creation_state.startup_command.push(c);
    }

    /// Deletes the last character from the startup command.
    pub fn backspace_startup_cmd(&mut self) {
        self.creation_state.startup_command.pop();
    }

    /// Confirms the startup command and moves to final confirmation.
    pub fn confirm_startup_command(&mut self) {
        self.mode = DockerManagerMode::CreateConfirm;
    }

    /// Shows an error in the creation workflow.
    pub fn show_creation_error(&mut self, error: String, suggest_log: bool) {
        self.creation_state.set_error(error, suggest_log);
        self.mode = DockerManagerMode::CreationError;
    }

    /// Dismisses the creation error and returns to the command form.
    pub fn dismiss_creation_error(&mut self) {
        self.creation_state.clear_error();
        // Return to the startup command form to retry
        self.mode = DockerManagerMode::StartupCommand;
    }

    /// Returns the docker run command that would be executed.
    #[must_use]
    pub fn get_creation_run_command(&self) -> Option<String> {
        self.creation_state.build_run_command()
    }
}
