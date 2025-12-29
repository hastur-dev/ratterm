//! Popup and dialog operations for the App.

use crate::config::ShellType;
use crate::theme::ThemePreset;
use crate::ui::{
    layout::FocusedPane,
    popup::{ModeSwitcher, PopupKind, ShellInstallPrompt, ShellSelector, ThemeSelector},
};

use super::{App, AppMode};

impl App {
    /// Shows a popup dialog.
    pub fn show_popup(&mut self, kind: PopupKind) {
        self.popup.set_kind(kind);
        self.popup.clear();

        if matches!(kind, PopupKind::CreateFile) {
            if let Some(ext) = self.file_browser.common_extension() {
                self.popup.set_suggestion(Some(format!(".{}", ext)));
            }
        }

        // Initialize command palette with all commands
        if matches!(kind, PopupKind::CommandPalette) {
            self.command_palette.filter("");
            self.popup.set_results(self.command_palette.results());
        }

        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Hides the popup.
    pub fn hide_popup(&mut self) {
        self.popup.hide();
        self.mode_switcher = None;
        self.shell_selector = None;
        self.shell_install_prompt = None;
        self.mode = if self.file_browser.is_visible() {
            AppMode::FileBrowser
        } else {
            AppMode::Normal
        };
    }

    /// Shows the mode switcher popup.
    pub fn show_mode_switcher(&mut self) {
        self.mode_switcher = Some(ModeSwitcher::new(self.config.mode));
        self.popup.set_kind(PopupKind::ModeSwitcher);
        self.popup.clear();
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Cycles to the next editor mode in the mode switcher.
    pub fn cycle_mode_next(&mut self) {
        if let Some(ref mut switcher) = self.mode_switcher {
            switcher.next();
        }
    }

    /// Cycles to the previous editor mode in the mode switcher.
    pub fn cycle_mode_prev(&mut self) {
        if let Some(ref mut switcher) = self.mode_switcher {
            switcher.prev();
        }
    }

    /// Applies the selected mode from the mode switcher and closes it.
    pub fn apply_mode_switch(&mut self) {
        if let Some(ref switcher) = self.mode_switcher {
            let new_mode = switcher.selected_mode();
            self.config.mode = new_mode;
            self.set_status(format!(
                "Switched to {} mode",
                ModeSwitcher::mode_name(new_mode)
            ));
        }
        self.hide_popup();
    }

    /// Cancels the mode switch and reverts to the original mode.
    pub fn cancel_mode_switch(&mut self) {
        self.hide_popup();
    }

    /// Returns true if the mode switcher is currently active.
    #[must_use]
    pub fn is_mode_switcher_active(&self) -> bool {
        self.mode_switcher.is_some() && self.popup.kind().is_mode_switcher()
    }

    /// Shows the shell selector popup.
    pub fn show_shell_selector(&mut self) {
        self.shell_selector = Some(ShellSelector::new(self.config.shell));
        self.popup.set_kind(PopupKind::ShellSelector);
        self.popup.clear();
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Cycles to the next shell in the shell selector.
    pub fn cycle_shell_next(&mut self) {
        if let Some(ref mut selector) = self.shell_selector {
            selector.next();
        }
    }

    /// Cycles to the previous shell in the shell selector.
    pub fn cycle_shell_prev(&mut self) {
        if let Some(ref mut selector) = self.shell_selector {
            selector.prev();
        }
    }

    /// Applies the selected shell from the selector.
    /// If the shell is not available, shows the install prompt instead.
    /// Automatically creates a new tab with the selected shell.
    pub fn apply_shell_selection(&mut self) {
        if let Some(ref selector) = self.shell_selector {
            let selected_shell = selector.selected_shell();

            if !selector.is_selected_available() {
                // Shell not available - show install prompt
                self.shell_install_prompt = Some(ShellInstallPrompt::new(selected_shell));
                self.popup.set_kind(PopupKind::ShellInstallPrompt);
                self.shell_selector = None;
                return;
            }

            // Shell is available - apply the selection
            self.config.shell = selected_shell;

            // Close old tabs if configured
            if self.config.auto_close_tabs_on_shell_change {
                self.close_all_terminal_tabs();
            }

            // Hide popup first so we can create new tab
            self.shell_selector = None;
            self.popup.hide();
            self.mode = AppMode::Normal;

            // Create new tab with the selected shell
            self.add_terminal_tab();

            // Focus terminal pane
            self.layout.set_focused(FocusedPane::Terminal);

            return;
        }
        self.hide_popup();
    }

    /// Closes all terminal tabs except one, then closes the remaining one's shell.
    pub(crate) fn close_all_terminal_tabs(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            while terminals.tab_count() > 1 {
                terminals.close_tab();
            }
        }
    }

    /// Cancels the shell selection.
    pub fn cancel_shell_selection(&mut self) {
        self.hide_popup();
    }

    /// Returns true if the shell selector is currently active.
    #[must_use]
    pub fn is_shell_selector_active(&self) -> bool {
        self.shell_selector.is_some() && self.popup.kind().is_shell_selector()
    }

    /// Returns true if the shell install prompt is currently active.
    #[must_use]
    pub fn is_shell_install_prompt_active(&self) -> bool {
        self.shell_install_prompt.is_some() && self.popup.kind().is_shell_install_prompt()
    }

    /// Returns the current shell configuration.
    #[must_use]
    pub fn current_shell(&self) -> ShellType {
        self.config.shell
    }

    /// Shows the theme selector popup.
    pub fn show_theme_selector(&mut self) {
        let current_name = self.current_theme_name();
        let all_themes = self.available_themes();
        self.theme_selector = Some(ThemeSelector::new_with_themes(&current_name, all_themes));
        self.popup.set_kind(PopupKind::ThemeSelector);
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Applies the selected theme.
    pub fn apply_theme_selection(&mut self) {
        if let Some(ref selector) = self.theme_selector {
            let selected_name = selector.selected_theme_name().to_string();

            if let Err(e) = self.set_theme_by_name(&selected_name) {
                self.set_status(format!("Failed to set theme: {}", e));
            }
        }
        self.theme_selector = None;
        self.hide_popup();
    }

    /// Cancels the theme selection.
    pub fn cancel_theme_selection(&mut self) {
        self.theme_selector = None;
        self.hide_popup();
    }

    /// Returns true if the theme selector is currently active.
    #[must_use]
    pub fn is_theme_selector_active(&self) -> bool {
        self.theme_selector.is_some() && self.popup.kind().is_theme_selector()
    }

    /// Returns the current theme name.
    #[must_use]
    pub fn current_theme_name(&self) -> String {
        self.config.theme_manager.current().name().to_string()
    }

    /// Returns the current theme preset, if using one.
    #[must_use]
    pub fn current_theme_preset(&self) -> Option<ThemePreset> {
        self.config.theme_manager.current_preset()
    }

    /// Returns all available theme names.
    #[must_use]
    pub fn available_themes(&self) -> Vec<String> {
        self.config.theme_manager.all_available_themes()
    }

    /// Sets the theme to a specific preset.
    pub fn set_theme(&mut self, preset: ThemePreset) {
        self.config.theme_manager.set_preset(preset);
        if let Err(e) = self.config.save_theme() {
            self.set_status(format!("Failed to save theme: {}", e));
        } else {
            self.set_status(format!("Theme changed to: {}", preset.name()));
        }
    }

    /// Sets the theme by name, supporting both presets and custom themes.
    pub fn set_theme_by_name(&mut self, name: &str) -> Result<(), String> {
        // First try preset themes
        if let Some(preset) = ThemePreset::from_name(name) {
            self.set_theme(preset);
            return Ok(());
        }

        // Try custom themes
        let custom_themes = crate::theme::list_custom_theme_info();
        for info in custom_themes {
            if info.name == name {
                match self.config.theme_manager.load_custom_theme(&info.path) {
                    Ok(()) => {
                        self.set_status(format!("Theme changed to: {}", name));
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(format!("Failed to load custom theme: {}", e));
                    }
                }
            }
        }

        Err(format!("Unknown theme: {}", name))
    }

    /// Updates popup results based on current input.
    pub(crate) fn update_popup_results(&mut self) {
        let input = self.popup.input().to_string();

        let results: Vec<String> = match self.popup.kind() {
            PopupKind::SearchFiles => self
                .file_browser
                .search_files(&input)
                .into_iter()
                .take(10)
                .map(|e| e.name().to_string())
                .collect(),
            PopupKind::SearchDirectories => self
                .file_browser
                .search_directories(&input)
                .into_iter()
                .take(10)
                .map(|e| e.name().to_string())
                .collect(),
            PopupKind::CommandPalette => {
                self.command_palette.filter(&input);
                self.command_palette.results()
            }
            _ => Vec::new(),
        };

        self.popup.set_results(results);
    }

    /// Executes the popup action.
    pub(crate) fn execute_popup_action(&mut self) {
        let input = self.popup.final_input();

        match self.popup.kind() {
            PopupKind::SearchInFile => {
                self.set_status(format!("Searching for: {}", input));
                self.hide_popup();
            }
            PopupKind::SearchInFiles => {
                self.set_status(format!("Searching all files for: {}", input));
                self.hide_popup();
            }
            PopupKind::SearchFiles | PopupKind::SearchDirectories => {
                if let Some(result) = self.popup.selected_result() {
                    let path = self.file_browser.path().join(result);
                    if path.is_file() {
                        let _ = self.open_file(path);
                    } else if path.is_dir() {
                        let _ = self.file_browser.change_dir(&path);
                        self.show_file_browser();
                    }
                }
                self.hide_popup();
            }
            PopupKind::CreateFile => {
                if !input.is_empty() {
                    let path = self.file_browser.path().join(&input);
                    match std::fs::write(&path, "") {
                        Ok(()) => {
                            let _ = self.file_browser.refresh();
                            let _ = self.open_file(path);
                        }
                        Err(e) => {
                            self.popup.set_error(Some(format!("Error: {}", e)));
                            return;
                        }
                    }
                }
                self.hide_popup();
            }
            PopupKind::CreateFolder => {
                if !input.is_empty() {
                    let path = self.file_browser.path().join(&input);
                    match std::fs::create_dir(&path) {
                        Ok(()) => {
                            let _ = self.file_browser.refresh();
                            self.set_status(format!("Created folder: {}", path.display()));
                        }
                        Err(e) => {
                            self.popup.set_error(Some(format!("Error: {}", e)));
                            return;
                        }
                    }
                }
                self.hide_popup();
            }
            PopupKind::ConfirmSaveBeforeExit => {
                self.hide_popup();
            }
            PopupKind::CommandPalette => {
                let selected_idx = self
                    .popup
                    .results()
                    .iter()
                    .position(|r| self.popup.selected_result() == Some(r))
                    .unwrap_or(0);

                if let Some(cmd) = self.command_palette.get_command(selected_idx) {
                    let cmd_id = cmd.id.to_string();
                    self.hide_popup();
                    self.execute_command(&cmd_id);
                } else {
                    self.hide_popup();
                }
            }
            PopupKind::ModeSwitcher => {
                self.apply_mode_switch();
            }
            PopupKind::ShellSelector => {
                self.apply_shell_selection();
            }
            PopupKind::ShellInstallPrompt => {
                self.hide_popup();
            }
            PopupKind::ThemeSelector => {
                self.apply_theme_selection();
            }
            PopupKind::ExtensionApproval => {
                self.handle_extension_approval(true);
            }
            PopupKind::SSHManager
            | PopupKind::SSHCredentialPrompt
            | PopupKind::SSHStorageSetup
            | PopupKind::SSHMasterPassword
            | PopupKind::SSHSubnetEntry => {
                self.hide_popup();
            }
        }
    }
}
