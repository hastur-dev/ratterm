//! SSH Manager widget for displaying and managing SSH connections.
//!
//! Provides a popup interface for:
//! - Viewing saved SSH hosts
//! - Connecting to hosts
//! - Scanning the network for new hosts
//! - Managing credentials

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, Widget},
};

use crate::ssh::{ConnectionStatus, SSHHost, SSHHostList};

/// Maximum number of hosts to display in the list.
const MAX_DISPLAY_HOSTS: usize = 10;

/// SSH Manager mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SSHManagerMode {
    /// Viewing the host list.
    #[default]
    List,
    /// Network scan in progress.
    Scanning,
    /// Entering credentials for connection.
    CredentialEntry,
    /// Connection attempt in progress.
    Connecting,
    /// Adding a new host manually.
    AddHost,
    /// Entering credentials for authenticated scan.
    ScanCredentialEntry,
    /// Authenticated scan in progress.
    AuthenticatedScanning,
    /// Editing the display name of a host.
    EditName,
}

/// Field being edited in credential entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CredentialField {
    #[default]
    Username,
    Password,
}

impl CredentialField {
    /// Moves to the next field.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Username => Self::Password,
            Self::Password => Self::Username,
        }
    }
}

/// Display information for an SSH host.
#[derive(Debug, Clone)]
pub struct SSHHostDisplay {
    /// The SSH host data.
    pub host: SSHHost,
    /// Current connection status.
    pub status: ConnectionStatus,
    /// Whether credentials are saved for this host.
    pub has_credentials: bool,
}

impl SSHHostDisplay {
    /// Creates a new display item from a host.
    #[must_use]
    pub fn new(host: SSHHost, has_credentials: bool) -> Self {
        Self {
            host,
            status: ConnectionStatus::Unknown,
            has_credentials,
        }
    }
}

/// Field being edited in scan credential entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanCredentialField {
    #[default]
    Username,
    Password,
    Subnet,
}

impl ScanCredentialField {
    /// Moves to the next field.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Username => Self::Password,
            Self::Password => Self::Subnet,
            Self::Subnet => Self::Username,
        }
    }

    /// Moves to the previous field.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Username => Self::Subnet,
            Self::Password => Self::Username,
            Self::Subnet => Self::Password,
        }
    }
}

/// Field being edited in add host mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddHostField {
    #[default]
    Hostname,
    Port,
    DisplayName,
    Username,
    Password,
}

impl AddHostField {
    /// Moves to the next field.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Hostname => Self::Port,
            Self::Port => Self::DisplayName,
            Self::DisplayName => Self::Username,
            Self::Username => Self::Password,
            Self::Password => Self::Hostname,
        }
    }

    /// Moves to the previous field.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Hostname => Self::Password,
            Self::Port => Self::Hostname,
            Self::DisplayName => Self::Port,
            Self::Username => Self::DisplayName,
            Self::Password => Self::Username,
        }
    }
}

/// SSH Manager selector state.
#[derive(Debug, Clone)]
pub struct SSHManagerSelector {
    /// List of hosts to display.
    hosts: Vec<SSHHostDisplay>,
    /// Currently selected host index.
    selected_index: usize,
    /// Current mode.
    mode: SSHManagerMode,
    /// Scan progress (scanned, total).
    scan_progress: Option<(usize, usize)>,
    /// Subnet currently being scanned.
    scanning_subnet: Option<String>,
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
    // --- Scan with credentials fields ---
    /// Current field in scan credential entry.
    scan_credential_field: ScanCredentialField,
    /// Username for authenticated scan.
    scan_username: String,
    /// Password for authenticated scan.
    scan_password: String,
    /// Subnet for authenticated scan.
    scan_subnet: String,
    /// Number of hosts authenticated during scan.
    auth_success_count: usize,
    /// Number of hosts that failed auth during scan.
    auth_fail_count: usize,
    /// Display name being edited.
    edit_name_input: String,
    /// Host ID being renamed.
    edit_name_target: Option<u32>,
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
            // Scan credential fields
            scan_credential_field: ScanCredentialField::Username,
            scan_username: String::new(),
            scan_password: String::new(),
            scan_subnet: String::new(),
            auth_success_count: 0,
            auth_fail_count: 0,
            // Edit name fields
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

        // Ensure selected index is valid
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

    // =========================================================================
    // Scan with Credentials Methods
    // =========================================================================

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

    // =========================================================================
    // Edit Name Methods
    // =========================================================================

