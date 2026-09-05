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
| `Ctrl+G` | Open Git Dashboard |
| `Ctrl+Shift+K` | Open or close the Kubernetes screens |
| `Ctrl+Shift+M` | Open or close the Docker fleet view (every container on every host) |
| `Ctrl+O` | Open File Browser |
| `Ctrl+T` | New editor tab (works whether or not the IDE pane is showing) |
| `Ctrl+Shift+C` | Copy selection |
| `Ctrl+V` | Paste from clipboard |
| `Alt+Left` | Focus Terminal pane |
| `Alt+Right` | Focus Editor pane (when IDE visible) |
| `Alt+Tab` | Toggle focus between panes (when IDE visible) — **not available on Windows** |
| `Alt+Up` / `Alt+Down` | Navigate between terminal grid panes |
| `Alt+[` | Shrink split (move divider left) |
| `Alt+]` | Expand split (move divider right) |
| `Alt+Shift+Left` | Previous file tab |
| `Alt+Shift+Right` | Next file tab |

### Windows differences

The key hint bar at the bottom of the screen adapts to the host operating
system, so it always shows hotkeys that actually reach Ratterm.

| Action | Non-Windows | Windows 10 | Windows 11 | Why |
|--------|-------------|------------|------------|-----|
| Command Palette | `Ctrl+Shift+P` | `Ctrl+Shift+P` | `F1` | Windows 11 reserves `Ctrl+Shift+P` for its own terminal command palette |
| Switch Pane | `Alt+Tab` | `Alt+Left` / `Alt+Right` (shown as `Alt+Arrows`) | `Alt+Left` / `Alt+Right` (shown as `Alt+Arrows`) | Windows reserves `Alt+Tab` for the system window switcher, so the application never receives it |

Windows consoles report every keystroke twice — once as a key-press event and
once as a key-release event. Ratterm pairs the two so each hotkey fires once.
Release events that arrive with no matching press are still honoured, because a
spawned child process (`plink.exe`) can corrupt the console input mode so that
`Esc` and modified keys produce only a release.

## Debugger Hotkeys

These hotkeys control the integrated debugger (DAP).

### Session Control

| Hotkey | Action |
|--------|--------|
| `F5` | Continue execution / Start debug session |
| `Shift+F5` | Stop debugging |
| `Ctrl+Shift+F5` | Restart debugging |

### Breakpoints

| Hotkey | Action |
|--------|--------|
| `F9` | Toggle breakpoint on current line |

### Stepping

| Hotkey | Action |
|--------|--------|
| `F10` | Step over |
| `F11` | Step into |
| `Shift+F11` | Step out |

### Debug Panel

| Hotkey | Action |
|--------|--------|
| `Tab` | Switch panel tab (Call Stack / Variables / Console) |
| `Up/Down` or `j/k` | Navigate items in current tab |
| `Enter` | Expand variable / select frame |

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

### LSP Features

Ratterm includes a full Language Server Protocol (LSP) client for intelligent code editing.

| Hotkey | Action |
|--------|--------|
| `Ctrl+K` | Show hover information at cursor |
| `F12` / `gd` (Vim) | Go to definition |
| `Shift+F12` / `gr` (Vim) | Find all references |
| `F2` | Rename symbol |
| `Ctrl+.` | Show code actions (quick fixes) |
| `Ctrl+Shift+O` | Document symbols (outline) |
| `Ctrl+T` | Workspace symbols search |

#### Hover Popup

Shows type information and documentation for the symbol under the cursor.

| Hotkey | Action |
|--------|--------|
| `Ctrl+K` | Show hover (or idle 500ms) |
| Any key | Dismiss hover |

#### References Panel

Shows all references to the symbol under the cursor.

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Previous reference |
| `Down` / `j` | Next reference |
| `Enter` | Jump to reference location |
| `Esc` | Close panel |

#### Code Actions

