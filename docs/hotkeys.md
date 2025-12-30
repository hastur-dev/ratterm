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

---

## SSH Manager

The SSH Manager provides a convenient way to manage SSH connections.

### Opening SSH Manager

| Hotkey | Action |
|--------|--------|
| `Ctrl+Shift+U` | Open SSH Manager |

### SSH Manager Navigation

When the SSH Manager is open:

| Hotkey | Action |
|--------|--------|
| `Esc` | Close SSH Manager |
| `Up` / `k` | Previous host |
| `Down` / `j` | Next host |
| `Home` | First host |
| `End` | Last host |
| `Enter` | Connect to selected host |
| `S` | Scan network for SSH hosts (auto-detect subnet) |
| `C` | Credential scan (scan with username/password to auto-save) |
| `A` | Add host manually (with display name, credentials) |
| `E` | Edit display name of selected host |
| `D` / `Delete` | Delete selected host |

### Add Host Form

When adding a host manually (`A`), fill in these fields:

| Field | Description |
|-------|-------------|
| Hostname/IP | The SSH server address (required) |
| Port | SSH port (default: 22) |
| Display Name | Friendly name shown in list (optional, uses hostname if blank) |
| Username | SSH username (optional, prompted on connect if not saved) |
| Password | SSH password (optional, auto-entered on connect if saved) |

**Navigation:** Use `Tab` to move between fields, `Enter` to submit, `Esc` to cancel.

### Edit Display Name

Press `E` on a selected host to edit its display name. This lets you give hosts friendly names without re-adding them.

### SSH Credential Entry

When entering credentials:

| Hotkey | Action |
|--------|--------|
| `Tab` | Next field |
| `Shift+Tab` | Previous field |
| `Enter` | Submit and connect |
| `Esc` | Cancel |

### SSH Quick Connect

Connect directly to saved hosts using number hotkeys:

| Hotkey | Action |
|--------|--------|
| `Ctrl+1` | Connect to host #1 |
| `Ctrl+2` | Connect to host #2 |
| ... | ... |
| `Ctrl+9` | Connect to host #9 |

**Note:** Quick connect hotkeys can be customized via `set_ssh_tab` in `.ratrc`.

### SSH Commands in Command Palette

Press `Ctrl+P` and type "ssh" to access these commands:

| Command | Description |
|---------|-------------|
| `SSH: Open SSH Manager` | Open the SSH Manager popup |
| `SSH: Scan Network` | Scan local network for SSH hosts |
| `SSH: Add Host` | Manually add a new SSH host |
| `SSH: Quick Connect #1` | Connect to saved host #1 |
| `SSH: Quick Connect #2` | Connect to saved host #2 |
| `SSH: Quick Connect #3` | Connect to saved host #3 |

---

## Docker Manager

The Docker Manager provides container and image management capabilities.

### Opening Docker Manager

| Hotkey | Action |
|--------|--------|
| `Ctrl+Shift+D` | Open Docker Manager |

### Docker Manager Navigation

When the Docker Manager is open:

| Hotkey | Action |
|--------|--------|
| `Esc` | Close Docker Manager |
| `Up` / `k` | Previous container/image |
| `Down` / `j` | Next container/image |
| `Home` / `g` | First item |
| `End` / `G` | Last item |
| `Tab` | Switch section (Running → Stopped → Images) |
| `Shift+Tab` | Previous section |

### Section Quick Jump

| Hotkey | Action |
|--------|--------|
| `Shift+R` | Jump to Running Containers section |
| `Shift+S` | Jump to Stopped Containers section |
| `Shift+I` | Jump to Images section |

### Container/Image Actions

| Hotkey | Action |
|--------|--------|
| `Enter` | Connect to container / Run image |
| `Ctrl+O` | Run image with options (ports, volumes, env) |
| `r` | Refresh container/image discovery |
| `d` / `Delete` | Remove stopped container or image |
| `1-9` | Assign to quick connect slot (Ctrl+Alt+1-9) |

### Docker Quick Connect

Connect directly to assigned containers/images:

| Hotkey | Action |
|--------|--------|
| `Ctrl+Alt+1` | Quick connect to slot #1 |
| `Ctrl+Alt+2` | Quick connect to slot #2 |
| ... | ... |
| `Ctrl+Alt+9` | Quick connect to slot #9 |

### Docker Session Hotkeys

When inside a Docker session (after exec into container):

| Hotkey | Action |
|--------|--------|
| `Ctrl+T` | Show container stats (split panel) |
| `Ctrl+L` | Show container logs (split panel) |

### Run Options Form

When running an image with options (`Ctrl+O`):

| Field | Description |
|-------|-------------|
| Name | Container name (optional) |
| Ports | Port mappings, e.g., `8080:80` (comma-separated) |
| Volumes | Volume mounts, e.g., `/host:/container` (comma-separated) |
| Env Vars | Environment variables, e.g., `KEY=VALUE` (comma-separated) |
| Shell | Shell to use (default: /bin/sh) |

**Navigation:** Use `Tab` to move between fields, `Enter` to submit, `Esc` to cancel.

### Docker Container Actions by Section

#### Running Containers
- `Enter` - Execute into container (`docker exec -it`)
- `1-9` - Assign to quick connect slot

#### Stopped Containers
- `Enter` - Start container and execute into it
- `d` - Remove container

#### Images
- `Enter` - Run image with default settings (shows confirm dialog)
- `Ctrl+O` - Run image with custom options
- `d` - Remove image
