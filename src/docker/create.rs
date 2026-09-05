//! Docker Hub search and the container-creation workflow.
//!
//! Split out of `container.rs`. Everything here is form state and command
//! assembly for the "run a new container" screens; nothing here talks to a
//! daemon.

/// Maximum number of search results to display.
pub const MAX_SEARCH_RESULTS: usize = 25;

/// Represents a Docker Hub search result.
#[derive(Debug, Clone, Default)]
pub struct DockerSearchResult {
    /// Image name (e.g., "nginx", "ubuntu").
    pub name: String,
    /// Description from Docker Hub.
    pub description: String,
    /// Number of stars.
    pub stars: u32,
    /// Whether it's an official image.
    pub official: bool,
}

impl DockerSearchResult {
    /// Creates a new search result.
    ///
    /// # Panics
    /// Panics if `name` is empty.
    #[must_use]
    pub fn new(name: String, description: String, stars: u32, official: bool) -> Self {
        assert!(!name.is_empty(), "search result name must not be empty");
        Self {
            name,
            description,
            stars,
            official,
        }
    }

    /// Returns a formatted display string for the search result.
    #[must_use]
    pub fn display(&self) -> String {
        let official_badge = if self.official { " [OFFICIAL]" } else { "" };
        let desc = if self.description.len() > 50 {
            format!("{}...", &self.description[..47])
        } else {
            self.description.clone()
        };
        format!(
            "{}{} - {} ({}★)",
            self.name, official_badge, desc, self.stars
        )
    }
}

/// Volume mount configuration for container creation.
#[derive(Debug, Clone, Default)]
pub struct VolumeMountConfig {
    /// Host path (on the remote SSH host or local machine).
    pub host_path: String,
    /// Container path (mount point inside container).
    pub container_path: String,
}

impl VolumeMountConfig {
    /// Creates a new volume mount configuration.
    ///
    /// # Panics
    /// Panics if either path is empty.
    #[must_use]
    pub fn new(host_path: String, container_path: String) -> Self {
        assert!(!host_path.is_empty(), "host_path must not be empty");
        assert!(
            !container_path.is_empty(),
            "container_path must not be empty"
        );
        Self {
            host_path,
            container_path,
        }
    }

    /// Formats the mount as a docker -v argument.
    #[must_use]
    pub fn to_docker_arg(&self) -> String {
        format!("{}:{}", self.host_path, self.container_path)
    }
}

/// State for the container creation workflow.
#[derive(Debug, Clone, Default)]
pub struct ContainerCreationState {
    /// Search term for Docker Hub.
    pub search_term: String,
    /// Search results from Docker Hub.
    pub search_results: Vec<DockerSearchResult>,
    /// Selected search result index.
    pub selected_result_idx: usize,
    /// Selected image name (after search or from existing images).
    pub selected_image: Option<String>,
    /// Whether the selected image exists on the remote host.
    pub image_exists: bool,
    /// Whether image is currently downloading.
    pub downloading: bool,
    /// Download status message.
    pub download_status: Option<String>,
    /// Volume mounts to apply.
    pub volume_mounts: Vec<VolumeMountConfig>,
    /// Current host path input (during volume configuration).
    pub current_host_path: String,
    /// Current container path input (during volume configuration).
    pub current_container_path: String,
    /// Startup command to append to docker run.
    pub startup_command: String,
    /// Error message (if any).
    pub error: Option<String>,
    /// Whether to suggest checking log file for full error.
    pub suggest_log_file: bool,
}

impl ContainerCreationState {
    /// Creates a new empty creation state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a creation state with a pre-selected image.
    ///
    /// # Panics
    /// Panics if `image_name` is empty.
    #[must_use]
    pub fn with_image(image_name: String) -> Self {
        assert!(!image_name.is_empty(), "image_name must not be empty");
        Self {
            selected_image: Some(image_name),
            ..Default::default()
        }
    }

    /// Resets the state for a new creation workflow.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Sets the search results.
    pub fn set_search_results(&mut self, results: Vec<DockerSearchResult>) {
        self.search_results = results;
        self.selected_result_idx = 0;
    }

    /// Selects the next search result.
    pub fn select_next_result(&mut self) {
        if !self.search_results.is_empty() {
            self.selected_result_idx = (self.selected_result_idx + 1) % self.search_results.len();
        }
    }

    /// Selects the previous search result.
    pub fn select_prev_result(&mut self) {
        if !self.search_results.is_empty() {
            if self.selected_result_idx == 0 {
                self.selected_result_idx = self.search_results.len() - 1;
            } else {
                self.selected_result_idx -= 1;
            }
        }
    }

    /// Returns the currently selected search result.
    #[must_use]
    pub fn selected_result(&self) -> Option<&DockerSearchResult> {
        self.search_results.get(self.selected_result_idx)
    }

    /// Confirms the current search result selection.
    pub fn confirm_selection(&mut self) {
        if let Some(result) = self.selected_result() {
            self.selected_image = Some(result.name.clone());
        }
    }

