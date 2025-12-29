//! SSH host and credential data structures.
//!
//! This module defines the core types for representing SSH hosts,
//! credentials, and their connection status.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum number of saved SSH hosts.
const MAX_HOSTS: usize = 100;

/// Connection status for an SSH host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Host status is unknown (not yet checked).
    #[default]
    Unknown,
    /// Host is reachable (port 22 responds).
    Reachable,
    /// Host is not reachable.
    Unreachable,
    /// Successfully authenticated.
    Authenticated,
}

impl ConnectionStatus {
    /// Returns a display string for the status.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Reachable => "Reachable",
            Self::Unreachable => "Unreachable",
            Self::Authenticated => "Connected",
        }
    }

    /// Returns true if the host is reachable.
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable | Self::Authenticated)
    }
}

/// Represents a saved SSH host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SSHHost {
    /// Unique identifier for the host.
    pub id: u32,
    /// Hostname or IP address.
    pub hostname: String,
    /// SSH port (default: 22).
    pub port: u16,
    /// User-friendly display name.
    pub display_name: Option<String>,
    /// Last successful connection timestamp (ISO 8601).
    pub last_connected: Option<String>,
    /// Number of successful connections.
    pub connection_count: u32,
}

impl SSHHost {
    /// Creates a new SSH host with the given hostname.
    ///
    /// # Arguments
    /// * `id` - Unique identifier
    /// * `hostname` - Hostname or IP address
    ///
    /// # Panics
    /// Does not panic.
    #[must_use]
    pub fn new(id: u32, hostname: String) -> Self {
        assert!(!hostname.is_empty(), "hostname must not be empty");

        Self {
            id,
            hostname,
            port: 22,
            display_name: None,
            last_connected: None,
            connection_count: 0,
        }
    }

    /// Creates a new SSH host with all parameters.
    #[must_use]
    pub fn with_details(
        id: u32,
        hostname: String,
        port: u16,
        display_name: Option<String>,
    ) -> Self {
        assert!(!hostname.is_empty(), "hostname must not be empty");
        assert!(port > 0, "port must be greater than 0");

        Self {
            id,
            hostname,
            port,
            display_name,
            last_connected: None,
            connection_count: 0,
        }
    }

    /// Returns the display name or hostname if no display name is set.
    #[must_use]
    pub fn display(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.hostname)
    }

    /// Returns the connection string (user@host:port format without user).
    #[must_use]
    pub fn connection_string(&self) -> String {
        if self.port == 22 {
            self.hostname.clone()
        } else {
            format!("{}:{}", self.hostname, self.port)
        }
    }

    /// Updates the last connected timestamp to now.
    pub fn mark_connected(&mut self) {
        use std::time::SystemTime;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Simple ISO 8601 format
        self.last_connected = Some(format!("{}", now));
        self.connection_count = self.connection_count.saturating_add(1);
    }
}

impl Default for SSHHost {
    fn default() -> Self {
        Self {
            id: 0,
            hostname: String::new(),
            port: 22,
            display_name: None,
            last_connected: None,
            connection_count: 0,
        }
    }
}

/// SSH credentials for a host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSHCredentials {
    /// Username for authentication.
    pub username: String,
    /// Password (may be encrypted based on storage mode).
    pub password: Option<String>,
    /// Path to SSH private key file.
    pub key_path: Option<String>,
    /// Whether to save these credentials.
    pub save: bool,
}

impl SSHCredentials {
    /// Creates new credentials with username and password.
    #[must_use]
    pub fn new(username: String, password: Option<String>) -> Self {
        assert!(!username.is_empty(), "username must not be empty");

        Self {
            username,
            password,
            key_path: None,
            save: true,
        }
    }

    /// Creates credentials with an SSH key.
    #[must_use]
    pub fn with_key(username: String, key_path: String) -> Self {
        assert!(!username.is_empty(), "username must not be empty");
        assert!(!key_path.is_empty(), "key_path must not be empty");

        Self {
            username,
            password: None,
            key_path: Some(key_path),
            save: true,
        }
    }

