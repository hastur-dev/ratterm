//! The character walker word motions are built on.
//!
//! Separated from `motion.rs` so neither file grows past the project's size
//! limit.

use crate::editor::buffer::{Buffer, Position};

use super::motion::{Class, class, last_line, line_chars};

/// A character-at-a-time walker that crosses line boundaries.
///
/// The position one past a line's last character reads as `'\n'` so word
/// motions treat line breaks as whitespace without special cases.
pub(super) struct Walker<'a> {
    buffer: &'a Buffer,
    line: usize,
    chars: Vec<char>,
    col: usize,
    last: usize,
}

impl<'a> Walker<'a> {
    pub(super) fn new(buffer: &'a Buffer, pos: Position) -> Self {
        let last = last_line(buffer);
        let line = pos.line.min(last);
        let chars = line_chars(buffer, line);
        let col = pos.col.min(chars.len());
        Self {
            buffer,
            line,
            chars,
            col,
            last,
        }
    }

    pub(super) fn pos(&self) -> Position {
        Position::new(self.line, self.col)
    }

    fn goto(&mut self, line: usize, col: usize) {
        if line != self.line {
            self.line = line;
            self.chars = line_chars(self.buffer, line);
        }
        self.col = col.min(self.chars.len());
    }

    fn current(&self) -> Option<char> {
        if self.col < self.chars.len() {
            self.chars.get(self.col).copied()
        } else if self.line < self.last {
            Some('\n')
        } else {
            None
        }
    }

    fn advance(&mut self) -> bool {
        if self.col < self.chars.len() {
            self.col += 1;
            true
        } else if self.line < self.last {
            self.goto(self.line + 1, 0);
            true
        } else {
            false
        }
    }

    fn retreat(&mut self) -> bool {
        if self.col > 0 {
            self.col -= 1;
            true
        } else if self.line > 0 {
            let line = self.line - 1;
            let len = line_chars(self.buffer, line).len();
            self.goto(line, len);
            true
        } else {
            false
        }
    }

    fn peek_next(&self) -> Option<char> {
        if self.col + 1 < self.chars.len() {
            self.chars.get(self.col + 1).copied()
        } else if self.col < self.chars.len() && self.line < self.last {
            Some('\n')
        } else if self.line < self.last {
            line_chars(self.buffer, self.line + 1)
                .first()
                .copied()
                .or(Some('\n'))
        } else {
            None
        }
    }
}

/// Steps one `w` motion. Returns false when the buffer end stopped it.
pub(super) fn step_word_forward(walker: &mut Walker<'_>, big: bool, budget: &mut usize) -> bool {
    let start = walker.current().map(class);
    if let Some(cls) = start
        && cls != Class::Blank
    {
        while let Some(c) = walker.current() {
            let same = if big {
                class(c) != Class::Blank
            } else {
                class(c) == cls
            };
            if !same || *budget == 0 {
                break;
            }
            *budget -= 1;
            if !walker.advance() {
                return false;
            }
        }
    }
    while let Some(c) = walker.current() {
        if class(c) != Class::Blank || *budget == 0 {
            break;
        }
        *budget -= 1;
        if !walker.advance() {
            return false;
        }
    }
    true
}

/// Steps one `b` motion.
pub(super) fn step_word_back(walker: &mut Walker<'_>, big: bool, budget: &mut usize) -> bool {
    if !walker.retreat() {
        return false;
    }
    while let Some(c) = walker.current() {
        if class(c) != Class::Blank || *budget == 0 {
            break;
        }
        *budget -= 1;
        if !walker.retreat() {
            return false;
        }
    }
    let Some(cls) = walker.current().map(class) else {
        return false;
    };
    loop {
        let (line, col) = (walker.line, walker.col);
        if *budget == 0 || !walker.retreat() {
            break;
        }
        *budget -= 1;
        let matches = walker.current().is_some_and(|c| {
            if big {
                class(c) != Class::Blank
            } else {
                class(c) == cls
            }
        });
        if !matches {
            walker.goto(line, col);
            break;
        }
    }
    true
}

/// Steps one `e` motion.
pub(super) fn step_word_end(walker: &mut Walker<'_>, big: bool, budget: &mut usize) -> bool {
    if !walker.advance() {
        return false;
    }
    while let Some(c) = walker.current() {
        if class(c) != Class::Blank || *budget == 0 {
            break;
        }
        *budget -= 1;
        if !walker.advance() {
            return false;
        }
    }
    let Some(cls) = walker.current().map(class) else {
        return false;
    };
    while let Some(next) = walker.peek_next() {
        let same = if big {
            class(next) != Class::Blank
        } else {
            class(next) == cls
        };
        if !same || *budget == 0 {
            break;
        }
        *budget -= 1;
        if !walker.advance() {
            break;
        }
    }
    true
}
