//! Keybinding definitions and parsing.

use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::HashMap;

/// Keybinding mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeybindingMode {
    /// Vim-style keybindings.
    #[default]
    Vim,
    /// Emacs-style keybindings.
    Emacs,
    /// Default/simple keybindings.
    Default,
    /// VSCode-style keybindings.
    VsCode,
}

/// Actions that can be bound to keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAction {
    // Global
    Quit,
    FocusTerminal,
    FocusEditor,
    ToggleFocus,
    SplitLeft,
    SplitRight,

    // File browser
    OpenFileBrowser,
    NextFile,
    PrevFile,

    // Search & Create
    FindInFile,
    FindInFiles,
    SearchDirectories,
    SearchFiles,
    NewFile,
    NewFolder,

    // Clipboard
    Copy,
    Paste,

    // Terminal
    TerminalNewTab,
    TerminalSplit,
    TerminalNextTab,
    TerminalPrevTab,
    TerminalCloseTab,
    TerminalInterrupt,
    TerminalScrollUp,
    TerminalScrollDown,

    // Editor Normal Mode
    EditorInsert,
    EditorAppend,
    EditorVisual,
    EditorCommand,
    EditorLeft,
    EditorRight,
    EditorUp,
    EditorDown,
    EditorLineStart,
    EditorLineEnd,
    EditorWordRight,
    EditorWordLeft,
    EditorBufferStart,
    EditorBufferEnd,
    EditorDelete,
    EditorUndo,
    EditorRedo,
    EditorSave,

    // SSH Manager
    SSHManager,
    SSHQuickConnect1,
    SSHQuickConnect2,
    SSHQuickConnect3,
    SSHQuickConnect4,
    SSHQuickConnect5,
    SSHQuickConnect6,
    SSHQuickConnect7,
    SSHQuickConnect8,
    SSHQuickConnect9,
}

impl KeyAction {
    /// Parses an action from a string key name.
    #[must_use]
    pub fn parse_action(s: &str) -> Option<Self> {
        match s {
            "quit" => Some(Self::Quit),
            "focus_terminal" => Some(Self::FocusTerminal),
            "focus_editor" => Some(Self::FocusEditor),
            "toggle_focus" => Some(Self::ToggleFocus),
            "split_left" => Some(Self::SplitLeft),
            "split_right" => Some(Self::SplitRight),
            "open_file_browser" => Some(Self::OpenFileBrowser),
            "next_file" => Some(Self::NextFile),
            "prev_file" => Some(Self::PrevFile),
            "find_in_file" => Some(Self::FindInFile),
            "find_in_files" => Some(Self::FindInFiles),
            "search_directories" => Some(Self::SearchDirectories),
            "search_files" => Some(Self::SearchFiles),
            "new_file" => Some(Self::NewFile),
            "new_folder" => Some(Self::NewFolder),
            "copy" => Some(Self::Copy),
            "paste" => Some(Self::Paste),
            "terminal_new_tab" => Some(Self::TerminalNewTab),
            "terminal_split" => Some(Self::TerminalSplit),
            "terminal_next_tab" => Some(Self::TerminalNextTab),
            "terminal_prev_tab" => Some(Self::TerminalPrevTab),
            "terminal_close_tab" => Some(Self::TerminalCloseTab),
            "terminal_interrupt" => Some(Self::TerminalInterrupt),
            "terminal_scroll_up" => Some(Self::TerminalScrollUp),
            "terminal_scroll_down" => Some(Self::TerminalScrollDown),
            "editor_insert" => Some(Self::EditorInsert),
            "editor_append" => Some(Self::EditorAppend),
            "editor_visual" => Some(Self::EditorVisual),
            "editor_command" => Some(Self::EditorCommand),
            "editor_left" => Some(Self::EditorLeft),
            "editor_right" => Some(Self::EditorRight),
            "editor_up" => Some(Self::EditorUp),
            "editor_down" => Some(Self::EditorDown),
            "editor_line_start" => Some(Self::EditorLineStart),
            "editor_line_end" => Some(Self::EditorLineEnd),
            "editor_word_right" => Some(Self::EditorWordRight),
            "editor_word_left" => Some(Self::EditorWordLeft),
            "editor_buffer_start" => Some(Self::EditorBufferStart),
            "editor_buffer_end" => Some(Self::EditorBufferEnd),
            "editor_delete" => Some(Self::EditorDelete),
            "editor_undo" => Some(Self::EditorUndo),
            "editor_redo" => Some(Self::EditorRedo),
            "editor_save" => Some(Self::EditorSave),
            "ssh_manager" => Some(Self::SSHManager),
            "ssh_quick_connect_1" => Some(Self::SSHQuickConnect1),
            "ssh_quick_connect_2" => Some(Self::SSHQuickConnect2),
            "ssh_quick_connect_3" => Some(Self::SSHQuickConnect3),
            "ssh_quick_connect_4" => Some(Self::SSHQuickConnect4),
            "ssh_quick_connect_5" => Some(Self::SSHQuickConnect5),
            "ssh_quick_connect_6" => Some(Self::SSHQuickConnect6),
            "ssh_quick_connect_7" => Some(Self::SSHQuickConnect7),
            "ssh_quick_connect_8" => Some(Self::SSHQuickConnect8),
            "ssh_quick_connect_9" => Some(Self::SSHQuickConnect9),
            _ => None,
        }
    }
}