    /// Returns true if this uses key-based authentication.
    #[must_use]
    pub fn uses_key(&self) -> bool {
        self.key_path.is_some()
    }

    /// Returns the SSH command arguments for this credential.
    #[must_use]
    pub fn ssh_args(&self, host: &SSHHost) -> Vec<String> {
        let mut args = Vec::with_capacity(8);

        // Add port if non-standard
        if host.port != 22 {
            args.push("-p".to_string());
            args.push(host.port.to_string());
        }

        // Add key if specified
        if let Some(ref key) = self.key_path {
            args.push("-i".to_string());
            args.push(key.clone());
        }

        // Add user@host
        args.push(format!("{}@{}", self.username, host.hostname));

        args
    }
}

impl Default for SSHCredentials {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: None,
            key_path: None,
            save: true,
        }
    }
}

/// Collection of saved SSH hosts with their credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SSHHostList {
    /// List of saved hosts.
    hosts: Vec<SSHHost>,
    /// Credentials mapped by host ID (as string for TOML compatibility).
    credentials: HashMap<String, SSHCredentials>,
    /// Next available host ID.
    next_id: u32,
}

impl SSHHostList {
    /// Creates a new empty host list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hosts: Vec::new(),
            credentials: HashMap::new(),
            next_id: 1,
        }
    }

    /// Converts a host ID to a string key for the credentials map.
    fn id_to_key(id: u32) -> String {
        id.to_string()
    }

    /// Returns the number of saved hosts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    /// Returns true if there are no saved hosts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    /// Returns an iterator over all hosts.
    pub fn hosts(&self) -> impl Iterator<Item = &SSHHost> {
        self.hosts.iter()
    }

    /// Returns a host by index (for quick connect).
    #[must_use]
    pub fn get_by_index(&self, index: usize) -> Option<&SSHHost> {
        self.hosts.get(index)
    }

    /// Returns a host by ID.
    #[must_use]
    pub fn get_by_id(&self, id: u32) -> Option<&SSHHost> {
        self.hosts.iter().find(|h| h.id == id)
    }

    /// Returns credentials for a host ID.
    #[must_use]
    pub fn get_credentials(&self, host_id: u32) -> Option<&SSHCredentials> {
        self.credentials.get(&Self::id_to_key(host_id))
    }

    /// Returns mutable credentials for a host ID.
    pub fn get_credentials_mut(&mut self, host_id: u32) -> Option<&mut SSHCredentials> {
        self.credentials.get_mut(&Self::id_to_key(host_id))
    }

    /// Adds a new host to the list.
    ///
    /// Returns the new host ID, or None if the list is full.
    pub fn add_host(&mut self, hostname: String, port: u16) -> Option<u32> {
        if self.hosts.len() >= MAX_HOSTS {
            return None;
        }

        assert!(!hostname.is_empty(), "hostname must not be empty");

        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        let host = SSHHost::with_details(id, hostname, port, None);
        self.hosts.push(host);

        Some(id)
    }

    /// Adds a host with display name.
    pub fn add_host_with_name(
        &mut self,
        hostname: String,
        port: u16,
        display_name: String,
    ) -> Option<u32> {
        if self.hosts.len() >= MAX_HOSTS {
            return None;
        }

        assert!(!hostname.is_empty(), "hostname must not be empty");

        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        let host = SSHHost::with_details(id, hostname, port, Some(display_name));
        self.hosts.push(host);

        Some(id)
    }

    /// Sets credentials for a host.
    /// Returns true if the host exists and credentials were set, false otherwise.
    pub fn set_credentials(&mut self, host_id: u32, credentials: SSHCredentials) -> bool {
        // Check if host exists first - don't panic, just return false
        if !self.hosts.iter().any(|h| h.id == host_id) {
            return false;
        }

        if credentials.save {
            self.credentials
                .insert(Self::id_to_key(host_id), credentials);
        }
        true
    }

    /// Removes a host and its credentials.
    pub fn remove_host(&mut self, host_id: u32) -> bool {
        let initial_len = self.hosts.len();
        self.hosts.retain(|h| h.id != host_id);
        self.credentials.remove(&Self::id_to_key(host_id));
        self.hosts.len() < initial_len
    }

    /// Marks a host as connected (updates timestamp and count).
    pub fn mark_connected(&mut self, host_id: u32) {
        if let Some(host) = self.hosts.iter_mut().find(|h| h.id == host_id) {
            host.mark_connected();
        }
    }

    /// Updates the display name for a host.
    pub fn set_display_name(&mut self, host_id: u32, name: String) {
        if let Some(host) = self.hosts.iter_mut().find(|h| h.id == host_id) {
            host.display_name = if name.is_empty() { None } else { Some(name) };
        }
    }

    /// Returns hosts sorted by last connected (most recent first).
    #[must_use]
    pub fn sorted_by_recent(&self) -> Vec<&SSHHost> {
        let mut sorted: Vec<&SSHHost> = self.hosts.iter().collect();
        sorted.sort_by(|a, b| match (&b.last_connected, &a.last_connected) {
            (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.id.cmp(&b.id),
        });
        sorted
    }

    /// Checks if a hostname already exists in the list.
    #[must_use]
    pub fn contains_hostname(&self, hostname: &str) -> bool {
        self.hosts.iter().any(|h| h.hostname == hostname)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_host_creation() {
        let host = SSHHost::new(1, "192.168.1.100".to_string());

        assert_eq!(host.id, 1);
        assert_eq!(host.hostname, "192.168.1.100");
        assert_eq!(host.port, 22);
        assert!(host.display_name.is_none());
    }

    #[test]
    fn test_ssh_host_with_details() {
        let host = SSHHost::with_details(
            2,
            "server.example.com".to_string(),
            2222,
            Some("My Server".to_string()),
        );

        assert_eq!(host.id, 2);
        assert_eq!(host.port, 2222);
        assert_eq!(host.display(), "My Server");
    }

    #[test]
    fn test_connection_string() {
        let host1 = SSHHost::new(1, "example.com".to_string());
        assert_eq!(host1.connection_string(), "example.com");

        let mut host2 = SSHHost::new(2, "example.com".to_string());
        host2.port = 2222;
        assert_eq!(host2.connection_string(), "example.com:2222");
    }

    #[test]
    fn test_credentials_ssh_args() {
        let host = SSHHost::with_details(1, "server.com".to_string(), 2222, None);
        let creds = SSHCredentials::new("admin".to_string(), Some("secret".to_string()));

        let args = creds.ssh_args(&host);
        assert_eq!(args, vec!["-p", "2222", "admin@server.com"]);
    }

    #[test]
    fn test_host_list_operations() {
        let mut list = SSHHostList::new();

        assert!(list.is_empty());

        let id1 = list.add_host("host1.com".to_string(), 22).unwrap();
        let id2 = list.add_host("host2.com".to_string(), 22).unwrap();

        assert_eq!(list.len(), 2);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        let creds = SSHCredentials::new("user".to_string(), None);
        list.set_credentials(id1, creds);

        assert!(list.get_credentials(id1).is_some());
        assert!(list.get_credentials(id2).is_none());

        assert!(list.remove_host(id1));
        assert_eq!(list.len(), 1);
        assert!(list.get_credentials(id1).is_none());
    }

    #[test]
    fn test_contains_hostname() {
        let mut list = SSHHostList::new();
        list.add_host("existing.com".to_string(), 22);

        assert!(list.contains_hostname("existing.com"));
        assert!(!list.contains_hostname("new.com"));
    }
}