Shows quick fixes and refactoring options.

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Previous action |
| `Down` / `j` | Next action |
| `Enter` | Apply action |
| `Esc` | Cancel |

#### Rename

Renames a symbol across all files.

| Hotkey | Action |
|--------|--------|
| `F2` | Start rename |
| Type text | Enter new name |
| `Enter` | Confirm rename |
| `Esc` | Cancel |

#### Document Symbols (Outline)

Shows functions, structs, enums, etc. in the current file.

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Previous symbol |
| `Down` / `j` | Next symbol |
| `Enter` | Jump to symbol |
| `Esc` | Close |

#### Diagnostics

Compiler errors and warnings appear as colored underlines and gutter icons.

| Indicator | Meaning |
|-----------|---------|
| `E` (red) | Error |
| `W` (yellow) | Warning |
| `I` (blue) | Information |
| `H` (green) | Hint |

#### Signature Help

Shows function parameter hints when typing `(` or `,`.

The active parameter is highlighted in bold yellow.

---

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

All dashboards (SSH Manager, Docker Manager, Health Dashboard, Git Dashboard) share a consistent
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

### Docker Logs

Open from the Docker Manager list with `l`. Provides live log streaming from containers.

#### Container List

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Previous container |
| `Down` / `j` | Next container |
| `Home` / `g` | First container |
| `End` / `G` | Last container |
| `Enter` | Start streaming logs |
| `Esc` / `q` | Back to Docker Manager |
| `?` | Show all shortcuts |

#### Streaming / Paused

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Scroll up (auto-pauses) |
| `Down` / `j` | Scroll down |
| `Home` / `g` | Jump to top |
| `End` / `G` | Jump to bottom |
| `PgUp` | Page up |
| `PgDn` | Page down |
| `Space` | Toggle pause/resume |
| `/` | Start search/filter |
| `c` | Clear log buffer |
| `t` | Toggle timestamps |
| `s` | Open saved searches |
| `Esc` / `q` | Back to container list |
| `?` | Show all shortcuts |

#### Searching

| Hotkey | Action |
|--------|--------|
| Type text | Filter logs (live) |
| `Enter` | Apply filter |
| `Esc` | Cancel filter |
| `Backspace` | Delete character |
| `Ctrl+S` | Save current search |

#### Saved Searches

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Previous search |
| `Down` / `j` | Next search |
| `Enter` | Apply saved search |
| `d` | Delete saved search |
| `Esc` | Back |

---

## Git Dashboard

The Git Dashboard provides an integrated Git interface for staging, committing, branching, and viewing diffs.

### Opening Git Dashboard

| Hotkey | Action |
|--------|--------|
| `Ctrl+G` | Open Git Dashboard |

### Status View (Default)

| Hotkey | Action |
|--------|--------|
| `Up` / `k` | Previous file |
| `Down` / `j` | Next file |
| `Home` | First file |
| `End` | Last file |
| `Tab` / `Shift+Tab` | Switch section (Staged / Unstaged / Untracked) |
| `Enter` | View diff for selected file |
| `Backspace` | Back to status view (from other views) |
| `Esc` | Close dashboard |
| `s` | Stage selected file |
| `u` | Unstage selected file |
| `c` | Start commit (opens commit message editor) |
| `r` | Refresh status |
| `p` | Stash pop |
| `Shift+P` | Stash push |

### View Switching

| Hotkey | Action |
|--------|--------|
| `b` | Branch list view |
| `l` | Commit log view |
| `d` | Diff view |
| `Ctrl+B` | Toggle blame view |

### Commit Message Editor

When composing a commit message (`c`):

| Hotkey | Action |
|--------|--------|
| `Enter` | Execute commit |
| `Esc` | Cancel commit |
| `Ctrl+A` | Toggle amend mode |
| Type text | Edit commit message |
| `Backspace` | Delete character |

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

## Deterministic keys for scripted runs