/// A key binding (modifier + key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    /// Key modifiers (Ctrl, Alt, Shift).
    pub modifiers: KeyModifiers,
    /// The key code.
    pub code: KeyCode,
}

impl KeyBinding {
    /// Creates a new key binding.
    #[must_use]
    pub const fn new(modifiers: KeyModifiers, code: KeyCode) -> Self {
        Self { modifiers, code }
    }

    /// Parses a key binding from a string like "ctrl+shift+c".
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = KeyModifiers::NONE;
        let mut key_str = "";

        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                _ => key_str = part,
            }
        }

        let code = parse_key_code(key_str)?;
        Some(Self { modifiers, code })
    }

    /// Checks if this binding matches a key event.
    #[must_use]
    pub fn matches(&self, modifiers: KeyModifiers, code: KeyCode) -> bool {
        self.modifiers == modifiers && self.code == code
    }
}

/// Parses a key code from a string.
fn parse_key_code(s: &str) -> Option<KeyCode> {
    let s_lower = s.to_lowercase();
    match s_lower.as_str() {
        // Letters
        "a" => Some(KeyCode::Char('a')),
        "b" => Some(KeyCode::Char('b')),
        "c" => Some(KeyCode::Char('c')),
        "d" => Some(KeyCode::Char('d')),
        "e" => Some(KeyCode::Char('e')),
        "f" => Some(KeyCode::Char('f')),
        "g" => Some(KeyCode::Char('g')),
        "h" => Some(KeyCode::Char('h')),
        "i" => Some(KeyCode::Char('i')),
        "j" => Some(KeyCode::Char('j')),
        "k" => Some(KeyCode::Char('k')),
        "l" => Some(KeyCode::Char('l')),
        "m" => Some(KeyCode::Char('m')),
        "n" => Some(KeyCode::Char('n')),
        "o" => Some(KeyCode::Char('o')),
        "p" => Some(KeyCode::Char('p')),
        "q" => Some(KeyCode::Char('q')),
        "r" => Some(KeyCode::Char('r')),
        "s" => Some(KeyCode::Char('s')),
        "t" => Some(KeyCode::Char('t')),
        "u" => Some(KeyCode::Char('u')),
        "v" => Some(KeyCode::Char('v')),
        "w" => Some(KeyCode::Char('w')),
        "x" => Some(KeyCode::Char('x')),
        "y" => Some(KeyCode::Char('y')),
        "z" => Some(KeyCode::Char('z')),

        // Special characters
        "[" => Some(KeyCode::Char('[')),
        "]" => Some(KeyCode::Char(']')),
        "0" => Some(KeyCode::Char('0')),
        "$" => Some(KeyCode::Char('$')),
        ":" => Some(KeyCode::Char(':')),

        // Navigation
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),

        // Special
        "enter" | "return" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "esc" | "escape" => Some(KeyCode::Esc),
        "space" => Some(KeyCode::Char(' ')),
        "backspace" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),

        // Function keys
        "f1" => Some(KeyCode::F(1)),
        "f2" => Some(KeyCode::F(2)),
        "f3" => Some(KeyCode::F(3)),
        "f4" => Some(KeyCode::F(4)),
        "f5" => Some(KeyCode::F(5)),
        "f6" => Some(KeyCode::F(6)),
        "f7" => Some(KeyCode::F(7)),
        "f8" => Some(KeyCode::F(8)),
        "f9" => Some(KeyCode::F(9)),
        "f10" => Some(KeyCode::F(10)),
        "f11" => Some(KeyCode::F(11)),
        "f12" => Some(KeyCode::F(12)),

        _ => {
            // Single character
            if s.len() == 1 {
                s.chars().next().map(KeyCode::Char)
            } else {
                None
            }
        }
    }
}

