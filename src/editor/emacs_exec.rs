//! Running an Emacs command against the editor.
//!
//! [`emacs`](super::emacs) owns the kill ring and the mark;
//! [`emacs_keys`](super::emacs_keys) turns chords into commands. This file is
//! where a command becomes an edit.

use super::emacs::{EmacsCommand, KillRing, MarkState};
use super::{Editor, EditorMode, Position};

/// The Emacs state that has to survive between keystrokes.
#[derive(Debug, Clone, Default)]
pub struct EmacsState {
    /// The kill ring.
    pub kill_ring: KillRing,
    /// The mark and whether the region is active.
    pub mark: MarkState,
    /// True while a `C-x` prefix is waiting for its second key.
    pub prefix_pending: bool,
    /// The `M-x` prompt's text, when it is open.
    pub prompt: Option<String>,
}

impl EmacsState {
    /// Creates empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kill_ring: KillRing::new(),
            mark: MarkState::new(),
            prefix_pending: false,
            prompt: None,
        }
    }

    /// Drops the things that belong to the keystroke in progress.
    ///
    /// The kill ring survives a document switch the way a real Emacs session
    /// does; a half-typed `C-x` or `M-x` does not.
    pub fn reset_transient(&mut self) {
        self.prefix_pending = false;
        self.prompt = None;
        self.mark.clear();
        self.kill_ring.end_kill_sequence();
    }
}

/// Something the editor cannot do itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmacsEffect {
    /// Nothing further is needed.
    None,
    /// `C-x C-s` — write the file.
    Save,
    /// `C-x C-f` — open the file picker.
    FindFile,
    /// `C-x C-c` — leave.
    Quit,
    /// `C-s` / `C-r` — open the find bar, forward or backward.
    Search {
        /// True for `C-s`.
        forward: bool,
    },
    /// `M-%` — open the find bar with the replace field.
    QueryReplace,
}

impl Editor {
    /// Returns the Emacs state.
    #[must_use]
    pub const fn emacs(&self) -> &EmacsState {
        &self.emacs
    }

    /// Returns the Emacs state for mutation.
    pub fn emacs_mut(&mut self) -> &mut EmacsState {
        &mut self.emacs
    }

    /// Returns the text a yank would insert.
    #[must_use]
    pub fn kill_ring_head(&self) -> Option<&str> {
        self.emacs.kill_ring.yank()
    }

    /// Runs one Emacs command.
    #[allow(clippy::too_many_lines)]
    pub fn run_emacs(&mut self, command: EmacsCommand) -> EmacsEffect {
        // Every command that is not a kill breaks the run of kills, so the next
        // `C-k` starts a fresh entry rather than appending to the last one.
        if !is_kill(command) {
            self.emacs.kill_ring.end_kill_sequence();
        }

        match command {
            EmacsCommand::ForwardChar => self.move_with_mark(|e| e.move_right()),
            EmacsCommand::BackwardChar => self.move_with_mark(|e| e.move_left()),
            EmacsCommand::NextLine => self.move_with_mark(Editor::move_down_visible),
            EmacsCommand::PreviousLine => self.move_with_mark(Editor::move_up_visible),
            EmacsCommand::MoveBeginningOfLine => self.move_with_mark(|e| e.move_to_line_start()),
            EmacsCommand::MoveEndOfLine => self.move_with_mark(|e| e.move_to_line_end()),
            EmacsCommand::ForwardWord => self.move_with_mark(|e| e.move_word_right()),
            EmacsCommand::BackwardWord => self.move_with_mark(|e| e.move_word_left()),
            EmacsCommand::BeginningOfBuffer => self.move_with_mark(|e| e.move_to_buffer_start()),
            EmacsCommand::EndOfBuffer => self.move_with_mark(|e| e.move_to_buffer_end()),
            EmacsCommand::ScrollUpCommand => self.move_with_mark(|e| e.page_down()),
            EmacsCommand::ScrollDownCommand => self.move_with_mark(|e| e.page_up()),
            EmacsCommand::GotoLine => {}
            EmacsCommand::RecenterTopBottom => {
                let line = self.cursor.position().line;
                let total = self.buffer.len_lines();
                self.view.center_on_line(line, total);
            }
            EmacsCommand::SetMarkCommand => {
                let pos = self.cursor.position();
                self.emacs.mark.set_mark(pos);
                self.cursor.start_selection();
            }
            EmacsCommand::ExchangePointAndMark => {
                let point = self.cursor.position();
                if let Some(target) = self.emacs.mark.exchange_point_and_mark(point) {
                    // The region stays where it is; only which end the cursor
                    // sits on changes, so the old point becomes the anchor.
                    self.cursor.move_to(point);
                    self.cursor.start_selection();
                    self.cursor.extend_to(self.buffer.clamp_position(target));
                    self.ensure_cursor_visible();
                }
            }
            EmacsCommand::MarkWholeBuffer => {
                self.emacs.mark.set_mark(Position::new(0, 0));
                self.select_all();
            }
            EmacsCommand::KillLine => {
                let text = self.delete_to_line_end();
                self.emacs.kill_ring.kill_append(text);
            }
            EmacsCommand::KillWord => {
                let start = self.cursor.position();
                let end = self.word_boundary_right();
                let text = self.apply_delete(start, end);
                self.emacs.kill_ring.kill_append(text);
                self.after_edit();
            }
            EmacsCommand::BackwardKillWord => {
                let end = self.cursor.position();
                let start = self.word_boundary_left();
                let text = self.apply_delete(start, end);
                self.cursor.move_to(self.buffer.clamp_position(start));
                self.emacs.kill_ring.kill_prepend(text);
                self.after_edit();
            }
            EmacsCommand::KillRegion | EmacsCommand::KillRingSave => {
                if let Some((start, end)) = self.emacs_region() {
                    let text = self.buffer.get_range(start, end).unwrap_or_default();
                    self.emacs.kill_ring.kill(text);
                    if command == EmacsCommand::KillRegion {
                        self.apply_delete(start, end);
                        self.cursor.move_to(self.buffer.clamp_position(start));
                        self.after_edit();
                    }
                    self.emacs.mark.deactivate();
                    self.cursor.clear_selection();
                }
            }
            EmacsCommand::Yank => {
                if let Some(text) = self.emacs.kill_ring.yank().map(str::to_string) {
                    self.insert_str(&text);
                }
            }
            EmacsCommand::YankPop => {
                if let Some(text) = self.emacs.kill_ring.yank_pop().map(str::to_string) {
                    self.undo();
                    self.insert_str(&text);
                }
            }
            EmacsCommand::DeleteChar => self.delete(),
            EmacsCommand::DeleteBackwardChar => self.backspace(),
            EmacsCommand::TransposeChars => self.transpose_chars(),
            EmacsCommand::OpenLine => {
                let pos = self.cursor.position();
                self.insert_char('\n');
                self.cursor.move_to(pos);
            }
            EmacsCommand::Newline => self.insert_char('\n'),
            EmacsCommand::NewlineAndIndent => self.insert_newline(),
            EmacsCommand::IndentForTabCommand => {
                if self.cursor.has_selection() {
                    self.indent_selection();
                } else {
                    self.insert_indent_unit();
                }
            }
            EmacsCommand::CommentDwim => self.toggle_comment(),
            EmacsCommand::Undo => self.undo(),
            EmacsCommand::KeyboardQuit => {
                self.emacs.mark.deactivate();
                self.emacs.prefix_pending = false;
                self.emacs.prompt = None;
                self.cursor.clear_selection();
                self.clear_extra_cursors();
                self.close_search();
                self.set_mode(EditorMode::Normal);
            }
            EmacsCommand::WhatCursorPosition => {
                let pos = self.cursor.position();
                self.set_status(format!("Line {} Column {}", pos.line + 1, pos.col + 1));
            }
            EmacsCommand::IsearchForward => return EmacsEffect::Search { forward: true },
            EmacsCommand::IsearchBackward => return EmacsEffect::Search { forward: false },
            EmacsCommand::QueryReplace => return EmacsEffect::QueryReplace,
            EmacsCommand::SaveBuffer => return EmacsEffect::Save,
            EmacsCommand::FindFile => return EmacsEffect::FindFile,
            EmacsCommand::SaveBuffersKillTerminal => return EmacsEffect::Quit,
        }
        EmacsEffect::None
    }

