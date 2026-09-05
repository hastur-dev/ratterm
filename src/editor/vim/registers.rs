//! Vim registers.
//!
//! Yanks go to register `0` and to the unnamed register; deletes go to the
//! numbered ring and to the unnamed register. Naming a register with `"a`
//! redirects the write; an uppercase name appends to the lowercase one.

use std::collections::{HashMap, VecDeque};

/// How many numbered delete registers (`"1` through `"9`) are kept.
pub const NUMBERED_REGISTERS: usize = 9;

/// The text in a register, and how it should be put back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterContent {
    /// The stored text.
    pub text: String,
    /// True when the text was taken as whole lines and should be put on its own
    /// lines rather than inside the current one.
    pub linewise: bool,
}

impl RegisterContent {
    /// Creates character-wise content.
    #[must_use]
    pub fn charwise(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            linewise: false,
        }
    }

    /// Creates line-wise content.
    #[must_use]
    pub fn linewise(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            linewise: true,
        }
    }
}

/// The full set of registers for one editing session.
#[derive(Debug, Clone, Default)]
pub struct Registers {
    named: HashMap<char, RegisterContent>,
    unnamed: Option<RegisterContent>,
    yank: Option<RegisterContent>,
    numbered: VecDeque<RegisterContent>,
}

impl Registers {
    /// Creates an empty register set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a yank.
    ///
    /// Without a name this fills register `0` and the unnamed register; with one
    /// it fills that register and the unnamed register, leaving `0` alone.
    pub fn set_yank(&mut self, register: Option<char>, content: RegisterContent) {
        match register {
            Some(name) => self.write_named(name, &content),
            None => self.yank = Some(content.clone()),
        }
        self.unnamed = Some(content);
    }

    /// Stores a delete or change.
    ///
    /// Without a name this pushes onto the numbered ring and fills the unnamed
    /// register.
    pub fn set_delete(&mut self, register: Option<char>, content: RegisterContent) {
        match register {
            Some(name) => self.write_named(name, &content),
            None => {
                self.numbered.push_front(content.clone());
                while self.numbered.len() > NUMBERED_REGISTERS {
                    self.numbered.pop_back();
                }
            }
        }
        self.unnamed = Some(content);
    }

    /// Writes a named register, appending when the name is uppercase.
    fn write_named(&mut self, name: char, content: &RegisterContent) {
        if name.is_uppercase() {
            let key = name.to_ascii_lowercase();
            let entry = self.named.entry(key).or_default();
            entry.text.push_str(&content.text);
            entry.linewise = entry.linewise || content.linewise;
        } else {
            self.named.insert(name, content.clone());
        }
    }

    /// Reads a register. `None` and `"` both mean the unnamed register.
    #[must_use]
    pub fn get(&self, register: Option<char>) -> Option<&RegisterContent> {
        match register {
            None | Some('"') => self.unnamed.as_ref(),
            Some('0') => self.yank.as_ref(),
            Some(d) if d.is_ascii_digit() => {
                let index = d.to_digit(10)? as usize;
                self.numbered.get(index.saturating_sub(1))
            }
            Some(name) if name.is_alphabetic() => self.named.get(&name.to_ascii_lowercase()),
            Some(_) => None,
        }
    }

    /// Empties every register.
    pub fn clear(&mut self) {
        self.named.clear();
        self.unnamed = None;
        self.yank = None;
        self.numbered.clear();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_yank_fills_register_zero_and_the_unnamed_register() {
        let mut regs = Registers::new();
        regs.set_yank(None, RegisterContent::linewise("line\n"));
        assert_eq!(regs.get(None).map(|c| c.text.as_str()), Some("line\n"));
        assert_eq!(regs.get(Some('0')).map(|c| c.text.as_str()), Some("line\n"));
        assert!(regs.get(None).map(|c| c.linewise) == Some(true));
    }

    #[test]
    fn a_named_yank_round_trips() {
        let mut regs = Registers::new();
        regs.set_yank(Some('a'), RegisterContent::linewise("one\n"));
        assert_eq!(regs.get(Some('a')).map(|c| c.text.as_str()), Some("one\n"));
        // The unnamed register follows along, but register 0 does not.
        assert_eq!(regs.get(None).map(|c| c.text.as_str()), Some("one\n"));
        assert_eq!(regs.get(Some('0')), None);
    }

    #[test]
    fn deletes_push_the_numbered_ring_and_leave_register_zero_alone() {
        let mut regs = Registers::new();
        regs.set_yank(None, RegisterContent::charwise("yanked"));
        regs.set_delete(None, RegisterContent::charwise("first"));
        regs.set_delete(None, RegisterContent::charwise("second"));

        assert_eq!(regs.get(Some('0')).map(|c| c.text.as_str()), Some("yanked"));
        assert_eq!(regs.get(Some('1')).map(|c| c.text.as_str()), Some("second"));
        assert_eq!(regs.get(Some('2')).map(|c| c.text.as_str()), Some("first"));
        assert_eq!(regs.get(None).map(|c| c.text.as_str()), Some("second"));
    }

    #[test]
    fn the_numbered_ring_is_bounded() {
        let mut regs = Registers::new();
        for i in 0..12 {
            regs.set_delete(None, RegisterContent::charwise(format!("d{i}")));
        }
        assert_eq!(regs.get(Some('1')).map(|c| c.text.as_str()), Some("d11"));
        assert_eq!(regs.get(Some('9')).map(|c| c.text.as_str()), Some("d3"));
    }

    #[test]
    fn an_uppercase_name_appends() {
        let mut regs = Registers::new();
        regs.set_yank(Some('a'), RegisterContent::charwise("one"));
        regs.set_yank(Some('A'), RegisterContent::charwise("two"));
        assert_eq!(regs.get(Some('a')).map(|c| c.text.as_str()), Some("onetwo"));
        assert_eq!(regs.get(Some('A')).map(|c| c.text.as_str()), Some("onetwo"));
    }

    #[test]
    fn unknown_and_empty_registers_read_as_none() {
        let regs = Registers::new();
        assert_eq!(regs.get(None), None);
        assert_eq!(regs.get(Some('z')), None);
        assert_eq!(regs.get(Some('%')), None);
    }

    #[test]
    fn clearing_empties_everything() {
        let mut regs = Registers::new();
        regs.set_yank(Some('a'), RegisterContent::charwise("x"));
        regs.set_delete(None, RegisterContent::charwise("y"));
        regs.clear();
        assert_eq!(regs.get(Some('a')), None);
        assert_eq!(regs.get(None), None);
        assert_eq!(regs.get(Some('1')), None);
    }
}
