//! Editor complex editing operations.

use super::{Editor, edit::Position};

impl Editor {
    /// Duplicates the current line.
    pub fn duplicate_line(&mut self) {
        let line_idx = self.cursor.position().line;
        let line_content = self.buffer.line(line_idx).unwrap_or_default();

        // Insert newline at end of current line, then the line content
        let line_len = self.buffer.line_len_chars(line_idx);
        let end_pos = Position::new(line_idx, line_len);

        // Insert newline + duplicate content
        let insert_text = format!("\n{}", line_content.trim_end_matches('\n'));
        self.buffer.insert_str(end_pos, &insert_text);

        // Move cursor to duplicated line
        self.cursor
            .set_position(Position::new(line_idx + 1, self.cursor.position().col));
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Deletes the current line.
    pub fn delete_line(&mut self) {
        let line_idx = self.cursor.position().line;
        let total_lines = self.buffer.len_lines();

        if total_lines == 0 {
            return;
        }

        // Calculate range to delete (entire line including newline)
        let start = Position::new(line_idx, 0);
        let end = if line_idx < total_lines - 1 {
            // Not the last line - delete up to start of next line
            Position::new(line_idx + 1, 0)
        } else if line_idx > 0 {
            // Last line - delete from end of previous line
            let prev_line_len = self.buffer.line_len_chars(line_idx - 1);
            let new_start = Position::new(line_idx - 1, prev_line_len);
            self.buffer.delete_range(
                new_start,
                Position::new(line_idx, self.buffer.line_len_chars(line_idx)),
            );
            self.cursor.set_position(Position::new(
                line_idx - 1,
                prev_line_len.min(self.cursor.position().col),
            ));
            self.view.update_gutter_width(self.buffer.len_lines());
            self.ensure_cursor_visible();
            return;
        } else {
            // Only line - just clear it
            Position::new(0, self.buffer.line_len_chars(0))
        };

        self.buffer.delete_range(start, end);

        // Adjust cursor position
        let new_line = line_idx.min(self.buffer.len_lines().saturating_sub(1));
        let new_col = self
            .cursor
            .position()
            .col
            .min(self.buffer.line_len_chars(new_line));
        self.cursor.set_position(Position::new(new_line, new_col));

        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Moves the current line up.
    pub fn move_line_up(&mut self) {
        let line_idx = self.cursor.position().line;
        if line_idx == 0 {
            return;
        }

        // Get both lines
        let current_line = self.buffer.line(line_idx).unwrap_or_default();
        let prev_line = self.buffer.line(line_idx - 1).unwrap_or_default();

        // Delete both lines and reinsert in swapped order
        let start = Position::new(line_idx - 1, 0);
        let end = if line_idx < self.buffer.len_lines() - 1 {
            Position::new(line_idx + 1, 0)
        } else {
            Position::new(line_idx, self.buffer.line_len_chars(line_idx))
        };

        self.buffer.delete_range(start, end);

        // Insert swapped content
        let new_content = if line_idx < self.buffer.len_lines() {
            format!("{}{}", current_line.trim_end_matches('\n'), prev_line)
        } else {
            format!(
                "{}\n{}",
                current_line.trim_end_matches('\n'),
                prev_line.trim_end_matches('\n')
            )
        };
        self.buffer.insert_str(start, &new_content);

        // Move cursor up
        self.cursor
            .set_position(Position::new(line_idx - 1, self.cursor.position().col));
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Moves the current line down.
    pub fn move_line_down(&mut self) {
        let line_idx = self.cursor.position().line;
        let last_line = self.buffer.len_lines().saturating_sub(1);

        if line_idx >= last_line {
            return;
        }

        // Get both lines
        let current_line = self.buffer.line(line_idx).unwrap_or_default();
        let next_line = self.buffer.line(line_idx + 1).unwrap_or_default();

        // Delete both lines and reinsert in swapped order
        let start = Position::new(line_idx, 0);
        let end = if line_idx + 1 < last_line {
            Position::new(line_idx + 2, 0)
        } else {
            Position::new(line_idx + 1, self.buffer.line_len_chars(line_idx + 1))
        };

        self.buffer.delete_range(start, end);

        // Insert swapped content
        let new_content = if line_idx + 1 < last_line {
            format!("{}{}", next_line.trim_end_matches('\n'), current_line)
        } else {
            format!(
                "{}\n{}",
                next_line.trim_end_matches('\n'),
                current_line.trim_end_matches('\n')
            )
        };
        self.buffer.insert_str(start, &new_content);

        // Move cursor down
        self.cursor
            .set_position(Position::new(line_idx + 1, self.cursor.position().col));
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Toggles line comment on current line or selection.
    pub fn toggle_comment(&mut self) {
        let line_idx = self.cursor.position().line;
        let line = self.buffer.line(line_idx).unwrap_or_default();
        let trimmed = line.trim_start();

        // Detect comment prefix based on file extension or use //
        let comment_prefix = self.detect_comment_prefix();

        if trimmed.starts_with(&comment_prefix) {
            // Uncomment: remove the comment prefix
            if let Some(idx) = line.find(&comment_prefix) {
                let remove_len = if line.len() > idx + comment_prefix.len()
                    && line.chars().nth(idx + comment_prefix.len()) == Some(' ')
                {
                    comment_prefix.len() + 1
                } else {
                    comment_prefix.len()
                };

                let start = Position::new(line_idx, idx);
                let end = Position::new(line_idx, idx + remove_len);
                self.buffer.delete_range(start, end);

                // Adjust cursor if needed
                let cursor_col = self.cursor.position().col;
                if cursor_col > idx {
                    let new_col = cursor_col.saturating_sub(remove_len);
                    self.cursor.set_position(Position::new(line_idx, new_col));
                }
            }
        } else {
            // Comment: add comment prefix at first non-whitespace
            let indent = line.len() - trimmed.len();
            let insert_pos = Position::new(line_idx, indent);
            let comment_text = format!("{} ", comment_prefix);
            self.buffer.insert_str(insert_pos, &comment_text);

            // Adjust cursor
            let cursor_col = self.cursor.position().col;
            if cursor_col >= indent {
                self.cursor
                    .set_position(Position::new(line_idx, cursor_col + comment_text.len()));
            }
        }

        self.view.update_gutter_width(self.buffer.len_lines());
    }

    /// Detects the appropriate comment prefix based on file extension.
    fn detect_comment_prefix(&self) -> String {
        let ext = self
            .path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "py" | "sh" | "bash" | "zsh" | "yaml" | "yml" | "toml" | "rb" | "pl" => "#".to_string(),
            "html" | "xml" => "<!--".to_string(),
            "css" | "scss" | "less" => "/*".to_string(),
            "sql" => "--".to_string(),
            "lua" => "--".to_string(),
            "vim" => "\"".to_string(),
            _ => "//".to_string(),
        }
    }

    /// Indents the current line or selection.
    pub fn indent(&mut self) {
        let line_idx = self.cursor.position().line;
        let indent_str = "    "; // 4 spaces

        let insert_pos = Position::new(line_idx, 0);
        self.buffer.insert_str(insert_pos, indent_str);

        // Move cursor by indent amount
        let new_col = self.cursor.position().col + indent_str.len();
        self.cursor.set_position(Position::new(line_idx, new_col));

        self.view.update_gutter_width(self.buffer.len_lines());
    }

    /// Removes indentation from the current line.
    pub fn outdent(&mut self) {
        let line_idx = self.cursor.position().line;
        let line = self.buffer.line(line_idx).unwrap_or_default();

        // Count leading spaces/tabs to remove (up to 4 spaces or 1 tab)
        let mut remove_count = 0;
        for (i, c) in line.chars().enumerate() {
            if c == ' ' && remove_count < 4 {
                remove_count += 1;
            } else if c == '\t' && remove_count == 0 {
                remove_count = 1;
                break;
            } else {
                break;
            }
            if i >= 3 {
                break;
            }
        }

        if remove_count > 0 {
            let start = Position::new(line_idx, 0);
            let end = Position::new(line_idx, remove_count);
            self.buffer.delete_range(start, end);

            // Adjust cursor position
            let cursor_col = self.cursor.position().col;
            let new_col = cursor_col.saturating_sub(remove_count);
            self.cursor.set_position(Position::new(line_idx, new_col));

            self.view.update_gutter_width(self.buffer.len_lines());
        }
    }
}
