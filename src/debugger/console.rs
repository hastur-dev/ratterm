//! Debug console for expression evaluation and output display.

/// A single entry in the debug console.
#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    /// The kind of entry.
    pub kind: ConsoleEntryKind,
    /// The text content.
    pub text: String,
}

/// Kind of console entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleEntryKind {
    /// User input (expression to evaluate).
    Input,
    /// Result of expression evaluation.
    Output,
    /// Error message.
    Error,
    /// Informational message from the adapter.
    Info,
}

/// Debug console state.
#[derive(Debug, Clone)]
pub struct DebugConsole {
    /// History of console entries.
    entries: Vec<ConsoleEntry>,
    /// Current input text.
    input: String,
    /// Command history for up/down navigation.
    history: Vec<String>,
    /// Current position in command history (-1 = new input).
    history_index: Option<usize>,
    /// Scroll offset for viewing entries.
    scroll_offset: usize,
    /// Maximum number of entries to keep.
    max_entries: usize,
}

impl Default for DebugConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugConsole {
    /// Maximum entries before oldest are discarded.
    const DEFAULT_MAX_ENTRIES: usize = 1000;

    /// Creates a new empty debug console.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            input: String::new(),
            history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
            max_entries: Self::DEFAULT_MAX_ENTRIES,
        }
    }

    /// Adds an entry to the console.
    pub fn push(&mut self, kind: ConsoleEntryKind, text: impl Into<String>) {
        self.entries.push(ConsoleEntry {
            kind,
            text: text.into(),
        });

        // Trim old entries if over limit
        if self.entries.len() > self.max_entries {
            let excess = self.entries.len() - self.max_entries;
            self.entries.drain(..excess);
            self.scroll_offset = self.scroll_offset.saturating_sub(excess);
        }
    }

    /// Adds output text to the console.
    pub fn push_output(&mut self, text: impl Into<String>) {
        self.push(ConsoleEntryKind::Output, text);
    }

    /// Adds an error message to the console.
    pub fn push_error(&mut self, text: impl Into<String>) {
        self.push(ConsoleEntryKind::Error, text);
    }

    /// Adds an info message to the console.
    pub fn push_info(&mut self, text: impl Into<String>) {
        self.push(ConsoleEntryKind::Info, text);
    }

    /// Submits the current input, adding it to entries and history.
    /// Returns the input text if non-empty.
    pub fn submit_input(&mut self) -> Option<String> {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return None;
        }

        self.push(ConsoleEntryKind::Input, text.clone());
        self.history.push(text.clone());
        self.history_index = None;
        self.input.clear();
        Some(text)
    }

    /// Returns the current input text.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Inserts a character into the input.
    pub fn insert_char(&mut self, c: char) {
        self.input.push(c);
    }

    /// Removes the last character from the input.
    pub fn backspace(&mut self) {
        self.input.pop();
    }

    /// Clears the input.
    pub fn clear_input(&mut self) {
        self.input.clear();
        self.history_index = None;
    }

    /// Navigates to the previous history entry.
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let idx = match self.history_index {
            Some(i) if i > 0 => i - 1,
            Some(_) => return, // Already at oldest
            None => self.history.len() - 1,
        };

        self.history_index = Some(idx);
        self.input = self.history[idx].clone();
    }

    /// Navigates to the next history entry.
    pub fn history_next(&mut self) {
        let Some(idx) = self.history_index else {
            return;
        };

        if idx + 1 >= self.history.len() {
            self.history_index = None;
            self.input.clear();
        } else {
            self.history_index = Some(idx + 1);
            self.input = self.history[idx + 1].clone();
        }
    }

    /// Returns all entries.
    #[must_use]
    pub fn entries(&self) -> &[ConsoleEntry] {
        &self.entries
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the current scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls up by one line.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scrolls down by one line.
    pub fn scroll_down(&mut self) {
        if self.scroll_offset < self.entries.len().saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_console_is_empty() {
        let console = DebugConsole::new();
        assert!(console.entries().is_empty());
        assert!(console.input().is_empty());
        assert_eq!(console.entry_count(), 0);
    }

    #[test]
    fn test_push_entries() {
        let mut console = DebugConsole::new();
        console.push_output("hello");
        console.push_error("oops");
        console.push_info("info");
        assert_eq!(console.entry_count(), 3);
        assert_eq!(console.entries()[0].kind, ConsoleEntryKind::Output);
        assert_eq!(console.entries()[1].kind, ConsoleEntryKind::Error);
        assert_eq!(console.entries()[2].kind, ConsoleEntryKind::Info);
    }

    #[test]
    fn test_input_and_submit() {
        let mut console = DebugConsole::new();
        console.insert_char('x');
        console.insert_char(' ');
        console.insert_char('+');
        console.insert_char(' ');
        console.insert_char('1');
        assert_eq!(console.input(), "x + 1");

        let submitted = console.submit_input();
        assert_eq!(submitted.as_deref(), Some("x + 1"));
        assert!(console.input().is_empty());
        assert_eq!(console.entry_count(), 1);
        assert_eq!(console.entries()[0].kind, ConsoleEntryKind::Input);
    }

    #[test]
    fn test_submit_empty_returns_none() {
        let mut console = DebugConsole::new();
        assert!(console.submit_input().is_none());
    }

    #[test]
    fn test_backspace() {
        let mut console = DebugConsole::new();
        console.insert_char('a');
        console.insert_char('b');
        console.backspace();
        assert_eq!(console.input(), "a");
    }

    #[test]
    fn test_history_navigation() {
        let mut console = DebugConsole::new();

        // Submit two commands
        console.insert_char('a');
        console.submit_input();
        console.insert_char('b');
        console.submit_input();

        // Navigate back
        console.history_prev();
        assert_eq!(console.input(), "b");
        console.history_prev();
        assert_eq!(console.input(), "a");

        // Navigate forward
        console.history_next();
        assert_eq!(console.input(), "b");
        console.history_next();
        assert!(console.input().is_empty());
    }

    #[test]
    fn test_history_prev_empty() {
        let mut console = DebugConsole::new();
        console.history_prev(); // Should not panic
        assert!(console.input().is_empty());
    }

    #[test]
    fn test_clear() {
        let mut console = DebugConsole::new();
        console.push_output("test");
        console.push_output("test2");
        console.clear();
        assert_eq!(console.entry_count(), 0);
        assert_eq!(console.scroll_offset(), 0);
    }

    #[test]
    fn test_scroll() {
        let mut console = DebugConsole::new();
        for i in 0..10 {
            console.push_output(format!("line {}", i));
        }
        assert_eq!(console.scroll_offset(), 0);
        console.scroll_down();
        assert_eq!(console.scroll_offset(), 1);
        console.scroll_up();
        assert_eq!(console.scroll_offset(), 0);
        // Can't scroll above 0
        console.scroll_up();
        assert_eq!(console.scroll_offset(), 0);
    }
}
