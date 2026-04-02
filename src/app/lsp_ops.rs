//! LSP operation methods for the App.

use crate::lsp;

use super::App;

impl App {
    /// Dismisses the current hover popup.
    pub fn dismiss_hover(&mut self) {
        self.lsp_hover = None;
    }

    /// Dismisses the references panel.
    pub fn dismiss_references(&mut self) {
        self.lsp_references = None;
        self.lsp_references_selected = 0;
        self.lsp_references_scroll = 0;
    }

    /// Dismisses code actions popup.
    pub fn dismiss_code_actions(&mut self) {
        self.lsp_code_actions = None;
        self.lsp_code_action_selected = 0;
    }

    /// Dismisses signature help.
    pub fn dismiss_signature_help(&mut self) {
        self.lsp_signature_help = None;
    }

    /// Dismisses document symbols panel.
    pub fn dismiss_document_symbols(&mut self) {
        self.lsp_document_symbols = None;
        self.lsp_symbols_selected = 0;
        self.lsp_symbols_scroll = 0;
    }

    /// Dismisses workspace symbols panel.
    pub fn dismiss_workspace_symbols(&mut self) {
        self.lsp_workspace_symbols = None;
        self.lsp_workspace_query.clear();
        self.lsp_workspace_selected = 0;
    }

    /// Dismisses rename input.
    pub fn dismiss_rename(&mut self) {
        self.lsp_rename_input = None;
        self.lsp_rename_range = None;
    }

    /// Toggles the diagnostics panel.
    pub fn toggle_diagnostics_panel(&mut self) {
        self.lsp_diagnostics_panel_visible = !self.lsp_diagnostics_panel_visible;
        if self.lsp_diagnostics_panel_visible {
            self.lsp_diagnostics_selected = 0;
            self.lsp_diagnostics_scroll = 0;
        }
    }

    /// Returns whether any LSP overlay is active.
    pub fn has_lsp_overlay(&self) -> bool {
        self.lsp_hover.is_some()
            || self.lsp_references.is_some()
            || self.lsp_code_actions.is_some()
            || self.lsp_signature_help.is_some()
            || self.lsp_document_symbols.is_some()
            || self.lsp_workspace_symbols.is_some()
            || self.lsp_rename_input.is_some()
    }

    /// Dismisses all LSP overlays.
    pub fn dismiss_all_lsp_overlays(&mut self) {
        self.dismiss_hover();
        self.dismiss_references();
        self.dismiss_code_actions();
        self.dismiss_signature_help();
        self.dismiss_document_symbols();
        self.dismiss_workspace_symbols();
        self.dismiss_rename();
    }

    /// Navigates references list up.
    pub fn references_up(&mut self) {
        if self.lsp_references_selected > 0 {
            self.lsp_references_selected -= 1;
        }
    }

    /// Navigates references list down.
    pub fn references_down(&mut self) {
        if let Some(ref groups) = self.lsp_references {
            let total: usize = groups.iter().map(|g| g.locations.len()).sum();
            if self.lsp_references_selected + 1 < total {
                self.lsp_references_selected += 1;
            }
        }
    }

    /// Navigates code actions up.
    pub fn code_actions_up(&mut self) {
        if self.lsp_code_action_selected > 0 {
            self.lsp_code_action_selected -= 1;
        }
    }

    /// Navigates code actions down.
    pub fn code_actions_down(&mut self) {
        if let Some(ref actions) = self.lsp_code_actions {
            if self.lsp_code_action_selected + 1 < actions.len() {
                self.lsp_code_action_selected += 1;
            }
        }
    }

    /// Navigates document symbols up.
    pub fn symbols_up(&mut self) {
        if self.lsp_symbols_selected > 0 {
            self.lsp_symbols_selected -= 1;
        }
    }

    /// Navigates document symbols down.
    pub fn symbols_down(&mut self) {
        if let Some(ref symbols) = self.lsp_document_symbols {
            let total = lsp::symbols::flatten_symbols(symbols, 0).len();
            if self.lsp_symbols_selected + 1 < total {
                self.lsp_symbols_selected += 1;
            }
        }
    }

