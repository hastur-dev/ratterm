//! The key alphabet the Vim state machine accepts.
//!
//! Deliberately not crossterm's `KeyEvent`: the state machine has to be
//! testable without a terminal, so the input layer translates once at the edge.

/// A single key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimKey {
    /// A printable character, including digits and punctuation.
    Char(char),
    /// Escape.
    Escape,
    /// Return or Enter.
    Enter,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
    /// A control chord, stored as the lowercase base character.
    Ctrl(char),
}

impl VimKey {
    /// Returns the printable character, if this is one.
    #[must_use]
    pub const fn as_char(self) -> Option<char> {
        match self {
            Self::Char(c) => Some(c),
            _ => None,
        }
    }

    /// Returns the decimal digit this key carries, if any.
    #[must_use]
    pub fn digit(self) -> Option<usize> {
        match self {
            Self::Char(c) => c.to_digit(10).map(|d| d as usize),
            _ => None,
        }
    }
}

/// Turns a literal string into the keys that spell it.
///
/// Used to replay a recorded change and to write key sequences in tests.
#[must_use]
pub fn keys(text: &str) -> Vec<VimKey> {
    text.chars().map(VimKey::Char).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn char_keys_expose_their_character_and_digit() {
        assert_eq!(VimKey::Char('d').as_char(), Some('d'));
        assert_eq!(VimKey::Char('7').digit(), Some(7));
        assert_eq!(VimKey::Char('d').digit(), None);
        assert_eq!(VimKey::Escape.as_char(), None);
        assert_eq!(VimKey::Ctrl('r').digit(), None);
    }

    #[test]
    fn keys_spells_out_a_sequence() {
        assert_eq!(
            keys("d2w"),
            vec![VimKey::Char('d'), VimKey::Char('2'), VimKey::Char('w')]
        );
        assert!(keys("").is_empty());
    }
}
