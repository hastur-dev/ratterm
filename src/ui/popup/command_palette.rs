//! Command palette for quick command access.

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::config::command_palette_hotkey;

/// A command that can be executed from the command palette.
#[derive(Debug, Clone)]
pub struct Command {
    /// Unique identifier for the command.
    pub id: &'static str,
    /// Display label for the command.
    pub label: &'static str,
    /// Category for grouping commands.
    pub category: &'static str,
    /// Keyboard shortcut hint.
    pub keybinding: Option<&'static str>,
}

impl Command {
    /// Creates a new command.
    #[must_use]
    pub const fn new(
        id: &'static str,
        label: &'static str,
        category: &'static str,
        keybinding: Option<&'static str>,
    ) -> Self {
        Self {
            id,
            label,
            category,
            keybinding,
        }
    }

    /// Returns formatted display string for command palette.
    #[must_use]
    pub fn display(&self) -> String {
        // Special handling for the command palette keybinding on Windows 11
        let kb = if self.id == "app.commandPalette" {
            Some(command_palette_hotkey())
        } else {
            self.keybinding
        };

        if let Some(key) = kb {
            format!("{}: {}  ({})", self.category, self.label, key)
        } else {
            format!("{}: {}", self.category, self.label)
        }
    }
}

/// Command palette state and filtering.
pub struct CommandPalette {
    /// All available commands.
    commands: Vec<Command>,
    /// Filtered commands matching current query.
    filtered: Vec<(usize, i64)>, // (index, score)
    /// Fuzzy matcher for filtering.
    matcher: SkimMatcherV2,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    /// Creates a new command palette with all available commands.
    #[must_use]
    pub fn new() -> Self {
        let commands = Self::all_commands();
        let filtered: Vec<(usize, i64)> =
            commands.iter().enumerate().map(|(i, _)| (i, 0)).collect();

        Self {
            commands,
            filtered,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Returns all available commands.
    fn all_commands() -> Vec<Command> {
        vec![
            // File commands
            Command::new("file.new", "New File", "File", Some("Ctrl+N")),
            Command::new("file.newFolder", "New Folder", "File", Some("Ctrl+Shift+N")),
            Command::new("file.open", "Open File Browser", "File", Some("Ctrl+O")),
            Command::new("file.save", "Save", "File", Some("Ctrl+S")),
            Command::new("file.close", "Close File", "File", None),
            // Edit commands
            Command::new("edit.undo", "Undo", "Edit", Some("Ctrl+Z")),
            Command::new("edit.redo", "Redo", "Edit", Some("Ctrl+Y")),
            Command::new("edit.copy", "Copy", "Edit", Some("Ctrl+Shift+C")),
            Command::new("edit.paste", "Paste", "Edit", Some("Ctrl+V")),
            Command::new("edit.selectAll", "Select All", "Edit", Some("Ctrl+A")),
            Command::new("edit.selectLine", "Select Line", "Edit", Some("Ctrl+L")),
            Command::new(
                "edit.duplicateLine",
                "Duplicate Line",
                "Edit",
                Some("Ctrl+D"),
            ),
            Command::new(
                "edit.deleteLine",
                "Delete Line",
                "Edit",
                Some("Ctrl+Shift+K"),
            ),
            Command::new("edit.moveLineUp", "Move Line Up", "Edit", Some("Alt+Up")),
            Command::new(
                "edit.moveLineDown",
                "Move Line Down",
                "Edit",
                Some("Alt+Down"),
            ),
            Command::new(
                "edit.toggleComment",
                "Toggle Comment",
                "Edit",
                Some("Ctrl+/"),
            ),
            Command::new("edit.indent", "Indent", "Edit", Some("Tab")),
            Command::new("edit.outdent", "Outdent", "Edit", Some("Shift+Tab")),
            // Search commands
            Command::new("search.inFile", "Find in File", "Search", Some("Ctrl+F")),
            Command::new(
                "search.inFiles",
                "Find in All Files",
                "Search",
                Some("Ctrl+Shift+F"),
            ),
            Command::new(
                "search.files",
                "Search Files",
                "Search",
                Some("Ctrl+Shift+E"),
            ),
            Command::new(
                "search.directories",
                "Search Directories",
                "Search",
                Some("Ctrl+Shift+D"),
            ),
            // View commands
            Command::new(
                "view.focusTerminal",
                "Focus Terminal",
                "View",
                Some("Alt+Left"),
            ),
            Command::new(
                "view.focusEditor",
                "Focus Editor",
                "View",
                Some("Alt+Right"),
            ),
            Command::new("view.toggleFocus", "Toggle Focus", "View", Some("Alt+Tab")),
            Command::new("view.splitLeft", "Shrink Split", "View", Some("Alt+[")),
            Command::new("view.splitRight", "Expand Split", "View", Some("Alt+]")),
            // Terminal commands
            Command::new("terminal.new", "New Terminal", "Terminal", Some("Ctrl+T")),
            Command::new(
                "terminal.split",
                "Split Terminal",
                "Terminal",
                Some("Ctrl+S"),
            ),
            Command::new(
                "terminal.close",
                "Close Terminal",
                "Terminal",
                Some("Ctrl+W"),
            ),
            Command::new(
                "terminal.nextTab",
                "Next Terminal Tab",
                "Terminal",
                Some("Ctrl+Right"),
            ),
            Command::new(
                "terminal.prevTab",
                "Previous Terminal Tab",
                "Terminal",
                Some("Ctrl+Left"),
            ),
            Command::new("terminal.selectShell", "Select Shell", "Terminal", None),
            // SSH commands
            Command::new(
                "ssh.manager",
                "Open SSH Manager",
                "SSH",
                Some("Ctrl+Shift+U"),
            ),
            Command::new("ssh.scan", "Scan Network", "SSH", None),
            Command::new("ssh.addHost", "Add Host", "SSH", None),
            Command::new("ssh.connect1", "Quick Connect #1", "SSH", Some("Ctrl+1")),
            Command::new("ssh.connect2", "Quick Connect #2", "SSH", Some("Ctrl+2")),
            Command::new("ssh.connect3", "Quick Connect #3", "SSH", Some("Ctrl+3")),
            // Docker commands
            Command::new(
                "docker.manager",
                "Open Docker Manager",
                "Docker",
                Some("Ctrl+Shift+D"),
            ),
            Command::new("docker.refresh", "Refresh Containers", "Docker", None),
            Command::new(
                "docker.connect1",
                "Quick Connect #1",
                "Docker",
                Some("Ctrl+Alt+1"),
            ),
            Command::new(
                "docker.connect2",
                "Quick Connect #2",
                "Docker",
                Some("Ctrl+Alt+2"),
            ),
            Command::new(
                "docker.connect3",
                "Quick Connect #3",
                "Docker",
                Some("Ctrl+Alt+3"),
            ),
            Command::new(
                "docker.connect4",
                "Quick Connect #4",
                "Docker",
                Some("Ctrl+Alt+4"),
            ),
            Command::new(
                "docker.connect5",
                "Quick Connect #5",
                "Docker",
                Some("Ctrl+Alt+5"),
            ),
            Command::new(
                "docker.connect6",
                "Quick Connect #6",
                "Docker",
                Some("Ctrl+Alt+6"),
            ),
            Command::new(
                "docker.connect7",
                "Quick Connect #7",
                "Docker",
                Some("Ctrl+Alt+7"),
            ),
            Command::new(
                "docker.connect8",
                "Quick Connect #8",
                "Docker",
                Some("Ctrl+Alt+8"),
            ),
            Command::new(
                "docker.connect9",
                "Quick Connect #9",
                "Docker",
                Some("Ctrl+Alt+9"),
            ),
            Command::new("docker.stats", "Show Stats Panel", "Docker", Some("Ctrl+T")),
            Command::new("docker.logs", "Show Logs Panel", "Docker", Some("Ctrl+L")),
            // Add-on commands
            Command::new(
                "addon.manager",
                "Open Add-ons Manager",
                "Add-ons",
                Some("Ctrl+Shift+A"),
            ),
            Command::new("addon.refresh", "Refresh Add-ons List", "Add-ons", None),
            // Theme commands
            Command::new("theme.select", "Select Theme", "Theme", None),
            Command::new("theme.dark", "Dark Theme", "Theme", None),
            Command::new("theme.light", "Light Theme", "Theme", None),
            Command::new("theme.dracula", "Dracula Theme", "Theme", None),
            Command::new("theme.gruvbox", "Gruvbox Theme", "Theme", None),
            Command::new("theme.nord", "Nord Theme", "Theme", None),
            // Extension commands
            Command::new("extension.list", "List Installed", "Extension", None),
            Command::new(
                "extension.install",
                "Install from GitHub",
                "Extension",
                None,
            ),
            Command::new("extension.update", "Update All", "Extension", None),
            Command::new("extension.remove", "Remove Extension", "Extension", None),
            // Application commands
            Command::new("app.quit", "Quit", "Application", Some("Ctrl+Q")),
            Command::new(
                "app.commandPalette",
                "Command Palette",
                "Application",
                Some("Ctrl+Shift+P"),
            ),
            Command::new(
                "app.switchEditorMode",
                "Switch Editor Mode",
                "Application",
                Some("Ctrl+Shift+Tab"),
            ),
        ]
    }

    /// Filters commands based on query string.
    pub fn filter(&mut self, query: &str) {
        if query.is_empty() {
            // Show all commands when query is empty
            self.filtered = self
                .commands
                .iter()
                .enumerate()
                .map(|(i, _)| (i, 0))
                .collect();
            return;
        }

        let mut matches: Vec<(usize, i64)> = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(idx, cmd)| {
                let search_text = format!("{} {}", cmd.category, cmd.label);
                self.matcher
                    .fuzzy_match(&search_text, query)
                    .map(|score| (idx, score))
            })
            .collect();

        // Sort by score descending
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered = matches;
    }

    /// Returns filtered command display strings.
    #[must_use]
    pub fn results(&self) -> Vec<String> {
        self.filtered
            .iter()
            .filter_map(|(idx, _)| self.commands.get(*idx))
            .map(Command::display)
            .collect()
    }

    /// Returns the command at the given filtered index.
    #[must_use]
    pub fn get_command(&self, filtered_index: usize) -> Option<&Command> {
        self.filtered
            .get(filtered_index)
            .and_then(|(idx, _)| self.commands.get(*idx))
    }

    /// Returns number of filtered results.
    #[must_use]
    pub fn len(&self) -> usize {
        self.filtered.len()
    }

    /// Returns true if no filtered results.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filtered.is_empty()
    }
}
