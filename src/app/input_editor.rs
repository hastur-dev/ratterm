//! Editor input handling for the application.
//!
//! This file is a translator and nothing else: it turns a `crossterm`
//! [`KeyEvent`] into a call on [`Editor`](crate::editor::Editor), whose API
//! layer holds every editing rule. Anything that looks like a decision here —
//! whether a bracket pairs, where a new line's indent comes from, what `d2w`
//! covers — is answered in `src/editor/`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::KeybindingMode;
use crate::editor::EditorMode;
use crate::editor::emacs_exec::EmacsEffect;
use crate::editor::emacs_keys::{EmacsKey, NavKey};
use crate::editor::search_ops::{SearchKey, SearchOutcome};
use crate::editor::vim::{VimEffect, VimKey};

/// Translates a key event into a Vim key, if it is one the parser accepts.
#[must_use]
pub fn vim_key(key: KeyEvent) -> Option<VimKey> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(c) if ctrl => Some(VimKey::Ctrl(c.to_ascii_lowercase())),
        KeyCode::Char(c) => Some(VimKey::Char(c)),
        KeyCode::Esc => Some(VimKey::Escape),
        KeyCode::Enter => Some(VimKey::Enter),
        KeyCode::Backspace => Some(VimKey::Backspace),
        KeyCode::Tab => Some(VimKey::Tab),
        // Arrow keys spell the same motions as the letter keys.
        KeyCode::Left => Some(VimKey::Char('h')),
        KeyCode::Right => Some(VimKey::Char('l')),
        KeyCode::Up => Some(VimKey::Char('k')),
        KeyCode::Down => Some(VimKey::Char('j')),
        KeyCode::Home => Some(VimKey::Char('0')),
        KeyCode::End => Some(VimKey::Char('$')),
        _ => None,
    }
}

/// Translates a key event into an Emacs chord.
#[must_use]
pub fn emacs_key(key: KeyEvent) -> Option<EmacsKey> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char(c) if ctrl && alt => Some(EmacsKey::CtrlAlt(c.to_ascii_lowercase())),
        KeyCode::Char(c) if ctrl => Some(EmacsKey::Ctrl(c.to_ascii_lowercase())),
        KeyCode::Char(c) if alt => Some(EmacsKey::Alt(c)),
        KeyCode::Char(c) => Some(EmacsKey::Char(c)),
        KeyCode::Backspace if alt => Some(EmacsKey::AltBackspace),
        KeyCode::Backspace => Some(EmacsKey::Backspace),
        KeyCode::Delete => Some(EmacsKey::Delete),
        KeyCode::Enter => Some(EmacsKey::Enter),
        KeyCode::Tab => Some(EmacsKey::Tab),
        KeyCode::Left => Some(EmacsKey::Nav(NavKey::Left)),
        KeyCode::Right => Some(EmacsKey::Nav(NavKey::Right)),
        KeyCode::Up => Some(EmacsKey::Nav(NavKey::Up)),
        KeyCode::Down => Some(EmacsKey::Nav(NavKey::Down)),
        KeyCode::Home => Some(EmacsKey::Nav(NavKey::Home)),
        KeyCode::End => Some(EmacsKey::Nav(NavKey::End)),
        KeyCode::PageUp => Some(EmacsKey::Nav(NavKey::PageUp)),
        KeyCode::PageDown => Some(EmacsKey::Nav(NavKey::PageDown)),
        _ => None,
    }
}

/// Translates a key event into a find-bar key.
#[must_use]
pub fn search_key(key: KeyEvent) -> Option<SearchKey> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Esc => Some(SearchKey::Escape),
        KeyCode::Enter if shift => Some(SearchKey::ShiftEnter),
        KeyCode::Enter if alt => Some(SearchKey::ReplaceAll),
        KeyCode::Enter if ctrl => Some(SearchKey::Replace),
        KeyCode::Enter => Some(SearchKey::Enter),
        KeyCode::Tab | KeyCode::BackTab => Some(SearchKey::Tab),
        KeyCode::Backspace => Some(SearchKey::Backspace),
        KeyCode::Up => Some(SearchKey::ShiftEnter),
        KeyCode::Down => Some(SearchKey::Enter),
        KeyCode::Char('c') if alt => Some(SearchKey::ToggleCase),
        KeyCode::Char(c) if !ctrl => Some(SearchKey::Char(c)),
        _ => None,
    }
}

