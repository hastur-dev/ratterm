# Ratterm Configuration (.ratrc)

The `.ratrc` file is Ratterm's configuration file, located at `~/.ratrc` (your home directory). It is automatically created on first launch with default settings.

## File Format

- Lines starting with `#` are comments
- Settings use the format: `setting = value`
- Settings are case-insensitive for values

## Available Settings

### Shell Configuration

```
shell = <shell_type>
```

Sets the default terminal shell.

| Value | Description |
|-------|-------------|
| `system` | Use system default shell (default) |
| `powershell` / `pwsh` / `ps` | PowerShell (Windows PowerShell or PowerShell Core) |
| `bash` | Bash (requires Git Bash on Windows) |
| `cmd` / `command` | Windows Command Prompt (Windows only) |
| `zsh` | Z Shell (macOS/Linux) |
| `fish` | Fish Shell (macOS/Linux) |

**Platform defaults:**
- **Windows:** PowerShell
- **macOS:** Zsh
- **Linux:** Bash

**Example:**
```
shell = bash
```

---

### Auto-Close Tabs on Shell Change

```
auto_close_tabs_on_shell_change = <true|false>
```

When enabled, all existing terminal tabs are automatically closed when you select a new shell from the shell selector.

| Value | Description |
|-------|-------------|
| `false` | Keep existing tabs (default) |
| `true` / `yes` / `1` / `on` | Close all tabs when changing shell |

**Example:**
```
auto_close_tabs_on_shell_change = true
```

---

### IDE Configuration

```
ide-always = <true|false>
```

Controls whether the IDE pane (editor) is always visible.

| Value | Description |
|-------|-------------|
| `false` | Terminal-first mode (default) - IDE hidden until `open` command or `Ctrl+I` |
| `true` / `yes` / `1` / `on` | Always show IDE pane alongside terminals |

**Behavior:**
- **When `false` (default):** The application starts with only terminals visible. The IDE pane appears when:
  - You type `open` or `open <file>` in the terminal
  - You press `Ctrl+I`
  - The IDE auto-hides when all editor tabs are closed

- **When `true`:** The IDE pane is always visible alongside terminals, similar to the traditional split layout.

**Example:**
```
ide-always = true
```

---

### Keybinding Mode

```
mode = <mode>
```

Sets the editor keybinding mode.

| Value | Description |
|-------|-------------|
| `default` | Standard arrow-key navigation with common shortcuts |
| `vim` | Modal editing with Normal/Insert/Visual/Command modes |
| `emacs` | Emacs-style keybindings (Ctrl+key navigation) |

**Example:**
```
mode = vim
```

---

### Theme Configuration

Ratterm supports full color customization through theme presets and individual color settings.

#### Theme Preset

```
theme = <preset>
```

Sets the overall color theme.

| Value | Description |
|-------|-------------|
| `dark` | Dark theme with muted colors (default) |
| `light` | Light theme with dark text on light background |
| `dracula` | Dracula color scheme (purple/pink tones) |
| `gruvbox` | Gruvbox retro groove colors (warm earth tones) |
| `nord` | Nord arctic color palette (cool blue tones) |

**Example:**
```
theme = dracula
```

---

#### Individual Color Settings

You can override specific colors using hex values (`#RRGGBB`) or named colors.

##### Terminal Colors

| Setting | Description |
|---------|-------------|
| `terminal.foreground` | Terminal text color |
| `terminal.background` | Terminal background color |
| `terminal.cursor` | Terminal cursor color |
| `terminal.selection` | Selection highlight color |
| `terminal.border` | Terminal pane border color |
| `terminal.border_focused` | Border color when terminal is focused |

**Example:**
```
terminal.foreground = #d4d4d4
terminal.background = #1e1e1e
terminal.cursor = #ffffff
terminal.selection = #264f78
```

##### Editor Colors

