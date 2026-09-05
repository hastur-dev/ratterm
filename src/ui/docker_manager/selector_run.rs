//! Run options and the confirmation step.
//!
//! Split out of `selector.rs`, which had grown past this project's file-size
//! limit. These are `impl DockerManagerSelector` blocks, so every call site is
//! unchanged.

use crate::docker::DockerRunOptions;

use super::selector::DockerManagerSelector;
use super::types::{DockerManagerMode, RunOptionsField};

impl DockerManagerSelector {
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
    pub(super) fn save_current_field(&mut self) {
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
    pub(super) fn load_current_field(&mut self) {
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