use super::App;

impl App {
    /// Handles key events for the editor pane.
    pub(super) fn handle_editor_key(&mut self, key: KeyEvent) {
        // The find bar takes precedence over every keybinding mode while open.
        if self.editor.search().is_active()
            && let Some(search) = search_key(key)
            && self.editor.feed_search_key(search) != SearchOutcome::Ignored
        {
            return;
        }
        if self.handle_editor_shared_key(key) {
            return;
        }
        match self.keybinding_mode() {
            KeybindingMode::Vim => self.handle_editor_key_vim(key),
            KeybindingMode::Emacs => self.handle_editor_key_emacs(key),
            KeybindingMode::Default => self.handle_editor_key_default(key),
        }
    }

    /// Handles the keys that mean the same thing in every keybinding mode.
    ///
    /// Returns true when the key was consumed.
    fn handle_editor_shared_key(&mut self, key: KeyEvent) -> bool {
        // Ctrl+Alt is the one modifier pair the global handler leaves alone
        // apart from Ctrl+Alt+1..9, so folding and extra cursors live there and
        // work the same in Vim, Emacs, and Default mode.
        let ctrl_alt = KeyModifiers::CONTROL | KeyModifiers::ALT;
        if !key.modifiers.contains(ctrl_alt) {
            return false;
        }

        match key.code {
            // The find bar. Plain Ctrl+F already opens the older
            // search-in-file popup at the application level, so the in-pane bar
            // takes the Ctrl+Alt slot rather than shadowing it.
            KeyCode::Char('f') => {
                self.editor.open_search(false);
                true
            }
            KeyCode::Char('h') => {
                self.editor.open_search(true);
                true
            }
            // Folding.
            KeyCode::Char('[') => {
                self.editor.fold_current();
                true
            }
            KeyCode::Char(']') => {
                self.editor.unfold_current();
                true
            }
            KeyCode::Char('k') => {
                self.editor.fold_all();
                true
            }
            KeyCode::Char('j') => {
                self.editor.unfold_all();
                true
            }
            // Multiple cursors.
            KeyCode::Down => {
                self.editor.add_cursor_below();
                true
            }
            KeyCode::Up => {
                self.editor.add_cursor_above();
                true
            }
            KeyCode::Char('d') => {
                self.editor.add_cursor_at_next_occurrence();
                true
            }
            KeyCode::Char('l') => {
                self.editor.add_cursors_at_all_occurrences();
                true
            }
            _ => false,
        }
    }

    /// Handles editor keys in Vim mode (modal editing).
    fn handle_editor_key_vim(&mut self, key: KeyEvent) {
        if self.editor.mode() == EditorMode::Insert {
            self.handle_editor_insert_key(key);
            return;
        }
        let Some(vim) = vim_key(key) else { return };
        let feed = self.editor.feed_vim_key(vim);
        self.apply_vim_effect(feed.effect);
    }

    /// Carries out the part of a Vim command the editor cannot do itself.
    fn apply_vim_effect(&mut self, effect: VimEffect) {
        match effect {
            VimEffect::Save | VimEffect::SaveAndQuit => {
                self.save_current_file();
                if effect == VimEffect::SaveAndQuit {
                    self.close_current_file();
                }
            }
            VimEffect::Quit => {
                if self.editor.is_modified() {
                    self.editor
                        .set_status("Unsaved changes; use :q! to discard or :wq to save");
                } else {
                    self.close_current_file();
                }
            }
            VimEffect::QuitWithoutSaving => self.close_current_file(),
            VimEffect::Ex(command) => {
                self.editor
                    .set_status(format!("Unknown command: {command}"));
            }
            VimEffect::None => {}
        }
    }

