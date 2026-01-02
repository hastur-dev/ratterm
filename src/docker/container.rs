//! Docker container and image data structures.
//!
//! This module defines the core types for representing Docker containers,
//! images, run options, and quick-connect assignments.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum number of Docker items to track (for bounded iteration).
#[allow(dead_code)]
const MAX_DOCKER_ITEMS: usize = 100;

/// Maximum quick-connect slots (Ctrl+Alt+1-9).
pub const MAX_QUICK_CONNECT: usize = 9;

/// Represents where Docker commands should be executed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DockerHost {
    /// Local Docker daemon on this machine.
    #[default]
    Local,
    /// Remote Docker daemon accessible via SSH.
    Remote {
        /// SSH host ID from the SSH manager.
        host_id: u32,
        /// Hostname or IP address.
        hostname: String,
        /// SSH port.
        port: u16,
        /// Username for SSH connection.
        username: String,
        /// Optional display name.
        display_name: Option<String>,
        /// Optional password for SSH (not serialized for security).
        #[serde(skip)]
        password: Option<String>,
    },
}

impl DockerHost {
    /// Creates a new remote Docker host without password.
    #[must_use]
    pub fn remote(
        host_id: u32,
        hostname: String,
        port: u16,
        username: String,
        display_name: Option<String>,
    ) -> Self {
        Self::remote_with_password(host_id, hostname, port, username, display_name, None)
    }

    /// Creates a new remote Docker host with optional password.
    #[must_use]
    pub fn remote_with_password(
        host_id: u32,
        hostname: String,
        port: u16,
        username: String,
        display_name: Option<String>,
        password: Option<String>,
    ) -> Self {
        assert!(!hostname.is_empty(), "hostname must not be empty");
        assert!(!username.is_empty(), "username must not be empty");
        assert!(port > 0, "port must be greater than 0");

        Self::Remote {
            host_id,
            hostname,
            port,
            username,
            display_name,
            password,
        }
    }

    /// Returns the password for remote hosts, None for local.
    #[must_use]
    pub fn password(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Remote { password, .. } => password.as_deref(),
        }
    }

    /// Returns true if this is the local host.
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    /// Returns true if this is a remote host.
    #[must_use]
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }

    /// Returns the host ID for remote hosts, None for local.
    #[must_use]
    pub fn host_id(&self) -> Option<u32> {
        match self {
            Self::Local => None,
            Self::Remote { host_id, .. } => Some(*host_id),
        }
    }

    /// Returns a display name for the host.
    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Local => "Local".to_string(),
            Self::Remote {
                display_name: Some(name),
                ..
            } => name.clone(),
            Self::Remote { hostname, .. } => hostname.clone(),
        }
    }

    /// Returns the storage key for per-host quick-connect.
    #[must_use]
    pub fn storage_key(&self) -> String {
        match self {
            Self::Local => "local".to_string(),
            Self::Remote { host_id, .. } => format!("remote:{}", host_id),
        }
    }

    /// Builds SSH command arguments for remote execution.
    /// Returns None for local host.
    #[must_use]
    pub fn ssh_args(&self) -> Option<Vec<String>> {
        match self {
            Self::Local => None,
            Self::Remote {
                hostname,
                port,
                username,
                ..
            } => {
                let mut args = Vec::with_capacity(4);
                if *port != 22 {
                    args.push("-p".to_string());
                    args.push(port.to_string());
                }
                args.push(format!("{}@{}", username, hostname));
                Some(args)
            }
        }
    }
}

/// Docker item type for quick-connect assignments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerItemType {
    /// Running container.
    RunningContainer,
    /// Stopped container.
    StoppedContainer,
    /// Docker image (not running as container).
    Image,
}

impl DockerItemType {
    /// Returns a display string for the item type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RunningContainer => "Running",
            Self::StoppedContainer => "Stopped",
            Self::Image => "Image",
        }
    }

    /// Returns a short label for UI display.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::RunningContainer => "[R]",
            Self::StoppedContainer => "[S]",
            Self::Image => "[I]",
        }
    }
}

