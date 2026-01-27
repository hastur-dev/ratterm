# What's New in This Release

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

## Docker Manager Improvements

- Fixed command execution on remote Linux hosts (was incorrectly using Windows paths)
- Improved password authentication via plink
- Better handling of special characters in docker commands

## Windows 11 Fixes

- Resolved hotkey detection issues specific to Windows 11
- Improved console input handling

## Code Quality

- Resolved all clippy warnings
- Applied consistent code formatting
- Added comprehensive test coverage for daemon system
