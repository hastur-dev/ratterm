//! Extension management operations for the App.

use tracing::{info, warn};

use crate::extension::ExtensionManager;
use crate::ui::popup::{ExtensionApprovalPrompt, PopupKind};

use super::{App, AppMode};

impl App {
    /// Initializes extensions and discovers installed ones.
    ///
    /// Extensions that are approved will be noted for startup.
    /// Extensions that need approval will queue an approval popup.
    pub fn init_extensions(&mut self) {
        if let Err(e) = self.extension_manager.init() {
            warn!("Failed to initialize extension manager: {}", e);
            return;
        }

        let pending = self.extension_manager.pending_approval();
        let approved = self.extension_manager.approved_extensions();

        info!(
            "Extensions: {} installed, {} approved, {} pending approval",
            self.extension_manager.count(),
            approved.len(),
            pending.len()
        );

        // Show approval popup for first pending extension
        self.show_next_extension_approval();
    }

    /// Shows the approval popup for the next pending extension.
    fn show_next_extension_approval(&mut self) {
        let pending = self.extension_manager.pending_approval();

        if let Some(ext) = pending.first() {
            let prompt = ExtensionApprovalPrompt::new(
                ext.name.clone(),
                ext.version.clone(),
                ext.author().map(String::from),
                ext.description().map(String::from),
                ext.command().unwrap_or("unknown").to_string(),
            );

            self.extension_approval_prompt = Some(prompt);
            self.popup.set_kind(PopupKind::ExtensionApproval);
            self.popup.show();
            self.mode = AppMode::Popup;
        }
    }

    /// Returns the current extension approval prompt if any.
    #[must_use]
    pub fn extension_approval_prompt(&self) -> Option<&ExtensionApprovalPrompt> {
        self.extension_approval_prompt.as_ref()
    }

    /// Handles extension approval response from the user.
    ///
    /// If approved, the extension is marked as approved and can run.
    /// If denied, the extension is skipped.
    /// Then shows the next pending approval if any.
    pub fn handle_extension_approval(&mut self, approved: bool) {
        if let Some(ref prompt) = self.extension_approval_prompt {
            let name = prompt.name().to_string();

            if approved {
                match self.extension_manager.approve(&name) {
                    Ok(()) => {
                        info!("Extension approved: {}", name);
                        self.set_status(format!("Extension '{}' approved", name));
                    }
                    Err(e) => {
                        warn!("Failed to approve extension {}: {}", name, e);
                        self.set_status(format!("Failed to approve '{}': {}", name, e));
                    }
                }
            } else {
                info!("Extension denied: {}", name);
                self.set_status(format!("Extension '{}' denied", name));
            }
        }

        // Clear current prompt
        self.extension_approval_prompt = None;
        self.popup.hide();
        self.mode = AppMode::Normal;

        // Show next pending approval if any
        self.show_next_extension_approval();
    }

    /// Returns the extension manager.
    #[must_use]
    pub fn extension_manager(&self) -> &ExtensionManager {
        &self.extension_manager
    }

    /// Returns a mutable reference to the extension manager.
    pub fn extension_manager_mut(&mut self) -> &mut ExtensionManager {
        &mut self.extension_manager
    }

    /// Shows installed extensions in the status bar.
    pub fn show_installed_extensions(&mut self) {
        let mut manager = ExtensionManager::new();
        if let Err(e) = manager.init() {
            self.set_status(format!("Failed to load extensions: {}", e));
            return;
        }

        let extensions = manager.installed();
        if extensions.is_empty() {
            self.set_status(
                "No extensions installed. Use: rat ext install <user/repo>".to_string(),
            );
        } else {
            let names: Vec<_> = extensions
                .values()
                .map(|e| format!("{} v{}", e.name, e.version))
                .collect();
            self.set_status(format!("Extensions: {}", names.join(", ")));
        }
    }
}