| Setting | Description |
|---------|-------------|
| `editor.foreground` | Editor text color |
| `editor.background` | Editor background color |
| `editor.cursor` | Editor cursor color |
| `editor.selection` | Selection highlight color |
| `editor.line_numbers_fg` | Line number text color |
| `editor.current_line` | Current line highlight color |
| `editor.border` | Editor pane border color |
| `editor.border_focused` | Border color when editor is focused |

**Example:**
```
editor.foreground = #d4d4d4
editor.background = #1e1e1e
editor.line_numbers_fg = #858585
editor.current_line = #2d2d2d
```

##### Status Bar Colors

| Setting | Description |
|---------|-------------|
| `statusbar.foreground` | Status bar text color |
| `statusbar.background` | Status bar background color |
| `statusbar.mode_normal` | Normal mode indicator color |
| `statusbar.mode_insert` | Insert mode indicator color |
| `statusbar.mode_visual` | Visual mode indicator color |
| `statusbar.mode_command` | Command mode indicator color |

**Example:**
```
statusbar.background = #007acc
statusbar.mode_normal = #569cd6
statusbar.mode_insert = #608b4e
```

##### Tab Bar Colors

| Setting | Description |
|---------|-------------|
| `tabs.background` | Tab bar background color |
| `tabs.active_bg` | Active tab background color |
| `tabs.active_fg` | Active tab text color |
| `tabs.inactive_bg` | Inactive tab background color |
| `tabs.inactive_fg` | Inactive tab text color |
| `tabs.border` | Tab border color |

**Example:**
```
tabs.active_bg = #2d2d2d
tabs.inactive_bg = #1e1e1e
tabs.active_fg = #ffffff
```

##### Popup Colors

| Setting | Description |
|---------|-------------|
| `popup.background` | Popup/dialog background color |
| `popup.foreground` | Popup text color |
| `popup.border` | Popup border color |
| `popup.selected_bg` | Selected item background |
| `popup.selected_fg` | Selected item text color |

**Example:**
```
popup.background = #252526
popup.border = #454545
popup.selected_bg = #094771
```

##### File Browser Colors

| Setting | Description |
|---------|-------------|
| `filebrowser.background` | File browser background color |
| `filebrowser.foreground` | File browser text color |
| `filebrowser.directory` | Directory name color |
| `filebrowser.file` | File name color |
| `filebrowser.selected_bg` | Selected item background |

**Example:**
```
filebrowser.directory = #569cd6
filebrowser.file = #d4d4d4
```

---

#### Named Colors

In addition to hex values, you can use these named colors:

| Name | Color |
|------|-------|
| `black` | Black |
| `red` | Red |
| `green` | Green |
| `yellow` | Yellow |
| `blue` | Blue |
| `magenta` | Magenta |
| `cyan` | Cyan |
| `white` | White |
| `gray` / `grey` | Gray |
| `darkgray` / `darkgrey` | Dark Gray |
| `lightred` | Light Red |
| `lightgreen` | Light Green |
| `lightyellow` | Light Yellow |
| `lightblue` | Light Blue |
| `lightmagenta` | Light Magenta |
| `lightcyan` | Light Cyan |
| `reset` | Terminal default |

**Example:**
```
terminal.foreground = white
terminal.background = black
editor.cursor = yellow
```

---

#### Tab Theme Patterns

Control how themes are applied to new tabs.

```
tab_theme_pattern = <pattern>
```

| Value | Description |
|-------|-------------|
| `same` | All tabs use the current theme (default) |
| `sequential` | Cycle through themes for each new tab |
| `random` | Randomly assign themes to new tabs |

```
tab_themes = <theme1>, <theme2>, ...
```

Specify which themes to cycle through when using `sequential` or `random` patterns.

**Example:**
```
tab_theme_pattern = sequential
tab_themes = dark, dracula, nord
```

---

### Custom Keybindings

You can customize keybindings using the format:

```
action = modifier+key
```

