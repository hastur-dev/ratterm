//! The Emacs key map, and the `C-x` prefix it needs.
//!
//! Chords arrive as [`EmacsKey`] rather than crossterm events so the map can be
//! exercised without a terminal; `input_editor.rs` does the one translation at
//! the edge.

use super::emacs::EmacsCommand;
use super::{Editor, Position};

/// One key press, as the Emacs map sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmacsKey {
    /// A printable character with no modifier.
    Char(char),
    /// `C-<char>`.
    Ctrl(char),
    /// `M-<char>`, from Alt or from a preceding Escape.
    Alt(char),
    /// `C-M-<char>`.
    CtrlAlt(char),
    /// Return.
    Enter,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Tab.
    Tab,
    /// `M-DEL`.
    AltBackspace,
    /// An arrow or navigation key.
    Nav(NavKey),
}

/// The navigation keys Emacs mode maps to movement commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
}

/// What a key press meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmacsInput {
    /// A resolved command.
    Command(EmacsCommand),
    /// A character to insert.
    Insert(char),
    /// A prefix was started; the next key completes it.
    Prefix,
    /// The key means nothing here.
    Unbound,
}

/// Maps a key that is not part of a prefix.
#[must_use]
pub fn command_for(key: EmacsKey) -> EmacsInput {
    use EmacsCommand as C;
    let command = match key {
        EmacsKey::Ctrl('x') => return EmacsInput::Prefix,
        EmacsKey::Ctrl('a') => C::MoveBeginningOfLine,
        EmacsKey::Ctrl('e') => C::MoveEndOfLine,
        EmacsKey::Ctrl('f') => C::ForwardChar,
        EmacsKey::Ctrl('b') => C::BackwardChar,
        EmacsKey::Ctrl('n') => C::NextLine,
        EmacsKey::Ctrl('p') => C::PreviousLine,
        EmacsKey::Ctrl('v') => C::ScrollUpCommand,
        EmacsKey::Ctrl('l') => C::RecenterTopBottom,
        EmacsKey::Ctrl('k') => C::KillLine,
        EmacsKey::Ctrl('w') => C::KillRegion,
        EmacsKey::Ctrl('y') => C::Yank,
        EmacsKey::Ctrl(' ') | EmacsKey::Ctrl('@') => C::SetMarkCommand,
        EmacsKey::Ctrl('d') => C::DeleteChar,
        EmacsKey::Ctrl('t') => C::TransposeChars,
        EmacsKey::Ctrl('o') => C::OpenLine,
        EmacsKey::Ctrl('j') => C::NewlineAndIndent,
        EmacsKey::Ctrl('_') | EmacsKey::Ctrl('/') => C::Undo,
        EmacsKey::Ctrl('s') => C::IsearchForward,
        EmacsKey::Ctrl('r') => C::IsearchBackward,
        EmacsKey::Ctrl('g') => C::KeyboardQuit,
        EmacsKey::Alt('f') => C::ForwardWord,
        EmacsKey::Alt('b') => C::BackwardWord,
        EmacsKey::Alt('<') => C::BeginningOfBuffer,
        EmacsKey::Alt('>') => C::EndOfBuffer,
        EmacsKey::Alt('v') => C::ScrollDownCommand,
        EmacsKey::Alt('d') => C::KillWord,
        EmacsKey::Alt('w') => C::KillRingSave,
        EmacsKey::Alt('y') => C::YankPop,
        EmacsKey::Alt(';') => C::CommentDwim,
        EmacsKey::Alt('%') => C::QueryReplace,
        EmacsKey::Alt('g') => C::GotoLine,
        EmacsKey::AltBackspace => C::BackwardKillWord,
        EmacsKey::Enter => C::Newline,
        EmacsKey::Tab => C::IndentForTabCommand,
        EmacsKey::Backspace => C::DeleteBackwardChar,
        EmacsKey::Delete => C::DeleteChar,
        EmacsKey::Nav(NavKey::Left) => C::BackwardChar,
        EmacsKey::Nav(NavKey::Right) => C::ForwardChar,
        EmacsKey::Nav(NavKey::Up) => C::PreviousLine,
        EmacsKey::Nav(NavKey::Down) => C::NextLine,
        EmacsKey::Nav(NavKey::Home) => C::MoveBeginningOfLine,
        EmacsKey::Nav(NavKey::End) => C::MoveEndOfLine,
        EmacsKey::Nav(NavKey::PageUp) => C::ScrollDownCommand,
        EmacsKey::Nav(NavKey::PageDown) => C::ScrollUpCommand,
        EmacsKey::Char(c) => return EmacsInput::Insert(c),
        _ => return EmacsInput::Unbound,
    };
    EmacsInput::Command(command)
}

/// Maps the key that follows `C-x`.
#[must_use]
pub fn command_after_prefix(key: EmacsKey) -> EmacsInput {
    use EmacsCommand as C;
    let command = match key {
        EmacsKey::Ctrl('s') => C::SaveBuffer,
        EmacsKey::Ctrl('f') => C::FindFile,
        EmacsKey::Ctrl('c') => C::SaveBuffersKillTerminal,
        EmacsKey::Ctrl('x') => C::ExchangePointAndMark,
        EmacsKey::Char('h') => C::MarkWholeBuffer,
        EmacsKey::Char('=') => C::WhatCursorPosition,
        EmacsKey::Char('u') => C::Undo,
        _ => return EmacsInput::Unbound,
    };
    EmacsInput::Command(command)
}

