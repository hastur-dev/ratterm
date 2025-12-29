//! SSH Manager selector state and methods.

use crate::ssh::{ConnectionStatus, SSHHostList};

use super::types::{
    AddHostField, CredentialField, MAX_DISPLAY_HOSTS, SSHHostDisplay, SSHManagerMode,
    ScanCredentialField,
};

/// SSH Manager selector state.
#[derive(Debug, Clone)]
pub struct SSHManagerSelector {
    /// List of hosts to display.
    pub(super) hosts: Vec<SSHHostDisplay>,
    /// Currently selected host index.
    pub(super) selected_index: usize,
    /// Current mode.
    pub(super) mode: SSHManagerMode,
    /// Scan progress (scanned, total).
    scan_progress: Option<(usize, usize)>,
    /// Subnet currently being scanned.
    pub(super) scanning_subnet: Option<String>,
    /// Current credential field being edited.
    credential_field: CredentialField,
    /// Username input.
    username: String,
    /// Password input.
    password: String,
    /// Whether to save credentials.
    save_credentials: bool,
    /// Target host for credential entry.
    credential_target: Option<u32>,
    /// Hostname input for manual add.
    hostname_input: String,
    /// Port input for manual add.
    port_input: String,
    /// Display name input for manual add.
    add_host_display_name: String,
    /// Username input for manual add.
    add_host_username: String,
    /// Password input for manual add.
    add_host_password: String,
    /// Current field in add host mode.
    add_host_field: AddHostField,
    /// Error message to display.
    error: Option<String>,
    /// Scroll offset for long lists.
    scroll_offset: usize,
    /// Current field in scan credential entry.
    pub(super) scan_credential_field: ScanCredentialField,
    /// Username for authenticated scan.
    pub(super) scan_username: String,
    /// Password for authenticated scan.
    pub(super) scan_password: String,
    /// Subnet for authenticated scan.
    pub(super) scan_subnet: String,
    /// Number of hosts authenticated during scan.
    pub(super) auth_success_count: usize,
    /// Number of hosts that failed auth during scan.
    pub(super) auth_fail_count: usize,
    /// Display name being edited.
    pub(super) edit_name_input: String,
    /// Host ID being renamed.
    pub(super) edit_name_target: Option<u32>,
}

