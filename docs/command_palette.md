# Command Palette Reference

The Command Palette provides quick access to all Ratterm commands. Open it with:

- **Windows 11**: `F1`
- **Other platforms**: `Ctrl+Shift+P` or `Ctrl+P`

Type to filter commands, use `Up`/`Down` to navigate, and `Enter` to execute.

---

## File Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `File: New File` | `Ctrl+N` | Create a new file |
| `File: New Folder` | `Ctrl+Shift+N` | Create a new folder |
| `File: Open` | `Ctrl+O` | Open file browser |
| `File: Save` | `Ctrl+S` | Save current file |
| `File: Close` | `Ctrl+W` | Close current file |

---

## Edit Commands

### Basic Editing

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Edit: Undo` | `Ctrl+Z` | Undo last action |
| `Edit: Redo` | `Ctrl+Y` | Redo undone action |
| `Edit: Copy` | `Ctrl+Shift+C` | Copy selection to clipboard |
| `Edit: Paste` | `Ctrl+V` | Paste from clipboard |

### Selection

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Edit: Select All` | `Ctrl+A` | Select all text |
| `Edit: Select Line` | `Ctrl+L` | Select current line |

### Line Operations

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Edit: Duplicate Line` | `Ctrl+D` | Duplicate current line |
| `Edit: Delete Line` | `Ctrl+Shift+K` | Delete current line |
| `Edit: Move Line Up` | `Alt+Up` | Move line up |
| `Edit: Move Line Down` | `Alt+Down` | Move line down |

### Code Formatting

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Edit: Toggle Comment` | `Ctrl+/` | Toggle line comment |
| `Edit: Indent` | `Tab` | Increase indentation |
| `Edit: Outdent` | `Shift+Tab` | Decrease indentation |

---

## Search Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Search: Find in File` | `Ctrl+F` | Search in current file |
| `Search: Find in Files` | `Ctrl+Shift+F` | Search across all files |
| `Search: Search Files` | `Ctrl+Shift+E` | Search for files by name |
| `Search: Search Directories` | `Ctrl+Shift+D` | Search for directories |

---

## View Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `View: Focus Terminal` | `Alt+Left` | Focus terminal pane |
| `View: Focus Editor` | `Alt+Right` | Focus editor pane |
| `View: Toggle Focus` | `Alt+Tab` | Toggle between panes |
| `View: Toggle IDE` | `Ctrl+I` | Show/hide IDE pane |
| `View: Shrink Split` | `Alt+[` | Move divider left |
| `View: Expand Split` | `Alt+]` | Move divider right |

---

## Terminal Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Terminal: New Tab` | `Ctrl+T` | Create new terminal tab |
| `Terminal: Close Tab` | `Ctrl+W` | Close current terminal tab |
| `Terminal: Next Tab` | `Ctrl+Right` | Switch to next tab |
| `Terminal: Previous Tab` | `Ctrl+Left` | Switch to previous tab |
| `Terminal: Split Horizontal` | `Ctrl+S` | Split terminal horizontally |
| `Terminal: Split Vertical` | `Ctrl+Shift+S` | Split terminal vertically |
| `Terminal: Close Split` | `Ctrl+Shift+W` | Close current split |
| `Terminal: Select Shell` | - | Open shell selector |

---

## SSH Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `SSH: Open Manager` | `Ctrl+Shift+U` | Open SSH Manager |
| `SSH: Scan Network` | - | Scan for SSH hosts |
| `SSH: Add Host` | - | Add SSH host manually |
| `SSH: Quick Connect #1` | `Ctrl+1` | Connect to saved host #1 |
| `SSH: Quick Connect #2` | `Ctrl+2` | Connect to saved host #2 |
| `SSH: Quick Connect #3` | `Ctrl+3` | Connect to saved host #3 |
| `SSH: Health Dashboard` | - | Open SSH health monitoring |

---

## Docker Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Docker: Open Manager` | `Ctrl+Shift+D` | Open Docker Manager |
| `Docker: Refresh` | - | Refresh container list |
| `Docker: Quick Connect #1-9` | `Ctrl+Alt+1-9` | Connect to container slot |

---

## Theme Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Theme: Select Theme` | - | Open theme selector |
| `Theme: Dark` | - | Apply Dark theme |
| `Theme: Light` | - | Apply Light theme |
| `Theme: Dracula` | - | Apply Dracula theme |
| `Theme: Gruvbox` | - | Apply Gruvbox theme |
| `Theme: Nord` | - | Apply Nord theme |

---

## Editor Mode Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Mode: Switch Editor Mode` | `Ctrl+Shift+Tab` | Cycle through Vim/Emacs/Default |
| `Mode: Vim` | - | Switch to Vim mode |
| `Mode: Emacs` | - | Switch to Emacs mode |
| `Mode: Default` | - | Switch to Default mode |

---

## Extension Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Extension: List Installed` | - | Show installed extensions |
| `Extension: Install from GitHub` | - | Shows install command |
| `Extension: Update All` | - | Shows update command |
| `Extension: Remove` | - | Shows remove command |

---

## Application Commands

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Application: Quit` | `Ctrl+Q` | Quit Ratterm |
| `Application: Check Updates` | - | Check for updates |

---

## Command Palette Navigation

| Key | Action |
|-----|--------|
| `Esc` | Close palette |
| `Enter` | Execute selected command |
| `Up` / `Down` | Navigate commands |
| `Ctrl+P` / `Ctrl+N` | Navigate commands (Emacs style) |
| Type text | Filter commands |

---

## Quick Tips

1. **Fuzzy Search**: Type partial command names to filter (e.g., "th sel" matches "Theme: Select Theme")
2. **Recent Commands**: Recently used commands appear at the top
3. **Hotkey Display**: Commands show their keyboard shortcut on the right
4. **Categories**: Commands are grouped by category (File, Edit, View, etc.)
