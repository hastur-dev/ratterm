//! Quick-connect slots and the persisted Docker item list.
//!
//! Split out of `container.rs`. The list is what `docker_items.toml` holds:
//! per-host hotkey assignments plus a few display preferences.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::container::{DockerContainer, DockerImage, DockerItemType};
use super::host::DockerHost;

/// Maximum quick-connect slots (Ctrl+Alt+1-9).
pub const MAX_QUICK_CONNECT: usize = 9;

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
            self.host_quick_connect
                .insert("local".to_string(), migrated);
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
            self.current_host_slots().and_then(|slots| slots.get(&key))
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn container(id: &str, name: &str) -> DockerContainer {
        DockerContainer::new(
            id.to_string(),
            name.to_string(),
            "myimage".to_string(),
            "Up".to_string(),
        )
    }

    #[test]
    fn test_quick_connect() {
        let mut list = DockerItemList::new();

        let item = DockerQuickConnectItem::from_container(&container("abc123", "my-app"));
        assert!(list.set_quick_connect(0, item.clone()));
        assert_eq!(list.quick_connect_count(), 1);

        let retrieved = list.get_quick_connect(0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "abc123");

        assert_eq!(list.find_quick_connect_for_id("abc123"), Some(0));
        assert_eq!(list.find_quick_connect_for_id("xyz"), None);
    }

    #[test]
    fn a_slot_past_the_last_hotkey_is_refused() {
        let mut list = DockerItemList::new();
        let item = DockerQuickConnectItem::from_container(&container("abc", "app"));
        assert!(!list.set_quick_connect(MAX_QUICK_CONNECT, item));
        assert!(!list.remove_quick_connect(MAX_QUICK_CONNECT));
        assert!(list.get_quick_connect(MAX_QUICK_CONNECT).is_none());
        assert_eq!(list.quick_connect_count(), 0);
    }

    #[test]
    fn removing_a_slot_leaves_the_others_alone() {
        let mut list = DockerItemList::new();
        list.set_quick_connect(
            0,
            DockerQuickConnectItem::from_container(&container("a", "first")),
        );
        list.set_quick_connect(
            1,
            DockerQuickConnectItem::from_container(&container("b", "second")),
        );
        assert!(list.remove_quick_connect(0));
        assert!(list.get_quick_connect(0).is_none());
        assert_eq!(list.get_quick_connect(1).unwrap().id, "b");
    }

    #[test]
    fn a_legacy_file_migrates_its_slots_to_the_local_host() {
        let mut list = DockerItemList::new();
        list.quick_connect[2] = Some(DockerQuickConnectItem::from_container(&container(
            "legacy", "old-app",
        )));
        list.migrate_legacy_quick_connect();

        assert_eq!(
            list.get_quick_connect(2).map(|i| i.id.as_str()),
            Some("legacy")
        );
        assert!(
            list.quick_connect.iter().all(Option::is_none),
            "the legacy array is cleared once migrated"
        );
    }

    #[test]
    fn migration_does_not_overwrite_slots_that_already_exist() {
        let mut list = DockerItemList::new();
        list.set_quick_connect(
            0,
            DockerQuickConnectItem::from_container(&container("current", "app")),
        );
        list.quick_connect[0] = Some(DockerQuickConnectItem::from_container(&container(
            "legacy", "old",
        )));
        list.migrate_legacy_quick_connect();
        assert_eq!(
            list.get_quick_connect(0).map(|i| i.id.as_str()),
            Some("current")
        );
    }

    #[test]
    fn test_per_host_quick_connect() {
        let mut list = DockerItemList::new();

        // Set quick-connect on local host
        list.set_selected_host(DockerHost::Local);
        let item1 = DockerQuickConnectItem::from_container(&container("local123", "local-app"));
        list.set_quick_connect(0, item1);
        assert_eq!(list.quick_connect_count(), 1);
        assert_eq!(list.get_quick_connect(0).unwrap().id, "local123");

        // Switch to remote host
        let remote_host = DockerHost::remote(1);
        list.set_selected_host(remote_host.clone());

        // Remote should have no quick-connect yet
        assert_eq!(list.quick_connect_count(), 0);
        assert!(list.get_quick_connect(0).is_none());

        // Set quick-connect on remote host
        let item2 = DockerQuickConnectItem::from_container(&container("remote456", "remote-app"));
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

    #[test]
    fn a_slot_can_be_read_for_a_host_that_is_not_selected() {
        let mut list = DockerItemList::new();
        list.set_selected_host(DockerHost::remote(2));
        list.set_quick_connect(
            3,
            DockerQuickConnectItem::from_container(&container("r2", "on-two")),
        );
        list.set_selected_host(DockerHost::Local);

        let found = list.get_quick_connect_for_host(&DockerHost::remote(2), 3);
        assert_eq!(found.map(|i| i.id.as_str()), Some("r2"));
        assert!(
            list.get_quick_connect_for_host(&DockerHost::remote(2), MAX_QUICK_CONNECT)
                .is_none()
        );
    }

    #[test]
    fn an_image_slot_records_the_image_type() {
        let image = DockerImage::new(
            "sha256:abc".to_string(),
            "nginx".to_string(),
            "latest".to_string(),
        );
        let item = DockerQuickConnectItem::from_image(&image);
        assert_eq!(item.item_type, DockerItemType::Image);
        assert_eq!(item.name, "nginx:latest");
    }
}