    /// Runs a movement, extending the region when the mark is active.
    fn move_with_mark(&mut self, movement: impl FnOnce(&mut Self)) {
        let anchor = if self.emacs.mark.is_active() {
            self.emacs.mark.mark()
        } else {
            None
        };
        movement(self);
        match anchor {
            Some(mark) => {
                let point = self.cursor.position();
                self.cursor.move_to(mark);
                self.cursor.start_selection();
                self.cursor.extend_to(point);
            }
            None => self.cursor.clear_selection(),
        }
    }

    /// Returns the active region, ordered.
    fn emacs_region(&self) -> Option<(Position, Position)> {
        self.emacs
            .mark
            .region(self.cursor.position())
            .filter(|(start, end)| start != end)
    }

    /// Returns where `M-f` would land.
    fn word_boundary_right(&self) -> Position {
        let mut cursor = self.cursor.clone();
        cursor.move_word_right(&self.buffer);
        cursor.position()
    }

    /// Returns where `M-b` would land.
    fn word_boundary_left(&self) -> Position {
        let mut cursor = self.cursor.clone();
        cursor.move_word_left(&self.buffer);
        cursor.position()
    }

    /// `C-t`: swaps the two characters around the cursor.
    fn transpose_chars(&mut self) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let len = self.buffer.line_len_chars(pos.line);
        if len < 2 {
            return;
        }
        // At the end of a line Emacs swaps the last two characters.
        let right = pos.col.clamp(1, len - 1);
        let start = Position::new(pos.line, right - 1);
        let end = Position::new(pos.line, right + 1);
        let Some(text) = self.buffer.get_range(start, end) else {
            return;
        };
        let mut chars = text.chars();
        let (Some(a), Some(b)) = (chars.next(), chars.next()) else {
            return;
        };
        self.buffer.begin_undo_group();
        self.apply_delete(start, end);
        self.apply_insert(start, &format!("{b}{a}"));
        self.buffer.end_undo_group();
        self.cursor.move_to(Position::new(pos.line, right + 1));
        self.after_edit();
    }
}

/// Returns true when a command adds to the kill ring.
const fn is_kill(command: EmacsCommand) -> bool {
    matches!(
        command,
        EmacsCommand::KillLine
            | EmacsCommand::KillWord
            | EmacsCommand::BackwardKillWord
            | EmacsCommand::KillRegion
            | EmacsCommand::KillRingSave
    )
}

#[cfg(test)]
mod tests;