impl Editor {
    /// Feeds one key to the Emacs key map, running whatever it resolves to.
    ///
    /// Returns the effect the application still has to carry out. A `C-x`
    /// prefix is remembered between calls.
    pub fn feed_emacs_key(&mut self, key: EmacsKey) -> super::emacs_exec::EmacsEffect {
        use super::emacs_exec::EmacsEffect;

        let input = if self.emacs.prefix_pending {
            self.emacs.prefix_pending = false;
            command_after_prefix(key)
        } else {
            command_for(key)
        };

        match input {
            EmacsInput::Prefix => {
                self.emacs.prefix_pending = true;
                EmacsEffect::None
            }
            EmacsInput::Command(command) => self.run_emacs(command),
            EmacsInput::Insert(c) => {
                self.emacs.kill_ring.end_kill_sequence();
                self.emacs.mark.deactivate();
                self.type_char(c);
                EmacsEffect::None
            }
            EmacsInput::Unbound => EmacsEffect::None,
        }
    }

    /// Returns true while a `C-x` prefix is waiting for its second key.
    #[must_use]
    pub const fn emacs_prefix_pending(&self) -> bool {
        self.emacs.prefix_pending
    }

    /// Moves the cursor to a 1-based line number, as `M-g g` does.
    pub fn goto_line_number(&mut self, line: usize) {
        let target = line.saturating_sub(1);
        self.goto_line(target);
        let landing = self
            .buffer
            .first_non_whitespace(self.cursor.position().line)
            .unwrap_or(Position::new(self.cursor.position().line, 0));
        self.cursor.move_to(landing);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::emacs_exec::EmacsEffect;

    fn editor_with(text: &str) -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(text);
        editor.set_cursor_position(Position::new(0, 0));
        editor
    }

    #[test]
    fn the_movement_chords_resolve() {
        assert_eq!(
            command_for(EmacsKey::Ctrl('a')),
            EmacsInput::Command(EmacsCommand::MoveBeginningOfLine)
        );
        assert_eq!(
            command_for(EmacsKey::Alt('f')),
            EmacsInput::Command(EmacsCommand::ForwardWord)
        );
        assert_eq!(
            command_for(EmacsKey::Nav(NavKey::Down)),
            EmacsInput::Command(EmacsCommand::NextLine)
        );
    }

    #[test]
    fn a_plain_character_is_an_insertion() {
        assert_eq!(command_for(EmacsKey::Char('z')), EmacsInput::Insert('z'));
    }

    #[test]
    fn an_unbound_chord_reports_itself() {
        assert_eq!(command_for(EmacsKey::CtrlAlt('q')), EmacsInput::Unbound);
        assert_eq!(
            command_after_prefix(EmacsKey::Char('z')),
            EmacsInput::Unbound
        );
    }

    #[test]
    fn ctrl_x_starts_a_prefix_that_the_next_key_completes() {
        assert_eq!(command_for(EmacsKey::Ctrl('x')), EmacsInput::Prefix);
        assert_eq!(
            command_after_prefix(EmacsKey::Ctrl('s')),
            EmacsInput::Command(EmacsCommand::SaveBuffer)
        );
    }

    #[test]
    fn the_editor_remembers_the_prefix_between_keys() {
        let mut editor = editor_with("abc\n");
        assert_eq!(
            editor.feed_emacs_key(EmacsKey::Ctrl('x')),
            EmacsEffect::None
        );
        assert!(editor.emacs_prefix_pending());
        assert_eq!(
            editor.feed_emacs_key(EmacsKey::Ctrl('s')),
            EmacsEffect::Save
        );
        assert!(!editor.emacs_prefix_pending());
    }

    #[test]
    fn typing_through_the_map_inserts_text() {
        let mut editor = editor_with("");
        for c in "hi".chars() {
            editor.feed_emacs_key(EmacsKey::Char(c));
        }
        assert_eq!(editor.buffer().text(), "hi");
    }

    #[test]
    fn ctrl_k_then_ctrl_y_moves_a_line_through_the_kill_ring() {
        let mut editor = editor_with("one\ntwo\n");
        editor.set_cursor_position(Position::new(0, 0));
        editor.feed_emacs_key(EmacsKey::Ctrl('k'));
        assert_eq!(editor.buffer().text(), "\ntwo\n");
        editor.feed_emacs_key(EmacsKey::Ctrl('e'));
        editor.set_cursor_position(Position::new(1, 3));
        editor.feed_emacs_key(EmacsKey::Ctrl('y'));
        assert_eq!(editor.buffer().text(), "\ntwoone\n");
    }

    #[test]
    fn ctrl_space_then_a_move_selects() {
        let mut editor = editor_with("hello\n");
        editor.feed_emacs_key(EmacsKey::Ctrl(' '));
        editor.feed_emacs_key(EmacsKey::Ctrl('e'));
        assert_eq!(editor.selected_text().as_deref(), Some("hello"));
    }

    #[test]
    fn tab_inserts_the_documents_indent_unit() {
        let mut editor = editor_with("");
        editor.feed_emacs_key(EmacsKey::Tab);
        assert_eq!(editor.buffer().text(), "    ");
    }

    #[test]
    fn goto_line_lands_on_the_first_non_blank_character() {
        let mut editor = editor_with("a\n    b\nc\n");
        editor.goto_line_number(2);
        assert_eq!(editor.cursor_position(), Position::new(1, 4));
        // Past the end it clamps rather than failing.
        editor.goto_line_number(999);
        assert!(editor.cursor_position().line < editor.buffer().len_lines());
    }

    #[test]
    fn an_unbound_key_changes_nothing() {
        let mut editor = editor_with("abc\n");
        editor.feed_emacs_key(EmacsKey::CtrlAlt('q'));
        assert_eq!(editor.buffer().text(), "abc\n");
        assert_eq!(editor.cursor_position(), Position::new(0, 0));
    }
}