    /// Starts editing the name of the selected host.
    pub fn start_edit_name(&mut self) {
        // Extract values first to avoid borrow conflict
        let host_info = self.selected_host().map(|h| (h.host.id, h.host.display().to_string()));

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

impl Default for SSHManagerSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// Widget for rendering the SSH Manager popup.
pub struct SSHManagerWidget<'a> {
    /// Selector state.
    selector: &'a SSHManagerSelector,
    /// Whether the widget is focused.
    focused: bool,
}

impl<'a> SSHManagerWidget<'a> {
    /// Creates a new SSH Manager widget.
    #[must_use]
    pub fn new(selector: &'a SSHManagerSelector) -> Self {
        Self {
            selector,
            focused: true,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Calculates the popup area centered in the terminal.
    fn popup_area(&self, area: Rect) -> Rect {
        let width = area.width.saturating_sub(8).min(80);
        let height = area.height.saturating_sub(4).min(20);

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }

    /// Renders the host list mode.
    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        let inner = area;

        // Header with keybindings - clearer labels
        // s=auto-scan, c=credential scan, a=add, d=delete, e=edit
        let header = Line::from(vec![
            Span::styled("[s]", Style::default().fg(Color::Yellow)),
            Span::raw("can "),
            Span::styled("[c]", Style::default().fg(Color::Magenta)),
            Span::raw("red-scan "),
            Span::styled("[a]", Style::default().fg(Color::Yellow)),
            Span::raw("dd "),
            Span::styled("[e]", Style::default().fg(Color::Cyan)),
            Span::raw("dit "),
            Span::styled("[d]", Style::default().fg(Color::Yellow)),
            Span::raw("el "),
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Connect"),
        ]);

        let header_para = Paragraph::new(header).alignment(Alignment::Center);

        // Layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Length(1), // Separator
                Constraint::Min(3),    // Host list
                Constraint::Length(1), // Footer
            ])
            .split(inner);

        header_para.render(chunks[0], buf);

        // Separator
        let sep = "─".repeat(chunks[1].width as usize);
        buf.set_string(
            chunks[1].x,
            chunks[1].y,
            &sep,
            Style::default().fg(Color::DarkGray),
        );

        // Host list
        if self.selector.is_empty() {
            let empty = Paragraph::new("No SSH hosts saved. Press [S] to scan or [A] to add.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            empty.render(chunks[2], buf);
        } else {
            self.render_host_table(chunks[2], buf);
        }

        // Footer with tips
        let footer = Line::from(vec![
            Span::styled("Tip: ", Style::default().fg(Color::DarkGray)),
            Span::raw(
                "Ctrl+1-9 quick connect | Shift+S to scan specific subnet (e.g. 10.0.0.0/24)",
            ),
        ]);
        let footer_para = Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        footer_para.render(chunks[3], buf);
    }

