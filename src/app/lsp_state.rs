//! Everything the language-server screens keep between key presses.
//!
//! `App` carried nineteen `lsp_*` fields — five panels' worth of items,
//! selected indices and scroll offsets, plus hover, signature help and rename.
//! Adding a panel meant adding three more, and each panel's navigation was
//! written again from scratch, so an off-by-one fixed in one was still present
//! in the others.
//!
//! This is that state in one place, with the panels sharing
//! [`ListPanel`](super::panel::ListPanel). The behaviour is unchanged; what
//! changes is that there is one implementation of "move down without falling
//! off the end", and it is tested.

use crate::lsp::DiagnosticStore;
use crate::lsp::actions::CodeActionResult;
use crate::lsp::hover::HoverResult;
use crate::lsp::references::ReferenceGroup;
use crate::lsp::rename::RenameRange;
use crate::lsp::signature::SignatureHelpResult;
use crate::lsp::symbols::{DocumentSymbolResult, SymbolInfoResult};

use super::panel::ListPanel;

/// A rename in progress.
#[derive(Debug, Clone)]
pub struct RenameState {
    /// The new name as typed so far.
    pub input: String,
    /// Where in the document the old name is.
    pub range: Option<RenameRange>,
}

impl RenameState {
    /// Starts a rename with the current name already filled in.
    #[must_use]
    pub fn new(initial: String, range: Option<RenameRange>) -> Self {
        Self {
            input: initial,
            range,
        }
    }

    /// True when the typed name would be a no-op.
    ///
    /// An empty rename is refused rather than sent: a language server asked to
    /// rename a symbol to nothing does something different in each
    /// implementation, and none of them is what the user meant.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.input.trim().is_empty()
    }
}

/// The language-server interface state.
#[derive(Debug, Default)]
pub struct LspUiState {
    /// Diagnostics for every open file.
    pub diagnostics: DiagnosticStore,
    /// The hover popup, when one is showing.
    pub hover: Option<HoverResult>,
    /// Where the cursor was when a popup was opened, so it appears beside it.
    pub hover_cursor: (u16, u16),
    /// Signature help, when one is showing.
    pub signature_help: Option<SignatureHelpResult>,
    /// References, grouped by file.
    pub references: ListPanel<ReferenceGroup>,
    /// Code actions offered at the cursor.
    pub code_actions: ListPanel<CodeActionResult>,
    /// Symbols in the current document.
    pub document_symbols: ListPanel<DocumentSymbolResult>,
    /// Symbols matching a workspace search.
    pub workspace_symbols: ListPanel<SymbolInfoResult>,
    /// What was typed into the workspace symbol search.
    pub workspace_query: String,
    /// The diagnostics panel: open, with a selection and a scroll offset.
    ///
    /// It holds unit items because the rows come from `diagnostics` and are
    /// rebuilt each frame; what it keeps is the position within them.
    pub diagnostics_panel: ListPanel<()>,
    /// A rename in progress.
    pub rename: Option<RenameState>,
    /// Format through the language server on save.
    pub format_on_save: bool,
}

impl LspUiState {
    /// A state with nothing open.
    #[must_use]
    pub fn new(format_on_save: bool) -> Self {
        Self {
            format_on_save,
            ..Self::default()
        }
    }

    /// Closes every popup and panel.
    ///
    /// One call, so a new panel cannot be forgotten here — which is how a
    /// stale hover used to survive a file switch and point at a line that had
    /// moved.
    pub fn close_all(&mut self) {
        self.hover = None;
        self.signature_help = None;
        self.references.close();
        self.code_actions.close();
        self.document_symbols.close();
        self.workspace_symbols.close();
        self.workspace_query.clear();
        self.diagnostics_panel.close();
        self.rename = None;
    }

    /// Closes everything that floats over the editor.
    ///
    /// The diagnostics panel is deliberately left alone: it is a pane the user
    /// opened and expects to stay, not a popup that appeared on its own.
    pub fn close_overlays(&mut self) {
        self.hover = None;
        self.signature_help = None;
        self.references.close();
        self.code_actions.close();
        self.document_symbols.close();
        self.workspace_symbols.close();
        self.workspace_query.clear();
        self.rename = None;
    }

    /// True if anything floats over the editor.
    #[must_use]
    pub fn has_overlay(&self) -> bool {
        self.hover.is_some()
            || self.signature_help.is_some()
            || self.references.is_open()
            || self.code_actions.is_open()
            || self.document_symbols.is_open()
            || self.workspace_symbols.is_open()
            || self.rename.is_some()
    }