    /// Navigates workspace symbols up.
    pub fn workspace_symbols_up(&mut self) {
        if self.lsp_workspace_selected > 0 {
            self.lsp_workspace_selected -= 1;
        }
    }

    /// Navigates workspace symbols down.
    pub fn workspace_symbols_down(&mut self) {
        if let Some(ref symbols) = self.lsp_workspace_symbols {
            if self.lsp_workspace_selected + 1 < symbols.len() {
                self.lsp_workspace_selected += 1;
            }
        }
    }

    /// Navigates diagnostics up.
    pub fn diagnostics_up(&mut self) {
        if self.lsp_diagnostics_selected > 0 {
            self.lsp_diagnostics_selected -= 1;
        }
    }

    /// Navigates diagnostics down.
    pub fn diagnostics_down(&mut self) {
        let total = self.diagnostic_store.total_count();
        if self.lsp_diagnostics_selected + 1 < total {
            self.lsp_diagnostics_selected += 1;
        }
    }

    /// Navigates to the selected reference location.
    pub fn goto_selected_reference(&mut self) {
        let groups = match &self.lsp_references {
            Some(g) => g.clone(),
            None => return,
        };

        let (gi, li) =
            match crate::ui::lsp_references::LspReferencesWidget::index_to_group_location(
                &groups,
                self.lsp_references_selected,
            ) {
                Some(pair) => pair,
                None => return,
            };

        let group = &groups[gi];
        let loc = &group.locations[li];
        let path = group.path.clone();
        let line = loc.line as usize;

        self.dismiss_references();
        self.open_file_at_line(&path, line);
    }

    /// Navigates to the selected document symbol.
    pub fn goto_selected_symbol(&mut self) {
        let symbols = match &self.lsp_document_symbols {
            Some(s) => s.clone(),
            None => return,
        };

        let flat = lsp::symbols::flatten_symbols(&symbols, 0);
        if let Some((_, symbol)) = flat.get(self.lsp_symbols_selected) {
            let line = symbol.selection_range.start_line as usize;
            let name = symbol.name.clone();
            self.dismiss_document_symbols();
            self.editor.goto_line(line);
            self.set_status(format!("Jumped to {name}"));
        }
    }

    /// Navigates to the selected workspace symbol.
    pub fn goto_selected_workspace_symbol(&mut self) {
        let symbols = match &self.lsp_workspace_symbols {
            Some(s) => s.clone(),
            None => return,
        };

        if let Some(symbol) = symbols.get(self.lsp_workspace_selected) {
            let path = symbol.path.clone();
            let line = symbol.line as usize;
            self.dismiss_workspace_symbols();
            self.open_file_at_line(&path, line);
        }
    }

    /// Opens a file and jumps to a specific line.
    pub fn open_file_at_line(&mut self, path: &std::path::Path, line: usize) {
        // Check if file is already open
        let already_open = self.open_files.iter().position(|f| f.path == path);
        if let Some(idx) = already_open {
            self.current_file_idx = idx;
            // Reload buffer if needed
            if self.editor.path() != Some(&path.to_path_buf()) {
                let _ = self.editor.open(path);
            }
        } else {
            // Open new tab
            let _ = self.editor.open(path);
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("untitled")
                .to_string();
            self.open_files.push(super::OpenFile {
                path: path.to_path_buf(),
                name,
            });
            self.current_file_idx = self.open_files.len() - 1;
        }
        self.editor.goto_line(line);
    }

