//! Docker manager operations.

use crate::docker::DockerDiscovery;
use crate::ui::docker_manager::{DockerItemDisplay, DockerManagerMode, DockerManagerSelector};
use crate::ui::popup::PopupKind;

use super::{App, AppMode};

impl App {
    /// Shows the Docker manager popup.
    pub fn show_docker_manager(&mut self) {
        // Initialize Docker manager if not already
        if self.docker_manager.is_none() {
            self.docker_manager = Some(DockerManagerSelector::new());
        }

        // Load Docker items if not loaded
        if !self.docker_storage.is_initialized() {
            match self.docker_storage.load() {
                Ok(items) => {
                    self.docker_items = items;
                }
                Err(e) => {
                    self.set_status(format!("Failed to load Docker settings: {}", e));
                }
            }
        }

        // Start discovery
        self.refresh_docker_discovery();

        // Show popup
        self.popup.set_kind(PopupKind::DockerManager);
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Hides the Docker manager popup.
    pub fn hide_docker_manager(&mut self) {
        self.popup.hide();
        self.mode = AppMode::Normal;
    }

    /// Refreshes Docker container/image discovery.
    pub fn refresh_docker_discovery(&mut self) {
        if let Some(ref mut manager) = self.docker_manager {
            manager.set_mode(DockerManagerMode::Discovering);
        }

        // Perform discovery
        let result = DockerDiscovery::discover_all();

        if let Some(ref mut manager) = self.docker_manager {
            manager.update_from_discovery(result);
            manager.set_mode(DockerManagerMode::List);
        }
    }

    /// Saves Docker items to storage.
    pub fn save_docker_items(&mut self) {
        if let Err(e) = self.docker_storage.save(&self.docker_items) {
            self.set_status(format!("Failed to save Docker settings: {}", e));
        }
    }

    /// Assigns a quick-connect slot to the selected Docker item.
    pub fn assign_docker_quick_connect(&mut self, slot: usize) {
        assert!(slot < 9, "slot must be 0-8");

        let Some(ref manager) = self.docker_manager else {
            return;
        };

        let Some(item) = manager.selected_item() else {
            self.set_status("No item selected".to_string());
            return;
        };

        use crate::docker::DockerQuickConnectItem;

        let qc_item = match item {
            DockerItemDisplay::Container(c) => DockerQuickConnectItem::from_container(&c),
            DockerItemDisplay::Image(i) => DockerQuickConnectItem::from_image(&i),
        };

        self.docker_items.set_quick_connect(slot, qc_item.clone());
        self.save_docker_items();
        self.set_status(format!(
            "Assigned {} to Ctrl+Alt+{}",
            qc_item.name,
            slot + 1
        ));
    }

    /// Connects to Docker quick-connect slot by index (0-8).
    pub fn docker_connect_by_index(&mut self, index: usize) {
        assert!(index < 9, "index must be 0-8");

        // Load Docker items if not loaded
        if !self.docker_storage.is_initialized() {
            match self.docker_storage.load() {
                Ok(items) => {
                    self.docker_items = items;
                }
                Err(e) => {
                    self.set_status(format!("Failed to load Docker settings: {}", e));
                    return;
                }
            }
        }

        let Some(qc_item) = self.docker_items.get_quick_connect(index) else {
            self.set_status(format!("No Docker item assigned to Ctrl+Alt+{}", index + 1));
            return;
        };

        let item_id = qc_item.id.clone();
        let item_name = qc_item.name.clone();
        let item_type = qc_item.item_type;

        use crate::docker::DockerItemType;

        match item_type {
            DockerItemType::RunningContainer => {
                self.exec_into_container(&item_id, &item_name);
            }
            DockerItemType::StoppedContainer => {
                // Start the container first, then exec
                self.start_and_exec_container(&item_id, &item_name);
            }
            DockerItemType::Image => {
                // Run the image as a new container
                self.run_image_interactive(&item_id, &item_name);
            }
        }
    }

    /// Returns the Docker manager selector.
    #[must_use]
    pub fn docker_manager(&self) -> Option<&DockerManagerSelector> {
        self.docker_manager.as_ref()
    }

    /// Returns mutable Docker manager selector.
    pub fn docker_manager_mut(&mut self) -> Option<&mut DockerManagerSelector> {
        self.docker_manager.as_mut()
    }

    /// Returns the default shell for Docker exec.
    #[must_use]
    pub fn docker_default_shell(&self) -> &str {
        &self.docker_items.default_shell
    }
}
