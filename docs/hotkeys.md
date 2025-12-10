# Ratterm Hotkeys Reference

This document lists all keyboard shortcuts available in Ratterm.

## Global Hotkeys (Work Everywhere)

These hotkeys work regardless of which pane is focused or what mode you're in.

| Hotkey | Action |
|--------|--------|
| `Ctrl+Q` | Quit application |
| `Ctrl+I` | Toggle IDE pane visibility |
| `Ctrl+P` | Open Command Palette |
| `Ctrl+Shift+P` | Open Command Palette (alternative) |
| `Ctrl+Shift+Tab` | Switch Editor Mode (cycles Vim/Emacs/Default/VSCode) |
| `Ctrl+O` | Open File Browser |
| `Ctrl+Shift+C` | Copy selection |
| `Ctrl+V` | Paste from clipboard |
| `Alt+Left` | Focus Terminal pane |
| `Alt+Right` | Focus Editor pane (when IDE visible) |
| `Alt+Tab` | Toggle focus between panes (when IDE visible) |
| `Alt+Up` / `Alt+Down` | Navigate between terminal grid panes |
| `Alt+[` | Shrink split (move divider left) |
| `Alt+]` | Expand split (move divider right) |
| `Alt+Shift+Left` | Previous file tab |
| `Alt+Shift+Right` | Next file tab |

---

## Terminal Hotkeys

These hotkeys work when the terminal pane is focused.

### Tab Management

| Hotkey | Action |
|--------|--------|
| `Ctrl+T` | New terminal tab |
| `Ctrl+W` | Close current terminal tab |
| `Ctrl+Left` | Previous terminal tab |
| `Ctrl+Right` | Next terminal tab |

### Terminal Grid (Split Management)

Terminals can be split into a 2x2 grid:
- First split creates 2 panes side-by-side (vertical split)
- Second split creates a 2x2 grid (4 panes)

| Hotkey | Action |
|--------|--------|
| `Ctrl+S` | Split current terminal (progressive: 1→2→4) |
| `Ctrl+Shift+S` | Split current terminal (same as Ctrl+S) |
| `Ctrl+Shift+W` | Close current terminal pane |
| `Ctrl+Tab` | Cycle focus between grid panes |
| `Alt+Up` / `Alt+Down` | Navigate grid vertically |
| `Alt+Left` / `Alt+Right` | Navigate grid horizontally (when in grid) |

### Scrolling & Input

| Hotkey | Action |
|--------|--------|
| `Shift+PageUp` | Scroll up in terminal history |
| `Shift+PageDown` | Scroll down in terminal history |
| `Ctrl+C` | Send interrupt signal |

### Text Selection

| Hotkey | Action |
|--------|--------|
| `Click+Drag` | Select text with mouse |
| `Shift+Left` | Extend selection left by one character |
| `Shift+Right` | Extend selection right by one character |
| `Shift+Up` | Extend selection up by one line |
| `Shift+Down` | Extend selection down by one line |
| `Ctrl+Shift+C` | Copy selection (or current line if no selection) |
| `Mouse Scroll` | Scroll terminal view up/down |

### Terminal Commands

Type these commands directly in the terminal:

| Command | Action |
|---------|--------|
| `open` | Open file browser (shows IDE pane if hidden) |
| `open <file>` | Open specific file in editor (shows IDE pane) |
| `update` | Check for updates and auto-update if available |

**Note:** The `open` command will automatically show the IDE pane if it's hidden.

---

## Editor Hotkeys (Common to All Modes)

These hotkeys work in the editor regardless of keybinding mode.

| Hotkey | Action |
|--------|--------|
| `Ctrl+T` | New editor tab (untitled buffer) |
| `Ctrl+W` | Close current editor tab |
| `Ctrl+F` | Find in file |
| `Ctrl+Shift+F` | Find in all files |
| `Ctrl+Shift+D` | Search directories |
| `Ctrl+Shift+E` | Search files |
| `Ctrl+N` | Create new file |
| `Ctrl+Shift+N` | Create new folder |

---

## Editor Hotkeys by Mode

### Default Mode

Standard editing with arrow key navigation.