    /// Adds a volume mount from the current inputs.
    ///
    /// # Returns
    /// `true` if mount was added, `false` if inputs are empty.
    pub fn add_current_volume_mount(&mut self) -> bool {
        if self.current_host_path.is_empty() || self.current_container_path.is_empty() {
            return false;
        }

        let mount = VolumeMountConfig::new(
            self.current_host_path.clone(),
            self.current_container_path.clone(),
        );
        self.volume_mounts.push(mount);
        self.current_host_path.clear();
        self.current_container_path.clear();
        true
    }

    /// Clears all volume mounts.
    pub fn clear_volume_mounts(&mut self) {
        self.volume_mounts.clear();
    }

    /// Sets an error with optional log file suggestion.
    pub fn set_error(&mut self, error: String, suggest_log: bool) {
        self.suggest_log_file = suggest_log || error.len() > 200;
        self.error = Some(error);
    }

    /// Clears any error.
    pub fn clear_error(&mut self) {
        self.error = None;
        self.suggest_log_file = false;
    }

    /// Builds the complete docker run command.
    #[must_use]
    pub fn build_run_command(&self) -> Option<String> {
        let image = self.selected_image.as_ref()?;

        let mut parts = vec![
            "docker".to_string(),
            "run".to_string(),
            "-it".to_string(),
            "--rm".to_string(),
        ];

        // Add volume mounts
        for mount in &self.volume_mounts {
            parts.push("-v".to_string());
            parts.push(mount.to_docker_arg());
        }

        // Add image name
        parts.push(image.clone());

        // Add startup command if provided
        if !self.startup_command.trim().is_empty() {
            for arg in self.startup_command.split_whitespace() {
                parts.push(arg.to_string());
            }
        }

        Some(parts.join(" "))
    }

    /// Returns the number of volume mounts configured.
    #[must_use]
    pub fn volume_mount_count(&self) -> usize {
        self.volume_mounts.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_creation_state_without_an_image_builds_no_command() {
        let state = ContainerCreationState::new();
        assert!(state.build_run_command().is_none());
    }

    #[test]
    fn a_creation_state_builds_a_command_with_mounts_and_a_startup_command() {
        let mut state = ContainerCreationState::with_image("nginx:latest".to_string());
        state.current_host_path = "/srv/data".to_string();
        state.current_container_path = "/data".to_string();
        assert!(state.add_current_volume_mount());
        state.startup_command = "  sh -c ls  ".to_string();

        let command = state.build_run_command().expect("image is set");
        assert!(command.contains("-v /srv/data:/data"), "{command}");
        assert!(command.ends_with("nginx:latest sh -c ls"), "{command}");
        assert_eq!(state.volume_mount_count(), 1);
    }

    #[test]
    fn a_half_filled_volume_mount_is_refused() {
        let mut state = ContainerCreationState::new();
        state.current_host_path = "/srv".to_string();
        assert!(!state.add_current_volume_mount());
        state.current_host_path.clear();
        state.current_container_path = "/data".to_string();
        assert!(!state.add_current_volume_mount());
        assert_eq!(state.volume_mount_count(), 0);
    }

    #[test]
    fn search_selection_wraps_in_both_directions() {
        let mut state = ContainerCreationState::new();
        state.set_search_results(vec![
            DockerSearchResult::new("a".into(), String::new(), 1, false),
            DockerSearchResult::new("b".into(), String::new(), 2, true),
        ]);

        state.select_prev_result();
        assert_eq!(state.selected_result().map(|r| r.name.as_str()), Some("b"));
        state.select_next_result();
        assert_eq!(state.selected_result().map(|r| r.name.as_str()), Some("a"));

        state.confirm_selection();
        assert_eq!(state.selected_image.as_deref(), Some("a"));
    }

    #[test]
    fn selecting_in_an_empty_result_list_does_nothing() {
        let mut state = ContainerCreationState::new();
        state.select_next_result();
        state.select_prev_result();
        assert!(state.selected_result().is_none());
        state.confirm_selection();
        assert!(state.selected_image.is_none());
    }

    #[test]
    fn a_long_error_suggests_the_log_file_on_its_own() {
        let mut state = ContainerCreationState::new();
        state.set_error("x".repeat(201), false);
        assert!(state.suggest_log_file);
        state.clear_error();
        assert!(!state.suggest_log_file);
        assert!(state.error.is_none());
    }

    #[test]
    fn a_long_search_description_is_truncated_for_display() {
        let result = DockerSearchResult::new("nginx".into(), "d".repeat(80), 42, true);
        let line = result.display();
        assert!(line.contains("[OFFICIAL]"), "{line}");
        assert!(line.contains("..."), "{line}");
        assert!(line.contains("42"), "{line}");
    }

    #[test]
    fn a_volume_mount_renders_as_a_docker_argument() {
        let mount = VolumeMountConfig::new("/host".into(), "/container".into());
        assert_eq!(mount.to_docker_arg(), "/host:/container");
    }

    #[test]
    fn resetting_clears_every_field() {
        let mut state = ContainerCreationState::with_image("nginx".to_string());
        state.search_term = "ngin".to_string();
        state.downloading = true;
        state.reset();
        assert!(state.selected_image.is_none());
        assert!(state.search_term.is_empty());
        assert!(!state.downloading);
    }
}
