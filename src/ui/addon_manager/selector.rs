//! Add-on Manager selector state.
//!
//! Manages state for the add-on manager popup, including navigation
//! and installation progress.

use super::types::{AddonDisplay, AddonListSection, AddonManagerMode, MAX_DISPLAY_ADDONS};
use crate::addons::{Addon, AddonInstaller, InstallProgress, InstalledAddon};

/// State manager for the add-on manager popup.
#[derive(Debug, Clone)]
pub struct AddonManagerSelector {
    // List state
    /// Available addons from GitHub.
    available_addons: Vec<AddonDisplay>,
    /// Installed addons.
    installed_addons: Vec<AddonDisplay>,
    /// Current list section.
    section: AddonListSection,
    /// Selected index within current section.
    selected_index: usize,
    /// Scroll offset for long lists.
    scroll_offset: usize,
    /// Current mode.
    mode: AddonManagerMode,

    // Filter state
    /// Search query for filtering.
    filter_query: String,

    // Installation state
    /// Current installation progress.
    install_progress: Option<InstallProgress>,
    /// Addon being installed/uninstalled.
    pending_addon_id: Option<String>,
    /// True if the pending operation is an uninstall.
    is_uninstalling: bool,

    // Error state
    /// Error message to display.
    error: Option<String>,

    // Repository info
    /// Current repository.
    repository: String,
    /// Current branch.
    branch: String,
}