impl SSHManagerSelector {
    /// Creates a new SSH manager selector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hosts: Vec::new(),
            selected_index: 0,
            mode: SSHManagerMode::List,
            scan_progress: None,
            scanning_subnet: None,
            credential_field: CredentialField::Username,
            username: String::new(),
            password: String::new(),
            save_credentials: true,
            credential_target: None,
            hostname_input: String::new(),
            port_input: "22".to_string(),
            add_host_display_name: String::new(),
            add_host_username: String::new(),
            add_host_password: String::new(),
            add_host_field: AddHostField::Hostname,
            error: None,
            scroll_offset: 0,
            scan_credential_field: ScanCredentialField::Username,
            scan_username: String::new(),
            scan_password: String::new(),
            scan_subnet: String::new(),
            auth_success_count: 0,
            auth_fail_count: 0,
            edit_name_input: String::new(),
            edit_name_target: None,
        }
    }

    /// Updates the host list from storage.
    pub fn update_from_list(&mut self, list: &SSHHostList) {
        self.hosts = list
            .hosts()
            .map(|h| SSHHostDisplay::new(h.clone(), list.get_credentials(h.id).is_some()))
            .collect();

        if self.selected_index >= self.hosts.len() && !self.hosts.is_empty() {
            self.selected_index = self.hosts.len() - 1;
        }
    }

    /// Returns the number of hosts.
    #[must_use]
    pub fn host_count(&self) -> usize {
        self.hosts.len()
    }

    /// Returns true if the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    /// Returns the currently selected host.
    #[must_use]
    pub fn selected_host(&self) -> Option<&SSHHostDisplay> {
        self.hosts.get(self.selected_index)
    }

    /// Returns the selected host ID.
    #[must_use]
    pub fn selected_host_id(&self) -> Option<u32> {
        self.selected_host().map(|h| h.host.id)
    }

    /// Returns the current mode.
    #[must_use]
    pub fn mode(&self) -> SSHManagerMode {
        self.mode
    }

    /// Sets the mode.
    pub fn set_mode(&mut self, mode: SSHManagerMode) {
        self.mode = mode;
        if mode == SSHManagerMode::CredentialEntry {
            self.credential_field = CredentialField::Username;
        }
    }

    /// Moves selection up.
    pub fn select_prev(&mut self) {
        if !self.hosts.is_empty() {
            self.selected_index = self.selected_index.saturating_sub(1);
            self.update_scroll();
        }
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        if !self.hosts.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.hosts.len() - 1);
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
        if !self.hosts.is_empty() {
            self.selected_index = self.hosts.len() - 1;
            self.update_scroll();
        }
    }

    /// Updates scroll offset to keep selection visible.
    fn update_scroll(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + MAX_DISPLAY_HOSTS {
            self.scroll_offset = self.selected_index - MAX_DISPLAY_HOSTS + 1;
        }
    }

    /// Moves to the next credential field.
    pub fn next_field(&mut self) {
        self.credential_field = self.credential_field.next();
    }

    /// Alias for next_field used in input handlers.
    pub fn next_credential_field(&mut self) {
        self.next_field();
    }

    /// Moves to the previous credential field.
    pub fn prev_credential_field(&mut self) {
        self.credential_field = self.credential_field.next();
    }

    /// Inserts a character into the credential field.
    pub fn credential_insert(&mut self, c: char) {
        match self.credential_field {
            CredentialField::Username => self.username.push(c),
            CredentialField::Password => self.password.push(c),
        }
    }

    /// Backspace in the credential field.
    pub fn credential_backspace(&mut self) {
        match self.credential_field {
            CredentialField::Username => {
                self.username.pop();
            }
            CredentialField::Password => {
                self.password.pop();
            }
        }
    }

    /// Cancels credential entry and returns to list mode.
    pub fn cancel_credential_entry(&mut self) {
        self.clear_credentials();
        self.mode = SSHManagerMode::List;
    }

    /// Returns the current credential field.
    #[must_use]
    pub fn credential_field(&self) -> CredentialField {
        self.credential_field
    }

    /// Returns the username input.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the password input.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    /// Returns whether to save credentials.
    #[must_use]
    pub fn save_credentials(&self) -> bool {
        self.save_credentials
    }

    /// Toggles the save credentials option.
    pub fn toggle_save_credentials(&mut self) {
        self.save_credentials = !self.save_credentials;
    }

    /// Inserts a character into the current field.
    pub fn insert_char(&mut self, c: char) {
        match self.mode {
            SSHManagerMode::CredentialEntry => match self.credential_field {
                CredentialField::Username => self.username.push(c),
                CredentialField::Password => self.password.push(c),
            },
            SSHManagerMode::AddHost => {
                self.hostname_input.push(c);
            }
            _ => {}
        }
    }

    /// Deletes the last character from the current field.
    pub fn backspace(&mut self) {
        match self.mode {
            SSHManagerMode::CredentialEntry => match self.credential_field {
                CredentialField::Username => {
                    self.username.pop();
                }
                CredentialField::Password => {
                    self.password.pop();
                }
            },
            SSHManagerMode::AddHost => {
                self.hostname_input.pop();
            }
            _ => {}
        }
    }

    /// Clears credential inputs.
    pub fn clear_credentials(&mut self) {
        self.username.clear();
        self.password.clear();
        self.credential_field = CredentialField::Username;
        self.credential_target = None;
    }

    /// Sets the credential target host.
    pub fn set_credential_target(&mut self, host_id: u32) {
        self.credential_target = Some(host_id);
    }

    /// Returns the credential target host ID.
    #[must_use]
    pub fn credential_target(&self) -> Option<u32> {
        self.credential_target
    }

    /// Sets the scan progress.
    pub fn set_scan_progress(&mut self, scanned: usize, total: usize) {
        self.scan_progress = Some((scanned, total));
    }

    /// Clears the scan progress.
    pub fn clear_scan_progress(&mut self) {
        self.scan_progress = None;
        self.scanning_subnet = None;
    }

    /// Returns the scan progress.
    #[must_use]
    pub fn scan_progress(&self) -> Option<(usize, usize)> {
        self.scan_progress
    }

    /// Sets the subnet being scanned.
    pub fn set_scanning_subnet(&mut self, subnet: String) {
        self.scanning_subnet = Some(subnet);
    }

    /// Returns the subnet being scanned.
    #[must_use]
    pub fn scanning_subnet(&self) -> Option<&str> {
        self.scanning_subnet.as_deref()
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

    /// Returns the hostname input.
    #[must_use]
    pub fn hostname_input(&self) -> &str {
        &self.hostname_input
    }

    /// Returns the port input.
    #[must_use]
    pub fn port_input(&self) -> &str {
        &self.port_input
    }

    /// Returns the add host username.
    #[must_use]
    pub fn add_host_username(&self) -> &str {
        &self.add_host_username
    }

    /// Returns the add host password.
    #[must_use]
    pub fn add_host_password(&self) -> &str {
        &self.add_host_password
    }

    /// Returns the current add host field.
    #[must_use]
    pub fn add_host_field(&self) -> AddHostField {
        self.add_host_field
    }

    /// Returns the add host display name.
    #[must_use]
    pub fn add_host_display_name(&self) -> &str {
        &self.add_host_display_name
    }

    /// Clears the add host inputs.
    pub fn clear_add_host(&mut self) {
        self.hostname_input.clear();
        self.port_input = "22".to_string();
        self.add_host_display_name.clear();
        self.add_host_username.clear();
        self.add_host_password.clear();
        self.add_host_field = AddHostField::Hostname;
    }

    /// Starts the add host mode.
    pub fn start_add_host(&mut self) {
        self.clear_add_host();
        self.mode = SSHManagerMode::AddHost;
    }

    /// Cancels add host and returns to list mode.
    pub fn cancel_add_host(&mut self) {
        self.clear_add_host();
        self.mode = SSHManagerMode::List;
    }

    /// Moves to the next add host field.
    pub fn next_add_host_field(&mut self) {
        self.add_host_field = self.add_host_field.next();
    }

    /// Moves to the previous add host field.
    pub fn prev_add_host_field(&mut self) {
        self.add_host_field = self.add_host_field.prev();
    }

    /// Inserts a character into the current add host field.
    pub fn add_host_insert(&mut self, c: char) {
        match self.add_host_field {
            AddHostField::Hostname => self.hostname_input.push(c),
            AddHostField::Port => {
                if c.is_ascii_digit() {
                    self.port_input.push(c);
                }
            }
            AddHostField::DisplayName => self.add_host_display_name.push(c),
            AddHostField::Username => self.add_host_username.push(c),
            AddHostField::Password => self.add_host_password.push(c),
        }
    }

    /// Backspace in the current add host field.
    pub fn add_host_backspace(&mut self) {
        match self.add_host_field {
            AddHostField::Hostname => {
                self.hostname_input.pop();
            }
            AddHostField::Port => {
                self.port_input.pop();
            }
            AddHostField::DisplayName => {
                self.add_host_display_name.pop();
            }
            AddHostField::Username => {
                self.add_host_username.pop();
            }
            AddHostField::Password => {
                self.add_host_password.pop();
            }
        }
    }

    /// Marks a host's connection status.
    pub fn set_host_status(&mut self, host_id: u32, status: ConnectionStatus) {
        if let Some(host) = self.hosts.iter_mut().find(|h| h.host.id == host_id) {
            host.status = status;
        }
    }

    /// Returns visible hosts for rendering.
    pub fn visible_hosts(&self) -> impl Iterator<Item = (usize, &SSHHostDisplay)> {
        self.hosts
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(MAX_DISPLAY_HOSTS)
    }
}

impl Default for SSHManagerSelector {
    fn default() -> Self {
        Self::new()
    }
}
