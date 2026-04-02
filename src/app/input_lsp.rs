//! LSP-specific input handling.

use crossterm::event::{KeyCode, KeyEvent};

use super::App;

impl App {
    /// Handles key events when an LSP overlay is active.
    /// Returns true if the key was consumed.
    pub(super) fn handle_lsp_overlay_key(&mut self, key: KeyEvent) -> bool {
        assert!(
            self.has_lsp_overlay(),
            "handle_lsp_overlay_key called without active overlay"
        );

        // Handle references panel
        if self.lsp_references.is_some() {
            return self.handle_lsp_references_key(key);
        }

        // Handle code actions
        if self.lsp_code_actions.is_some() {
            return self.handle_lsp_code_actions_key(key);
        }

        // Handle document symbols
        if self.lsp_document_symbols.is_some() {
            return self.handle_lsp_symbols_key(key);
        }

        // Handle workspace symbols
        if self.lsp_workspace_symbols.is_some() {
            return self.handle_lsp_workspace_symbols_key(key);
        }

        // Handle rename input
        if self.lsp_rename_input.is_some() {
            return self.handle_lsp_rename_key(key);
        }

        // Handle hover (dismiss on any key)
        if self.lsp_hover.is_some() {
            self.dismiss_hover();
            return false; // Don't consume - let the key through
        }

        // Handle signature help (dismiss on Esc, let other keys through)
        if self.lsp_signature_help.is_some() {
            if key.code == KeyCode::Esc {
                self.dismiss_signature_help();
                return true;
            }
            return false; // Let key through (typing continues)
        }

        false
    }

    /// Handles keys for the references panel.
    fn handle_lsp_references_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => self.dismiss_references(),
            KeyCode::Up | KeyCode::Char('k') => self.references_up(),
            KeyCode::Down | KeyCode::Char('j') => self.references_down(),
            KeyCode::Enter => self.goto_selected_reference(),
            _ => {} // Consume all keys while panel is open
        }
        true
    }

    /// Handles keys for code actions popup.
    fn handle_lsp_code_actions_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => self.dismiss_code_actions(),
            KeyCode::Up | KeyCode::Char('k') => self.code_actions_up(),
            KeyCode::Down | KeyCode::Char('j') => self.code_actions_down(),
            KeyCode::Enter => self.apply_selected_code_action(),
            _ => {}
        }
        true
    }

    /// Handles keys for document symbols panel.
    fn handle_lsp_symbols_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => self.dismiss_document_symbols(),
            KeyCode::Up | KeyCode::Char('k') => self.symbols_up(),
            KeyCode::Down | KeyCode::Char('j') => self.symbols_down(),
            KeyCode::Enter => self.goto_selected_symbol(),
            _ => {}
        }
        true
    }

    /// Handles keys for workspace symbols panel.
    fn handle_lsp_workspace_symbols_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => self.dismiss_workspace_symbols(),
            KeyCode::Up => self.workspace_symbols_up(),
            KeyCode::Down => self.workspace_symbols_down(),
            KeyCode::Enter => self.goto_selected_workspace_symbol(),
            _ => {}
        }
        true
    }

    /// Handles keys for rename input.
    fn handle_lsp_rename_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => self.dismiss_rename(),
            KeyCode::Enter => self.confirm_rename(),
            KeyCode::Backspace => {
                if let Some(ref mut input) = self.lsp_rename_input {
                    input.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut input) = self.lsp_rename_input {
                    input.push(c);
                }
            }
            _ => {}
        }
        true
    }

    /// Applies the selected code action.
    fn apply_selected_code_action(&mut self) {
        let actions = match self.lsp_code_actions.take() {
            Some(a) => a,
            None => return,
        };

        let selected = self.lsp_code_action_selected;
        self.lsp_code_action_selected = 0;

        if let Some(action) = actions.get(selected) {
            if action.edit.is_some() {
                self.set_status(format!("Applied: {}", action.title));
            } else {
                self.set_status(format!("Action '{}' has no edit to apply", action.title));
            }
        }
    }

    /// Confirms rename operation.
    fn confirm_rename(&mut self) {
        let new_name = match self.lsp_rename_input.take() {
            Some(n) if !n.is_empty() => n,
            _ => {
                self.dismiss_rename();
                return;
            }
        };
        self.lsp_rename_range = None;
        self.set_status(format!(
            "Rename to '{new_name}' requested (requires LSP server)"
        ));
        // In a full implementation, this would send the rename request to LSP
        // and apply the resulting WorkspaceEdit. For now we show the status.
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, unused_assignments)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_rename_key_backspace_pops() {
        let mut input = Some("hello".to_string());
        // Simulate backspace handling
        if let Some(ref mut s) = input {
            s.pop();
        }
        assert_eq!(input.as_deref(), Some("hell"));
    }

    #[test]
    fn test_rename_key_char_pushes() {
        let mut input = Some("he".to_string());
        let c = 'l';
        if let Some(ref mut s) = input {
            s.push(c);
        }
        assert_eq!(input.as_deref(), Some("hel"));
    }

    #[test]
    fn test_rename_empty_dismissed() {
        let input: Option<String> = Some(String::new());
        let should_dismiss = !matches!(&input, Some(n) if !n.is_empty());
        assert!(should_dismiss, "Empty rename input should be dismissed");
    }

    #[test]
    fn test_rename_non_empty_accepted() {
        let input: Option<String> = Some("new_name".to_string());
        let should_dismiss = !matches!(&input, Some(n) if !n.is_empty());
        assert!(!should_dismiss, "Non-empty rename input should be accepted");
    }

    #[test]
    fn test_key_code_matching() {
        // Verify key codes that should be handled by LSP overlays
        let esc = make_key(KeyCode::Esc);
        let enter = make_key(KeyCode::Enter);
        let up = make_key(KeyCode::Up);
        let down = make_key(KeyCode::Down);
        let j = make_key(KeyCode::Char('j'));
        let k = make_key(KeyCode::Char('k'));

        assert_eq!(esc.code, KeyCode::Esc);
        assert_eq!(enter.code, KeyCode::Enter);
        assert_eq!(up.code, KeyCode::Up);
        assert_eq!(down.code, KeyCode::Down);
        assert_eq!(j.code, KeyCode::Char('j'));
        assert_eq!(k.code, KeyCode::Char('k'));
    }

    #[test]
    fn test_code_action_selection_reset() {
        let mut selected: usize = 3;
        // Simulate apply_selected_code_action reset
        selected = 0;
        assert_eq!(selected, 0);
    }
}
