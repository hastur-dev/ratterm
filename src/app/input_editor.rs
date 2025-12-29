//! Editor input handling for the application.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::KeybindingMode;
use crate::editor::EditorMode;

use super::App;

impl App {
    /// Handles key events for the editor pane.
    pub(super) fn handle_editor_key(&mut self, key: KeyEvent) {
        match self.keybinding_mode() {
            KeybindingMode::Vim => self.handle_editor_key_vim(key),
            KeybindingMode::Emacs => self.handle_editor_key_emacs(key),
            KeybindingMode::Default => self.handle_editor_key_default(key),
            KeybindingMode::VsCode => self.handle_editor_key_vscode(key),
        }
    }

    /// Handles editor keys in Vim mode (modal editing).
    fn handle_editor_key_vim(&mut self, key: KeyEvent) {
        match self.editor.mode() {
            EditorMode::Normal => self.handle_editor_normal_key(key),
            EditorMode::Insert => self.handle_editor_insert_key(key),
            EditorMode::Visual => self.handle_editor_visual_key(key),
            EditorMode::Command => self.handle_editor_command_key(key),
        }
    }

    /// Handles editor keys in Emacs mode (non-modal).
    fn handle_editor_key_emacs(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => self.editor.move_left(),
            (KeyModifiers::CONTROL, KeyCode::Char('f')) => self.editor.move_right(),
            (KeyModifiers::CONTROL, KeyCode::Char('p')) => self.editor.move_up(),
            (KeyModifiers::CONTROL, KeyCode::Char('n')) => self.editor.move_down(),
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => self.editor.move_to_line_start(),
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => self.editor.move_to_line_end(),
            (KeyModifiers::ALT, KeyCode::Char('f')) => self.editor.move_word_right(),
            (KeyModifiers::ALT, KeyCode::Char('b')) => self.editor.move_word_left(),
            (m, KeyCode::Char('<')) if m == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                self.editor.move_to_buffer_start();
            }
            (m, KeyCode::Char('>')) if m == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                self.editor.move_to_buffer_end();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => self.editor.delete(),
            (KeyModifiers::CONTROL, KeyCode::Char('/')) => self.editor.undo(),
            (m, KeyCode::Char('/')) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                self.editor.redo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => self.editor.delete_to_line_end(),
            (KeyModifiers::CONTROL, KeyCode::Char('x')) => self.save_current_file(),
            (KeyModifiers::NONE, KeyCode::Backspace) => self.editor.backspace(),
            (KeyModifiers::NONE, KeyCode::Delete) => self.editor.delete(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.editor.insert_char('\n'),
            (KeyModifiers::NONE, KeyCode::Tab) => self.editor.insert_str("    "),
            (KeyModifiers::NONE, KeyCode::Left) => self.editor.move_left(),
            (KeyModifiers::NONE, KeyCode::Right) => self.editor.move_right(),
            (KeyModifiers::NONE, KeyCode::Up) => self.editor.move_up(),
            (KeyModifiers::NONE, KeyCode::Down) => self.editor.move_down(),
            (KeyModifiers::NONE, KeyCode::Home) => self.editor.move_to_line_start(),
            (KeyModifiers::NONE, KeyCode::End) => self.editor.move_to_line_end(),
            (KeyModifiers::NONE, KeyCode::PageUp) => self.editor.page_up(),
            (KeyModifiers::NONE, KeyCode::PageDown) => self.editor.page_down(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            _ => {}
        }
    }