    /// Renders the host table.
    fn render_host_table(&self, area: Rect, buf: &mut Buffer) {
        let header = Row::new(vec!["#", "Name", "Host", "Status"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .height(1);

        let rows: Vec<Row> = self
            .selector
            .visible_hosts()
            .map(|(idx, host)| {
                let is_selected = idx == self.selector.selected_index;
                let style = if is_selected {
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let _status_style = match host.status {
                    ConnectionStatus::Unknown => Style::default().fg(Color::DarkGray),
                    ConnectionStatus::Reachable => Style::default().fg(Color::Green),
                    ConnectionStatus::Unreachable => Style::default().fg(Color::Red),
                    ConnectionStatus::Authenticated => Style::default().fg(Color::Cyan),
                };

                let creds_indicator = if host.has_credentials { "*" } else { "" };

                Row::new(vec![
                    format!("{}", idx + 1),
                    format!("{}{}", host.host.display(), creds_indicator),
                    host.host.connection_string(),
                    host.status.as_str().to_string(),
                ])
                .style(style)
                .height(1)
            })
            .collect();

        let widths = [
            Constraint::Length(3),
            Constraint::Percentage(35),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ];

        let table = Table::new(rows, widths).header(header).column_spacing(1);

        Widget::render(table, area, buf);
    }

    /// Renders the credential entry mode.
    fn render_credential_entry(&self, area: Rect, buf: &mut Buffer) {
        let target_name = self
            .selector
            .credential_target()
            .and_then(|id| {
                self.selector
                    .hosts
                    .iter()
                    .find(|h| h.host.id == id)
                    .map(|h| h.host.display().to_string())
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Target
                Constraint::Length(1), // Separator
                Constraint::Length(1), // Username label
                Constraint::Length(1), // Username input
                Constraint::Length(1), // Password label
                Constraint::Length(1), // Password input
                Constraint::Length(1), // Save checkbox
                Constraint::Min(1),    // Spacer
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Target host
        let target = Paragraph::new(format!("Connect to: {}", target_name))
            .alignment(Alignment::Center)
            .style(Style::default().add_modifier(Modifier::BOLD));
        target.render(chunks[0], buf);

        // Username
        let username_style = if self.selector.credential_field() == CredentialField::Username {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let username_label = Paragraph::new("Username:").style(username_style);
        username_label.render(chunks[2], buf);

        let username_input =
            Paragraph::new(self.selector.username()).style(Style::default().bg(Color::DarkGray));
        username_input.render(chunks[3], buf);

        // Password
        let password_style = if self.selector.credential_field() == CredentialField::Password {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let password_label = Paragraph::new("Password:").style(password_style);
        password_label.render(chunks[4], buf);

        let masked_password = "*".repeat(self.selector.password().len());
        let password_input =
            Paragraph::new(masked_password).style(Style::default().bg(Color::DarkGray));
        password_input.render(chunks[5], buf);

        // Save checkbox
        let checkbox = if self.selector.save_credentials() {
            "[x] Save credentials"
        } else {
            "[ ] Save credentials"
        };
        let save_para = Paragraph::new(checkbox).style(Style::default().fg(Color::Cyan));
        save_para.render(chunks[6], buf);

        // Footer
        let footer = Line::from(vec![
            Span::styled("[Tab]", Style::default().fg(Color::Yellow)),
            Span::raw(" Next field "),
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Connect "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]);
        let footer_para = Paragraph::new(footer).alignment(Alignment::Center);
        footer_para.render(chunks[8], buf);
    }

    /// Renders the scanning mode.
    fn render_scanning(&self, area: Rect, buf: &mut Buffer) {
        let (scanned, total) = self.selector.scan_progress().unwrap_or((0, 0));
        let percentage = if total > 0 {
            (scanned * 100) / total
        } else {
            0
        };

        let subnet_info = self
            .selector
            .scanning_subnet()
            .map(|s| format!("Subnet: {}", s))
            .unwrap_or_else(|| "Detecting network...".to_string());

        let progress_text = format!(
            "Scanning for SSH hosts... {}/{} ({}%)",
            scanned, total, percentage
        );

        // Progress bar visual
        let bar_width = area.width.saturating_sub(4) as usize;
        let filled = if total > 0 {
            (bar_width * scanned) / total
        } else {
            0
        };
        let empty = bar_width.saturating_sub(filled);
        let progress_bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Length(1), // Subnet info
                Constraint::Length(1), // Progress text
                Constraint::Length(1), // Progress bar
                Constraint::Length(1), // Cancel hint
                Constraint::Percentage(30),
            ])
            .split(area);

        // Subnet info
        let subnet_para = Paragraph::new(subnet_info)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Cyan));
        subnet_para.render(chunks[1], buf);

        // Progress text
        let progress_para = Paragraph::new(progress_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        progress_para.render(chunks[2], buf);

        // Progress bar
        let bar_para = Paragraph::new(progress_bar)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green));
        bar_para.render(chunks[3], buf);

        // Cancel hint
        let cancel_hint = Paragraph::new("Press [Esc] to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        cancel_hint.render(chunks[4], buf);
    }

    /// Renders the add host mode.
    fn render_add_host(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // 0: Title
                Constraint::Length(1), // 1: Description
                Constraint::Length(1), // 2: Spacer
                Constraint::Length(1), // 3: Hostname label
                Constraint::Length(1), // 4: Hostname input
                Constraint::Length(1), // 5: Port label
                Constraint::Length(1), // 6: Port input
                Constraint::Length(1), // 7: Display name label
                Constraint::Length(1), // 8: Display name input
                Constraint::Length(1), // 9: Username label
                Constraint::Length(1), // 10: Username input
                Constraint::Length(1), // 11: Password label
                Constraint::Length(1), // 12: Password input
                Constraint::Min(1),    // 13: Spacer
                Constraint::Length(1), // 14: Footer
            ])
            .split(area);

        let current_field = self.selector.add_host_field();

        // Title
        let title = Paragraph::new("Add New SSH Host")
            .alignment(Alignment::Center)
            .style(Style::default().add_modifier(Modifier::BOLD));
        title.render(chunks[0], buf);

        // Description
        let desc = Paragraph::new("Enter host details and credentials (Tab to switch fields)")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        desc.render(chunks[1], buf);

        // Hostname label
        let hostname_label_style = if current_field == AddHostField::Hostname {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let hostname_label = Paragraph::new("Hostname/IP:").style(hostname_label_style);
        hostname_label.render(chunks[3], buf);

        // Hostname input
        let hostname = self.selector.hostname_input();
        let hostname_text = if current_field == AddHostField::Hostname {
            format!("{}_", hostname)
        } else {
            hostname.to_string()
        };
        let hostname_style = if current_field == AddHostField::Hostname {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let hostname_input = Paragraph::new(hostname_text).style(hostname_style);
        hostname_input.render(chunks[4], buf);

        // Port label
        let port_label_style = if current_field == AddHostField::Port {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let port_label = Paragraph::new("Port:").style(port_label_style);
        port_label.render(chunks[5], buf);

        // Port input
        let port = self.selector.port_input();
        let port_text = if current_field == AddHostField::Port {
            format!("{}_", port)
        } else {
            port.to_string()
        };
        let port_style = if current_field == AddHostField::Port {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let port_input = Paragraph::new(port_text).style(port_style);
        port_input.render(chunks[6], buf);

        // Display name label
        let display_name_label_style = if current_field == AddHostField::DisplayName {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let display_name_label = Paragraph::new("Display Name (optional):").style(display_name_label_style);
        display_name_label.render(chunks[7], buf);

        // Display name input
        let display_name = self.selector.add_host_display_name();
        let display_name_text = if current_field == AddHostField::DisplayName {
            format!("{}_", display_name)
        } else {
            display_name.to_string()
        };
        let display_name_style = if current_field == AddHostField::DisplayName {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let display_name_input = Paragraph::new(display_name_text).style(display_name_style);
        display_name_input.render(chunks[8], buf);

        // Username label
        let username_label_style = if current_field == AddHostField::Username {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let username_label = Paragraph::new("Username:").style(username_label_style);
        username_label.render(chunks[9], buf);

        // Username input
        let username = self.selector.add_host_username();
        let username_text = if current_field == AddHostField::Username {
            format!("{}_", username)
        } else {
            username.to_string()
        };
        let username_style = if current_field == AddHostField::Username {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let username_input = Paragraph::new(username_text).style(username_style);
        username_input.render(chunks[10], buf);

        // Password label
        let password_label_style = if current_field == AddHostField::Password {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let password_label = Paragraph::new("Password:").style(password_label_style);
        password_label.render(chunks[11], buf);

        // Password input (masked)
        let password = self.selector.add_host_password();
        let masked_len = password.len();
        let password_text = if current_field == AddHostField::Password {
            format!("{}_", "*".repeat(masked_len))
        } else {
            "*".repeat(masked_len)
        };
        let password_style = if current_field == AddHostField::Password {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let password_input = Paragraph::new(password_text).style(password_style);
        password_input.render(chunks[12], buf);

        // Footer
        let footer = Line::from(vec![
            Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
            Span::raw(" Next Field "),
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Add Host "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]);
        let footer_para = Paragraph::new(footer).alignment(Alignment::Center);
        footer_para.render(chunks[14], buf);
    }

    /// Renders the scan credential entry mode.
    fn render_scan_credential_entry(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Length(1), // Description
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Username label
                Constraint::Length(1), // Username input
                Constraint::Length(1), // Password label
                Constraint::Length(1), // Password input
                Constraint::Length(1), // Subnet label
                Constraint::Length(1), // Subnet input
                Constraint::Min(1),    // Spacer
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Scan Network with Credentials")
            .alignment(Alignment::Center)
            .style(Style::default().add_modifier(Modifier::BOLD));
        title.render(chunks[0], buf);

        // Description
        let desc = Paragraph::new("Only hosts that accept these credentials will be added")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        desc.render(chunks[1], buf);

        // Username
        let username_style =
            if self.selector.scan_credential_field() == ScanCredentialField::Username {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
        let username_label = Paragraph::new("Username:").style(username_style);
        username_label.render(chunks[3], buf);

        let username_input =
            if self.selector.scan_credential_field() == ScanCredentialField::Username {
                format!("{}_", self.selector.scan_username())
            } else {
                self.selector.scan_username().to_string()
            };
        let username_para =
            Paragraph::new(username_input).style(Style::default().bg(Color::DarkGray));
        username_para.render(chunks[4], buf);

        // Password
        let password_style =
            if self.selector.scan_credential_field() == ScanCredentialField::Password {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
        let password_label = Paragraph::new("Password:").style(password_style);
        password_label.render(chunks[5], buf);

        let masked_len = self.selector.scan_password().len();
        let password_display =
            if self.selector.scan_credential_field() == ScanCredentialField::Password {
                format!("{}_", "*".repeat(masked_len))
            } else {
                "*".repeat(masked_len)
            };
        let password_para =
            Paragraph::new(password_display).style(Style::default().bg(Color::DarkGray));
        password_para.render(chunks[6], buf);

        // Subnet
        let subnet_style = if self.selector.scan_credential_field() == ScanCredentialField::Subnet {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let subnet_label =
            Paragraph::new("Subnet (e.g. 10.0.0.0/24, blank=auto):").style(subnet_style);
        subnet_label.render(chunks[7], buf);

        let subnet_input = if self.selector.scan_credential_field() == ScanCredentialField::Subnet {
            format!("{}_", self.selector.scan_subnet())
        } else {
            self.selector.scan_subnet().to_string()
        };
        let subnet_para = Paragraph::new(subnet_input).style(Style::default().bg(Color::DarkGray));
        subnet_para.render(chunks[8], buf);

        // Footer
        let footer = Line::from(vec![
            Span::styled("[Tab]", Style::default().fg(Color::Yellow)),
            Span::raw(" Next "),
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Start Scan "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]);
        let footer_para = Paragraph::new(footer).alignment(Alignment::Center);
        footer_para.render(chunks[10], buf);
    }

    /// Renders the authenticated scanning mode.
    fn render_authenticated_scanning(&self, area: Rect, buf: &mut Buffer) {
        let (scanned, total) = self.selector.scan_progress().unwrap_or((0, 0));
        let percentage = if total > 0 {
            (scanned * 100) / total
        } else {
            0
        };

        let subnet_info = self
            .selector
            .scanning_subnet()
            .map(|s| format!("Subnet: {}", s))
            .unwrap_or_else(|| "Detecting network...".to_string());

        let progress_text = format!(
            "Scanning and authenticating... {}/{} ({}%)",
            scanned, total, percentage
        );

        let auth_stats = format!(
            "Authenticated: {} | Failed: {}",
            self.selector.auth_success_count(),
            self.selector.auth_fail_count()
        );

        // Progress bar
        let bar_width = area.width.saturating_sub(4) as usize;
        let filled = if total > 0 {
            (bar_width * scanned) / total
        } else {
            0
        };
        let empty = bar_width.saturating_sub(filled);
        let progress_bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Length(1), // Subnet info
                Constraint::Length(1), // Progress text
                Constraint::Length(1), // Progress bar
                Constraint::Length(1), // Auth stats
                Constraint::Length(1), // Cancel hint
                Constraint::Percentage(25),
            ])
            .split(area);

        // Subnet info
        let subnet_para = Paragraph::new(subnet_info)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Cyan));
        subnet_para.render(chunks[1], buf);

        // Progress text
        let progress_para = Paragraph::new(progress_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        progress_para.render(chunks[2], buf);

        // Progress bar
        let bar_para = Paragraph::new(progress_bar)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green));
        bar_para.render(chunks[3], buf);

        // Auth stats
        let stats_para = Paragraph::new(auth_stats)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Magenta));
        stats_para.render(chunks[4], buf);

        // Cancel hint
        let cancel_hint = Paragraph::new("Press [Esc] to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        cancel_hint.render(chunks[5], buf);
    }
}

impl Widget for SSHManagerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);

        // Clear the popup area
        Clear.render(popup_area, buf);

        // Draw border
        let title = match self.selector.mode() {
            SSHManagerMode::List => "SSH Manager",
            SSHManagerMode::Scanning => "SSH Manager - Scanning",
            SSHManagerMode::CredentialEntry => "SSH Manager - Credentials",
            SSHManagerMode::Connecting => "SSH Manager - Connecting",
            SSHManagerMode::AddHost => "SSH Manager - Add Host",
            SSHManagerMode::ScanCredentialEntry => "SSH Manager - Scan with Credentials",
            SSHManagerMode::AuthenticatedScanning => "SSH Manager - Authenticated Scan",
            SSHManagerMode::EditName => "SSH Manager - Edit Name",
        };

        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Render error if present
        if let Some(error) = self.selector.error() {
            let error_area = Rect::new(inner.x, inner.y, inner.width, 1);
            let error_para = Paragraph::new(error)
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center);
            error_para.render(error_area, buf);

            let remaining = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
            self.render_mode_content(remaining, buf);
        } else {
            self.render_mode_content(inner, buf);
        }
    }
}

impl SSHManagerWidget<'_> {
    /// Renders the content based on current mode.
    fn render_mode_content(&self, area: Rect, buf: &mut Buffer) {
        match self.selector.mode() {
            SSHManagerMode::List => self.render_list(area, buf),
            SSHManagerMode::Scanning => self.render_scanning(area, buf),
            SSHManagerMode::CredentialEntry => self.render_credential_entry(area, buf),
            SSHManagerMode::Connecting => {
                let text = Paragraph::new("Connecting...")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Yellow));
                text.render(area, buf);
            }
            SSHManagerMode::AddHost => {
                self.render_add_host(area, buf);
            }
            SSHManagerMode::ScanCredentialEntry => {
                self.render_scan_credential_entry(area, buf);
            }
            SSHManagerMode::AuthenticatedScanning => {
                self.render_authenticated_scanning(area, buf);
            }
            SSHManagerMode::EditName => {
                self.render_edit_name(area, buf);
            }
        }
    }

    /// Renders the edit name mode.
    fn render_edit_name(&self, area: Rect, buf: &mut Buffer) {
        let host_info = self
            .selector
            .edit_name_target()
            .and_then(|id| {
                self.selector
                    .hosts
                    .iter()
                    .find(|h| h.host.id == id)
                    .map(|h| h.host.hostname.clone())
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Length(1), // Host info
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Name label
                Constraint::Length(1), // Name input
                Constraint::Min(1),    // Spacer
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Edit Display Name")
            .alignment(Alignment::Center)
            .style(Style::default().add_modifier(Modifier::BOLD));
        title.render(chunks[0], buf);

        // Host info
        let host_para = Paragraph::new(format!("Host: {}", host_info))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        host_para.render(chunks[1], buf);

        // Name label
        let name_label = Paragraph::new("Display Name:").style(Style::default().fg(Color::Yellow));
        name_label.render(chunks[3], buf);

        // Name input
        let name_text = format!("{}_", self.selector.edit_name_input());
        let name_input = Paragraph::new(name_text).style(Style::default().bg(Color::DarkGray));
        name_input.render(chunks[4], buf);

        // Footer
        let footer = Line::from(vec![
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" Save "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]);
        let footer_para = Paragraph::new(footer).alignment(Alignment::Center);
        footer_para.render(chunks[6], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_creation() {
        let selector = SSHManagerSelector::new();
        assert!(selector.is_empty());
        assert_eq!(selector.mode(), SSHManagerMode::List);
    }

    #[test]
    fn test_selector_navigation() {
        let mut selector = SSHManagerSelector::new();

        // Add some test hosts
        let host1 = SSHHost::new(1, "host1.com".to_string());
        let host2 = SSHHost::new(2, "host2.com".to_string());

        selector.hosts.push(SSHHostDisplay::new(host1, false));
        selector.hosts.push(SSHHostDisplay::new(host2, false));

        assert_eq!(selector.selected_index, 0);

        selector.select_next();
        assert_eq!(selector.selected_index, 1);

        selector.select_next();
        assert_eq!(selector.selected_index, 1); // Should not go past end

        selector.select_prev();
        assert_eq!(selector.selected_index, 0);

        selector.select_prev();
        assert_eq!(selector.selected_index, 0); // Should not go negative
    }

    #[test]
    fn test_credential_field_cycling() {
        let field = CredentialField::Username;
        assert_eq!(field.next(), CredentialField::Password);
        assert_eq!(field.next().next(), CredentialField::Username);
    }

    #[test]
    fn test_credential_input() {
        let mut selector = SSHManagerSelector::new();
        selector.set_mode(SSHManagerMode::CredentialEntry);

        selector.insert_char('u');
        selector.insert_char('s');
        selector.insert_char('e');
        selector.insert_char('r');
        assert_eq!(selector.username(), "user");

        selector.next_field();
        selector.insert_char('p');
        selector.insert_char('a');
        selector.insert_char('s');
        selector.insert_char('s');
        assert_eq!(selector.password(), "pass");

        selector.backspace();
        assert_eq!(selector.password(), "pas");
    }
}
