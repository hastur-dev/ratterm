//! Docker Manager selector state and methods.

use crate::docker::{
    DockerAvailability, DockerContainer, DockerDiscoveryResult, DockerImage, DockerRunOptions,
};

use super::types::{
    DockerItemDisplay, DockerListSection, DockerManagerMode, MAX_DISPLAY_ITEMS, RunOptionsField,
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
    fn validate_selection(&mut self) {
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
    fn update_scroll(&mut self) {
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

    // --- Run Options Mode ---

    /// Starts run options mode for an image.
    pub fn start_run_options(&mut self, image_name: String) {
        self.run_target = Some(image_name);
        self.run_options = DockerRunOptions::new();
        self.run_options_field = RunOptionsField::Name;
        self.input_buffer.clear();
        self.mode = DockerManagerMode::RunOptions;
    }

    /// Cancels run options and returns to list mode.
    pub fn cancel_run_options(&mut self) {
        self.run_target = None;
        self.run_options = DockerRunOptions::new();
        self.input_buffer.clear();
        self.mode = DockerManagerMode::List;
    }

    /// Returns the current run options.
    #[must_use]
    pub fn run_options(&self) -> &DockerRunOptions {
        &self.run_options
    }

    /// Returns the run target image.
    #[must_use]
    pub fn run_target(&self) -> Option<&str> {
        self.run_target.as_deref()
    }

    /// Returns the current run options field.
    #[must_use]
    pub fn run_options_field(&self) -> RunOptionsField {
        self.run_options_field
    }

    /// Moves to the next run options field.
    pub fn next_run_options_field(&mut self) {
        // Save current field before moving
        self.save_current_field();
        self.run_options_field = self.run_options_field.next();
        self.load_current_field();
    }

    /// Moves to the previous run options field.
    pub fn prev_run_options_field(&mut self) {
        self.save_current_field();
        self.run_options_field = self.run_options_field.prev();
        self.load_current_field();
    }

    /// Returns the input buffer.
    #[must_use]
    pub fn input_buffer(&self) -> &str {
        &self.input_buffer
    }

    /// Inserts a character into the input buffer.
    pub fn insert_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    /// Removes the last character from the input buffer.
    pub fn backspace(&mut self) {
        self.input_buffer.pop();
    }

    /// Clears the input buffer.
    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
    }

    /// Saves the current field's value from input buffer.
    fn save_current_field(&mut self) {
        let value = self.input_buffer.trim().to_string();
        match self.run_options_field {
            RunOptionsField::Name => {
                self.run_options.name = if value.is_empty() { None } else { Some(value) };
            }
            RunOptionsField::Ports => {
                self.run_options.port_mappings = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            RunOptionsField::Volumes => {
                self.run_options.volume_mounts = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            RunOptionsField::EnvVars => {
                self.run_options.env_vars = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            RunOptionsField::Shell => {
                self.run_options.shell = if value.is_empty() {
                    "/bin/sh".to_string()
                } else {
                    value
                };
            }
        }
    }

    /// Loads the current field's value into input buffer.
    fn load_current_field(&mut self) {
        self.input_buffer = match self.run_options_field {
            RunOptionsField::Name => self.run_options.name.clone().unwrap_or_default(),
            RunOptionsField::Ports => self.run_options.port_mappings.join(", "),
            RunOptionsField::Volumes => self.run_options.volume_mounts.join(", "),
            RunOptionsField::EnvVars => self.run_options.env_vars.join(", "),
            RunOptionsField::Shell => self.run_options.shell.clone(),
        };
    }

    /// Finishes run options and validates.
    pub fn finish_run_options(&mut self) -> Result<DockerRunOptions, String> {
        self.save_current_field();
        self.run_options.validate()?;
        Ok(self.run_options.clone())
    }

    // --- Confirm Mode ---

    /// Starts confirm mode for running an image.
    pub fn start_confirm(&mut self, target: String) {
        self.confirm_target = Some(target);
        self.mode = DockerManagerMode::Confirming;
    }

    /// Cancels confirmation and returns to list mode.
    pub fn cancel_confirm(&mut self) {
        self.confirm_target = None;
        self.mode = DockerManagerMode::List;
    }

    /// Returns the confirm target.
    #[must_use]
    pub fn confirm_target(&self) -> Option<&str> {
        self.confirm_target.as_deref()
    }

    /// Returns the selected index.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns the scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
}

impl Default for DockerManagerSelector {
    fn default() -> Self {
        Self::new()
    }
}
