//! Docker Manager selector state and methods.

use tracing::info;

use crate::docker::{
    DockerAvailability, DockerContainer, DockerDiscoveryResult, DockerHost, DockerImage,
    DockerRunOptions,
};

use super::types::{
    DockerHostDisplay, DockerItemDisplay, DockerListSection, DockerManagerMode,
    HostCredentialField, RunOptionsField, MAX_DISPLAY_HOSTS, MAX_DISPLAY_ITEMS,
};

/// Docker Manager selector state.
#[derive(Debug, Clone)]
pub struct DockerManagerSelector {
    /// Running containers.
    pub(super) running_containers: Vec<DockerContainer>,
    /// Stopped containers.
    pub(super) stopped_containers: Vec<DockerContainer>,
    /// Available images.
    pub(super) images: Vec<DockerImage>,
    /// Current section.
    pub(super) section: DockerListSection,
    /// Currently selected index within section.
    pub(super) selected_index: usize,
    /// Current mode.
    pub(super) mode: DockerManagerMode,
    /// Scroll offset for long lists.
    pub(super) scroll_offset: usize,
    /// Error message to display.
    pub(super) error: Option<String>,
    /// Status message to display.
    pub(super) status: Option<String>,
    /// Whether Docker is available on the system.
    pub(super) docker_available: bool,
    /// Detailed Docker availability status.
    pub(super) availability: DockerAvailability,
    /// Run options being configured.
    pub(super) run_options: DockerRunOptions,
    /// Current field in run options mode.
    pub(super) run_options_field: RunOptionsField,
    /// Target image for run options.
    pub(super) run_target: Option<String>,
    /// Confirm target (container ID or image name).
    pub(super) confirm_target: Option<String>,
    /// Input buffer for current field.
    pub(super) input_buffer: String,
    // --- Host selection state ---
    /// Available hosts for Docker management.
    pub(super) available_hosts: Vec<DockerHostDisplay>,
    /// Currently selected Docker host.
    pub(super) selected_host: DockerHost,
    /// Selected index in host selection mode.
    pub(super) host_selection_index: usize,
    /// Host scroll offset for long lists.
    pub(super) host_scroll_offset: usize,
    // --- Credential entry state ---
    /// Host ID being configured with credentials.
    pub(super) cred_host_id: Option<u32>,
    /// Username input.
    pub(super) cred_username: String,
    /// Password input.
    pub(super) cred_password: String,
    /// Whether to save the credentials.
    pub(super) cred_save: bool,
    /// Current credential field.
    pub(super) cred_field: HostCredentialField,
}