**Available Modifiers:**
- `ctrl` - Control key
- `alt` - Alt key
- `shift` - Shift key

Combine modifiers with `+`: `ctrl+shift+p`

#### Global Actions

| Action | Default | Description |
|--------|---------|-------------|
| `quit` | `ctrl+q` | Quit the application |
| `focus_terminal` | `alt+left` | Focus terminal pane |
| `focus_editor` | `alt+right` | Focus editor pane |
| `toggle_focus` | `alt+tab` | Toggle focus between panes |
| `split_left` | `alt+[` | Move split divider left |
| `split_right` | `alt+]` | Move split divider right |

#### File Browser Actions

| Action | Default | Description |
|--------|---------|-------------|
| `open_file_browser` | `ctrl+o` | Open file browser |
| `next_file` | `alt+shift+right` | Switch to next open file |
| `prev_file` | `alt+shift+left` | Switch to previous open file |

#### Search & Create Actions

| Action | Default | Description |
|--------|---------|-------------|
| `find_in_file` | `ctrl+f` | Find in current file |
| `find_in_files` | `ctrl+shift+f` | Find in all files |
| `search_directories` | `ctrl+shift+d` | Search for directories |
| `search_files` | `ctrl+shift+e` | Search for files |
| `new_file` | `ctrl+n` | Create new file |
| `new_folder` | `ctrl+shift+n` | Create new folder |

#### Clipboard Actions

| Action | Default | Description |
|--------|---------|-------------|
| `copy` | `ctrl+shift+c` | Copy selection or line |
| `paste` | `ctrl+v` | Paste from clipboard |

#### Terminal Actions

| Action | Default | Description |
|--------|---------|-------------|
| `terminal_new_tab` | `ctrl+t` | New terminal tab |
| `terminal_split` | `ctrl+s` | Split terminal horizontally |
| `terminal_next_tab` | `ctrl+right` | Next terminal tab |
| `terminal_prev_tab` | `ctrl+left` | Previous terminal tab |
| `terminal_close_tab` | `ctrl+w` | Close current terminal tab |
| `terminal_interrupt` | `ctrl+c` | Send interrupt (Ctrl+C) |
| `terminal_scroll_up` | `shift+pageup` | Scroll terminal up |
| `terminal_scroll_down` | `shift+pagedown` | Scroll terminal down |

#### Editor Actions (Vim Mode)

| Action | Default | Description |
|--------|---------|-------------|
| `editor_insert` | `i` | Enter insert mode |
| `editor_append` | `a` | Append after cursor |
| `editor_visual` | `v` | Enter visual mode |
| `editor_command` | `:` | Enter command mode |
| `editor_left` | `h` | Move cursor left |
| `editor_right` | `l` | Move cursor right |
| `editor_up` | `k` | Move cursor up |
| `editor_down` | `j` | Move cursor down |
| `editor_line_start` | `0` | Move to line start |
| `editor_line_end` | `$` | Move to line end |
| `editor_word_right` | `w` | Move to next word |
| `editor_word_left` | `b` | Move to previous word |
| `editor_buffer_start` | `g` | Move to buffer start |
| `editor_buffer_end` | `G` | Move to buffer end |
| `editor_delete` | `x` | Delete character |
| `editor_undo` | `u` | Undo |
| `editor_redo` | `ctrl+r` | Redo |
| `editor_save` | `ctrl+s` | Save file |

---

### Autocomplete

Ratterm includes an intelligent autocomplete system that provides inline suggestions as you type in the editor.

**Features:**
- **Ghost text suggestions**: Grayed-out completions appear at the cursor position
- **Debounced triggering**: Suggestions appear after 300ms of idle typing
- **LSP support**: Intelligent completions from language servers when available
- **Keyword fallback**: Language keywords and buffer words when LSP is unavailable
- **Accept with Ctrl+Space**: Press Ctrl+Space (or Tab in Insert mode) to accept