/// Container status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockerStatus {
    /// Status unknown.
    #[default]
    Unknown,
    /// Container is running.
    Running,
    /// Container has exited.
    Exited,
    /// Container is paused.
    Paused,
    /// Container is restarting.
    Restarting,
    /// Container was stopped gracefully.
    Stopped,
    /// Container is being created.
    Created,
    /// Container is dead (abnormal state).
    Dead,
}

impl DockerStatus {
    /// Parses status from Docker CLI output.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.starts_with("up") || lower.contains("running") {
            Self::Running
        } else if lower.starts_with("exited") {
            Self::Exited
        } else if lower.contains("paused") {
            Self::Paused
        } else if lower.contains("restarting") {
            Self::Restarting
        } else if lower.contains("created") {
            Self::Created
        } else if lower.contains("dead") {
            Self::Dead
        } else {
            Self::Unknown
        }
    }

    /// Returns display string for status.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Running => "Running",
            Self::Exited => "Exited",
            Self::Paused => "Paused",
            Self::Restarting => "Restarting",
            Self::Stopped => "Stopped",
            Self::Created => "Created",
            Self::Dead => "Dead",
        }
    }

    /// Returns true if the container is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns true if the container is in a stopped state.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Exited | Self::Stopped | Self::Dead)
    }
}

/// Represents a Docker container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerContainer {
    /// Container ID (short form, 12 chars).
    pub id: String,
    /// Container name (without leading slash).
    pub name: String,
    /// Image name used to create the container.
    pub image: String,
    /// Current status.
    pub status: DockerStatus,
    /// Full status text from `docker ps`.
    pub status_text: String,
    /// Port mappings (e.g., "8080->80/tcp").
    pub ports: Vec<String>,
    /// Creation time.
    pub created: String,
    /// User-friendly display name (optional override).
    pub display_name: Option<String>,
}

impl DockerContainer {
    /// Creates a new container from parsed Docker CLI output.
    ///
    /// # Arguments
    /// * `id` - Container ID
    /// * `name` - Container name
    /// * `image` - Image name
    /// * `status_text` - Raw status string from Docker
    ///
    /// # Panics
    /// Panics if `id` is empty.
    #[must_use]
    pub fn new(id: String, name: String, image: String, status_text: String) -> Self {
        assert!(!id.is_empty(), "container id must not be empty");

        let status = DockerStatus::parse(&status_text);

        Self {
            id,
            name,
            image,
            status,
            status_text,
            ports: Vec::new(),
            created: String::new(),
            display_name: None,
        }
    }

    /// Returns the display name or container name.
    #[must_use]
    pub fn display(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }

    /// Returns a short summary for list display.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.ports.is_empty() {
            format!("{} ({})", self.display(), self.image)
        } else {
            format!(
                "{} ({}) [{}]",
                self.display(),
                self.image,
                self.ports.join(", ")
            )
        }
    }

    /// Returns true if container is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.status.is_running()
    }

    /// Returns the item type for quick-connect.
    #[must_use]
    pub fn item_type(&self) -> DockerItemType {
        if self.status.is_running() {
            DockerItemType::RunningContainer
        } else {
            DockerItemType::StoppedContainer
        }
    }
}

impl Default for DockerContainer {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            image: String::new(),
            status: DockerStatus::Unknown,
            status_text: String::new(),
            ports: Vec::new(),
            created: String::new(),
            display_name: None,
        }
    }
}

/// Represents a Docker image.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImage {
    /// Image ID (short form).
    pub id: String,
    /// Repository name.
    pub repository: String,
    /// Tag (e.g., "latest").
    pub tag: String,
    /// Image size.
    pub size: String,
    /// Creation time.
    pub created: String,
    /// User-friendly display name (optional override).
    pub display_name: Option<String>,
}