/// Collection of keybindings.
#[derive(Debug, Clone)]
pub struct Keybindings {
    /// Map from action to binding.
    bindings: HashMap<KeyAction, KeyBinding>,
    /// Reverse map from binding to action.
    reverse: HashMap<KeyBinding, KeyAction>,
}

impl Default for Keybindings {
    fn default() -> Self {
        let mut kb = Self {
            bindings: HashMap::new(),
            reverse: HashMap::new(),
        };

        // Set default bindings
        kb.set_defaults();
        kb
    }
}

impl Keybindings {
    /// Creates keybindings for a specific mode.
    #[must_use]
    pub fn for_mode(mode: KeybindingMode) -> Self {
        let mut kb = Self {
            bindings: HashMap::new(),
            reverse: HashMap::new(),
        };

        // Set common bindings first
        kb.set_common_bindings();

        // Then set mode-specific bindings
        match mode {
            KeybindingMode::Vim => kb.set_vim_bindings(),
            KeybindingMode::Emacs => kb.set_emacs_bindings(),
            KeybindingMode::Default => kb.set_default_bindings(),
            KeybindingMode::VsCode => kb.set_vscode_bindings(),
        }

        kb
    }

    /// Sets common bindings (same across all modes).
    fn set_common_bindings(&mut self) {
        use KeyAction::*;
        use KeyCode::*;

        // Global
        self.set(Quit, KeyBinding::new(KeyModifiers::CONTROL, Char('q')));
        self.set(FocusTerminal, KeyBinding::new(KeyModifiers::ALT, Left));
        self.set(FocusEditor, KeyBinding::new(KeyModifiers::ALT, Right));
        self.set(ToggleFocus, KeyBinding::new(KeyModifiers::ALT, Tab));
        self.set(SplitLeft, KeyBinding::new(KeyModifiers::ALT, Char('[')));
        self.set(SplitRight, KeyBinding::new(KeyModifiers::ALT, Char(']')));

        // File browser
        self.set(
            OpenFileBrowser,
            KeyBinding::new(KeyModifiers::CONTROL, Char('o')),
        );
        self.set(
            NextFile,
            KeyBinding::new(KeyModifiers::ALT | KeyModifiers::SHIFT, Right),
        );
        self.set(
            PrevFile,
            KeyBinding::new(KeyModifiers::ALT | KeyModifiers::SHIFT, Left),
        );

        // Search & Create
        self.set(
            FindInFile,
            KeyBinding::new(KeyModifiers::CONTROL, Char('f')),
        );
        self.set(
            FindInFiles,
            KeyBinding::new(KeyModifiers::CONTROL | KeyModifiers::SHIFT, Char('f')),
        );
        self.set(
            SearchDirectories,
            KeyBinding::new(KeyModifiers::CONTROL | KeyModifiers::SHIFT, Char('d')),
        );
        self.set(
            SearchFiles,
            KeyBinding::new(KeyModifiers::CONTROL | KeyModifiers::SHIFT, Char('e')),
        );
        self.set(NewFile, KeyBinding::new(KeyModifiers::CONTROL, Char('n')));
        self.set(
            NewFolder,
            KeyBinding::new(KeyModifiers::CONTROL | KeyModifiers::SHIFT, Char('n')),
        );

        // Clipboard
        self.set(
            Copy,
            KeyBinding::new(KeyModifiers::CONTROL | KeyModifiers::SHIFT, Char('c')),
        );
        self.set(Paste, KeyBinding::new(KeyModifiers::CONTROL, Char('v')));

        // Terminal
        self.set(
            TerminalNewTab,
            KeyBinding::new(KeyModifiers::CONTROL, Char('t')),
        );
        self.set(
            TerminalSplit,
            KeyBinding::new(KeyModifiers::CONTROL, Char('s')),
        );
        self.set(
            TerminalNextTab,
            KeyBinding::new(KeyModifiers::CONTROL, Right),
        );
        self.set(
            TerminalPrevTab,
            KeyBinding::new(KeyModifiers::CONTROL, Left),
        );
        self.set(
            TerminalCloseTab,
            KeyBinding::new(KeyModifiers::CONTROL, Char('w')),
        );
        self.set(
            TerminalInterrupt,
            KeyBinding::new(KeyModifiers::CONTROL, Char('c')),
        );
        self.set(
            TerminalScrollUp,
            KeyBinding::new(KeyModifiers::SHIFT, PageUp),
        );
        self.set(
            TerminalScrollDown,
            KeyBinding::new(KeyModifiers::SHIFT, PageDown),
        );

        // Always have Ctrl+S for save
        self.set(
            EditorSave,
            KeyBinding::new(KeyModifiers::CONTROL, Char('s')),
        );

        // SSH Manager (Ctrl+Shift+U)
        self.set(
            SSHManager,
            KeyBinding::new(KeyModifiers::CONTROL | KeyModifiers::SHIFT, Char('u')),
        );

        // SSH Quick Connect (Ctrl+1-9 by default, configurable)
        self.set(
            SSHQuickConnect1,
            KeyBinding::new(KeyModifiers::CONTROL, Char('1')),
        );
        self.set(
            SSHQuickConnect2,
            KeyBinding::new(KeyModifiers::CONTROL, Char('2')),
        );
        self.set(
            SSHQuickConnect3,
            KeyBinding::new(KeyModifiers::CONTROL, Char('3')),
        );
        self.set(
            SSHQuickConnect4,
            KeyBinding::new(KeyModifiers::CONTROL, Char('4')),
        );
        self.set(
            SSHQuickConnect5,
            KeyBinding::new(KeyModifiers::CONTROL, Char('5')),
        );
        self.set(
            SSHQuickConnect6,
            KeyBinding::new(KeyModifiers::CONTROL, Char('6')),
        );
        self.set(
            SSHQuickConnect7,
            KeyBinding::new(KeyModifiers::CONTROL, Char('7')),
        );
        self.set(
            SSHQuickConnect8,
            KeyBinding::new(KeyModifiers::CONTROL, Char('8')),
        );
        self.set(
            SSHQuickConnect9,
            KeyBinding::new(KeyModifiers::CONTROL, Char('9')),
        );
    }

