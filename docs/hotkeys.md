# Ratterm Hotkeys Reference

This document lists all keyboard shortcuts available in Ratterm.

## Global Hotkeys (Work Everywhere)

These hotkeys work regardless of which pane is focused or what mode you're in.

| Hotkey | Action |
|--------|--------|
| `Ctrl+Q` | Quit application |
| `Ctrl+I` | Toggle IDE pane visibility |
| `F1` | Open Command Palette (Windows 11) |
| `Ctrl+P` | Open Command Palette (non-Windows 11) |
| `Ctrl+Shift+P` | Open Command Palette (non-Windows 11) |
| `Ctrl+Shift+Tab` | Switch Editor Mode (cycles Vim/Emacs/Default) |
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
| `Ctrl+Space` | Accept autocomplete suggestion |

### Autocomplete

Ratterm provides inline autocomplete suggestions that appear as grayed-out "ghost text" while typing. Suggestions are triggered automatically after a brief pause (300ms debounce) and show context-aware completions.

| Hotkey | Action |
|--------|--------|
| `Ctrl+Space` | Accept the current suggestion (inserts the ghost text) |
| `Esc` | Dismiss the current suggestion |

**Note:** Tab only inserts spaces (4 spaces) and does not accept completions. Use `Ctrl+Space` to accept autocomplete suggestions.

**Completion Sources:**
- **LSP (Language Server Protocol)**: When available, language servers provide intelligent completions for Rust, Python, JavaScript, TypeScript, Java, C#, PHP, SQL, HTML, and CSS.
- **Keyword Fallback**: If no LSP is available, keyword-based completions from the current buffer and language keywords are provided.

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

Press `F1` (Windows 11) or `Ctrl+Shift+P` (other platforms) to open.

> **Note for Windows 11 users:** The command palette keybinding has been changed from `Ctrl+Shift+P` to `F1` because Windows 11 uses `Ctrl+Shift+P` for its system-wide command palette in terminals and other applications.

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

## Dashboard Navigation (Universal)