impl DockerImage {
    /// Creates a new image from parsed Docker CLI output.
    ///
    /// # Arguments
    /// * `id` - Image ID
    /// * `repository` - Repository name
    /// * `tag` - Image tag
    ///
    /// # Panics
    /// Panics if `id` is empty.
    #[must_use]
    pub fn new(id: String, repository: String, tag: String) -> Self {
        assert!(!id.is_empty(), "image id must not be empty");

        Self {
            id,
            repository,
            tag,
            size: String::new(),
            created: String::new(),
            display_name: None,
        }
    }

    /// Returns the display name or repository:tag.
    #[must_use]
    pub fn display(&self) -> String {
        if let Some(ref name) = self.display_name {
            name.clone()
        } else {
            self.full_name()
        }
    }

    /// Returns the full image name (repository:tag).
    #[must_use]
    pub fn full_name(&self) -> String {
        if self.tag.is_empty() || self.tag == "<none>" {
            self.repository.clone()
        } else {
            format!("{}:{}", self.repository, self.tag)
        }
    }

    /// Returns a short summary for list display.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.size.is_empty() {
            self.full_name()
        } else {
            format!("{} ({})", self.full_name(), self.size)
        }
    }
}

/// Run options for starting a container from an image.
#[derive(Debug, Clone, Default)]
pub struct DockerRunOptions {
    /// Container name (--name).
    pub name: Option<String>,
    /// Port mappings (host:container, -p).
    pub port_mappings: Vec<String>,
    /// Volume mounts (host:container, -v).
    pub volume_mounts: Vec<String>,
    /// Environment variables (KEY=VALUE, -e).
    pub env_vars: Vec<String>,
    /// Run in detached mode (-d). Default false for interactive.
    pub detached: bool,
    /// Remove container on exit (--rm).
    pub remove_on_exit: bool,
    /// Shell to exec into (/bin/sh or /bin/bash).
    pub shell: String,
    /// Additional docker run arguments.
    pub extra_args: Vec<String>,
}

impl DockerRunOptions {
    /// Creates new run options with default shell.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shell: "/bin/sh".to_string(),
            remove_on_exit: true,
            ..Default::default()
        }
    }

    /// Builds the docker run command arguments.
    #[must_use]
    pub fn build_args(&self, image: &str) -> Vec<String> {
        let mut args = Vec::with_capacity(20);

        // Always interactive with TTY for exec
        args.push("-it".to_string());

        // Container name
        if let Some(ref name) = self.name {
            args.push("--name".to_string());
            args.push(name.clone());
        }

        // Remove on exit
        if self.remove_on_exit {
            args.push("--rm".to_string());
        }

        // Port mappings
        for port in &self.port_mappings {
            args.push("-p".to_string());
            args.push(port.clone());
        }

        // Volume mounts
        for vol in &self.volume_mounts {
            args.push("-v".to_string());
            args.push(vol.clone());
        }

        // Environment variables
        for env in &self.env_vars {
            args.push("-e".to_string());
            args.push(env.clone());
        }

        // Extra args
        for extra in &self.extra_args {
            args.push(extra.clone());
        }

        // Image name
        args.push(image.to_string());

        // Shell command
        args.push(self.shell.clone());

        args
    }

    /// Validates the options.
    ///
    /// # Returns
    /// Ok(()) if valid, Err with message if invalid.
    pub fn validate(&self) -> Result<(), String> {
        // Validate port mappings format
        for port in &self.port_mappings {
            if !port.contains(':') {
                return Err(format!(
                    "Invalid port mapping: {} (expected host:container)",
                    port
                ));
            }
        }

        // Validate volume mount format
        for vol in &self.volume_mounts {
            if !vol.contains(':') {
                return Err(format!(
                    "Invalid volume mount: {} (expected host:container)",
                    vol
                ));
            }
        }

        // Validate env var format
        for env in &self.env_vars {
            if !env.contains('=') {
                return Err(format!("Invalid env var: {} (expected KEY=VALUE)", env));
            }
        }

        Ok(())
    }
}