    /// Sets vim-style editor keybindings.
    fn set_vim_bindings(&mut self) {
        use KeyAction::*;
        use KeyCode::*;

        // Vim normal mode keys
        self.set(EditorInsert, KeyBinding::new(KeyModifiers::NONE, Char('i')));
        self.set(EditorAppend, KeyBinding::new(KeyModifiers::NONE, Char('a')));
        self.set(EditorVisual, KeyBinding::new(KeyModifiers::NONE, Char('v')));
        self.set(
            EditorCommand,
            KeyBinding::new(KeyModifiers::NONE, Char(':')),
        );

        // Vim navigation (hjkl)
        self.set(EditorLeft, KeyBinding::new(KeyModifiers::NONE, Char('h')));
        self.set(EditorRight, KeyBinding::new(KeyModifiers::NONE, Char('l')));
        self.set(EditorUp, KeyBinding::new(KeyModifiers::NONE, Char('k')));
        self.set(EditorDown, KeyBinding::new(KeyModifiers::NONE, Char('j')));

        // Vim motions
        self.set(
            EditorLineStart,
            KeyBinding::new(KeyModifiers::NONE, Char('0')),
        );
        self.set(
            EditorLineEnd,
            KeyBinding::new(KeyModifiers::NONE, Char('$')),
        );
        self.set(
            EditorWordRight,
            KeyBinding::new(KeyModifiers::NONE, Char('w')),
        );
        self.set(
            EditorWordLeft,
            KeyBinding::new(KeyModifiers::NONE, Char('b')),
        );
        self.set(
            EditorBufferStart,
            KeyBinding::new(KeyModifiers::NONE, Char('g')),
        );
        self.set(
            EditorBufferEnd,
            KeyBinding::new(KeyModifiers::SHIFT, Char('G')),
        );

        // Vim editing
        self.set(EditorDelete, KeyBinding::new(KeyModifiers::NONE, Char('x')));
        self.set(EditorUndo, KeyBinding::new(KeyModifiers::NONE, Char('u')));
        self.set(
            EditorRedo,
            KeyBinding::new(KeyModifiers::CONTROL, Char('r')),
        );
    }

