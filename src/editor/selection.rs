//! Editor selection methods.

use super::{Editor, edit::Position};

impl Editor {
    /// Extends selection left by one character.
    pub fn select_left(&mut self) {
        let buffer = &self.buffer;
        let old_pos = self.cursor.position();

        let new_pos = if old_pos.col > 0 {
            Position::new(old_pos.line, old_pos.col - 1)
        } else if old_pos.line > 0 {
            let prev_line_len = buffer.line_len_chars(old_pos.line - 1);
            Position::new(old_pos.line - 1, prev_line_len)
        } else {
            old_pos
        };

        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection right by one character.
    pub fn select_right(&mut self) {
        let buffer = &self.buffer;
        let old_pos = self.cursor.position();
        let line_len = buffer.line_len_chars(old_pos.line);

        let new_pos = if old_pos.col < line_len {
            Position::new(old_pos.line, old_pos.col + 1)
        } else if old_pos.line < buffer.len_lines().saturating_sub(1) {
            Position::new(old_pos.line + 1, 0)
        } else {
            old_pos
        };

        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection up by one line.
    pub fn select_up(&mut self) {
        let old_pos = self.cursor.position();
        if old_pos.line == 0 {
            return;
        }

        let new_line = old_pos.line - 1;
        let line_len = self.buffer.line_len_chars(new_line);
        let new_col = old_pos.col.min(line_len);

        self.cursor.extend_to(Position::new(new_line, new_col));
        self.ensure_cursor_visible();
    }

    /// Extends selection down by one line.
    pub fn select_down(&mut self) {
        let old_pos = self.cursor.position();
        let last_line = self.buffer.len_lines().saturating_sub(1);

        if old_pos.line >= last_line {
            return;
        }

        let new_line = old_pos.line + 1;
        let line_len = self.buffer.line_len_chars(new_line);
        let new_col = old_pos.col.min(line_len);

        self.cursor.extend_to(Position::new(new_line, new_col));
        self.ensure_cursor_visible();
    }

    /// Extends selection left by one word.
    pub fn select_word_left(&mut self) {
        let mut idx = self.buffer.position_to_index(self.cursor.position());

        if idx == 0 {
            return;
        }

        let text = self.buffer.text();
        let chars: Vec<char> = text.chars().collect();

        // Skip whitespace
        while idx > 0 && chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        // Skip word characters
        while idx > 0 && !chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        let new_pos = self.buffer.index_to_position(idx);
        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection right by one word.
    pub fn select_word_right(&mut self) {
        let mut idx = self.buffer.position_to_index(self.cursor.position());
        let len = self.buffer.len_chars();

        if idx >= len {
            return;
        }

        let text = self.buffer.text();
        let chars: Vec<char> = text.chars().collect();

        // Skip word characters
        while idx < len && !chars[idx].is_whitespace() {
            idx += 1;
        }

        // Skip whitespace
        while idx < len && chars[idx].is_whitespace() {
            idx += 1;
        }

        let new_pos = self.buffer.index_to_position(idx);
        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection to line start.
    pub fn select_to_line_start(&mut self) {
        let pos = self.cursor.position();
        self.cursor.extend_to(Position::new(pos.line, 0));
        self.ensure_cursor_visible();
    }

    /// Extends selection to line end.
    pub fn select_to_line_end(&mut self) {
        let pos = self.cursor.position();
        let line_len = self.buffer.line_len_chars(pos.line);
        self.cursor.extend_to(Position::new(pos.line, line_len));
        self.ensure_cursor_visible();
    }

    /// Selects all text in the buffer.
    pub fn select_all(&mut self) {
        self.cursor.select_all(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Selects the current line.
    pub fn select_line(&mut self) {
        self.cursor.select_line(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Returns the currently selected text, if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        self.cursor
            .selection_range()
            .and_then(|(start, end)| self.buffer.get_range(start, end))
    }

    /// Returns the current line text.
    #[must_use]
    pub fn current_line(&self) -> String {
        let line = self.cursor.position().line;
        self.buffer.line(line).unwrap_or_default()
    }
}