/// Quick-connect item reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerQuickConnectItem {
    /// Item type.
    pub item_type: DockerItemType,
    /// Container ID or Image ID.
    pub id: String,
    /// Display name for the item.
    pub name: String,
}

impl DockerQuickConnectItem {
    /// Creates a new quick-connect item from a container.
    #[must_use]
    pub fn from_container(container: &DockerContainer) -> Self {
        Self {
            item_type: container.item_type(),
            id: container.id.clone(),
            name: container.display().to_string(),
        }
    }

    /// Creates a new quick-connect item from an image.
    #[must_use]
    pub fn from_image(image: &DockerImage) -> Self {
        Self {
            item_type: DockerItemType::Image,
            id: image.id.clone(),
            name: image.display(),
        }
    }
}

/// Quick-connect slots for a single host.
/// Uses HashMap with String keys for TOML serialization compatibility.
/// Keys are slot indices as strings ("0" through "8").
pub type QuickConnectSlots = HashMap<String, DockerQuickConnectItem>;

/// Collection of Docker containers and images with quick-connect assignments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DockerItemList {
    /// Per-host quick-connect assignments.
    /// Keys are "local" or "remote:{host_id}".
    #[serde(default)]
    pub host_quick_connect: HashMap<String, QuickConnectSlots>,
    /// Legacy quick-connect (for backwards compatibility, migrated to host_quick_connect).
    #[serde(default, skip_serializing)]
    quick_connect: [Option<DockerQuickConnectItem>; MAX_QUICK_CONNECT],
    /// Default shell for docker exec/run.
    #[serde(default = "default_shell")]
    pub default_shell: String,
    /// Whether to show stopped containers in the list.
    #[serde(default = "default_show_stopped")]
    pub show_stopped: bool,
    /// Currently selected Docker host.
    #[serde(default)]
    pub selected_host: DockerHost,
}

fn default_shell() -> String {
    "/bin/sh".to_string()
}

fn default_show_stopped() -> bool {
    true
}