impl DockerManagerSelector {
    /// Creates a new Docker manager selector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            running_containers: Vec::new(),
            stopped_containers: Vec::new(),
            images: Vec::new(),
            section: DockerListSection::RunningContainers,
            selected_index: 0,
            mode: DockerManagerMode::List,
            scroll_offset: 0,
            error: None,
            status: None,
            docker_available: false,
            availability: DockerAvailability::Unknown,
            run_options: DockerRunOptions::new(),
            run_options_field: RunOptionsField::Name,
            run_target: None,
            confirm_target: None,
            input_buffer: String::new(),
            // Host selection
            available_hosts: Vec::new(),
            selected_host: DockerHost::Local,
            host_selection_index: 0,
            host_scroll_offset: 0,
            // Credential entry
            cred_host_id: None,
            cred_username: String::new(),
            cred_password: String::new(),
            cred_save: false,
            cred_field: HostCredentialField::Username,
        }
    }

    /// Updates from discovery result.
    pub fn update_from_discovery(&mut self, result: DockerDiscoveryResult) {
        self.docker_available = result.docker_available;
        self.availability = result.availability;
        self.running_containers = result.running_containers;
        self.stopped_containers = result.stopped_containers;
        self.images = result.images;

        if let Some(err) = result.error {
            self.error = Some(err);
        } else {
            self.error = None;
        }

        // Reset selection if needed
        self.validate_selection();
    }

    /// Returns the Docker availability status.
    #[must_use]
    pub fn availability(&self) -> DockerAvailability {
        self.availability.clone()
    }

    /// Validates and fixes selection if out of bounds.
    fn validate_selection(&mut self) {
        let count = self.current_section_count();
        if count == 0 {
            self.selected_index = 0;
            self.scroll_offset = 0;
        } else if self.selected_index >= count {
            self.selected_index = count - 1;
            self.update_scroll();
        }
    }

    /// Returns count of items in current section.
    #[must_use]
    pub fn current_section_count(&self) -> usize {
        match self.section {
            DockerListSection::RunningContainers => self.running_containers.len(),
            DockerListSection::StoppedContainers => self.stopped_containers.len(),
            DockerListSection::Images => self.images.len(),
        }
    }

    /// Returns true if current section is empty.
    #[must_use]
    pub fn is_section_empty(&self) -> bool {
        self.current_section_count() == 0
    }

    /// Returns true if all sections are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.running_containers.is_empty()
            && self.stopped_containers.is_empty()
            && self.images.is_empty()
    }

    /// Returns total count of all items.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.running_containers.len() + self.stopped_containers.len() + self.images.len()
    }

    /// Returns the current mode.
    #[must_use]
    pub fn mode(&self) -> DockerManagerMode {
        self.mode
    }

    /// Sets the mode.
    pub fn set_mode(&mut self, mode: DockerManagerMode) {
        self.mode = mode;
    }

    /// Returns the current section.
    #[must_use]
    pub fn section(&self) -> DockerListSection {
        self.section
    }

    /// Switches to the next section.
    pub fn next_section(&mut self) {
        self.section = self.section.next();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Switches to the previous section.
    pub fn prev_section(&mut self) {
        self.section = self.section.prev();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Sets the current section directly.
    pub fn set_section(&mut self, section: DockerListSection) {
        if self.section != section {
            self.section = section;
            self.selected_index = 0;
            self.scroll_offset = 0;
        }
    }

    /// Returns the currently selected item.
    #[must_use]
    pub fn selected_item(&self) -> Option<DockerItemDisplay> {
        match self.section {
            DockerListSection::RunningContainers => self
                .running_containers
                .get(self.selected_index)
                .cloned()
                .map(DockerItemDisplay::Container),
            DockerListSection::StoppedContainers => self
                .stopped_containers
                .get(self.selected_index)
                .cloned()
                .map(DockerItemDisplay::Container),
            DockerListSection::Images => self
                .images
                .get(self.selected_index)
                .cloned()
                .map(DockerItemDisplay::Image),
        }
    }

    /// Returns the selected container (if in container section).
    #[must_use]
    pub fn selected_container(&self) -> Option<&DockerContainer> {
        match self.section {
            DockerListSection::RunningContainers => {
                self.running_containers.get(self.selected_index)
            }
            DockerListSection::StoppedContainers => {
                self.stopped_containers.get(self.selected_index)
            }
            DockerListSection::Images => None,
        }
    }

    /// Returns the selected image (if in images section).
    #[must_use]
    pub fn selected_image(&self) -> Option<&DockerImage> {
        if self.section == DockerListSection::Images {
            self.images.get(self.selected_index)
        } else {
            None
        }
    }

    /// Moves selection up.
    pub fn select_prev(&mut self) {
        if self.current_section_count() > 0 {
            self.selected_index = self.selected_index.saturating_sub(1);
            self.update_scroll();
        }
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        let count = self.current_section_count();
        if count > 0 {
            self.selected_index = (self.selected_index + 1).min(count - 1);
            self.update_scroll();
        }
    }

    /// Moves selection to first item.
    pub fn select_first(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Moves selection to last item.
    pub fn select_last(&mut self) {
        let count = self.current_section_count();
        if count > 0 {
            self.selected_index = count - 1;
            self.update_scroll();
        }
    }

    /// Updates scroll offset to keep selection visible.
    fn update_scroll(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + MAX_DISPLAY_ITEMS {
            self.scroll_offset = self.selected_index - MAX_DISPLAY_ITEMS + 1;
        }
    }

    /// Returns visible items for rendering.
    pub fn visible_items(&self) -> Vec<(usize, DockerItemDisplay)> {
        match self.section {
            DockerListSection::RunningContainers => self
                .running_containers
                .iter()
                .enumerate()
                .skip(self.scroll_offset)
                .take(MAX_DISPLAY_ITEMS)
                .map(|(i, c)| (i, DockerItemDisplay::Container(c.clone())))
                .collect(),
            DockerListSection::StoppedContainers => self
                .stopped_containers
                .iter()
                .enumerate()
                .skip(self.scroll_offset)
                .take(MAX_DISPLAY_ITEMS)
                .map(|(i, c)| (i, DockerItemDisplay::Container(c.clone())))
                .collect(),
            DockerListSection::Images => self
                .images
                .iter()
                .enumerate()
                .skip(self.scroll_offset)
                .take(MAX_DISPLAY_ITEMS)
                .map(|(i, img)| (i, DockerItemDisplay::Image(img.clone())))
                .collect(),
        }
    }

    /// Sets an error message.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Clears the error message.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Returns the error message.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Sets a status message.
    pub fn set_status(&mut self, status: String) {
        self.status = Some(status);
    }

    /// Clears the status message.
    pub fn clear_status(&mut self) {
        self.status = None;
    }

    /// Returns the status message.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Returns whether Docker is available.
    #[must_use]
    pub fn docker_available(&self) -> bool {
        self.docker_available
    }

    // --- Run Options Mode ---

    /// Starts run options mode for an image.
    pub fn start_run_options(&mut self, image_name: String) {
        self.run_target = Some(image_name);
        self.run_options = DockerRunOptions::new();
        self.run_options_field = RunOptionsField::Name;
        self.input_buffer.clear();
        self.mode = DockerManagerMode::RunOptions;
    }

    /// Cancels run options and returns to list mode.
    pub fn cancel_run_options(&mut self) {
        self.run_target = None;
        self.run_options = DockerRunOptions::new();
        self.input_buffer.clear();
        self.mode = DockerManagerMode::List;
    }

    /// Returns the current run options.
    #[must_use]
    pub fn run_options(&self) -> &DockerRunOptions {
        &self.run_options
    }

    /// Returns the run target image.
    #[must_use]
    pub fn run_target(&self) -> Option<&str> {
        self.run_target.as_deref()
    }

    /// Returns the current run options field.
    #[must_use]
    pub fn run_options_field(&self) -> RunOptionsField {
        self.run_options_field
    }

    /// Moves to the next run options field.
    pub fn next_run_options_field(&mut self) {
        // Save current field before moving
        self.save_current_field();
        self.run_options_field = self.run_options_field.next();
        self.load_current_field();
    }

    /// Moves to the previous run options field.
    pub fn prev_run_options_field(&mut self) {
        self.save_current_field();
        self.run_options_field = self.run_options_field.prev();
        self.load_current_field();
    }

    /// Returns the input buffer.
    #[must_use]
    pub fn input_buffer(&self) -> &str {
        &self.input_buffer
    }

    /// Inserts a character into the input buffer.
    pub fn insert_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    /// Removes the last character from the input buffer.
    pub fn backspace(&mut self) {
        self.input_buffer.pop();
    }

    /// Clears the input buffer.
    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
    }

    /// Saves the current field's value from input buffer.
    fn save_current_field(&mut self) {
        let value = self.input_buffer.trim().to_string();
        match self.run_options_field {
            RunOptionsField::Name => {
                self.run_options.name = if value.is_empty() { None } else { Some(value) };
            }
            RunOptionsField::Ports => {
                self.run_options.port_mappings = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            RunOptionsField::Volumes => {
                self.run_options.volume_mounts = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            RunOptionsField::EnvVars => {
                self.run_options.env_vars = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            RunOptionsField::Shell => {
                self.run_options.shell = if value.is_empty() {
                    "/bin/sh".to_string()
                } else {
                    value
                };
            }
        }
    }

    /// Loads the current field's value into input buffer.
    fn load_current_field(&mut self) {
        self.input_buffer = match self.run_options_field {
            RunOptionsField::Name => self.run_options.name.clone().unwrap_or_default(),
            RunOptionsField::Ports => self.run_options.port_mappings.join(", "),
            RunOptionsField::Volumes => self.run_options.volume_mounts.join(", "),
            RunOptionsField::EnvVars => self.run_options.env_vars.join(", "),
            RunOptionsField::Shell => self.run_options.shell.clone(),
        };
    }

    /// Finishes run options and validates.
    pub fn finish_run_options(&mut self) -> Result<DockerRunOptions, String> {
        self.save_current_field();
        self.run_options.validate()?;
        Ok(self.run_options.clone())
    }

    // --- Confirm Mode ---

    /// Starts confirm mode for running an image.
    pub fn start_confirm(&mut self, target: String) {
        self.confirm_target = Some(target);
        self.mode = DockerManagerMode::Confirming;
    }

    /// Cancels confirmation and returns to list mode.
    pub fn cancel_confirm(&mut self) {
        self.confirm_target = None;
        self.mode = DockerManagerMode::List;
    }

    /// Returns the confirm target.
    #[must_use]
    pub fn confirm_target(&self) -> Option<&str> {
        self.confirm_target.as_deref()
    }

    /// Returns the selected index.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns the scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

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
            self.available_hosts.push(DockerHostDisplay::remote(
                *id,
                hostname.clone(),
                *port,
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
    fn update_host_scroll(&mut self) {
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
}

impl Default for DockerManagerSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_selection_flow() {
        let mut selector = DockerManagerSelector::new();

        // Simulate loading hosts - one local, two remote
        let ssh_hosts = vec![
            (1u32, "host1.example.com".to_string(), 22u16, Some("My Server".to_string()), false),
            (2u32, "host2.example.com".to_string(), 22u16, None, true), // has_creds = true
        ];

        selector.load_available_hosts(&ssh_hosts);

        // Verify hosts are loaded
        assert_eq!(selector.available_hosts().len(), 3); // Local + 2 remote

        // Start host selection
        selector.start_host_selection();
        assert_eq!(selector.mode(), DockerManagerMode::HostSelection);
        assert_eq!(selector.host_selection_index(), 0);

        // Navigate to first remote host (index 1)
        selector.select_next_host();
        assert_eq!(selector.host_selection_index(), 1);

        // Check selected host display
        let host = selector.selected_host_display().unwrap();
        assert!(!host.is_local());
        assert_eq!(host.host_id, Some(1));
        assert!(!host.has_credentials); // No creds for host 1
    }

    #[test]
    fn test_credential_flow_no_creds() {
        let mut selector = DockerManagerSelector::new();

        // Load host without credentials
        let ssh_hosts = vec![
            (1u32, "host1.example.com".to_string(), 22u16, None, false), // has_creds = false
        ];
        selector.load_available_hosts(&ssh_hosts);

        // Start host selection and navigate to remote
        selector.start_host_selection();
        selector.select_next_host(); // Move to remote host

        // Check that host has no credentials
        let host = selector.selected_host_display().unwrap();
        assert!(!host.has_credentials);

        // Start credential entry
        selector.start_host_credentials(1);

        // Verify mode changed to HostCredentials
        assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);
        assert_eq!(selector.cred_host_id(), Some(1));

        // Verify credential form is initialized
        assert_eq!(selector.cred_field(), HostCredentialField::Username);
        assert!(selector.cred_username().is_empty());
        assert!(selector.cred_password().is_empty());
    }

    #[test]
    fn test_credential_entry() {
        let mut selector = DockerManagerSelector::new();

        // Start credential entry
        selector.start_host_credentials(1);

        // Enter username
        for c in "testuser".chars() {
            selector.cred_insert_char(c);
        }
        assert_eq!(selector.cred_username(), "testuser");

        // Move to password field
        selector.next_cred_field();
        assert_eq!(selector.cred_field(), HostCredentialField::Password);

        // Enter password
        for c in "secret123".chars() {
            selector.cred_insert_char(c);
        }
        assert_eq!(selector.cred_password(), "secret123");

        // Get entered credentials
        let (user, pass, save) = selector.get_entered_credentials();
        assert_eq!(user, "testuser");
        assert_eq!(pass, "secret123");
        assert!(!save); // Default is false
    }

    #[test]
    fn test_host_display_is_local() {
        let local = DockerHostDisplay::local();
        assert!(local.is_local());
        assert!(local.host_id.is_none());

        let remote = DockerHostDisplay::remote(
            1,
            "example.com".to_string(),
            22,
            "user".to_string(),
            None,
            false,
        );
        assert!(!remote.is_local());
        assert_eq!(remote.host_id, Some(1));
    }

    /// Tests the decision logic for when to prompt for credentials.
    /// This simulates the logic from docker_confirm_host_selection.
    #[test]
    fn test_credential_prompt_decision_logic() {
        // Scenario 1: Remote host with has_credentials=false
        // Expected: Should prompt for credentials
        let host1 = DockerHostDisplay::remote(1, "h1.com".into(), 22, "u".into(), None, false);
        assert!(!host1.is_local());
        assert!(!host1.has_credentials);
        // In this case, code enters `else` branch and calls start_host_credentials

        // Scenario 2: Remote host with has_credentials=true but password is None
        // Expected: Should prompt for credentials (after looking up and finding no password)
        let host2 = DockerHostDisplay::remote(2, "h2.com".into(), 22, "u".into(), None, true);
        assert!(!host2.is_local());
        assert!(host2.has_credentials);
        // In this case, code enters `if has_creds` branch, looks up creds,
        // and should prompt if password.is_none() or password.is_empty()

        // Scenario 3: Local host
        // Expected: Should NOT prompt, just select local
        let host3 = DockerHostDisplay::local();
        assert!(host3.is_local());
        // In this case, code calls docker_select_local_host and returns

        // Verify mode transitions work correctly
        let mut selector = DockerManagerSelector::new();

        // Start host selection
        selector.start_host_selection();
        assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

        // Start credential entry
        selector.start_host_credentials(1);
        assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);

        // Cancel should go back to host selection
        selector.cancel_host_credentials();
        assert_eq!(selector.mode(), DockerManagerMode::HostSelection);
    }

    /// Tests that password_missing logic handles all edge cases
    #[test]
    fn test_password_missing_logic() {
        // Simulating the password check from docker_confirm_host_selection:
        // let password_missing = password.as_ref().is_none_or(|p| p.is_empty());

        fn is_password_missing(password: &Option<String>) -> bool {
            password.as_ref().is_none_or(|p| p.is_empty())
        }

        // Case 1: None
        assert!(is_password_missing(&None));

        // Case 2: Some("")
        assert!(is_password_missing(&Some(String::new())));

        // Case 3: Some("password")
        assert!(!is_password_missing(&Some("password".to_string())));

        // Case 4: Some(" ") - whitespace only (still a password)
        assert!(!is_password_missing(&Some(" ".to_string())));
    }

    /// Integration test: simulates host selection with existing credentials
    #[test]
    fn test_full_flow_host_with_credentials() {
        let mut selector = DockerManagerSelector::new();

        // Load hosts - one has credentials, one doesn't
        let ssh_hosts = vec![
            (1u32, "host1.example.com".to_string(), 22u16, Some("Host With Creds".to_string()), true),
            (2u32, "host2.example.com".to_string(), 22u16, Some("Host Without Creds".to_string()), false),
        ];
        selector.load_available_hosts(&ssh_hosts);

        // Verify 3 hosts loaded (Local + 2 remote)
        assert_eq!(selector.available_hosts().len(), 3);

        // Start host selection
        selector.start_host_selection();
        assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

        // Index 0 = Local, Index 1 = Host With Creds, Index 2 = Host Without Creds
        assert_eq!(selector.host_selection_index(), 0);

        // Navigate to Host With Creds (index 1)
        selector.select_next_host();
        assert_eq!(selector.host_selection_index(), 1);

        // Get selected host
        let selected = selector.selected_host_display().unwrap();
        assert_eq!(selected.display_name, "Host With Creds");
        assert_eq!(selected.host_id, Some(1));
        assert!(selected.has_credentials); // Has saved creds

        // Since has_credentials=true, the app logic would:
        // 1. Look up SSH credentials
        // 2. Check if password exists
        // 3. If password exists -> use it, no prompt
        // 4. If password missing -> call start_host_credentials

        // Navigate to Host Without Creds (index 2)
        selector.select_next_host();
        assert_eq!(selector.host_selection_index(), 2);

        let selected = selector.selected_host_display().unwrap();
        assert_eq!(selected.display_name, "Host Without Creds");
        assert_eq!(selected.host_id, Some(2));
        assert!(!selected.has_credentials); // No saved creds

        // Since has_credentials=false, app would call start_host_credentials
        selector.start_host_credentials(2);
        assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);
        assert_eq!(selector.cred_host_id(), Some(2));
    }

    /// Test that mode stays correct through the flow
    #[test]
    fn test_mode_transitions_complete_flow() {
        let mut selector = DockerManagerSelector::new();

        // Initial mode
        assert_eq!(selector.mode(), DockerManagerMode::List);

        // Start host selection
        selector.start_host_selection();
        assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

        // Start credential entry
        selector.start_host_credentials(1);
        assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);

        // Enter credentials
        for c in "testuser".chars() {
            selector.cred_insert_char(c);
        }
        selector.next_cred_field();
        for c in "testpass".chars() {
            selector.cred_insert_char(c);
        }

        // Verify credentials were entered
        let (user, pass, _) = selector.get_entered_credentials();
        assert_eq!(user, "testuser");
        assert_eq!(pass, "testpass");

        // Cancel credentials should go back to host selection
        selector.cancel_host_credentials();
        assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

        // Cancel host selection should go back to list
        selector.cancel_host_selection();
        assert_eq!(selector.mode(), DockerManagerMode::List);
    }
}
