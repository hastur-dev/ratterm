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

/// Returns hotkey entries for the Git Dashboard.
#[must_use]
pub fn git_dashboard_hotkeys() -> Vec<HotkeyEntry> {
    vec![
        // Navigation
        HotkeyEntry {
            key: "Up/Down or j/k",
            description: "Navigate file list",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Home/End",
            description: "Jump to first/last",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Tab/Shift+Tab",
            description: "Switch section (staged/unstaged/untracked)",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Enter",
            description: "View diff for selected file",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Backspace",
            description: "Back to status view",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Esc",
            description: "Close dashboard",
            category: "Navigation",
        },
        // Actions
        HotkeyEntry {
            key: "s",
            description: "Stage selected file",
            category: "Actions",
        },
        HotkeyEntry {
            key: "u",
            description: "Unstage selected file",
            category: "Actions",
        },
        HotkeyEntry {
            key: "c",
            description: "Commit staged changes",
            category: "Actions",
        },
        HotkeyEntry {
            key: "r",
            description: "Refresh status",
            category: "Actions",
        },
        HotkeyEntry {
            key: "p",
            description: "Stash pop",
            category: "Actions",
        },
        HotkeyEntry {
            key: "Shift+P",
            description: "Stash push",
            category: "Actions",
        },
        // Views
        HotkeyEntry {
            key: "b",
            description: "Branch list view",
            category: "Views",
        },
        HotkeyEntry {
            key: "l",
            description: "Commit log view",
            category: "Views",
        },
        HotkeyEntry {
            key: "d",
            description: "Diff view",
            category: "Views",
        },
        HotkeyEntry {
            key: "Ctrl+B",
            description: "Toggle blame view",
            category: "Views",
        },
        // Help
        HotkeyEntry {
            key: "?",
            description: "Toggle this help",
            category: "Help",
        },
    ]
}

/// Returns hotkey entries for the Debugger.
#[must_use]
pub fn debugger_hotkeys() -> Vec<HotkeyEntry> {
    vec![
        // Session control
        HotkeyEntry {
            key: "F5",
            description: "Continue / Start debugging",
            category: "Session",
        },
        HotkeyEntry {
            key: "Shift+F5",
            description: "Stop debugging",
            category: "Session",
        },
        HotkeyEntry {
            key: "Ctrl+Shift+F5",
            description: "Restart debugging",
            category: "Session",
        },
        // Stepping
        HotkeyEntry {
            key: "F9",
            description: "Toggle breakpoint",
            category: "Breakpoints",
        },
        HotkeyEntry {
            key: "F10",
            description: "Step over",
            category: "Stepping",
        },
        HotkeyEntry {
            key: "F11",
            description: "Step into",
            category: "Stepping",
        },
        HotkeyEntry {
            key: "Shift+F11",
            description: "Step out",
            category: "Stepping",
        },
        // Panel
        HotkeyEntry {
            key: "Tab",
            description: "Switch debug panel tab",
            category: "Panel",
        },
        HotkeyEntry {
            key: "Up/Down or j/k",
            description: "Navigate list",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Enter",
            description: "Expand/select",
            category: "Navigation",
        },
        // Help
        HotkeyEntry {
            key: "?",
            description: "Toggle this help",
            category: "Help",
        },
    ]
}

/// Returns hotkey entries for the Docker Logs viewer.
#[must_use]
pub fn docker_logs_hotkeys() -> Vec<HotkeyEntry> {
    vec![
        // Navigation
        HotkeyEntry {
            key: "Up/Down or j/k",
            description: "Scroll logs / navigate list",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Home/End",
            description: "Jump to top/bottom",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "PgUp/PgDn",
            description: "Page scroll",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Enter",
            description: "Select container",
            category: "Navigation",
        },
        HotkeyEntry {
            key: "Esc/q",
            description: "Back / close",
            category: "Navigation",
        },
        // Streaming
        HotkeyEntry {
            key: "Space",
            description: "Pause/resume stream",
            category: "Streaming",
        },
        // Search
        HotkeyEntry {
            key: "/ or Ctrl+F",
            description: "Search/filter logs",
            category: "Search",
        },
        HotkeyEntry {
            key: "Shift+S",
            description: "Saved searches",
            category: "Search",
        },
        // Actions
        HotkeyEntry {
            key: "c",
            description: "Clear logs",
            category: "Actions",
        },
        HotkeyEntry {
            key: "t",
            description: "Toggle timestamps",
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
        assert!(!git_dashboard_hotkeys().is_empty());
    }

    #[test]
    fn test_git_hotkeys_include_navigation() {
        let hotkeys = git_dashboard_hotkeys();
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
    fn test_git_hotkeys_include_stage_unstage() {
        let hotkeys = git_dashboard_hotkeys();
        assert!(
            hotkeys.iter().any(|h| h.key == "s"),
            "Should include stage key"
        );
        assert!(
            hotkeys.iter().any(|h| h.key == "u"),
            "Should include unstage key"
        );
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