**Supported Languages:**
- Rust (rust-analyzer)
- Python (pylsp/pyright)
- JavaScript/TypeScript (typescript-language-server)
- Java (jdtls)
- C# (omnisharp-roslyn)
- PHP (intelephense)
- SQL (sql-language-server)
- HTML/CSS (vscode-html-languageserver)

**Note:** Autocomplete is enabled by default and requires no configuration. Language servers are started automatically when available.

---

### SSH Manager Configuration

Ratterm includes an SSH Manager for managing SSH connections.

#### Storage Mode

```
ssh_storage_mode = <mode>
```

Sets how SSH credentials are stored.

| Value | Description |
|-------|-------------|
| `plaintext` | Store credentials in plain text (default) |
| `masterpass` | Encrypt credentials with a master password |
| `external` | Use external password manager (future) |

**Example:**
```
ssh_storage_mode = masterpass
```

---

#### SSH Quick Connect Hotkeys

```
set_ssh_tab = <modifier>
```

Sets the modifier key prefix for SSH quick connect (1-9).

| Value | Description |
|-------|-------------|
| `ctrl` | Use Ctrl+1-9 for quick connect (default) |
| `alt` | Use Alt+1-9 for quick connect |
| `ctrl+shift` | Use Ctrl+Shift+1-9 for quick connect |

**Example:**
```
set_ssh_tab = ctrl
```

---

```
ssh_number_setting = <true|false>
```

Enables or disables SSH quick connect number hotkeys.

| Value | Description |
|-------|-------------|
| `true` / `yes` / `1` / `on` | Enable quick connect (default) |
| `false` / `no` / `0` / `off` | Disable quick connect |

**Example:**
```
ssh_number_setting = true
```

---

#### SSH Storage Location

SSH hosts and credentials are stored in:
- **All platforms:** `~/.ratterm/ssh_hosts.toml`

---

#### Auto-Password Feature

When you save credentials for an SSH host, Ratterm will automatically enter the password when connecting. The terminal detects the SSH password prompt and sends the saved password, so you don't need to type it manually.

**How it works:**
1. Save credentials when adding a host or after a successful connection
2. Connect to the host via SSH Manager or quick connect
3. Ratterm detects the "password:" prompt and auto-enters your saved password

**Security notes:**
- Credentials are stored based on your `ssh_storage_mode` setting
- Use `masterpass` mode for encrypted storage with a master password
- The master password is required once per session to unlock credentials

---

#### Credential Scan

The credential scan feature (`C` in SSH Manager) lets you scan a network and automatically save hosts that accept your credentials:

1. Enter username and password to test
2. Optionally specify a subnet (e.g., `192.168.1.0/24`) or leave blank for auto-detect
3. Ratterm scans for SSH hosts and tests authentication
4. Only hosts that successfully authenticate are saved with credentials

This is useful for quickly setting up access to multiple hosts with the same credentials.

---

### Docker Manager Configuration

Ratterm includes a Docker Manager for managing Docker containers and images.

#### Docker Storage Location

Docker quick-connect settings are stored in:
- **All platforms:** `~/.ratterm/docker_items.toml`

---

#### Docker Default Shell

The Docker Manager uses `/bin/sh` by default when executing into containers. This is automatically configured and currently not customizable via `.ratrc`.

---

#### Docker Quick Connect

Similar to SSH, you can assign containers and images to quick-connect slots:

1. Open Docker Manager with `Ctrl+Shift+D`
2. Select a container or image
3. Press `1-9` to assign it to a slot
4. Use `Ctrl+Alt+1-9` to quickly connect from anywhere

**Container behavior:**
- Running containers: Direct exec with `docker exec -it`
- Stopped containers: Start first, then exec
- Images: Run as new container with interactive shell

---

### Docker Logs Configuration

The Docker Logs viewer can be customized with the following `.ratrc` settings:

#### Buffer and Display

