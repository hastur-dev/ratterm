//! Docker container and image data structures.
//!
//! This module holds the two things a Docker screen lists — a container and an
//! image — plus the small enums that classify them. The host type, the
//! quick-connect slots and the container-creation form used to live here too;
//! they are now in [`super::host`], [`super::items`] and [`super::create`],
//! which keeps every file in this module inside the project's size limit.

use serde::{Deserialize, Serialize};

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
    /// Parses status from Docker CLI output or the API's `State` field.
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

    /// Returns the first twelve characters of the id, as Docker prints it.
    #[must_use]
    pub fn short_id(&self) -> String {
        self.id.chars().take(12).collect()
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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
    fn every_api_state_word_maps_to_a_status() {
        // These are the strings the Docker API puts in `ContainerSummary.state`.
        assert_eq!(DockerStatus::parse("running"), DockerStatus::Running);
        assert_eq!(DockerStatus::parse("exited"), DockerStatus::Exited);
        assert_eq!(DockerStatus::parse("created"), DockerStatus::Created);
        assert_eq!(DockerStatus::parse("restarting"), DockerStatus::Restarting);
        assert_eq!(DockerStatus::parse("dead"), DockerStatus::Dead);
        assert_eq!(DockerStatus::parse(""), DockerStatus::Unknown);
    }

    #[test]
    fn stopped_states_are_grouped() {
        assert!(DockerStatus::Exited.is_stopped());
        assert!(DockerStatus::Dead.is_stopped());
        assert!(DockerStatus::Stopped.is_stopped());
        assert!(!DockerStatus::Running.is_stopped());
        assert!(!DockerStatus::Unknown.is_stopped());
    }

    #[test]
    fn item_types_carry_a_label_and_a_name() {
        assert_eq!(DockerItemType::RunningContainer.label(), "[R]");
        assert_eq!(DockerItemType::StoppedContainer.as_str(), "Stopped");
        assert_eq!(DockerItemType::Image.label(), "[I]");
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
    fn a_stopped_container_reports_the_stopped_item_type() {
        let container = DockerContainer::new(
            "abc".to_string(),
            "app".to_string(),
            "img".to_string(),
            "Exited (137) 1 hour ago".to_string(),
        );
        assert_eq!(container.item_type(), DockerItemType::StoppedContainer);
        assert!(!container.is_running());
    }

    #[test]
    fn a_summary_lists_ports_when_there_are_any() {
        let mut container = DockerContainer::new(
            "abc".to_string(),
            "web".to_string(),
            "nginx".to_string(),
            "Up".to_string(),
        );
        assert_eq!(container.summary(), "web (nginx)");
        container.ports = vec!["0.0.0.0:8080->80/tcp".to_string()];
        assert_eq!(container.summary(), "web (nginx) [0.0.0.0:8080->80/tcp]");
    }

    #[test]
    fn a_display_name_overrides_the_container_name() {
        let mut container = DockerContainer::new(
            "abc".to_string(),
            "raw".to_string(),
            "img".to_string(),
            "Up".to_string(),
        );
        assert_eq!(container.display(), "raw");
        container.display_name = Some("friendly".to_string());
        assert_eq!(container.display(), "friendly");
    }

    #[test]
    fn a_long_id_is_shortened_the_way_docker_prints_it() {
        let container = DockerContainer::new(
            "0123456789abcdef0123".to_string(),
            "app".to_string(),
            "img".to_string(),
            "Up".to_string(),
        );
        assert_eq!(container.short_id(), "0123456789ab");

        let short = DockerContainer::new(
            "abc".to_string(),
            "app".to_string(),
            "img".to_string(),
            "Up".to_string(),
        );
        assert_eq!(short.short_id(), "abc");
    }

    #[test]
    #[should_panic(expected = "container id must not be empty")]
    fn a_container_without_an_id_is_refused() {
        let _ = DockerContainer::new(
            String::new(),
            "n".to_string(),
            "i".to_string(),
            "Up".to_string(),
        );
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
    fn an_image_summary_includes_the_size_when_known() {
        let mut image = DockerImage::new(
            "sha256:abc".to_string(),
            "nginx".to_string(),
            "latest".to_string(),
        );
        assert_eq!(image.summary(), "nginx:latest");
        image.size = "150MB".to_string();
        assert_eq!(image.summary(), "nginx:latest (150MB)");
        image.display_name = Some("the web server".to_string());
        assert_eq!(image.display(), "the web server");
    }

    #[test]
    #[should_panic(expected = "image id must not be empty")]
    fn an_image_without_an_id_is_refused() {
        let _ = DockerImage::new(String::new(), "repo".to_string(), "tag".to_string());
    }
}