All dashboards (SSH Manager, Docker Manager, Health Dashboard) share a consistent
navigation system. Press `?` in any dashboard to see the full shortcut list.

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Home` | Jump to first item |
| `End` | Jump to last item |
| `Enter` | Activate selected item |
| `Esc` | Close dashboard / go back |
| `?` | Show all available shortcuts |

The `?` key opens a shortcut overlay showing every hotkey available in the
current dashboard context. Press `?` again or `Esc` to dismiss it.
Arrow keys and `j`/`k` scroll the overlay when it is visible.

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
| `H` | Open SSH Health Dashboard |

### Add Host Form

When adding a host manually (`A`), fill in these fields:

| Field | Description |
|-------|-------------|
| Hostname/IP | The SSH server address (required) |
| Port | SSH port (default: 22) |
| Display Name | Friendly name shown in list (optional, uses hostname if blank) |
| Username | SSH username (optional, prompted on connect if not saved) |
| Password | SSH password (optional, auto-entered on connect if saved) |
| Jump Host | Select a registered SSH host to use as a bastion/jump host (optional) |

**Navigation:** Use `Tab` to move between fields, `Enter` to submit, `Esc` to cancel.

**Jump Host (SSH Hopping):** Use `Left`/`Right` arrows to cycle through available hosts when on the Jump Host field. This allows you to connect to internal servers via a bastion/head node using SSH's ProxyJump feature.

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

## SSH Health Dashboard

The SSH Health Dashboard displays live system metrics (CPU, RAM, Disk, GPU) for all registered SSH hosts.

### Opening Health Dashboard

| Method | How |
|--------|-----|
| From SSH Manager | Press `H` when SSH Manager is open |

### Overview Mode (Default)

Shows all hosts with their current metrics:

| Hotkey | Action |
|--------|--------|
| `Esc` / `q` | Close dashboard |
| `Up` / `k` | Previous host |
| `Down` / `j` | Next host |
| `Enter` | View detailed metrics for selected host |
| `r` | Manual refresh (collect fresh metrics) |
| `Space` | Toggle auto-refresh (1 second interval) |

### Detail Mode

Shows full metrics for a single host:

| Hotkey | Action |
|--------|--------|
| `Esc` / `q` | Close dashboard |
| `Backspace` | Return to overview |
| `r` | Manual refresh |
| `Space` | Toggle auto-refresh |

### Dashboard Features

- **Auto-refresh**: Updates every 1 second when enabled (toggle with `Space`)
- **Progress bars**: Visual CPU, RAM, Disk, and GPU usage bars
- **Status indicators**: Shows Online, Offline, Collecting, or Error state per host
- **GPU detection**: Automatically detects NVIDIA (nvidia-smi) and AMD (rocm-smi) GPUs

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

### Host Selection

Manage Docker containers on remote machines via SSH. This allows you to connect to Docker on servers registered in the SSH Manager.

| Hotkey | Action |
|--------|--------|
| `h` | Open host selection (choose local or SSH host) |
| `Up` / `k` | Previous host |
| `Down` / `j` | Next host |
| `l` | Quick-select Local host |
| `Enter` | Select host (may prompt for credentials) |
| `Esc` | Cancel host selection |
| `Shift+D` | Debug: Show current host configuration |

**Note:** Each host has its own set of quick-connect slots. Switching hosts switches which slots are displayed and used.

### Host Credential Entry

When selecting a remote host without saved credentials:

| Hotkey | Action |
|--------|--------|
| `Tab` | Next field (Username → Password → Save) |
| `Shift+Tab` | Previous field |
| `Space` | Toggle "Save credentials" checkbox |
| `Enter` | Submit credentials and connect |
| `Esc` | Cancel, return to host selection |

### Container/Image Actions

| Hotkey | Action |
|--------|--------|
| `Enter` | Connect to container / Run image |
| `Ctrl+O` | Run image with options (ports, volumes, env) |
| `r` | Refresh container/image discovery |
| `d` / `Delete` | Remove stopped container or image |
| `h` | Select Docker host (local or remote via SSH) |
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

---

## Mouse Support

Ratterm supports mouse interactions for selection and scrolling.

### Terminal Mouse Actions

| Action | Result |
|--------|--------|
| `Left Click` | Position cursor / focus pane |
| `Left Click + Drag` | Select text |
| `Left Release` | Finalize selection |
| `Scroll Wheel Up` | Scroll terminal history up |
| `Scroll Wheel Down` | Scroll terminal history down |

### Editor Mouse Actions

| Action | Result |
|--------|--------|
| `Left Click` | Position cursor |
| `Left Click + Drag` | Select text |
| `Double Click` | Select word |
| `Triple Click` | Select line |

---

## Edit Commands (Command Palette)

These commands are available via the Command Palette (`Ctrl+Shift+P` or `F1`):

### Line Operations

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Edit: Duplicate Line` | `Ctrl+D` | Duplicate current line below |
| `Edit: Delete Line` | `Ctrl+Shift+K` | Delete entire current line |
| `Edit: Move Line Up` | `Alt+Up` | Move current line up |
| `Edit: Move Line Down` | `Alt+Down` | Move current line down |

### Selection Operations

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Edit: Select All` | `Ctrl+A` | Select all text in editor |
| `Edit: Select Line` | `Ctrl+L` | Select current line |

### Code Editing

| Command | Hotkey | Description |
|---------|--------|-------------|
| `Edit: Toggle Comment` | `Ctrl+/` | Comment/uncomment selection |
| `Edit: Indent` | `Tab` | Increase indentation |
| `Edit: Outdent` | `Shift+Tab` | Decrease indentation |

---

## Custom Addon Hotkeys

You can define custom hotkeys in `.ratrc` that execute shell commands:

```
addon.<name> = <hotkey>|<command>
```

**Example Configuration:**
```
addon.git_status = ctrl+shift+g|git status
addon.npm_test = ctrl+shift+t|npm test
addon.docker_ps = ctrl+alt+d|docker ps -a
```

When triggered, the command executes in a new terminal tab.

See [ratrc_docs.md](ratrc_docs.md#custom-addon-commands) for full documentation

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