| Setting | Description | Default |
|---------|-------------|---------|
| `docker_log_buffer_size` | Max log entries kept in memory (1000–1000000) | `10000` |
| `docker_log_tail_lines` | Initial lines to fetch when connecting | `500` |
| `docker_log_auto_scroll` | Auto-scroll to new entries | `true` |
| `docker_log_timestamps` | Show timestamps on each log line | `true` |
| `docker_log_color` | Color-code log lines by level | `true` |

#### Storage

| Setting | Description | Default |
|---------|-------------|---------|
| `docker_log_storage` | Enable persistent log storage | `true` |
| `docker_log_retention` | Hours to keep stored logs (1–8760) | `168` (7 days) |

Log files are stored as gzip-compressed JSONL at `~/.ratterm/docker_logs/<container_id>/<date>.jsonl.gz`.

#### Saved Searches

Saved search patterns are stored at `~/.ratterm/docker_logs/saved_searches.json`. Use `Ctrl+S` in search mode to save a filter, and `s` in streaming mode to browse saved searches.

#### Example

```
# Docker Logs settings
docker_log_buffer_size = 20000
docker_log_tail_lines = 1000
docker_log_auto_scroll = true
docker_log_timestamps = true
docker_log_color = true
docker_log_storage = true
docker_log_retention = 336
```

---

## Example Configuration

```
# Ratterm Configuration
# ~/.ratrc

# Use Bash as default shell
shell = bash

# Auto-close tabs when changing shell
auto_close_tabs_on_shell_change = true

# IDE Configuration
# Show IDE pane always (false = terminal-first mode)
ide-always = false

# Use Vim keybinding mode
mode = vim

# Theme settings
theme = dracula

# Custom terminal colors (override theme)
terminal.background = #1e1e1e
terminal.selection = #44475a

# Status bar colors
statusbar.background = #282a36

# Tab theme cycling for new tabs
tab_theme_pattern = sequential
tab_themes = dracula, nord, gruvbox

# Custom keybindings
quit = ctrl+shift+q
copy = ctrl+c
paste = ctrl+v

# SSH Manager settings
ssh_storage_mode = plaintext
set_ssh_tab = ctrl
ssh_number_setting = true

# Docker Manager settings
# Docker quick-connect uses Ctrl+Alt+1-9 (not configurable yet)
# Docker settings are stored in ~/.ratterm/docker_items.toml

# Docker Logs settings
docker_log_buffer_size = 10000
docker_log_tail_lines = 500
docker_log_timestamps = true
docker_log_color = true
```

---

## Changing Themes via Command Palette

You can also change themes without editing `.ratrc` directly:

1. Press `Ctrl+P` to open the Command Palette
2. Type "theme" to filter commands
3. Select `Theme: Select Theme` to open the theme selector
4. Use `Up`/`Down` to preview themes in real-time
5. Press `Enter` to apply and save, or `Esc` to cancel

**Note:** When you select a theme via the Command Palette, the change is automatically saved to your `.ratrc` file.

---

---

### Window Positions

All popup windows (hotkey overlay, SSH manager, Docker manager) support configurable positioning via `.ratrc`.

Two position formats are supported:

#### Grid Positions

A 3×3 named grid aligned to screen edges (with a 2-cell margin):

```
hotkey_overlay_position = <grid-name>
ssh_manager_position = <grid-name>
docker_manager_position = <grid-name>
```

| Grid Name | Description |
|-----------|-------------|
| `top-left` | Upper-left corner |
| `top-center` | Top edge, horizontally centered |
| `top-right` | Upper-right corner |
| `middle-left` | Left edge, vertically centered |
| `middle-center` | Screen center (default) |
| `middle-right` | Right edge, vertically centered |
| `bottom-left` | Lower-left corner |
| `bottom-center` | Bottom edge, horizontally centered |
| `bottom-right` | Lower-right corner |

Grid names are **case-insensitive** and accept `-`, `_`, or space as separators (e.g., `top-left`, `top_left`, `Top Left` are all equivalent).