    /// Handles editor keys in insert mode (Vim).
    fn handle_editor_insert_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.editor.set_mode(EditorMode::Normal);
                self.editor.clear_extra_cursors();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => self.editor.backspace(),
            (KeyModifiers::NONE, KeyCode::Delete) => self.editor.delete(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.editor.insert_newline_smart(),
            (KeyModifiers::NONE, KeyCode::Tab) => self.editor.insert_indent_unit(),
            (KeyModifiers::NONE, KeyCode::Left) => self.editor.move_left(),
            (KeyModifiers::NONE, KeyCode::Right) => self.editor.move_right(),
            (KeyModifiers::NONE, KeyCode::Up) => self.editor.move_up_visible(),
            (KeyModifiers::NONE, KeyCode::Down) => self.editor.move_down_visible(),
            (KeyModifiers::NONE, KeyCode::Home) => self.editor.move_to_line_start(),
            (KeyModifiers::NONE, KeyCode::End) => self.editor.move_to_line_end(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.type_char(c);
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_current_file(),
            _ => {}
        }
    }

    /// Handles editor keys in Emacs mode (non-modal).
    fn handle_editor_key_emacs(&mut self, key: KeyEvent) {
        let Some(chord) = emacs_key(key) else { return };
        let effect = self.editor.feed_emacs_key(chord);
        self.apply_emacs_effect(effect);
    }

    /// Carries out the part of an Emacs command the editor cannot do itself.
    fn apply_emacs_effect(&mut self, effect: EmacsEffect) {
        match effect {
            EmacsEffect::Save => self.save_current_file(),
            EmacsEffect::Quit => self.close_current_file(),
            EmacsEffect::FindFile => {
                self.editor.set_status("Use Ctrl+O to open a file");
            }
            EmacsEffect::Search { forward } => {
                self.editor.open_search(false);
                if !forward {
                    self.editor.search_prev();
                }
            }
            EmacsEffect::QueryReplace => self.editor.open_search(true),
            EmacsEffect::None => {}
        }
    }

