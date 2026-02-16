# What's New in This Release

## UI Overhaul

### Context-Aware Key Hint Bar
A new bottom-of-screen bar shows available keyboard shortcuts for the current context:
- **Terminal focused**: Shows Palette, SSH, Docker, New Tab, Split, Switch Pane, Quit
- **Editor focused**: Shows Palette, Open, Save, Find, Switch Pane, Quit
- **Manager popups**: Two-row footer with primary and secondary action hints
- **Styled badges**: Color-coded key badges (Normal, Highlighted, Danger, Success) with overflow truncation

### Redesigned Status Bar
The status bar has been completely rewritten with a segmented layout:
- **Mode badge** (left): Color-coded by mode (Normal=Blue, Insert=Green, Visual=Magenta, Command=Yellow)
- **File path / message** (center): With smart path truncation
- **Background indicators** (right): Shows `BG:N` for running background processes and `ERR:N` for errored ones

### Visual Polish
- All popups now use rounded borders for a modern look
- Consistent black backgrounds on popups to prevent Windows rendering artifacts
- Explicit background fills on tab bars and status bars to prevent ghost characters

### Terminal-First Layout
- Terminal is now shown fullscreen by default; the editor pane appears when toggled
- `Ctrl+I` toggles IDE visibility
- `ide-always` option in `.ratrc` to always show the IDE pane

## Terminal Improvements

### Terminal Grid Split (2x2 Panes)
Split your terminal into up to 4 panes per tab:
- `Ctrl+S` for progressive splits (first: 2 side-by-side, second: 2x2 grid)
- `Ctrl+Tab` to cycle focus between panes
- `Alt+Arrow` keys for directional focus navigation
- `Ctrl+Shift+W` to close a grid pane

### Terminal Text Selection
- **Mouse selection**: Click and drag to select text
- **Keyboard selection**: Shift+Arrow keys
- **Three modes**: Normal (character), Line (full lines), Block (rectangular)

### Background Process Manager
Run terminal processes in the background without UI rendering:
- Process tracking with unique IDs, status, exit codes, and timestamps
- Output buffering up to 100K characters per process
- Up to 10 concurrent background processes
- Status bar integration with `BG:N` and `ERR:N` indicators

## Command Palette Enhancements
- **Category-based color coding**: File=Blue, Edit=Green, Search=Yellow, View=Cyan, Terminal=Green, SSH=Cyan, Docker=Magenta, Theme=LightYellow, Extension=LightBlue, Application=Gray
- **Fuzzy search** using `SkimMatcherV2` for better filtering
- **Bottom hint line** with navigation instructions
- **50+ commands** across 10 categories
- **Rounded borders** for a modern look

## Docker Container Creation Workflow
A complete container creation workflow in the Docker Manager:
- **Docker Hub search**: Search for images directly from the manager
- **Image management**: Check, download, and manage Docker images
- **Volume mount wizard**: Multi-step wizard for configuring volume mounts (host path, container path, confirmation)
- **Startup command configuration**: Set custom startup commands for containers
- **Section tabs** with counts (Running Containers, Stopped Containers, Images)
- **Scrollbar** when list items exceed viewport
- **Host selection mode** for remote Docker management with credential entry

## SSH Health Dashboard
A new SSH Health Dashboard provides real-time system metrics for your remote servers:
- **Real-time Monitoring**: View CPU, memory, disk, and GPU usage across all connected hosts
- **Daemon-based Collection**: Lightweight shell daemons run on remote hosts and send metrics back through SSH tunnels
- **Automatic Deployment**: Daemons are automatically deployed when you connect to a host
- **Dashboard UI**: Press the configured hotkey to open the health dashboard overlay

## SSH Multi-Hop Support
Connect to hosts behind bastion/jump servers:
- **ProxyJump Support**: Configure jump hosts in your SSH host list
- **Chained Connections**: Supports multiple hops for complex network topologies
- **Credential Management**: Handles authentication at each hop

## SSH Manager UI Improvements
- **Styled footer** with consistent key hints using the new ManagerFooter widget
- **Authenticated scanning mode**: Shows success/fail counts during credential-based network scanning
- **Edit display name**: Rename hosts directly from the manager
- **Jump host cycling**: Left/Right arrows cycle through available hosts for ProxyJump configuration

## Extension Hotkey Support
Extensions can now register custom hotkeys:
- **Auto-registration**: Hotkeys defined in `extension.toml` are automatically registered
- **Addon Integration**: Extensions can define commands that execute in the terminal
- **Configurable Bindings**: Override extension hotkeys in your `.ratrc`

## Editor Improvements

### Inline Completions
- Ghost-text suggestions appear at cursor position
- LSP-powered completions when language servers are available
- Fallback to buffer words and language keywords
- Accept with `Ctrl+Space` or `Tab` in insert mode
- 300ms debounce for smooth typing experience

### Character Handling
- Fixed carriage return (`\r`) handling in terminal rendering
- Improved character width calculation

## Logging System
A complete file-based logging system:
- **Log files** stored in `~/.ratterm/logs/` with timestamped filenames
- **Configurable via `.ratrc`**: `log_enabled`, `log_level` (trace/debug/info/warn/error), `log_retention` (hours)
- **Automatic rotation** when files exceed 10 MB
- **Automatic cleanup** of old log files based on retention period

## Windows 11 Fixes
- Resolved hotkey detection issues specific to Windows 11
- **Keybinding change notification**: Popup informs users that Command Palette changed from `Ctrl+Shift+P` to `F1` on Windows 11
- **Platform detection module**: Detects Windows 11 via build number with cached results
- Improved console input handling

## API Server Module
An internal API server for extension communication:
- Request/response protocol over named pipes
- Handler trait for extensibility
- Used for extension communication and AI integration hooks

## CI/CD Improvements
- **ARM Linux builds**: Added aarch64 target (ubuntu-24.04-arm)
- **Binary verification**: `--verify` flag tested on all platforms post-release
- **Install script testing**: Both install.sh and install.ps1 tested in CI
- **Security audit**: `cargo-audit` integrated
- **MSRV check**: Minimum supported Rust version (1.85) verified
- **Fail-fast builds**: All platform builds stop immediately if one fails

## Testing
- **E2E test suite** using `expectrl` — spawns the actual binary and verifies screen output
- **Test harness** (`TuiTestSession`) with reusable methods for F-key presses, text expectations, and clean exit
- **Smoke tests**: Binary version/verify flags, startup, clean exit
- **UI tests**: Key hint bar, manager footer, command palette (category colors, fuzzy filtering, no duplicate IDs)
- **Manager tests**: SSH Manager and Docker Manager E2E flows
- **Visual tests**: Rounded borders verified on all popups

## Documentation
- **`docs/hotkeys.md`**: Expanded with terminal grid splits, text selection, autocomplete, health dashboard, Docker creation workflow, mouse support, edit commands, and addon hotkeys
- **`docs/ratrc_docs.md`**: Added logging configuration, autocomplete settings, custom addon commands, environment variables, and file locations for all platforms

## Code Quality
- Resolved all clippy warnings
- Applied consistent code formatting
- Added comprehensive test coverage across all new features
- Input handling traits (`ListSelectable`, `TextInputField`, `FormNavigable`) to eliminate duplicate code across managers