#### Pixel Offsets

Absolute cell distance from the top-left corner of the screen:

```
hotkey_overlay_position = 10 x 5
```

Format: `<X> x <Y>` where X is the column and Y is the row. Values are clamped to keep the popup fully visible on screen.

#### Available Windows

| Setting | Window | Default |
|---------|--------|---------|
| `hotkey_overlay_position` | Hotkey help popup (`?`) | `middle-center` |
| `ssh_manager_position` | SSH Manager panel | `middle-center` |
| `docker_manager_position` | Docker Manager panel | `middle-center` |

#### Examples

```
# Center all popups (default behavior)
hotkey_overlay_position = middle-center
ssh_manager_position = middle-center
docker_manager_position = middle-center

# Place SSH manager in the top-right corner
ssh_manager_position = top-right

# Place hotkey overlay at exact coordinates
hotkey_overlay_position = 10 x 5

# Place Docker manager at bottom-center
docker_manager_position = bottom-center
```

---

---

### Logging Configuration

Ratterm includes a file-based logging system for debugging and troubleshooting.

#### Enable/Disable Logging

```
log_enabled = <true|false>
```

| Value | Description |
|-------|-------------|
| `false` | Logging disabled (default) |
| `true` / `yes` / `1` / `on` | Enable file logging |

---

#### Log Level

```
log_level = <level>
```

Sets the minimum severity level for logged messages.

| Value | Description |
|-------|-------------|
| `off` | No logging |
| `error` | Only errors |
| `warn` | Errors and warnings |
| `info` | General information (default when enabled) |
| `debug` | Detailed debugging information |
| `trace` | Most verbose - all messages |

**Example:**
```
log_level = debug
```

---

#### Log Retention

```
log_retention = <hours>
```

Sets how long log files are kept before automatic deletion.

| Value | Description |
|-------|-------------|
| `24` | Keep logs for 24 hours (default) |
| Any positive number | Hours to retain logs |

**Example:**
```
log_retention = 48
```

**Log file location:** `~/.ratterm/logs/`

---

### Custom Addon Commands

You can create custom hotkey bindings that execute shell commands in a new terminal tab.

```
addon.<name> = <hotkey>|<command>
```

| Component | Description |
|-----------|-------------|
| `<name>` | Unique identifier for this addon command |
| `<hotkey>` | Key combination (e.g., `ctrl+shift+g`) |
| `<command>` | Shell command to execute |

**Example:**
```
addon.git_status = ctrl+shift+g|git status
addon.npm_test = ctrl+shift+t|npm test
addon.docker_ps = ctrl+alt+d|docker ps -a
```

When triggered, the command runs in a new terminal tab.

---

## Reloading Configuration

Configuration is loaded on application startup. To apply changes, restart Ratterm.

Theme changes via the Command Palette take effect immediately and are automatically persisted.

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RATTERM_NO_UPDATE` | Set to `1` to disable auto-update checks |
| `RATTERM_INSTALL_DIR` | Custom installation directory |
| `RATTERM_LOG_LEVEL` | Override log level (trace/debug/info/warn/error/off) |

---

## File Locations

### Configuration File

| Platform | Path |
|----------|------|
| Windows | `C:\Users\<username>\.ratrc` |
| macOS | `/Users/<username>/.ratrc` |
| Linux | `/home/<username>/.ratrc` |

### Data Directory

| Platform | Path |
|----------|------|
| Windows | `C:\Users\<username>\.ratterm\` |
| macOS | `/Users/<username>/.ratterm/` |
| Linux | `/home/<username>/.ratterm/` |

**Data directory contents:**
- `logs/` - Log files (when logging enabled)
- `extensions/` - Installed extensions
- `themes/` - Custom themes
- `ssh_hosts.toml` - SSH hosts and credentials
- `docker_items.toml` - Docker quick-connect settings
- `approved_extensions.toml` - Extension approvals
