//! SSH Manager types and enums.

use crate::ssh::{ConnectionStatus, SSHHost};

/// Maximum number of hosts to display in the list.
pub const MAX_DISPLAY_HOSTS: usize = 10;

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
    /// Jump host selection (cycles through available hosts).
    JumpHost,
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
            Self::Password => Self::JumpHost,
            Self::JumpHost => Self::Hostname,
        }
    }

    /// Moves to the previous field.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Hostname => Self::JumpHost,
            Self::Port => Self::Hostname,
            Self::DisplayName => Self::Port,
            Self::Username => Self::DisplayName,
            Self::Password => Self::Username,
            Self::JumpHost => Self::Password,
        }
    }
}