impl DockerItemList {
    /// Creates a new empty item list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            host_quick_connect: HashMap::new(),
            quick_connect: Default::default(),
            default_shell: default_shell(),
            show_stopped: true,
            selected_host: DockerHost::Local,
        }
    }

    /// Migrates legacy quick_connect array to host_quick_connect HashMap.
    /// Call this after deserialization to handle old config files.
    pub fn migrate_legacy_quick_connect(&mut self) {
        // Check if there are any legacy quick-connect items
        let has_legacy = self.quick_connect.iter().any(Option::is_some);

        if has_legacy && !self.host_quick_connect.contains_key("local") {
            // Migrate legacy items to local host (convert array to HashMap with String keys)
            let mut migrated: QuickConnectSlots = HashMap::new();
            for (idx, item) in self.quick_connect.iter().enumerate() {
                if let Some(qc) = item {
                    migrated.insert(idx.to_string(), qc.clone());
                }
            }
            self.host_quick_connect.insert("local".to_string(), migrated);
            // Clear legacy array
            self.quick_connect = Default::default();
        }
    }

    /// Returns the quick-connect slots for the currently selected host.
    fn current_host_slots(&self) -> Option<&QuickConnectSlots> {
        let key = self.selected_host.storage_key();
        self.host_quick_connect.get(&key)
    }

    /// Returns mutable quick-connect slots for the currently selected host.
    /// Creates empty slots if none exist.
    fn current_host_slots_mut(&mut self) -> &mut QuickConnectSlots {
        let key = self.selected_host.storage_key();
        self.host_quick_connect.entry(key).or_default()
    }

    /// Returns the quick-connect item at the given index (0-8) for the current host.
    #[must_use]
    pub fn get_quick_connect(&self, index: usize) -> Option<&DockerQuickConnectItem> {
        if index < MAX_QUICK_CONNECT {
            let key = index.to_string();
            self.current_host_slots()
                .and_then(|slots| slots.get(&key))
        } else {
            None
        }
    }

    /// Returns the quick-connect item for a specific host.
    #[must_use]
    pub fn get_quick_connect_for_host(
        &self,
        host: &DockerHost,
        index: usize,
    ) -> Option<&DockerQuickConnectItem> {
        if index < MAX_QUICK_CONNECT {
            let host_key = host.storage_key();
            let slot_key = index.to_string();
            self.host_quick_connect
                .get(&host_key)
                .and_then(|slots| slots.get(&slot_key))
        } else {
            None
        }
    }

    /// Sets a quick-connect item at the given index (0-8) for the current host.
    ///
    /// Returns true if successful, false if index out of range.
    pub fn set_quick_connect(&mut self, index: usize, item: DockerQuickConnectItem) -> bool {
        if index < MAX_QUICK_CONNECT {
            let key = index.to_string();
            let slots = self.current_host_slots_mut();
            slots.insert(key, item);
            true
        } else {
            false
        }
    }

    /// Removes the quick-connect item at the given index for the current host.
    pub fn remove_quick_connect(&mut self, index: usize) -> bool {
        if index < MAX_QUICK_CONNECT {
            let key = index.to_string();
            let slots = self.current_host_slots_mut();
            slots.remove(&key);
            true
        } else {
            false
        }
    }

    /// Returns the number of assigned quick-connect slots for the current host.
    #[must_use]
    pub fn quick_connect_count(&self) -> usize {
        self.current_host_slots()
            .map(|slots| slots.len())
            .unwrap_or(0)
    }

    /// Finds the quick-connect slot for a container ID on the current host.
    #[must_use]
    pub fn find_quick_connect_for_id(&self, id: &str) -> Option<usize> {
        self.current_host_slots().and_then(|slots| {
            slots
                .iter()
                .find(|(_, item)| item.id == id)
                .and_then(|(key, _)| key.parse().ok())
        })
    }

    /// Sets the selected Docker host.
    pub fn set_selected_host(&mut self, host: DockerHost) {
        self.selected_host = host;
    }

    /// Returns the currently selected Docker host.
    #[must_use]
    pub fn selected_host(&self) -> &DockerHost {
        &self.selected_host
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_status_parse() {
        assert_eq!(DockerStatus::parse("Up 5 minutes"), DockerStatus::Running);
        assert_eq!(
            DockerStatus::parse("Exited (0) 2 hours ago"),
            DockerStatus::Exited
        );
        assert_eq!(DockerStatus::parse("Paused"), DockerStatus::Paused);
        assert_eq!(DockerStatus::parse("unknown status"), DockerStatus::Unknown);
    }

    #[test]
    fn test_container_creation() {
        let container = DockerContainer::new(
            "abc123".to_string(),
            "my-nginx".to_string(),
            "nginx:latest".to_string(),
            "Up 5 minutes".to_string(),
        );

        assert_eq!(container.id, "abc123");
        assert_eq!(container.name, "my-nginx");
        assert!(container.is_running());
        assert_eq!(container.item_type(), DockerItemType::RunningContainer);
    }

    #[test]
    fn test_image_full_name() {
        let image = DockerImage::new(
            "sha256:abc".to_string(),
            "nginx".to_string(),
            "latest".to_string(),
        );
        assert_eq!(image.full_name(), "nginx:latest");

        let image_no_tag = DockerImage::new(
            "sha256:def".to_string(),
            "custom-image".to_string(),
            "<none>".to_string(),
        );
        assert_eq!(image_no_tag.full_name(), "custom-image");
    }

    #[test]
    fn test_run_options_build_args() {
        let mut opts = DockerRunOptions::new();
        opts.name = Some("test-container".to_string());
        opts.port_mappings.push("8080:80".to_string());
        opts.env_vars.push("DEBUG=true".to_string());

        let args = opts.build_args("nginx:latest");

        assert!(args.contains(&"-it".to_string()));
        assert!(args.contains(&"--name".to_string()));
        assert!(args.contains(&"test-container".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"8080:80".to_string()));
        assert!(args.contains(&"nginx:latest".to_string()));
    }

    #[test]
    fn test_run_options_validate() {
        let mut opts = DockerRunOptions::new();
        assert!(opts.validate().is_ok());

        opts.port_mappings.push("invalid".to_string());
        assert!(opts.validate().is_err());

        opts.port_mappings.clear();
        opts.volume_mounts.push("no-colon".to_string());
        assert!(opts.validate().is_err());
    }

    #[test]
    fn test_quick_connect() {
        let mut list = DockerItemList::new();

        let container = DockerContainer::new(
            "abc123".to_string(),
            "my-app".to_string(),
            "myimage".to_string(),
            "Up".to_string(),
        );

        let item = DockerQuickConnectItem::from_container(&container);
        assert!(list.set_quick_connect(0, item.clone()));
        assert_eq!(list.quick_connect_count(), 1);

        let retrieved = list.get_quick_connect(0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "abc123");

        assert_eq!(list.find_quick_connect_for_id("abc123"), Some(0));
        assert_eq!(list.find_quick_connect_for_id("xyz"), None);
    }

    #[test]
    fn test_docker_host_local() {
        let host = DockerHost::Local;
        assert!(host.is_local());
        assert!(!host.is_remote());
        assert_eq!(host.host_id(), None);
        assert_eq!(host.display_name(), "Local");
        assert_eq!(host.storage_key(), "local");
        assert!(host.ssh_args().is_none());
    }

    #[test]
    fn test_docker_host_remote() {
        let host = DockerHost::remote(
            1,
            "server.example.com".to_string(),
            22,
            "admin".to_string(),
            Some("My Server".to_string()),
        );

        assert!(!host.is_local());
        assert!(host.is_remote());
        assert_eq!(host.host_id(), Some(1));
        assert_eq!(host.display_name(), "My Server");
        assert_eq!(host.storage_key(), "remote:1");

        let args = host.ssh_args().unwrap();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], "admin@server.example.com");
    }

    #[test]
    fn test_docker_host_remote_custom_port() {
        let host = DockerHost::remote(
            2,
            "192.168.1.100".to_string(),
            2222,
            "user".to_string(),
            None,
        );

        assert_eq!(host.display_name(), "192.168.1.100");

        let args = host.ssh_args().unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "2222");
        assert_eq!(args[2], "user@192.168.1.100");
    }

    #[test]
    fn test_per_host_quick_connect() {
        let mut list = DockerItemList::new();

        let container1 = DockerContainer::new(
            "local123".to_string(),
            "local-app".to_string(),
            "myimage".to_string(),
            "Up".to_string(),
        );

        let container2 = DockerContainer::new(
            "remote456".to_string(),
            "remote-app".to_string(),
            "myimage".to_string(),
            "Up".to_string(),
        );

        // Set quick-connect on local host
        list.set_selected_host(DockerHost::Local);
        let item1 = DockerQuickConnectItem::from_container(&container1);
        list.set_quick_connect(0, item1);
        assert_eq!(list.quick_connect_count(), 1);
        assert_eq!(list.get_quick_connect(0).unwrap().id, "local123");

        // Switch to remote host
        let remote_host = DockerHost::remote(
            1,
            "server.com".to_string(),
            22,
            "user".to_string(),
            None,
        );
        list.set_selected_host(remote_host.clone());

        // Remote should have no quick-connect yet
        assert_eq!(list.quick_connect_count(), 0);
        assert!(list.get_quick_connect(0).is_none());

        // Set quick-connect on remote host
        let item2 = DockerQuickConnectItem::from_container(&container2);
        list.set_quick_connect(0, item2);
        assert_eq!(list.quick_connect_count(), 1);
        assert_eq!(list.get_quick_connect(0).unwrap().id, "remote456");

        // Switch back to local - should still have local container
        list.set_selected_host(DockerHost::Local);
        assert_eq!(list.get_quick_connect(0).unwrap().id, "local123");

        // Switch back to remote - should still have remote container
        list.set_selected_host(remote_host);
        assert_eq!(list.get_quick_connect(0).unwrap().id, "remote456");
    }
}