impl Default for AddonManagerSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl AddonManagerSelector {
    /// Creates a new addon manager selector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            available_addons: Vec::new(),
            installed_addons: Vec::new(),
            section: AddonListSection::default(),
            selected_index: 0,
            scroll_offset: 0,
            mode: AddonManagerMode::default(),
            filter_query: String::new(),
            install_progress: None,
            pending_addon_id: None,
            is_uninstalling: false,
            error: None,
            repository: String::from("hastur-dev/installer-repo"),
            branch: String::from("main"),
        }
    }

    // =========================================================================
    // Mode Management
    // =========================================================================

    /// Returns the current mode.
    #[must_use]
    pub fn mode(&self) -> AddonManagerMode {
        self.mode
    }

    /// Sets the current mode.
    pub fn set_mode(&mut self, mode: AddonManagerMode) {
        self.mode = mode;
    }

    /// Returns to list mode, clearing any pending state.
    pub fn return_to_list(&mut self) {
        self.mode = AddonManagerMode::List;
        self.pending_addon_id = None;
        self.is_uninstalling = false;
        self.error = None;
    }

    // =========================================================================
    // Section and Navigation
    // =========================================================================

    /// Returns the current section.
    #[must_use]
    pub fn section(&self) -> AddonListSection {
        self.section
    }

    /// Toggles between available and installed sections.
    pub fn toggle_section(&mut self) {
        self.section = self.section.next();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Returns the current items for the active section (unfiltered).
    #[must_use]
    pub fn current_items(&self) -> &[AddonDisplay] {
        match self.section {
            AddonListSection::Available => &self.available_addons,
            AddonListSection::Installed => &self.installed_addons,
        }
    }

    /// Returns filtered items based on the current search query.
    #[must_use]
    pub fn filtered_items(&self) -> Vec<&AddonDisplay> {
        let items = self.current_items();

        if self.filter_query.is_empty() {
            return items.iter().collect();
        }

        let query = self.filter_query.to_lowercase();
        items
            .iter()
            .filter(|item| {
                item.addon.id.to_lowercase().contains(&query)
                    || item.addon.name.to_lowercase().contains(&query)
                    || item.addon.description.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Returns the count of filtered items.
    #[must_use]
    pub fn filtered_count(&self) -> usize {
        self.filtered_items().len()
    }

    /// Returns visible items with scroll applied (filtered).
    pub fn visible_items(&self) -> Vec<(usize, &AddonDisplay)> {
        self.filtered_items()
            .into_iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(MAX_DISPLAY_ADDONS)
            .collect()
    }

    /// Returns the selected index.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns true if the given index is selected.
    #[must_use]
    pub fn is_selected(&self, index: usize) -> bool {
        index == self.selected_index
    }

    /// Moves selection up.
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.update_scroll();
        }
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        let max_index = self.filtered_count().saturating_sub(1);
        if self.selected_index < max_index {
            self.selected_index += 1;
            self.update_scroll();
        }
    }

    /// Moves selection to the first item.
    pub fn select_first(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Moves selection to the last item.
    pub fn select_last(&mut self) {
        let len = self.filtered_count();
        if len > 0 {
            self.selected_index = len - 1;
            self.update_scroll();
        }
    }

    /// Updates scroll offset to keep selection visible.
    fn update_scroll(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + MAX_DISPLAY_ADDONS {
            self.scroll_offset = self.selected_index.saturating_sub(MAX_DISPLAY_ADDONS - 1);
        }
    }

    /// Returns the currently selected addon display (from filtered list).
    #[must_use]
    pub fn selected_addon(&self) -> Option<&AddonDisplay> {
        self.filtered_items().get(self.selected_index).copied()
    }

    // =========================================================================
    // Data Updates
    // =========================================================================

    /// Updates the available addons list.
    ///
    /// Also checks if each technology is already installed on the system.
    pub fn set_available_addons(&mut self, addons: Vec<Addon>, installed: &[InstalledAddon]) {
        self.available_addons = addons
            .into_iter()
            .map(|addon| {
                let is_installed = installed.iter().any(|i| i.id == addon.id);

                if is_installed {
                    AddonDisplay::installed(addon)
                } else {
                    // Check if technology is already on the system
                    let system_path = AddonInstaller::detect_installed(&addon.id);
                    AddonDisplay::available_with_detection(addon, system_path)
                }
            })
            .collect();
    }

    /// Updates the installed addons list.
    pub fn set_installed_addons(&mut self, installed: Vec<InstalledAddon>) {
        self.installed_addons = installed
            .into_iter()
            .map(|inst| {
                let addon = Addon::new(inst.id.clone())
                    .with_description(inst.display_name.clone())
                    .with_install(true);
                AddonDisplay::installed(addon)
            })
            .collect();
    }

    /// Updates both lists from config.
    pub fn update_from_config(
        &mut self,
        available: Vec<Addon>,
        installed: Vec<InstalledAddon>,
    ) {
        self.set_available_addons(available, &installed);
        self.set_installed_addons(installed);
    }

    /// Sets the repository and branch.
    pub fn set_repository(&mut self, repository: String, branch: String) {
        self.repository = repository;
        self.branch = branch;
    }

    /// Returns the current repository.
    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// Returns the current branch.
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }

    // =========================================================================
    // Installation Management
    // =========================================================================

    /// Sets the install progress.
    pub fn set_install_progress(&mut self, progress: Option<InstallProgress>) {
        self.install_progress = progress;
        if self.install_progress.is_some() {
            self.mode = AddonManagerMode::Installing;
        }
    }

    /// Returns the current install progress.
    #[must_use]
    pub fn install_progress(&self) -> Option<&InstallProgress> {
        self.install_progress.as_ref()
    }

    /// Handles install failure.
    pub fn install_failed(&mut self, error: String) {
        self.install_progress = None;
        self.error = Some(error);
        self.mode = AddonManagerMode::Error;
    }

    /// Returns the pending addon ID (addon being installed).
    #[must_use]
    pub fn pending_addon_id(&self) -> Option<&str> {
        self.pending_addon_id.as_deref()
    }

    /// Sets the pending addon ID.
    pub fn set_pending_addon_id(&mut self, addon_id: Option<String>) {
        self.pending_addon_id = addon_id;
    }

    /// Returns true if the pending operation is an uninstall.
    #[must_use]
    pub fn is_uninstalling(&self) -> bool {
        self.is_uninstalling
    }

    /// Sets whether the pending operation is an uninstall.
    pub fn set_uninstalling(&mut self, uninstalling: bool) {
        self.is_uninstalling = uninstalling;
    }

    // =========================================================================
    // Error Management
    // =========================================================================

    /// Sets an error message.
    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
        if self.error.is_some() {
            self.mode = AddonManagerMode::Error;
        }
    }

    /// Returns the current error message.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Clears the error and returns to list.
    pub fn clear_error(&mut self) {
        self.error = None;
        self.return_to_list();
    }

    // =========================================================================
    // Filter
    // =========================================================================

    /// Returns the current filter query.
    #[must_use]
    pub fn filter_query(&self) -> &str {
        &self.filter_query
    }

    /// Sets the filter query and resets selection.
    pub fn set_filter_query(&mut self, query: String) {
        self.filter_query = query;
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Appends a character to the filter query and resets selection.
    pub fn filter_insert_char(&mut self, c: char) {
        self.filter_query.push(c);
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Removes the last character from the filter query and resets selection.
    pub fn filter_backspace(&mut self) {
        self.filter_query.pop();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Clears the filter query and resets selection.
    pub fn filter_clear(&mut self) {
        self.filter_query.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Returns true if there's an active filter.
    #[must_use]
    pub fn has_filter(&self) -> bool {
        !self.filter_query.is_empty()
    }

    // =========================================================================
    // Counts
    // =========================================================================

    /// Returns the count of available addons.
    #[must_use]
    pub fn available_count(&self) -> usize {
        self.available_addons.len()
    }

    /// Returns the count of installed addons.
    #[must_use]
    pub fn installed_count(&self) -> usize {
        self.installed_addons.len()
    }

    /// Returns the count for the current section.
    #[must_use]
    pub fn current_count(&self) -> usize {
        self.current_items().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_creation() {
        let selector = AddonManagerSelector::new();
        assert_eq!(selector.mode(), AddonManagerMode::List);
        assert_eq!(selector.section(), AddonListSection::Available);
        assert_eq!(selector.selected_index(), 0);
    }

    #[test]
    fn test_section_toggle() {
        let mut selector = AddonManagerSelector::new();
        assert_eq!(selector.section(), AddonListSection::Available);

        selector.toggle_section();
        assert_eq!(selector.section(), AddonListSection::Installed);

        selector.toggle_section();
        assert_eq!(selector.section(), AddonListSection::Available);
    }

    #[test]
    fn test_navigation() {
        let mut selector = AddonManagerSelector::new();

        // Add some test addons
        let addons = vec![
            Addon::new("addon1".to_string()),
            Addon::new("addon2".to_string()),
            Addon::new("addon3".to_string()),
        ];
        selector.set_available_addons(addons, &[]);

        assert_eq!(selector.selected_index(), 0);

        selector.select_next();
        assert_eq!(selector.selected_index(), 1);

        selector.select_next();
        assert_eq!(selector.selected_index(), 2);

        // Should not go beyond last item
        selector.select_next();
        assert_eq!(selector.selected_index(), 2);

        selector.select_prev();
        assert_eq!(selector.selected_index(), 1);

        selector.select_first();
        assert_eq!(selector.selected_index(), 0);

        selector.select_last();
        assert_eq!(selector.selected_index(), 2);
    }

    #[test]
    fn test_install_flow() {
        let mut selector = AddonManagerSelector::new();

        // Start install
        let progress = InstallProgress::new("test-addon".to_string());
        selector.set_install_progress(Some(progress));
        assert_eq!(selector.mode(), AddonManagerMode::Installing);
    }
}