`--test-keys`, and every scenario unless it sets `test_keys: false`, enable six
extra keys:

| Hotkey | Action |
|--------|--------|
| `F1` | Command palette |
| `F2` | SSH manager |
| `F3` | Docker manager |
| `F4` | SSH health dashboard |
| `F6` | Kubernetes (F5 is the debugger) |
| `F7` | Docker fleet view |

They exist because the real shortcut for the command palette differs between
Windows 11 and every other platform, so a scenario written against it would not
be the same test everywhere. See `docs/automation.md`.

---

## Editor: folding, find, and multiple cursors

These work in the editor pane in every keybinding mode — Vim, Emacs and Default
alike. They sit on `Ctrl+Alt` because that is the one modifier pair the global
handler leaves alone apart from `Ctrl+Alt+1`–`9` (Docker quick connect). See
`docs/editor.md` for what each feature does.

### Code folding

| Hotkey | Action |
|--------|--------|
| `Ctrl+Alt+[` | Collapse the region under the cursor |
| `Ctrl+Alt+]` | Expand the region under the cursor |
| `Ctrl+Alt+K` | Collapse every region in the file |
| `Ctrl+Alt+J` | Expand every region |

A collapsed region draws as a single line ending in `⋯ N lines`, and the gutter
shows `▾` beside a region that can be collapsed and `▸` beside one that is. The
cursor cannot stand inside a collapsed region; arrow keys step over it.

### Find and replace

| Hotkey | Action |
|--------|--------|
| `Ctrl+Alt+F` | Open the find bar |
| `Ctrl+Alt+H` | Open the find bar with the replace field |
| `F3` | Next match |
| `Shift+F3` | Previous match |

`Ctrl+F` still opens the older search-in-file popup; the bar above is the
in-pane one, with live match highlighting and a count.

While the find bar has focus:

| Hotkey | Action |
|--------|--------|
| `Enter` / `Down` | Next match, wrapping at the end of the file |
| `Shift+Enter` / `Up` | Previous match, wrapping at the start |
| `Tab` | Switch between the find and replace fields |
| `Ctrl+Enter` | Replace the current match |
| `Alt+Enter` | Replace every match (one undo step) |
| `Alt+C` | Toggle case sensitivity |
| `Esc` | Close the bar |

The bar shows `3/12` for the current match and total, says `wrapped` when
navigation has just come round an end, and `Aa` while case-sensitive.

### Multiple cursors

| Hotkey | Action |
|--------|--------|
| `Ctrl+Alt+Down` | Add a cursor on the line below |
| `Ctrl+Alt+Up` | Add a cursor on the line above |
| `Ctrl+Alt+D` | Add a cursor at the next occurrence of the word under the cursor |
| `Ctrl+Alt+L` | Add a cursor at every occurrence |
| `Esc` (Default mode) | Drop back to one cursor |
| `Esc` (Vim insert mode) | Leave insert mode and drop back to one cursor |
| `Ctrl+G` (Emacs) | Drop back to one cursor, clear the mark, close the find bar |

Typing, `Backspace` and `Enter` reach every cursor, and the whole fan-out is a
single undo step. Auto-pairing is skipped while more than one cursor is active.

### Editing behaviour that has no hotkey

- **Enter** re-indents: one level deeper after `{`, `(`, `[` or a Python `:`,
  one level shallower before a closing bracket, and pressing it between a
  bracket pair opens an indented body with the closer on its own line.
- **Tab** inserts one indentation unit in the file's own style — tabs in a
  tab-indented file, two spaces in a two-space file — rather than four literal
  spaces. With a selection it indents the selected lines; `Shift+Tab` outdents.
- **Typing an opening bracket or quote** inserts its partner; typing the closer
  steps over it rather than doubling it. Typing `}` re-indents the line onto the
  line that opened the block.
- **The bracket under the cursor** and its partner are drawn bold and
  underlined, including when the cursor sits just past the closer.