    /// Handles editor keys in Default mode (non-modal, simple keybindings).
    fn handle_editor_key_default(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Left) => self.editor.move_left(),
            (KeyModifiers::NONE, KeyCode::Right) => self.editor.move_right(),
            (KeyModifiers::NONE, KeyCode::Up) => self.editor.move_up(),
            (KeyModifiers::NONE, KeyCode::Down) => self.editor.move_down(),
            (KeyModifiers::NONE, KeyCode::Home) => self.editor.move_to_line_start(),
            (KeyModifiers::NONE, KeyCode::End) => self.editor.move_to_line_end(),
            (KeyModifiers::NONE, KeyCode::PageUp) => self.editor.page_up(),
            (KeyModifiers::NONE, KeyCode::PageDown) => self.editor.page_down(),
            (KeyModifiers::CONTROL, KeyCode::Left) => self.editor.move_word_left(),
            (KeyModifiers::CONTROL, KeyCode::Right) => self.editor.move_word_right(),
            (KeyModifiers::CONTROL, KeyCode::Home) => self.editor.move_to_buffer_start(),
            (KeyModifiers::CONTROL, KeyCode::End) => self.editor.move_to_buffer_end(),
            (KeyModifiers::CONTROL, KeyCode::Char('z')) => self.editor.undo(),
            (KeyModifiers::CONTROL, KeyCode::Char('y')) => self.editor.redo(),
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_current_file(),
            (KeyModifiers::NONE, KeyCode::Backspace) => self.editor.backspace(),
            (KeyModifiers::NONE, KeyCode::Delete) => self.editor.delete(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.editor.insert_char('\n'),
            (KeyModifiers::NONE, KeyCode::Tab) => self.editor.insert_str("    "),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            _ => {}
        }
    }

    /// Handles editor keys in VSCode mode (non-modal, VSCode-like keybindings).
    fn handle_editor_key_vscode(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Left) => self.editor.move_left(),
            (KeyModifiers::NONE, KeyCode::Right) => self.editor.move_right(),
            (KeyModifiers::NONE, KeyCode::Up) => self.editor.move_up(),
            (KeyModifiers::NONE, KeyCode::Down) => self.editor.move_down(),
            (KeyModifiers::NONE, KeyCode::Home) => self.editor.move_to_line_start(),
            (KeyModifiers::NONE, KeyCode::End) => self.editor.move_to_line_end(),
            (KeyModifiers::NONE, KeyCode::PageUp) => self.editor.page_up(),
            (KeyModifiers::NONE, KeyCode::PageDown) => self.editor.page_down(),
            (KeyModifiers::CONTROL, KeyCode::Left) => self.editor.move_word_left(),
            (KeyModifiers::CONTROL, KeyCode::Right) => self.editor.move_word_right(),
            (KeyModifiers::CONTROL, KeyCode::Home) => self.editor.move_to_buffer_start(),
            (KeyModifiers::CONTROL, KeyCode::End) => self.editor.move_to_buffer_end(),
            (KeyModifiers::SHIFT, KeyCode::Left) => self.editor.select_left(),
            (KeyModifiers::SHIFT, KeyCode::Right) => self.editor.select_right(),
            (KeyModifiers::SHIFT, KeyCode::Up) => self.editor.select_up(),
            (KeyModifiers::SHIFT, KeyCode::Down) => self.editor.select_down(),
            (m, KeyCode::Left) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                self.editor.select_word_left();
            }
            (m, KeyCode::Right) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                self.editor.select_word_right();
            }
            (KeyModifiers::SHIFT, KeyCode::Home) => self.editor.select_to_line_start(),
            (KeyModifiers::SHIFT, KeyCode::End) => self.editor.select_to_line_end(),
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => self.editor.select_all(),
            (KeyModifiers::CONTROL, KeyCode::Char('l')) => self.editor.select_line(),
            (KeyModifiers::CONTROL, KeyCode::Char('z')) => self.editor.undo(),
            (KeyModifiers::CONTROL, KeyCode::Char('y')) => self.editor.redo(),
            (m, KeyCode::Char('z') | KeyCode::Char('Z'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.editor.redo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_current_file(),
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => self.editor.duplicate_line(),
            (m, KeyCode::Char('k') | KeyCode::Char('K'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.editor.delete_line();
            }
            (KeyModifiers::ALT, KeyCode::Up) => self.editor.move_line_up(),
            (KeyModifiers::ALT, KeyCode::Down) => self.editor.move_line_down(),
            (KeyModifiers::CONTROL, KeyCode::Char('/')) => self.editor.toggle_comment(),
            (KeyModifiers::CONTROL, KeyCode::Char(']')) => self.editor.indent(),
            (KeyModifiers::CONTROL, KeyCode::Char('[')) => self.editor.outdent(),
            (KeyModifiers::NONE, KeyCode::Backspace) => self.editor.backspace(),
            (KeyModifiers::NONE, KeyCode::Delete) => self.editor.delete(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.editor.insert_char('\n'),
            (KeyModifiers::NONE, KeyCode::Tab) => self.editor.insert_str("    "),
            (KeyModifiers::SHIFT, KeyCode::Tab) => self.editor.outdent(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            _ => {}
        }
    }

    /// Handles editor keys in normal mode (Vim).
    fn handle_editor_normal_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('i')) => self.editor.set_mode(EditorMode::Insert),
            (KeyModifiers::NONE, KeyCode::Char('a')) => {
                self.editor.move_right();
                self.editor.set_mode(EditorMode::Insert);
            }
            (KeyModifiers::NONE, KeyCode::Char('v')) => {
                self.editor.cursor_mut().start_selection();
                self.editor.set_mode(EditorMode::Visual);
            }
            (KeyModifiers::NONE, KeyCode::Char(':')) => self.editor.set_mode(EditorMode::Command),
            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                self.editor.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Char('l')) | (KeyModifiers::NONE, KeyCode::Right) => {
                self.editor.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                self.editor.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.editor.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Char('0')) => self.editor.move_to_line_start(),
            (KeyModifiers::NONE, KeyCode::Char('$')) | (KeyModifiers::NONE, KeyCode::End) => {
                self.editor.move_to_line_end();
            }
            (KeyModifiers::NONE, KeyCode::Char('w')) => self.editor.move_word_right(),
            (KeyModifiers::NONE, KeyCode::Char('b')) => self.editor.move_word_left(),
            (KeyModifiers::NONE, KeyCode::Char('g')) => self.editor.move_to_buffer_start(),
            (KeyModifiers::SHIFT, KeyCode::Char('G')) => self.editor.move_to_buffer_end(),
            (KeyModifiers::NONE, KeyCode::PageUp) => self.editor.page_up(),
            (KeyModifiers::NONE, KeyCode::PageDown) => self.editor.page_down(),
            (KeyModifiers::NONE, KeyCode::Char('x')) => self.editor.delete(),
            (KeyModifiers::NONE, KeyCode::Char('u')) => self.editor.undo(),
            (KeyModifiers::CONTROL, KeyCode::Char('r')) => self.editor.redo(),
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_current_file(),
            _ => {}
        }
    }

    /// Handles editor keys in insert mode (Vim).
    fn handle_editor_insert_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.editor.set_mode(EditorMode::Normal),
            (KeyModifiers::NONE, KeyCode::Backspace) => self.editor.backspace(),
            (KeyModifiers::NONE, KeyCode::Delete) => self.editor.delete(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.editor.insert_char('\n'),
            (KeyModifiers::NONE, KeyCode::Tab) => self.editor.insert_str("    "),
            (KeyModifiers::NONE, KeyCode::Left) => self.editor.move_left(),
            (KeyModifiers::NONE, KeyCode::Right) => self.editor.move_right(),
            (KeyModifiers::NONE, KeyCode::Up) => self.editor.move_up(),
            (KeyModifiers::NONE, KeyCode::Down) => self.editor.move_down(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_current_file(),
            _ => {}
        }
    }

    /// Handles editor keys in visual mode (Vim).
    fn handle_editor_visual_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.editor.cursor_mut().clear_selection();
                self.editor.set_mode(EditorMode::Normal);
            }
            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                let buffer = self.editor.buffer();
                let mut cursor = self.editor.cursor().clone();
                cursor.move_left(buffer);
                cursor.extend_to(cursor.position());
                *self.editor.cursor_mut() = cursor;
            }
            (KeyModifiers::NONE, KeyCode::Char('l')) | (KeyModifiers::NONE, KeyCode::Right) => {
                let buffer = self.editor.buffer();
                let mut cursor = self.editor.cursor().clone();
                cursor.move_right(buffer);
                cursor.extend_to(cursor.position());
                *self.editor.cursor_mut() = cursor;
            }
            (KeyModifiers::NONE, KeyCode::Char('d')) | (KeyModifiers::NONE, KeyCode::Char('x')) => {
                self.editor.delete_selection();
                self.editor.set_mode(EditorMode::Normal);
            }
            _ => {}
        }
    }

    /// Handles editor keys in command mode (Vim).
    fn handle_editor_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.editor.set_mode(EditorMode::Normal),
            _ => {}
        }
    }
}