| Hotkey | Action |
|--------|--------|
| `Arrow Keys` | Move cursor |
| `Home` / `End` | Line start/end |
| `Ctrl+Left` / `Ctrl+Right` | Word navigation |
| `Ctrl+Home` / `Ctrl+End` | Buffer start/end |
| `PageUp` / `PageDown` | Page navigation |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+S` | Save |
| `Backspace` | Delete before cursor |
| `Delete` | Delete at cursor |
| `Tab` | Insert spaces |

---

### Vim Mode

Modal editing with Normal, Insert, Visual, and Command modes.

#### Normal Mode

| Hotkey | Action |
|--------|--------|
| `i` | Enter Insert mode |
| `a` | Append after cursor (Insert mode) |
| `v` | Enter Visual mode |
| `:` | Enter Command mode |
| `h` / `Left` | Move left |
| `l` / `Right` | Move right |
| `k` / `Up` | Move up |
| `j` / `Down` | Move down |
| `0` | Line start |
| `$` / `End` | Line end |
| `w` | Next word |
| `b` | Previous word |
| `g` | Buffer start |
| `G` | Buffer end |
| `PageUp` / `PageDown` | Page navigation |
| `x` | Delete character |
| `u` | Undo |
| `Ctrl+R` | Redo |
| `Ctrl+S` | Save |

#### Insert Mode

| Hotkey | Action |
|--------|--------|
| `Esc` | Return to Normal mode |
| `Arrow Keys` | Move cursor |
| `Backspace` | Delete before cursor |
| `Delete` | Delete at cursor |
| `Enter` | New line |
| `Tab` | Insert spaces |
| `Ctrl+S` | Save |

#### Visual Mode

| Hotkey | Action |
|--------|--------|
| `Esc` | Return to Normal mode |
| `h` / `Left` | Extend selection left |
| `l` / `Right` | Extend selection right |
| `d` / `x` | Delete selection |

---

### Emacs Mode

Emacs-style keybindings with Ctrl+key navigation.

| Hotkey | Action |
|--------|--------|
| `Ctrl+B` | Move left |
| `Ctrl+F` | Move right |
| `Ctrl+P` | Move up |
| `Ctrl+N` | Move down |
| `Ctrl+A` | Line start |
| `Ctrl+E` | Line end |
| `Alt+F` | Word forward |
| `Alt+B` | Word backward |
| `Alt+<` | Buffer start |
| `Alt+>` | Buffer end |
| `Ctrl+D` | Delete character |
| `Ctrl+K` | Kill to end of line |
| `Ctrl+/` | Undo |
| `Ctrl+Shift+/` | Redo |
| `Ctrl+X` | Save |
| `Arrow Keys` | Move cursor |
| `Home` / `End` | Line start/end |
| `PageUp` / `PageDown` | Page navigation |
| `Backspace` | Delete before cursor |
| `Delete` | Delete at cursor |
| `Tab` | Insert spaces |

---

### VSCode Mode

VSCode-style keybindings with selection support.

#### Navigation

| Hotkey | Action |
|--------|--------|
| `Arrow Keys` | Move cursor |
| `Home` / `End` | Line start/end |
| `Ctrl+Home` / `Ctrl+End` | Buffer start/end |
| `Ctrl+Left` / `Ctrl+Right` | Word navigation |
| `PageUp` / `PageDown` | Page navigation |

#### Selection

| Hotkey | Action |
|--------|--------|
| `Shift+Arrow` | Extend selection |
| `Shift+Home` / `Shift+End` | Select to line start/end |
| `Ctrl+Shift+Left` / `Ctrl+Shift+Right` | Select word |
| `Ctrl+A` | Select all |
| `Ctrl+L` | Select line |

#### Editing

| Hotkey | Action |
|--------|--------|
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+Shift+Z` | Redo (alternative) |
| `Ctrl+S` | Save |
| `Ctrl+D` | Duplicate line |
| `Ctrl+Shift+K` | Delete line |
| `Alt+Up` | Move line up |
| `Alt+Down` | Move line down |
| `Ctrl+/` | Toggle comment |
| `Ctrl+]` | Indent |
| `Ctrl+[` | Outdent |
| `Tab` | Indent |
| `Shift+Tab` | Outdent |
| `Backspace` | Delete before cursor |
| `Delete` | Delete at cursor |

---

## File Browser Hotkeys

When the file browser is open.

| Hotkey | Action |
|--------|--------|
| `Esc` | Close file browser |
| `Up` / `k` / `w` | Move selection up |
| `Down` / `j` / `s` | Move selection down |
| `Left` / `h` / `a` | Go to parent directory |
| `Right` / `l` / `d` / `Enter` | Open selected file/directory |
| `PageUp` | Page up |
| `PageDown` | Page down |
| `Home` | Go to first item |
| `End` | Go to last item |
| `/` | Search files |

---

## Command Palette

Press `Ctrl+P` or `Ctrl+Shift+P` to open.

| Hotkey | Action |
|--------|--------|
| `Esc` | Close palette |
| `Enter` | Execute selected command |
| `Up` / `Down` | Navigate commands |
| Type text | Filter commands |

---

## Shell Selector

Opened via Command Palette > "Terminal: Select Shell"

| Hotkey | Action |
|--------|--------|
| `Esc` | Cancel |
| `Enter` | Select shell and create new tab |
| `Up` / `k` | Previous shell |
| `Down` / `j` | Next shell |

---

## Mode Switcher

Press `Ctrl+Shift+Tab` to open.

| Hotkey | Action |
|--------|--------|
| `Esc` | Cancel |
| `Enter` | Apply selected mode |
| `Tab` / `Down` / `j` | Next mode |
| `Shift+Tab` / `Up` / `k` | Previous mode |

---

## Theme Selector

Opened via Command Palette > "Theme: Select Theme"

| Hotkey | Action |
|--------|--------|
| `Esc` | Cancel and restore original theme |
| `Enter` | Apply selected theme and save to .ratrc |
| `Up` / `k` | Previous theme (with live preview) |
| `Down` / `j` | Next theme (with live preview) |

### Available Command Palette Theme Commands

| Command | Description |
|---------|-------------|
| `Theme: Select Theme` | Open theme selector with all presets |
| `Theme: Dark` | Apply Dark theme |
| `Theme: Light` | Apply Light theme |
| `Theme: Dracula` | Apply Dracula theme |
| `Theme: Gruvbox` | Apply Gruvbox theme |
| `Theme: Nord` | Apply Nord theme |