    /// Sets emacs-style editor keybindings.
    fn set_emacs_bindings(&mut self) {
        use KeyAction::*;
        use KeyCode::*;

        // Emacs navigation
        self.set(
            EditorLeft,
            KeyBinding::new(KeyModifiers::CONTROL, Char('b')),
        );
        self.set(
            EditorRight,
            KeyBinding::new(KeyModifiers::CONTROL, Char('f')),
        );
        self.set(EditorUp, KeyBinding::new(KeyModifiers::CONTROL, Char('p')));
        self.set(
            EditorDown,
            KeyBinding::new(KeyModifiers::CONTROL, Char('n')),
        );

        // Emacs line navigation
        self.set(
            EditorLineStart,
            KeyBinding::new(KeyModifiers::CONTROL, Char('a')),
        );
        self.set(
            EditorLineEnd,
            KeyBinding::new(KeyModifiers::CONTROL, Char('e')),
        );

        // Emacs word navigation
        self.set(
            EditorWordRight,
            KeyBinding::new(KeyModifiers::ALT, Char('f')),
        );
        self.set(
            EditorWordLeft,
            KeyBinding::new(KeyModifiers::ALT, Char('b')),
        );

        // Emacs buffer navigation
        self.set(
            EditorBufferStart,
            KeyBinding::new(KeyModifiers::ALT | KeyModifiers::SHIFT, Char('<')),
        );
        self.set(
            EditorBufferEnd,
            KeyBinding::new(KeyModifiers::ALT | KeyModifiers::SHIFT, Char('>')),
        );

        // Emacs editing
        self.set(
            EditorDelete,
            KeyBinding::new(KeyModifiers::CONTROL, Char('d')),
        );
        self.set(
            EditorUndo,
            KeyBinding::new(KeyModifiers::CONTROL, Char('/')),
        );
        self.set(
            EditorRedo,
            KeyBinding::new(KeyModifiers::CONTROL | KeyModifiers::SHIFT, Char('/')),
        );

        // Emacs has no modes - always in "insert mode"
        // So these don't apply, but we set them to avoid key conflicts
        self.set(EditorInsert, KeyBinding::new(KeyModifiers::NONE, Enter));
        self.set(EditorAppend, KeyBinding::new(KeyModifiers::NONE, Enter));
    }

    /// Sets simple/default keybindings (arrow keys + standard shortcuts).
    fn set_default_bindings(&mut self) {
        use KeyAction::*;
        use KeyCode::*;

        // Arrow key navigation
        self.set(EditorLeft, KeyBinding::new(KeyModifiers::NONE, Left));
        self.set(EditorRight, KeyBinding::new(KeyModifiers::NONE, Right));
        self.set(EditorUp, KeyBinding::new(KeyModifiers::NONE, Up));
        self.set(EditorDown, KeyBinding::new(KeyModifiers::NONE, Down));

        // Home/End
        self.set(EditorLineStart, KeyBinding::new(KeyModifiers::NONE, Home));
        self.set(EditorLineEnd, KeyBinding::new(KeyModifiers::NONE, End));

        // Ctrl+arrow for word navigation
        self.set(
            EditorWordRight,
            KeyBinding::new(KeyModifiers::CONTROL, Right),
        );
        self.set(EditorWordLeft, KeyBinding::new(KeyModifiers::CONTROL, Left));

        // Ctrl+Home/End for buffer
        self.set(
            EditorBufferStart,
            KeyBinding::new(KeyModifiers::CONTROL, Home),
        );
        self.set(EditorBufferEnd, KeyBinding::new(KeyModifiers::CONTROL, End));

        // Standard shortcuts
        self.set(EditorDelete, KeyBinding::new(KeyModifiers::NONE, Delete));
        self.set(
            EditorUndo,
            KeyBinding::new(KeyModifiers::CONTROL, Char('z')),
        );
        self.set(
            EditorRedo,
            KeyBinding::new(KeyModifiers::CONTROL, Char('y')),
        );

        // Default mode doesn't have insert/visual modes
        self.set(EditorInsert, KeyBinding::new(KeyModifiers::NONE, Enter));
        self.set(EditorAppend, KeyBinding::new(KeyModifiers::NONE, Enter));
    }