    /// Handles editor keys in Default mode (non-modal, simple keybindings).
    fn handle_editor_key_default(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Accept completion with Ctrl+Space
            (KeyModifiers::CONTROL, KeyCode::Char(' ')) => {
                if !self.accept_completion() {
                    self.trigger_completion();
                }
            }
            (KeyModifiers::NONE, KeyCode::F(3)) => {
                self.editor.search_next();
            }
            (KeyModifiers::SHIFT, KeyCode::F(3)) => {
                self.editor.search_prev();
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.dismiss_completion();
                self.editor.clear_extra_cursors();
                self.editor.cursor_mut().clear_selection();
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.dismiss_completion();
                self.editor.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.dismiss_completion();
                self.editor.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.dismiss_completion();
                self.editor.move_up_visible();
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.dismiss_completion();
                self.editor.move_down_visible();
            }
            (KeyModifiers::SHIFT, KeyCode::Left) => self.editor.select_left(),
            (KeyModifiers::SHIFT, KeyCode::Right) => self.editor.select_right(),
            (KeyModifiers::SHIFT, KeyCode::Up) => self.editor.select_up(),
            (KeyModifiers::SHIFT, KeyCode::Down) => self.editor.select_down(),
            (KeyModifiers::NONE, KeyCode::Home) => self.editor.move_to_line_start(),
            (KeyModifiers::NONE, KeyCode::End) => self.editor.move_to_line_end(),
            (KeyModifiers::NONE, KeyCode::PageUp) => self.editor.page_up(),
            (KeyModifiers::NONE, KeyCode::PageDown) => self.editor.page_down(),
            (KeyModifiers::CONTROL, KeyCode::Left) => self.editor.move_word_left(),
            (KeyModifiers::CONTROL, KeyCode::Right) => self.editor.move_word_right(),
            (KeyModifiers::CONTROL, KeyCode::Home) => self.editor.move_to_buffer_start(),
            (KeyModifiers::CONTROL, KeyCode::End) => self.editor.move_to_buffer_end(),
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => self.editor.select_all(),
            (KeyModifiers::CONTROL, KeyCode::Char('z')) => self.editor.undo(),
            (KeyModifiers::CONTROL, KeyCode::Char('y')) => self.editor.redo(),
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_current_file(),
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.editor.backspace();
                self.trigger_completion();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.editor.delete();
                self.dismiss_completion();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.dismiss_completion();
                self.editor.insert_newline_smart();
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.dismiss_completion();
                if self.editor.cursor().has_selection() {
                    self.editor.indent_selection();
                } else {
                    self.editor.insert_indent_unit();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab) | (KeyModifiers::NONE, KeyCode::BackTab) => {
                self.editor.outdent_selection();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.type_char(c);
                self.trigger_completion();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn vim_keys_carry_control_chords_and_arrows() {
        assert_eq!(
            vim_key(press(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(VimKey::Ctrl('r'))
        );
        assert_eq!(
            vim_key(press(KeyCode::Char('d'), KeyModifiers::NONE)),
            Some(VimKey::Char('d'))
        );
        assert_eq!(
            vim_key(press(KeyCode::Down, KeyModifiers::NONE)),
            Some(VimKey::Char('j'))
        );
        assert_eq!(
            vim_key(press(KeyCode::Esc, KeyModifiers::NONE)),
            Some(VimKey::Escape)
        );
        assert_eq!(vim_key(press(KeyCode::F(5), KeyModifiers::NONE)), None);
    }

    #[test]
    fn emacs_keys_distinguish_control_alt_and_plain() {
        assert_eq!(
            emacs_key(press(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            Some(EmacsKey::Ctrl('k'))
        );
        assert_eq!(
            emacs_key(press(KeyCode::Char('f'), KeyModifiers::ALT)),
            Some(EmacsKey::Alt('f'))
        );
        assert_eq!(
            emacs_key(press(
                KeyCode::Char('x'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )),
            Some(EmacsKey::CtrlAlt('x'))
        );
        assert_eq!(
            emacs_key(press(KeyCode::Backspace, KeyModifiers::ALT)),
            Some(EmacsKey::AltBackspace)
        );
        assert_eq!(
            emacs_key(press(KeyCode::Char('z'), KeyModifiers::NONE)),
            Some(EmacsKey::Char('z'))
        );
        assert_eq!(emacs_key(press(KeyCode::F(1), KeyModifiers::NONE)), None);
    }

    #[test]
    fn search_keys_cover_the_whole_bar() {
        assert_eq!(
            search_key(press(KeyCode::Enter, KeyModifiers::NONE)),
            Some(SearchKey::Enter)
        );
        assert_eq!(
            search_key(press(KeyCode::Enter, KeyModifiers::SHIFT)),
            Some(SearchKey::ShiftEnter)
        );
        assert_eq!(
            search_key(press(KeyCode::Enter, KeyModifiers::CONTROL)),
            Some(SearchKey::Replace)
        );
        assert_eq!(
            search_key(press(KeyCode::Enter, KeyModifiers::ALT)),
            Some(SearchKey::ReplaceAll)
        );
        assert_eq!(
            search_key(press(KeyCode::Tab, KeyModifiers::NONE)),
            Some(SearchKey::Tab)
        );
        assert_eq!(
            search_key(press(KeyCode::Esc, KeyModifiers::NONE)),
            Some(SearchKey::Escape)
        );
        assert_eq!(
            search_key(press(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(SearchKey::Char('a'))
        );
        // Control chords are not text for the bar.
        assert_eq!(
            search_key(press(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            None
        );
    }
}
