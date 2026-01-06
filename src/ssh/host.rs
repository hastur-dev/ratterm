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
    /// Optional jump host ID for SSH hopping (ProxyJump).
    /// When set, connections will first hop through the referenced host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_host_id: Option<u32>,
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
            jump_host_id: None,
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
            jump_host_id: None,
        }
    }

    /// Sets the jump host for SSH hopping.
    #[must_use]
    pub fn with_jump_host(mut self, jump_host_id: u32) -> Self {
        self.jump_host_id = Some(jump_host_id);
        self
    }

    /// Returns true if this host uses a jump host for connection.
    #[must_use]
    pub fn has_jump_host(&self) -> bool {
        self.jump_host_id.is_some()
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
            jump_host_id: None,
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

/// Information about a jump host for SSH ProxyJump.
///
/// This struct contains all the information needed to construct
/// the `-J` argument for SSH connections.
#[derive(Debug, Clone)]
pub struct JumpHostInfo {
    /// Username for the jump host.
    pub username: String,
    /// Hostname or IP of the jump host.
    pub hostname: String,
    /// SSH port of the jump host.
    pub port: u16,
    /// Optional password for the jump host (for auto-login).
    pub password: Option<String>,
    /// Optional key path for the jump host.
    pub key_path: Option<String>,
    /// Nested jump host (for multi-hop chains).
    pub next_jump: Option<Box<JumpHostInfo>>,
}

impl JumpHostInfo {
    /// Creates a new jump host info.
    #[must_use]
    pub fn new(username: String, hostname: String, port: u16) -> Self {
        assert!(!username.is_empty(), "username must not be empty");
        assert!(!hostname.is_empty(), "hostname must not be empty");
        assert!(port > 0, "port must be positive");

        Self {
            username,
            hostname,
            port,
            password: None,
            key_path: None,
            next_jump: None,
        }
    }

    /// Sets the password for this jump host.
    #[must_use]
    pub fn with_password(mut self, password: String) -> Self {
        self.password = Some(password);
        self
    }

    /// Sets the key path for this jump host.
    #[must_use]
    pub fn with_key(mut self, key_path: String) -> Self {
        self.key_path = Some(key_path);
        self
    }

    /// Chains another jump host (for multi-hop).
    #[must_use]
    pub fn with_next_jump(mut self, next: JumpHostInfo) -> Self {
        self.next_jump = Some(Box::new(next));
        self
    }

    /// Returns the ProxyJump string for this hop chain.
    ///
    /// Format: `user@host:port` or `user@host` if port is 22.
    /// For multi-hop: `user1@host1:port1,user2@host2:port2`
    #[must_use]
    pub fn proxy_jump_string(&self) -> String {
        let mut result = if self.port == 22 {
            format!("{}@{}", self.username, self.hostname)
        } else {
            format!("{}@{}:{}", self.username, self.hostname, self.port)
        };

        // Append nested jumps (recursive chain)
        if let Some(ref next) = self.next_jump {
            result.push(',');
            result.push_str(&next.proxy_jump_string());
        }

        result
    }

    /// Returns the depth of this hop chain (1 for single hop).
    #[must_use]
    pub fn chain_depth(&self) -> usize {
        match &self.next_jump {
            Some(next) => 1 + next.chain_depth(),
            None => 1,
        }
    }

    /// Collects all passwords in the chain in order (for multi-hop auto-login).
    /// Returns passwords from outermost jump host to innermost.
    /// Passwords that are None are skipped.
    #[must_use]
    pub fn collect_passwords(&self) -> Vec<String> {
        let mut passwords = Vec::new();

        // First, collect passwords from nested jumps (they get prompted first)
        if let Some(ref next) = self.next_jump {
            passwords.extend(next.collect_passwords());
        }

        // Then add this jump's password
        if let Some(ref pwd) = self.password {
            passwords.push(pwd.clone());
        }

        passwords
    }
}

/// Maximum depth for jump host chains to prevent infinite loops.
const MAX_JUMP_CHAIN_DEPTH: usize = 10;

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

    /// Sets the jump host for a target host.
    ///
    /// Returns true if successful, false if target or jump host doesn't exist,
    /// or if setting would create a circular reference.
    pub fn set_jump_host(&mut self, target_id: u32, jump_host_id: Option<u32>) -> bool {
        // Check target exists
        let target_exists = self.hosts.iter().any(|h| h.id == target_id);
        if !target_exists {
            return false;
        }

        // If setting a jump host, validate it
        if let Some(jump_id) = jump_host_id {
            // Check jump host exists
            if !self.hosts.iter().any(|h| h.id == jump_id) {
                return false;
            }

            // Prevent self-reference
            if target_id == jump_id {
                return false;
            }

            // Prevent circular references
            if self.would_create_cycle(target_id, jump_id) {
                return false;
            }
        }

        // Set the jump host
        if let Some(host) = self.hosts.iter_mut().find(|h| h.id == target_id) {
            host.jump_host_id = jump_host_id;
        }

        true
    }

    /// Checks if setting a jump host would create a circular reference.
    fn would_create_cycle(&self, target_id: u32, proposed_jump_id: u32) -> bool {
        let mut current_id = proposed_jump_id;
        let mut visited = std::collections::HashSet::new();
        visited.insert(target_id);

        // Walk the chain from proposed jump host
        for _ in 0..MAX_JUMP_CHAIN_DEPTH {
            if visited.contains(&current_id) {
                return true; // Cycle detected
            }
            visited.insert(current_id);

            // Get the next hop
            let next_hop = self
                .hosts
                .iter()
                .find(|h| h.id == current_id)
                .and_then(|h| h.jump_host_id);

            match next_hop {
                Some(next_id) => current_id = next_id,
                None => return false, // End of chain, no cycle
            }
        }

        // Chain too deep, treat as potential cycle
        true
    }

    /// Builds the complete jump host chain for a host.
    ///
    /// Returns None if the host has no jump host configured.
    /// Returns an error message if the chain is invalid (missing hosts, cycles, etc.).
    pub fn build_jump_chain(&self, host_id: u32) -> Result<Option<JumpHostInfo>, String> {
        let host = self
            .hosts
            .iter()
            .find(|h| h.id == host_id)
            .ok_or_else(|| "Host not found".to_string())?;

        let jump_id = match host.jump_host_id {
            Some(id) => id,
            None => return Ok(None),
        };

        self.build_jump_chain_recursive(jump_id, 0)
            .map(Some)
    }

    /// Recursively builds the jump chain.
    fn build_jump_chain_recursive(
        &self,
        host_id: u32,
        depth: usize,
    ) -> Result<JumpHostInfo, String> {
        if depth >= MAX_JUMP_CHAIN_DEPTH {
            return Err("Jump chain too deep (max 10 hops)".to_string());
        }

        let host = self
            .hosts
            .iter()
            .find(|h| h.id == host_id)
            .ok_or_else(|| format!("Jump host {} not found", host_id))?;

        let creds = self.get_credentials(host_id);

        let username = creds
            .map(|c| c.username.clone())
            .unwrap_or_else(|| "root".to_string());

        let mut info = JumpHostInfo::new(username, host.hostname.clone(), host.port);

        // Add credentials if available
        if let Some(c) = creds {
            if let Some(ref pwd) = c.password {
                info = info.with_password(pwd.clone());
            }
            if let Some(ref key) = c.key_path {
                info = info.with_key(key.clone());
            }
        }

        // Recursively build nested jumps
        if let Some(next_jump_id) = host.jump_host_id {
            let next_info = self.build_jump_chain_recursive(next_jump_id, depth + 1)?;
            info = info.with_next_jump(next_info);
        }

        Ok(info)
    }

    /// Returns all hosts that can be used as jump hosts for the given target.
    ///
    /// Excludes the target itself and any hosts that would create a cycle.
    #[must_use]
    pub fn available_jump_hosts(&self, target_id: u32) -> Vec<&SSHHost> {
        self.hosts
            .iter()
            .filter(|h| {
                h.id != target_id && !self.would_create_cycle(target_id, h.id)
            })
            .collect()
    }

    /// Returns a mutable reference to a host by ID.
    pub fn get_by_id_mut(&mut self, id: u32) -> Option<&mut SSHHost> {
        self.hosts.iter_mut().find(|h| h.id == id)
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

    // ==================== Jump Host Tests ====================

    #[test]
    fn test_ssh_host_with_jump_host() {
        let host = SSHHost::new(1, "internal.server.com".to_string()).with_jump_host(2);

        assert!(host.has_jump_host());
        assert_eq!(host.jump_host_id, Some(2));
    }

    #[test]
    fn test_ssh_host_default_no_jump() {
        let host = SSHHost::new(1, "server.com".to_string());

        assert!(!host.has_jump_host());
        assert!(host.jump_host_id.is_none());
    }

    #[test]
    fn test_jump_host_info_creation() {
        let info = JumpHostInfo::new("admin".to_string(), "bastion.example.com".to_string(), 22);

        assert_eq!(info.username, "admin");
        assert_eq!(info.hostname, "bastion.example.com");
        assert_eq!(info.port, 22);
        assert!(info.password.is_none());
        assert!(info.key_path.is_none());
        assert!(info.next_jump.is_none());
    }

    #[test]
    fn test_jump_host_info_with_password() {
        let info = JumpHostInfo::new("admin".to_string(), "bastion.com".to_string(), 22)
            .with_password("secret123".to_string());

        assert_eq!(info.password, Some("secret123".to_string()));
    }

    #[test]
    fn test_jump_host_info_with_key() {
        let info = JumpHostInfo::new("admin".to_string(), "bastion.com".to_string(), 22)
            .with_key("/home/user/.ssh/id_rsa".to_string());

        assert_eq!(info.key_path, Some("/home/user/.ssh/id_rsa".to_string()));
    }

    #[test]
    fn test_proxy_jump_string_standard_port() {
        let info = JumpHostInfo::new("admin".to_string(), "bastion.example.com".to_string(), 22);

        assert_eq!(info.proxy_jump_string(), "admin@bastion.example.com");
    }

    #[test]
    fn test_proxy_jump_string_non_standard_port() {
        let info = JumpHostInfo::new("admin".to_string(), "bastion.example.com".to_string(), 2222);

        assert_eq!(
            info.proxy_jump_string(),
            "admin@bastion.example.com:2222"
        );
    }

    #[test]
    fn test_proxy_jump_string_multi_hop() {
        let jump2 = JumpHostInfo::new("user2".to_string(), "hop2.com".to_string(), 22);
        let jump1 = JumpHostInfo::new("user1".to_string(), "hop1.com".to_string(), 2222)
            .with_next_jump(jump2);

        assert_eq!(
            jump1.proxy_jump_string(),
            "user1@hop1.com:2222,user2@hop2.com"
        );
    }

    #[test]
    fn test_chain_depth_single() {
        let info = JumpHostInfo::new("admin".to_string(), "bastion.com".to_string(), 22);

        assert_eq!(info.chain_depth(), 1);
    }

    #[test]
    fn test_chain_depth_multi_hop() {
        let jump3 = JumpHostInfo::new("user3".to_string(), "hop3.com".to_string(), 22);
        let jump2 =
            JumpHostInfo::new("user2".to_string(), "hop2.com".to_string(), 22).with_next_jump(jump3);
        let jump1 =
            JumpHostInfo::new("user1".to_string(), "hop1.com".to_string(), 22).with_next_jump(jump2);

        assert_eq!(jump1.chain_depth(), 3);
    }

    #[test]
    fn test_set_jump_host_success() {
        let mut list = SSHHostList::new();
        let bastion_id = list.add_host("bastion.com".to_string(), 22).unwrap();
        let internal_id = list.add_host("internal.server.com".to_string(), 22).unwrap();

        // Set bastion as jump host for internal
        assert!(list.set_jump_host(internal_id, Some(bastion_id)));

        // Verify it was set
        let host = list.get_by_id(internal_id).unwrap();
        assert_eq!(host.jump_host_id, Some(bastion_id));
    }

    #[test]
    fn test_set_jump_host_clear() {
        let mut list = SSHHostList::new();
        let bastion_id = list.add_host("bastion.com".to_string(), 22).unwrap();
        let internal_id = list.add_host("internal.com".to_string(), 22).unwrap();

        // Set then clear
        list.set_jump_host(internal_id, Some(bastion_id));
        assert!(list.set_jump_host(internal_id, None));

        let host = list.get_by_id(internal_id).unwrap();
        assert!(host.jump_host_id.is_none());
    }

    #[test]
    fn test_set_jump_host_self_reference_fails() {
        let mut list = SSHHostList::new();
        let host_id = list.add_host("server.com".to_string(), 22).unwrap();

        // Trying to set a host as its own jump host should fail
        assert!(!list.set_jump_host(host_id, Some(host_id)));
    }

    #[test]
    fn test_set_jump_host_circular_fails() {
        let mut list = SSHHostList::new();
        let host_a = list.add_host("host-a.com".to_string(), 22).unwrap();
        let host_b = list.add_host("host-b.com".to_string(), 22).unwrap();
        let host_c = list.add_host("host-c.com".to_string(), 22).unwrap();

        // A -> B (ok)
        assert!(list.set_jump_host(host_a, Some(host_b)));
        // B -> C (ok)
        assert!(list.set_jump_host(host_b, Some(host_c)));
        // C -> A would create cycle (fail)
        assert!(!list.set_jump_host(host_c, Some(host_a)));
    }

    #[test]
    fn test_set_jump_host_nonexistent_target_fails() {
        let mut list = SSHHostList::new();
        let bastion_id = list.add_host("bastion.com".to_string(), 22).unwrap();

        // Target doesn't exist
        assert!(!list.set_jump_host(999, Some(bastion_id)));
    }

    #[test]
    fn test_set_jump_host_nonexistent_jump_fails() {
        let mut list = SSHHostList::new();
        let internal_id = list.add_host("internal.com".to_string(), 22).unwrap();

        // Jump host doesn't exist
        assert!(!list.set_jump_host(internal_id, Some(999)));
    }

    #[test]
    fn test_build_jump_chain_no_jump() {
        let mut list = SSHHostList::new();
        let host_id = list.add_host("server.com".to_string(), 22).unwrap();

        let chain = list.build_jump_chain(host_id).unwrap();
        assert!(chain.is_none());
    }

    #[test]
    fn test_build_jump_chain_single_hop() {
        let mut list = SSHHostList::new();
        let bastion_id = list.add_host("bastion.example.com".to_string(), 22).unwrap();
        let internal_id = list.add_host("internal.server.com".to_string(), 22).unwrap();

        // Set credentials for bastion
        list.set_credentials(
            bastion_id,
            SSHCredentials::new("bastionuser".to_string(), None),
        );

        // Set bastion as jump for internal
        list.set_jump_host(internal_id, Some(bastion_id));

        let chain = list.build_jump_chain(internal_id).unwrap().unwrap();
        assert_eq!(chain.username, "bastionuser");
        assert_eq!(chain.hostname, "bastion.example.com");
        assert_eq!(chain.port, 22);
        assert!(chain.next_jump.is_none());
        assert_eq!(chain.proxy_jump_string(), "bastionuser@bastion.example.com");
    }

    #[test]
    fn test_build_jump_chain_multi_hop() {
        let mut list = SSHHostList::new();
        let hop1_id = list.add_host("hop1.example.com".to_string(), 22).unwrap();
        let hop2_id = list.add_host("hop2.example.com".to_string(), 2222).unwrap();
        let internal_id = list.add_host("internal.server.com".to_string(), 22).unwrap();

        // Set credentials
        list.set_credentials(hop1_id, SSHCredentials::new("user1".to_string(), None));
        list.set_credentials(hop2_id, SSHCredentials::new("user2".to_string(), None));

        // Create chain: internal -> hop2 -> hop1
        list.set_jump_host(hop2_id, Some(hop1_id));
        list.set_jump_host(internal_id, Some(hop2_id));

        let chain = list.build_jump_chain(internal_id).unwrap().unwrap();
        assert_eq!(chain.chain_depth(), 2);
        assert_eq!(
            chain.proxy_jump_string(),
            "user2@hop2.example.com:2222,user1@hop1.example.com"
        );
    }

    #[test]
    fn test_build_jump_chain_missing_jump_host_fails() {
        let mut list = SSHHostList::new();
        let internal_id = list.add_host("internal.com".to_string(), 22).unwrap();

        // Manually set an invalid jump host ID
        if let Some(host) = list.get_by_id_mut(internal_id) {
            host.jump_host_id = Some(999);
        }

        let result = list.build_jump_chain(internal_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_available_jump_hosts() {
        let mut list = SSHHostList::new();
        let bastion_id = list.add_host("bastion.com".to_string(), 22).unwrap();
        let internal_id = list.add_host("internal.com".to_string(), 22).unwrap();
        let other_id = list.add_host("other.com".to_string(), 22).unwrap();

        let available = list.available_jump_hosts(internal_id);
        assert_eq!(available.len(), 2);

        // Should not include itself
        assert!(!available.iter().any(|h| h.id == internal_id));

        // Should include bastion and other
        assert!(available.iter().any(|h| h.id == bastion_id));
        assert!(available.iter().any(|h| h.id == other_id));
    }

    #[test]
    fn test_available_jump_hosts_excludes_cycles() {
        let mut list = SSHHostList::new();
        let host_a = list.add_host("a.com".to_string(), 22).unwrap();
        let host_b = list.add_host("b.com".to_string(), 22).unwrap();
        let host_c = list.add_host("c.com".to_string(), 22).unwrap();

        // A -> B
        list.set_jump_host(host_a, Some(host_b));

        // Available for B should not include A (would create cycle)
        let available = list.available_jump_hosts(host_b);
        assert!(!available.iter().any(|h| h.id == host_a));
        assert!(available.iter().any(|h| h.id == host_c));
    }

    // ==================== Password Collection Tests ====================

    #[test]
    fn test_collect_passwords_single_hop() {
        let info = JumpHostInfo::new("user".to_string(), "bastion.com".to_string(), 22)
            .with_password("secret123".to_string());

        let passwords = info.collect_passwords();
        assert_eq!(passwords.len(), 1);
        assert_eq!(passwords[0], "secret123");
    }

    #[test]
    fn test_collect_passwords_no_password() {
        let info = JumpHostInfo::new("user".to_string(), "bastion.com".to_string(), 22);

        let passwords = info.collect_passwords();
        assert!(passwords.is_empty());
    }

    #[test]
    fn test_collect_passwords_multi_hop() {
        // Chain: hop1 (password1) -> hop2 (password2)
        let hop2 = JumpHostInfo::new("user2".to_string(), "hop2.com".to_string(), 22)
            .with_password("password2".to_string());
        let hop1 = JumpHostInfo::new("user1".to_string(), "hop1.com".to_string(), 22)
            .with_password("password1".to_string())
            .with_next_jump(hop2);

        let passwords = hop1.collect_passwords();
        // Should be in order: hop2 password first (outermost), then hop1
        assert_eq!(passwords.len(), 2);
        assert_eq!(passwords[0], "password2");
        assert_eq!(passwords[1], "password1");
    }

    #[test]
    fn test_collect_passwords_partial_chain() {
        // Chain: hop1 (password) -> hop2 (no password)
        let hop2 = JumpHostInfo::new("user2".to_string(), "hop2.com".to_string(), 22);
        let hop1 = JumpHostInfo::new("user1".to_string(), "hop1.com".to_string(), 22)
            .with_password("password1".to_string())
            .with_next_jump(hop2);

        let passwords = hop1.collect_passwords();
        // Only hop1 has a password
        assert_eq!(passwords.len(), 1);
        assert_eq!(passwords[0], "password1");
    }
}