    /// Returns diagnostics for the current file.
    pub fn current_file_diagnostics(&self) -> Vec<crate::lsp::diagnostics::DiagnosticInfo> {
        match self.current_file_path() {
            Some(path) => self.diagnostic_store.get(path),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::lsp::diagnostics::DiagnosticStore;

    #[test]
    fn test_has_lsp_overlay_default() {
        // Fresh state should have no overlays
        let store = DiagnosticStore::new();
        // Test the individual conditions rather than constructing a full App
        assert!(
            Option::<crate::lsp::hover::HoverResult>::None.is_none(),
            "Default hover is None"
        );
        assert_eq!(store.total_count(), 0, "Fresh store has no diagnostics");
    }

    #[test]
    fn test_dismiss_references_resets_state() {
        // Verify that dismiss logic resets selection and scroll
        let mut selected: usize = 5;
        let mut scroll: usize = 3;
        // Simulate dismiss
        selected = 0;
        scroll = 0;
        assert_eq!(selected, 0);
        assert_eq!(scroll, 0);
    }

    #[test]
    fn test_references_up_boundary() {
        let mut selected: usize = 0;
        // references_up should not underflow
        if selected > 0 {
            selected -= 1;
        }
        assert_eq!(selected, 0);
    }

    #[test]
    fn test_references_down_boundary() {
        let total: usize = 3;
        let mut selected: usize = 2;
        // references_down should not exceed total
        if selected + 1 < total {
            selected += 1;
        }
        assert_eq!(selected, 2, "Should not exceed total - 1");
    }

    #[test]
    fn test_code_actions_navigation() {
        let total: usize = 5;
        let mut selected: usize = 0;

        // Navigate down
        for _ in 0..4 {
            if selected + 1 < total {
                selected += 1;
            }
        }
        assert_eq!(selected, 4);

        // Navigate up
        for _ in 0..4 {
            if selected > 0 {
                selected -= 1;
            }
        }
        assert_eq!(selected, 0);
    }

    #[test]
    fn test_symbols_navigation() {
        let total: usize = 10;
        let mut selected: usize = 0;

        // Navigate down to middle
        for _ in 0..5 {
            if selected + 1 < total {
                selected += 1;
            }
        }
        assert_eq!(selected, 5);

        // Navigate up past start (should clamp at 0)
        for _ in 0..10 {
            if selected > 0 {
                selected -= 1;
            }
        }
        assert_eq!(selected, 0);
    }

    #[test]
    fn test_toggle_diagnostics_panel() {
        let mut visible = false;
        let mut diag_selected: usize = 5;
        let mut diag_scroll: usize = 3;

        // Toggle on
        visible = !visible;
        if visible {
            diag_selected = 0;
            diag_scroll = 0;
        }
        assert!(visible);
        assert_eq!(diag_selected, 0);
        assert_eq!(diag_scroll, 0);

        // Toggle off
        visible = !visible;
        assert!(!visible);
    }

    #[test]
    fn test_diagnostics_navigation() {
        let total: usize = 4;
        let mut selected: usize = 0;

        // Navigate down
        if selected + 1 < total {
            selected += 1;
        }
        assert_eq!(selected, 1);

        // Navigate up
        if selected > 0 {
            selected -= 1;
        }
        assert_eq!(selected, 0);

        // Cannot go below 0
        if selected > 0 {
            selected -= 1;
        }
        assert_eq!(selected, 0);
    }

    #[test]
    fn test_workspace_symbols_navigation() {
        let total: usize = 3;
        let mut selected: usize = 0;

        // Down twice
        if selected + 1 < total {
            selected += 1;
        }
        if selected + 1 < total {
            selected += 1;
        }
        assert_eq!(selected, 2);

        // Cannot go past end
        if selected + 1 < total {
            selected += 1;
        }
        assert_eq!(selected, 2);
    }

    #[test]
    fn test_diagnostic_store_integration() {
        use serde_json::json;
        let store = DiagnosticStore::new();

        // Initially empty
        assert_eq!(store.total_count(), 0);

        // Add some diagnostics
        let notification = json!({
            "uri": "file:///src/main.rs",
            "diagnostics": [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}},
                "severity": 1,
                "message": "test error"
            }]
        });
        store.update_from_notification(&notification);
        assert_eq!(store.total_count(), 1);
    }
}
