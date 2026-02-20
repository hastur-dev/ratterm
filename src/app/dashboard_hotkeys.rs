//! Hotkey entry definitions for each dashboard mode.
//!
//! Each function returns the complete list of available hotkeys for
//! a specific dashboard screen, enumerated from the actual input handlers.

use crate::ui::hotkey_overlay::HotkeyEntry;

/// Returns hotkey entries for the Health Dashboard overview mode.
#[must_use]
pub fn health_dashboard_overview_hotkeys() -> Vec<HotkeyEntry> {
    vec![
        // Navigation (from unified nav layer)
        HotkeyEntry {
            key: "Up/Down or j/k",
            description: "Navigate host list",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Home/End",
            description: "Jump to first/last host",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Enter",
            description: "View host details",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Esc",
            description: "Close dashboard",
            category: "Navigation",
        },
        // Actions
        HotkeyEntry {
            key: "r",
            description: "Refresh metrics",
            category: "Actions",
        },
        HotkeyEntry {
            key: "Space",
            description: "Toggle auto-refresh",
            category: "Actions",
        },
        HotkeyEntry {
            key: "q",
            description: "Close dashboard",
            category: "Actions",
        },
        // Help
        HotkeyEntry {
            key: "?",
            description: "Toggle this help",
            category: "Help",
        },
    ]
}

/// Returns hotkey entries for the Health Dashboard detail mode.
#[must_use]
pub fn health_dashboard_detail_hotkeys() -> Vec<HotkeyEntry> {
    vec![
        HotkeyEntry {
            key: "Backspace/Esc",
            description: "Back to overview",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "r",
            description: "Refresh metrics",
            category: "Actions",
        },
        HotkeyEntry {
            key: "q",
            description: "Close dashboard",
            category: "Actions",
        },
        HotkeyEntry {
            key: "?",
            description: "Toggle this help",
            category: "Help",
        },
    ]
}

/// Returns hotkey entries for the Docker Manager list mode.
#[must_use]
pub fn docker_manager_list_hotkeys() -> Vec<HotkeyEntry> {
    vec![
        // Navigation
        HotkeyEntry {
            key: "Up/Down or j/k",
            description: "Navigate list",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Home/End",
            description: "Jump to first/last",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "g/G",
            description: "Jump to first/last (vim)",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Tab/Shift+Tab",
            description: "Switch section",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Enter",
            description: "Attach/start/run",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Esc",
            description: "Close manager",
            category: "Navigation",
        },
        // Actions
        HotkeyEntry {
            key: "r",
            description: "Refresh containers",
            category: "Actions",
        },
        HotkeyEntry {
            key: "c",
            description: "Create new container",
            category: "Actions",
        },
        HotkeyEntry {
            key: "d/Delete",
            description: "Remove selected",
            category: "Actions",
        },
        HotkeyEntry {
            key: "h",
            description: "Host selection",
            category: "Actions",
        },
        HotkeyEntry {
            key: "Ctrl+O",
            description: "Run with options",
            category: "Actions",
        },
        // Sections
        HotkeyEntry {
            key: "Shift+R",
            description: "Running containers",
            category: "Sections",
        },
        HotkeyEntry {
            key: "Shift+S",
            description: "Stopped containers",
            category: "Sections",
        },
        HotkeyEntry {
            key: "Shift+I",
            description: "Images",
            category: "Sections",
        },
        // Quick connect
        HotkeyEntry {
            key: "1-9",
            description: "Assign quick connect",
            category: "Quick Connect",
        },
        // Help
        HotkeyEntry {
            key: "?",
            description: "Toggle this help",
            category: "Help",
        },
    ]
}

/// Returns hotkey entries for the SSH Manager list mode.
#[must_use]
pub fn ssh_manager_list_hotkeys() -> Vec<HotkeyEntry> {
    vec![
        // Navigation
        HotkeyEntry {
            key: "Up/Down or j/k",
            description: "Navigate host list",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Home/End",
            description: "Jump to first/last host",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Enter",
            description: "Connect to host",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Esc",
            description: "Close manager",
            category: "Navigation",
        },
        // Actions
        HotkeyEntry {
            key: "a/A",
            description: "Add host",
            category: "Actions",
        },
        HotkeyEntry {
            key: "e",
            description: "Edit host name",
            category: "Actions",
        },
        HotkeyEntry {
            key: "d/D/Delete",
            description: "Delete host",
            category: "Actions",
        },
        HotkeyEntry {
            key: "s",
            description: "Scan network",
            category: "Actions",
        },
        HotkeyEntry {
            key: "Shift+S",
            description: "Scan specific subnet",
            category: "Actions",
        },
        HotkeyEntry {
            key: "c",
            description: "Credential scan",
            category: "Actions",
        },
        HotkeyEntry {
            key: "h",
            description: "Health dashboard",
            category: "Actions",
        },
        // Help
        HotkeyEntry {
            key: "?",
            description: "Toggle this help",
            category: "Help",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_dashboard_hotkeys_include_navigation() {
        let hotkeys = health_dashboard_overview_hotkeys();
        assert!(
            hotkeys
                .iter()
                .any(|h| h.key.contains("Up") || h.key.contains("Down")),
            "Should include Up/Down navigation"
        );
        assert!(
            hotkeys.iter().any(|h| h.key == "?"),
            "Should include help key"
        );
    }

    #[test]
    fn test_docker_hotkeys_include_navigation() {
        let hotkeys = docker_manager_list_hotkeys();
        assert!(
            hotkeys
                .iter()
                .any(|h| h.key.contains("Up") || h.key.contains("Down")),
            "Should include Up/Down navigation"
        );
        assert!(
            hotkeys.iter().any(|h| h.key == "?"),
            "Should include help key"
        );
    }

    #[test]
    fn test_ssh_hotkeys_include_navigation() {
        let hotkeys = ssh_manager_list_hotkeys();
        assert!(
            hotkeys
                .iter()
                .any(|h| h.key.contains("Up") || h.key.contains("Down")),
            "Should include Up/Down navigation"
        );
        assert!(
            hotkeys.iter().any(|h| h.key == "?"),
            "Should include help key"
        );
    }

    #[test]
    fn test_all_hotkey_sets_non_empty() {
        assert!(!health_dashboard_overview_hotkeys().is_empty());
        assert!(!health_dashboard_detail_hotkeys().is_empty());
        assert!(!docker_manager_list_hotkeys().is_empty());
        assert!(!ssh_manager_list_hotkeys().is_empty());
    }

    #[test]
    fn test_hotkey_entries_have_categories() {
        for entry in health_dashboard_overview_hotkeys() {
            assert!(!entry.category.is_empty(), "Category must not be empty");
            assert!(!entry.key.is_empty(), "Key must not be empty");
            assert!(
                !entry.description.is_empty(),
                "Description must not be empty"
            );
        }
    }
}
