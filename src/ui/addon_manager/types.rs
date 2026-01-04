//! Add-on Manager types and enums.

use crate::addons::Addon;

/// Maximum number of addons to display in the list.
pub const MAX_DISPLAY_ADDONS: usize = 12;

/// Add-on Manager mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddonManagerMode {
    /// Viewing the addon list.
    #[default]
    List,
    /// Fetching addon list from GitHub.
    Fetching,
    /// Installing an addon (background).
    Installing,
    /// Confirming uninstall.
    ConfirmUninstall,
    /// Error display.
    Error,
}

impl AddonManagerMode {
    /// Returns a title for the current mode.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::List => "Add-ons Manager",
            Self::Fetching => "Fetching Add-ons...",
            Self::Installing => "Installing...",
            Self::ConfirmUninstall => "Confirm Uninstall",
            Self::Error => "Error",
        }
    }

    /// Returns true if this mode shows a form.
    #[must_use]
    pub fn is_form_mode(self) -> bool {
        matches!(self, Self::ConfirmUninstall)
    }

    /// Returns true if this mode shows a loading indicator.
    #[must_use]
    pub fn is_loading_mode(self) -> bool {
        matches!(self, Self::Fetching | Self::Installing)
    }
}

/// Addon list section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddonListSection {
    /// Available addons from GitHub.
    #[default]
    Available,
    /// Installed addons.
    Installed,
}

impl AddonListSection {
    /// Moves to the next section.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Available => Self::Installed,
            Self::Installed => Self::Available,
        }
    }

    /// Returns the display title for the section.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::Installed => "Installed",
        }
    }

    /// Returns a short label for the section.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => "[A]vailable",
            Self::Installed => "[I]nstalled",
        }
    }
}

/// Display information for an addon item.
#[derive(Debug, Clone)]
pub struct AddonDisplay {
    /// Addon information.
    pub addon: Addon,
    /// Whether this addon is installed via our system.
    pub is_installed: bool,
    /// Whether this technology is already on the system (detected via PATH).
    pub is_system_installed: bool,
    /// Path where the technology was found (if detected).
    pub system_path: Option<String>,
}

impl AddonDisplay {
    /// Creates a new addon display for an available addon.
    #[must_use]
    pub fn available(addon: Addon) -> Self {
        Self {
            addon,
            is_installed: false,
            is_system_installed: false,
            system_path: None,
        }
    }

    /// Creates a new addon display for an available addon that's already on the system.
    #[must_use]
    pub fn available_with_detection(addon: Addon, system_path: Option<String>) -> Self {
        Self {
            is_system_installed: system_path.is_some(),
            system_path,
            addon,
            is_installed: false,
        }
    }

    /// Creates a new addon display for an installed addon.
    #[must_use]
    pub fn installed(addon: Addon) -> Self {
        Self {
            addon,
            is_installed: true,
            is_system_installed: false,
            system_path: None,
        }
    }

    /// Returns a status indicator string.
    #[must_use]
    pub fn status_indicator(&self) -> &'static str {
        if self.is_installed {
            "[+]"  // Installed via addon system
        } else if self.is_system_installed {
            "[*]"  // Already on system
        } else if self.addon.is_installable() {
            "[ ]"  // Available to install
        } else {
            "[x]"  // Not available for platform
        }
    }

    /// Returns the display name.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.addon.name
    }

    /// Returns a summary of the addon.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_installed {
            self.addon.description.clone()
        } else if self.is_system_installed {
            if let Some(ref path) = self.system_path {
                format!("Already installed: {}", path)
            } else {
                "Already installed on system".to_string()
            }
        } else if !self.addon.is_installable() {
            format!("{} (not available for this platform)", self.addon.description)
        } else {
            self.addon.description.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addon_manager_mode() {
        assert_eq!(AddonManagerMode::List.title(), "Add-ons Manager");
        assert!(AddonManagerMode::Fetching.is_loading_mode());
        assert!(AddonManagerMode::ConfirmUninstall.is_form_mode());
        assert!(!AddonManagerMode::List.is_form_mode());
    }

    #[test]
    fn test_addon_list_section() {
        assert_eq!(AddonListSection::Available.next(), AddonListSection::Installed);
        assert_eq!(AddonListSection::Installed.next(), AddonListSection::Available);
    }

    #[test]
    fn test_addon_display() {
        let addon = Addon::new("test".to_string())
            .with_description("Test addon".to_string())
            .with_install(true);

        let available = AddonDisplay::available(addon.clone());
        assert!(!available.is_installed);
        assert_eq!(available.status_indicator(), "[ ]");

        let installed = AddonDisplay::installed(addon);
        assert!(installed.is_installed);
        assert_eq!(installed.status_indicator(), "[+]");
    }
}
