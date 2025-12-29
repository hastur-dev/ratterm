//! SSH Manager selector scan and edit name methods.

use super::selector::SSHManagerSelector;
use super::types::{SSHManagerMode, ScanCredentialField};

impl SSHManagerSelector {
    /// Starts the scan credential entry mode.
    pub fn start_scan_credential_entry(&mut self) {
        self.scan_username.clear();
        self.scan_password.clear();
        self.scan_subnet.clear();
        self.scan_credential_field = ScanCredentialField::Username;
        self.auth_success_count = 0;
        self.auth_fail_count = 0;
        self.mode = SSHManagerMode::ScanCredentialEntry;
    }

    /// Cancels scan credential entry.
    pub fn cancel_scan_credential_entry(&mut self) {
        self.mode = SSHManagerMode::List;
    }

    /// Returns the current scan credential field.
    #[must_use]
    pub fn scan_credential_field(&self) -> ScanCredentialField {
        self.scan_credential_field
    }

    /// Moves to the next scan credential field.
    pub fn next_scan_credential_field(&mut self) {
        self.scan_credential_field = self.scan_credential_field.next();
    }

    /// Moves to the previous scan credential field.
    pub fn prev_scan_credential_field(&mut self) {
        self.scan_credential_field = self.scan_credential_field.prev();
    }

    /// Inserts a character into the current scan credential field.
    pub fn scan_credential_insert(&mut self, c: char) {
        match self.scan_credential_field {
            ScanCredentialField::Username => self.scan_username.push(c),
            ScanCredentialField::Password => self.scan_password.push(c),
            ScanCredentialField::Subnet => self.scan_subnet.push(c),
        }
    }

    /// Backspace in the current scan credential field.
    pub fn scan_credential_backspace(&mut self) {
        match self.scan_credential_field {
            ScanCredentialField::Username => {
                self.scan_username.pop();
            }
            ScanCredentialField::Password => {
                self.scan_password.pop();
            }
            ScanCredentialField::Subnet => {
                self.scan_subnet.pop();
            }
        }
    }

    /// Returns the scan username.
    #[must_use]
    pub fn scan_username(&self) -> &str {
        &self.scan_username
    }

    /// Returns the scan password.
    #[must_use]
    pub fn scan_password(&self) -> &str {
        &self.scan_password
    }

    /// Returns the scan subnet.
    #[must_use]
    pub fn scan_subnet(&self) -> &str {
        &self.scan_subnet
    }

    /// Sets the authenticated scan mode with progress tracking.
    pub fn start_authenticated_scanning(&mut self, subnet: String) {
        self.scanning_subnet = Some(subnet);
        self.auth_success_count = 0;
        self.auth_fail_count = 0;
        self.mode = SSHManagerMode::AuthenticatedScanning;
    }

    /// Updates authentication counts during scan.
    pub fn update_auth_counts(&mut self, success: usize, fail: usize) {
        self.auth_success_count = success;
        self.auth_fail_count = fail;
    }

    /// Returns the auth success count.
    #[must_use]
    pub fn auth_success_count(&self) -> usize {
        self.auth_success_count
    }

    /// Returns the auth fail count.
    #[must_use]
    pub fn auth_fail_count(&self) -> usize {
        self.auth_fail_count
    }

    /// Starts editing the name of the selected host.
    pub fn start_edit_name(&mut self) {
        let host_info = self
            .selected_host()
            .map(|h| (h.host.id, h.host.display().to_string()));

        if let Some((id, display_name)) = host_info {
            self.edit_name_target = Some(id);
            self.edit_name_input = display_name;
            self.mode = SSHManagerMode::EditName;
        }
    }

    /// Cancels name editing and returns to list mode.
    pub fn cancel_edit_name(&mut self) {
        self.edit_name_input.clear();
        self.edit_name_target = None;
        self.mode = SSHManagerMode::List;
    }

    /// Inserts a character into the name input.
    pub fn edit_name_insert(&mut self, c: char) {
        self.edit_name_input.push(c);
    }

    /// Backspace in the name input.
    pub fn edit_name_backspace(&mut self) {
        self.edit_name_input.pop();
    }

    /// Returns the current name input.
    #[must_use]
    pub fn edit_name_input(&self) -> &str {
        &self.edit_name_input
    }

    /// Returns the host ID being renamed.
    #[must_use]
    pub fn edit_name_target(&self) -> Option<u32> {
        self.edit_name_target
    }

    /// Clears the edit name state after successful rename.
    pub fn clear_edit_name(&mut self) {
        self.edit_name_input.clear();
        self.edit_name_target = None;
        self.mode = SSHManagerMode::List;
    }
}
