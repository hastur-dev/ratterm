//! Docker Manager selector state and methods.

use crate::app::input_traits::ListSelectable;
use crate::docker::{
    ContainerCreationState, DockerAvailability, DockerContainer, DockerDiscoveryResult, DockerHost,
    DockerImage, DockerRunOptions,
};

use super::types::{
    DockerHostDisplay, DockerItemDisplay, DockerListSection, DockerManagerMode,
    HostCredentialField, MAX_DISPLAY_ITEMS, RunOptionsField,
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
    // --- Container creation state ---
    /// State for container creation workflow.
    pub(super) creation_state: ContainerCreationState,
    // --- Docker log viewer state ---
    /// State for Docker log viewer (when in LogView mode).
    pub(crate) docker_logs_state: Option<crate::docker_logs::ui::state::DockerLogsState>,
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
            // Container creation
            creation_state: ContainerCreationState::new(),
            // Docker logs
            docker_logs_state: None,
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
    pub(super) fn validate_selection(&mut self) {
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

    /// Returns all containers as log info entries for the log viewer.
    #[must_use]
    pub fn all_container_log_infos(&self) -> Vec<crate::docker_logs::types::ContainerLogInfo> {
        let mut result = Vec::new();
        for c in &self.running_containers {
            result.push(crate::docker_logs::types::ContainerLogInfo {
                id: c.id.clone(),
                name: c.display().to_string(),
                image: c.image.clone(),
                status: "running".to_string(),
                access: crate::docker_logs::types::AccessStatus::Unknown,
            });
        }
        for c in &self.stopped_containers {
            result.push(crate::docker_logs::types::ContainerLogInfo {
                id: c.id.clone(),
                name: c.display().to_string(),
                image: c.image.clone(),
                status: "exited".to_string(),
                access: crate::docker_logs::types::AccessStatus::Unknown,
            });
        }
        result
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
    pub(super) fn update_scroll(&mut self) {
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
}

impl Default for DockerManagerSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl ListSelectable for DockerManagerSelector {
    fn select_prev(&mut self) {
        DockerManagerSelector::select_prev(self);
    }

    fn select_next(&mut self) {
        DockerManagerSelector::select_next(self);
    }

    fn select_first(&mut self) {
        DockerManagerSelector::select_first(self);
    }

    fn select_last(&mut self) {
        DockerManagerSelector::select_last(self);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "selector_tests.rs"]
mod tests;