    /// Sets VSCode-style editor keybindings.
    fn set_vscode_bindings(&mut self) {
        use KeyAction::*;
        use KeyCode::*;

        // Arrow key navigation (same as default)
        self.set(EditorLeft, KeyBinding::new(KeyModifiers::NONE, Left));
        self.set(EditorRight, KeyBinding::new(KeyModifiers::NONE, Right));
        self.set(EditorUp, KeyBinding::new(KeyModifiers::NONE, Up));
        self.set(EditorDown, KeyBinding::new(KeyModifiers::NONE, Down));

        // Home/End for line navigation
        self.set(EditorLineStart, KeyBinding::new(KeyModifiers::NONE, Home));
        self.set(EditorLineEnd, KeyBinding::new(KeyModifiers::NONE, End));

        // Ctrl+arrow for word navigation
        self.set(
            EditorWordRight,
            KeyBinding::new(KeyModifiers::CONTROL, Right),
        );
        self.set(EditorWordLeft, KeyBinding::new(KeyModifiers::CONTROL, Left));

        // Ctrl+Home/End for buffer navigation
        self.set(
            EditorBufferStart,
            KeyBinding::new(KeyModifiers::CONTROL, Home),
        );
        self.set(EditorBufferEnd, KeyBinding::new(KeyModifiers::CONTROL, End));

        // VSCode standard shortcuts
        self.set(EditorDelete, KeyBinding::new(KeyModifiers::NONE, Delete));
        self.set(
            EditorUndo,
            KeyBinding::new(KeyModifiers::CONTROL, Char('z')),
        );
        self.set(
            EditorRedo,
            KeyBinding::new(KeyModifiers::CONTROL, Char('y')),
        );

        // VSCode mode doesn't have insert/visual modes - always in insert
        self.set(EditorInsert, KeyBinding::new(KeyModifiers::NONE, Enter));
        self.set(EditorAppend, KeyBinding::new(KeyModifiers::NONE, Enter));
    }

    /// Sets default keybindings (vim mode by default).
    fn set_defaults(&mut self) {
        self.set_common_bindings();
        self.set_vim_bindings();
    }

    /// Sets a keybinding for an action.
    pub fn set(&mut self, action: KeyAction, binding: KeyBinding) {
        // Remove old reverse mapping
        if let Some(old_binding) = self.bindings.get(&action) {
            self.reverse.remove(old_binding);
        }
        // Remove any action using this binding
        if let Some(old_action) = self.reverse.get(&binding) {
            self.bindings.remove(old_action);
        }

        self.bindings.insert(action, binding);
        self.reverse.insert(binding, action);
    }

    /// Gets the binding for an action.
    #[must_use]
    pub fn get(&self, action: KeyAction) -> Option<KeyBinding> {
        self.bindings.get(&action).copied()
    }

    /// Gets the action for a binding.
    #[must_use]
    pub fn action_for(&self, modifiers: KeyModifiers, code: KeyCode) -> Option<KeyAction> {
        let binding = KeyBinding::new(modifiers, code);
        self.reverse.get(&binding).copied()
    }

    /// Checks if a binding matches an action.
    #[must_use]
    pub fn matches(&self, action: KeyAction, modifiers: KeyModifiers, code: KeyCode) -> bool {
        self.bindings
            .get(&action)
            .map(|b| b.matches(modifiers, code))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_keybinding() {
        let binding = KeyBinding::parse("ctrl+c").unwrap();
        assert_eq!(binding.modifiers, KeyModifiers::CONTROL);
        assert_eq!(binding.code, KeyCode::Char('c'));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_keybinding_multi_modifier() {
        let binding = KeyBinding::parse("ctrl+shift+c").unwrap();
        assert_eq!(
            binding.modifiers,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        );
        assert_eq!(binding.code, KeyCode::Char('c'));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_keybinding_special_key() {
        let binding = KeyBinding::parse("shift+pageup").unwrap();
        assert_eq!(binding.modifiers, KeyModifiers::SHIFT);
        assert_eq!(binding.code, KeyCode::PageUp);
    }
}
