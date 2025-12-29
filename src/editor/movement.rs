//! Editor cursor movement methods.

use super::{Editor, edit::Position};

impl Editor {
    /// Moves cursor left.
    pub fn move_left(&mut self) {
        self.cursor.move_left(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor right.
    pub fn move_right(&mut self) {
        self.cursor.move_right(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor up.
    pub fn move_up(&mut self) {
        self.cursor.move_up(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor down.
    pub fn move_down(&mut self) {
        self.cursor.move_down(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to line start.
    pub fn move_to_line_start(&mut self) {
        self.cursor.move_to_line_start();
        self.ensure_cursor_visible();
    }

    /// Moves cursor to line end.
    pub fn move_to_line_end(&mut self) {
        self.cursor.move_to_line_end(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to buffer start.
    pub fn move_to_buffer_start(&mut self) {
        self.cursor.move_to_buffer_start();
        self.ensure_cursor_visible();
    }

    /// Moves cursor to buffer end.
    pub fn move_to_buffer_end(&mut self) {
        self.cursor.move_to_buffer_end(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor up by a page.
    pub fn page_up(&mut self) {
        let page_size = self.view.height().saturating_sub(2);
        self.cursor.move_page_up(&self.buffer, page_size);
        self.ensure_cursor_visible();
    }

    /// Moves cursor down by a page.
    pub fn page_down(&mut self) {
        let page_size = self.view.height().saturating_sub(2);
        self.cursor.move_page_down(&self.buffer, page_size);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to previous word.
    pub fn move_word_left(&mut self) {
        self.cursor.move_word_left(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to next word.
    pub fn move_word_right(&mut self) {
        self.cursor.move_word_right(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Gets the word at the cursor position.
    #[must_use]
    pub fn word_at_cursor(&self) -> Option<String> {
        let start = self.buffer.word_start(self.cursor.position());
        let end = self.buffer.word_end(self.cursor.position());
        self.buffer.get_range(start, end)
    }

    /// Returns the cursor position.
    #[must_use]
    pub fn cursor_position(&self) -> Position {
        self.cursor.position()
    }

    /// Sets the cursor position.
    pub fn set_cursor_position(&mut self, pos: Position) {
        self.cursor.set_position(self.buffer.clamp_position(pos));
        self.ensure_cursor_visible();
    }

    /// Goes to a specific line.
    pub fn goto_line(&mut self, line: usize) {
        let line = line.min(self.buffer.len_lines().saturating_sub(1));
        self.cursor.set_position(Position::new(line, 0));
        self.view.center_on_line(line, self.buffer.len_lines());
    }
}