    /// Closes the popups tied to a cursor position, leaving panels alone.
    ///
    /// Called when the cursor moves: hover and signature help describe the
    /// character they were opened on, and are wrong the moment it changes,
    /// while a references list stays useful.
    pub fn close_cursor_popups(&mut self) {
        self.hover = None;
        self.signature_help = None;
    }

    /// True if anything is open that should take a key before the editor does.
    #[must_use]
    pub fn is_capturing_keys(&self) -> bool {
        self.rename.is_some()
            || self.references.is_open()
            || self.code_actions.is_open()
            || self.document_symbols.is_open()
            || self.workspace_symbols.is_open()
            || self.diagnostics_panel.is_open()
    }

    /// True if anything at all is showing.
    #[must_use]
    pub fn is_showing_anything(&self) -> bool {
        self.is_capturing_keys() || self.hover.is_some() || self.signature_help.is_some()
    }

    /// Opens the diagnostics panel over `count` rows.
    pub fn open_diagnostics(&mut self, count: usize) {
        self.diagnostics_panel.open(vec![(); count]);
    }

    /// Updates the diagnostics panel's row count, keeping the position.
    ///
    /// Diagnostics change under the user as they type; the selection should
    /// stay where it was rather than jumping to the top on every keystroke.
    pub fn sync_diagnostics(&mut self, count: usize) {
        if self.diagnostics_panel.is_open() {
            self.diagnostics_panel.replace(vec![(); count]);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// An empty hover result. Its contents do not matter here; what matters is
    /// that one is showing.
    fn a_hover() -> HoverResult {
        HoverResult {
            contents: Vec::new(),
            range: None,
        }
    }

    #[test]
    fn a_new_state_has_nothing_open() {
        let state = LspUiState::new(false);
        assert!(!state.is_capturing_keys());
        assert!(!state.is_showing_anything());
        assert!(!state.references.is_open());
        assert!(state.rename.is_none());
        assert!(!state.format_on_save);
    }

    #[test]
    fn format_on_save_is_carried_from_the_configuration() {
        assert!(LspUiState::new(true).format_on_save);
    }

    #[test]
    fn a_hover_shows_without_capturing_keys() {
        // A hover popup must not swallow the next keystroke; a panel must.
        let mut state = LspUiState::new(false);
        state.hover = Some(a_hover());

        assert!(state.is_showing_anything());
        assert!(
            !state.is_capturing_keys(),
            "typing should still reach the editor"
        );
    }

    #[test]
    fn an_open_panel_captures_keys() {
        let mut state = LspUiState::new(false);
        state.references.open(Vec::new());
        assert!(state.is_capturing_keys());
    }

    #[test]
    fn a_rename_captures_keys() {
        let mut state = LspUiState::new(false);
        state.rename = Some(RenameState::new("old_name".to_string(), None));
        assert!(state.is_capturing_keys());
    }

    #[test]
    fn closing_everything_leaves_nothing_showing() {
        let mut state = LspUiState::new(true);
        state.hover = Some(a_hover());
        state.references.open(Vec::new());
        state.code_actions.open(Vec::new());
        state.workspace_query = "conf".to_string();
        state.rename = Some(RenameState::new("x".to_string(), None));

        state.close_all();

        assert!(!state.is_showing_anything());
        assert!(state.workspace_query.is_empty());
        assert!(
            state.format_on_save,
            "a setting is not something closing a panel should change"
        );
    }

    #[test]
    fn moving_the_cursor_closes_the_popups_but_not_the_panels() {
        let mut state = LspUiState::new(false);
        state.hover = Some(a_hover());
        state.references.open(Vec::new());

        state.close_cursor_popups();

        assert!(state.hover.is_none(), "a hover describes a character");
        assert!(state.references.is_open(), "a result list stays useful");
    }

    #[test]
    fn the_diagnostics_panel_keeps_its_place_while_the_list_changes() {
        let mut state = LspUiState::new(false);
        state.open_diagnostics(10);
        state.diagnostics_panel.select(7);

        state.sync_diagnostics(12);
        assert_eq!(
            state.diagnostics_panel.selected(),
            7,
            "still on the same row"
        );

        state.sync_diagnostics(3);
        assert_eq!(
            state.diagnostics_panel.selected(),
            2,
            "a shorter list moves the selection into it"
        );
    }

    #[test]
    fn syncing_a_closed_diagnostics_panel_does_not_open_it() {
        let mut state = LspUiState::new(false);
        state.sync_diagnostics(5);
        assert!(!state.diagnostics_panel.is_open());
    }

    #[test]
    fn a_rename_to_nothing_is_recognised_as_empty() {
        assert!(RenameState::new(String::new(), None).is_empty());
        assert!(RenameState::new("   ".to_string(), None).is_empty());
        assert!(!RenameState::new("new_name".to_string(), None).is_empty());
    }
}
