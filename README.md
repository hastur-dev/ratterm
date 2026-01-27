# Ratterm

#

A split-terminal TUI application with a PTY-based terminal emulator and code editor. 
Made with Ratatui and Crossterm

```
  ╦═╗╔═╗╔╦╗╔╦╗╔═╗╦═╗╔╦╗
  ╠╦╝╠═╣ ║  ║ ║╣ ╠╦╝║║║
  ╩╚═╩ ╩ ╩  ╩ ╚═╝╩╚═╩ ╩
```

## Installation

### Quick Install

```bash
# Linux/macOS
curl -fsSL https://raw.githubusercontent.com/hastur-dev/ratterm/main/install.sh | bash

# Windows (PowerShell)
irm https://raw.githubusercontent.com/hastur-dev/ratterm/main/install.ps1 | iex
```

### Quick Uninstall

```bash
# Uninstall Ratterm
curl -fsSL https://raw.githubusercontent.com/hastur-dev/ratterm/main/install.sh | bash -s -- --uninstall

irm https://raw.githubusercontent.com/hastur-dev/ratterm/main/install.ps1 | iex -Uninstall

rat uninstall

```

### From Source

```bash
git clone https://github.com/hastur-dev/ratterm
cd ratterm
cargo install --path .
```

### Usage

```bash
rat              # Start ratterm
rat myfile.rs    # Open with a file
rat --version    # Show version
rat --update     # Check for updates
```

### Extension Management

```bash
# Install extensions from GitHub
rat ext install user/repo
rat ext install user/repo@v1.0.0   # Specific version

# Manage extensions
rat ext list                       # List installed
rat ext update                     # Update all
rat ext update extension-name      # Update specific
rat ext remove extension-name      # Remove
rat ext help                       # Show help
```

## Features

### Terminal Emulator
- Full PTY (pseudo-terminal) support
- Multiple terminal tabs (`Ctrl+T` to create, `Ctrl+W` to close)
- 2x2 terminal grid layout (`Ctrl+S` to split)
- ANSI/VT100 escape sequence parsing
- 256-color and true color support
- Scrollback history (`Shift+PageUp/Down`)
- Alternate screen buffer support
- Mouse selection and scrolling
- Multiple shell support (PowerShell, Bash, Zsh, Fish, CMD)

### Code Editor
- **Three keybinding modes**: Vim, Emacs, and Default (configurable in `~/.ratrc`)
- Modal editing (Normal, Insert, Visual, Command modes in Vim mode)
- **LSP-powered autocomplete** with ghost text suggestions
- Syntax highlighting via tree-sitter
- Undo/redo support
- File browser (`Ctrl+O`)
- Search in file (`Ctrl+F`) and across files (`Ctrl+Shift+F`)
- Multiple file tabs (`Alt+Shift+Left/Right` to switch)

### Terminal-First Mode
By default, Ratterm starts with only the terminal visible:
- Type `open` or `open <file>` in terminal to show the editor
- Press `Ctrl+I` to toggle IDE visibility
- Set `ide-always = true` in `~/.ratrc` for traditional split view

### SSH Manager (`Ctrl+Shift+U`)
- Manage SSH host connections
- Network scanning for SSH hosts
- Quick connect hotkeys (`Ctrl+1-3`)
- Jump host / bastion support
- Credential storage (plaintext or encrypted)
- **SSH Health Dashboard** - Monitor CPU, RAM, disk, GPU across hosts

### Docker Manager (`Ctrl+Shift+D`)
- Browse containers and images
- Connect to running containers
- Create containers from images
- Remote Docker host support (via SSH)
- Quick connect hotkeys (`Ctrl+Alt+1-9`)
- Container stats and logs panels

### General
- **Command Palette** (`Ctrl+Shift+P` or `F1`) for quick access to all commands
- **Mode Switcher** (`Ctrl+Shift+Tab`) to cycle between Vim/Emacs/Default editor modes
- **Theming** - 6 built-in themes (Dark, Light, Dracula, Gruvbox, Nord, Matrix)
- Resizable split panes (`Alt+[` / `Alt+]`)
- Clipboard support (`Ctrl+Shift+C` to copy, `Ctrl+V` to paste)
- Custom hotkey bindings via `.ratrc`
- Save confirmation on exit
- Auto-updates (checks on startup)

### Extensions
- **REST API** for external process plugins
- **Any Language** - Write extensions in Python, Node.js, Rust, etc.
- **GitHub Installation** - `rat ext install user/repo`
- **Theme Extensions** - Custom TOML-based color schemes
- See [docs/extension_system.md](docs/extension_system.md) for API documentation

## Keybindings

### Global
| Key | Action |
|-----|--------|
| `Ctrl+Shift+P` | Open Command Palette |
| `Ctrl+Shift+Tab` | Switch Editor Mode (cycles through Vim/Emacs/Default) |
| `Alt+Left` | Focus terminal pane |
| `Alt+Right` | Focus editor pane |
| `Alt+Up/Down` | Switch between split terminals |
| `Alt+Tab` | Toggle focus between panes |
| `Alt+[` / `Alt+]` | Resize split |
| `Ctrl+Q` | Quit |
| `Ctrl+O` | Open file browser |
| `Ctrl+Shift+C` | Copy |
| `Ctrl+V` | Paste |

### Terminal
| Key | Action |
|-----|--------|
| `Ctrl+T` | New terminal tab |
| `Ctrl+W` | Close terminal tab |
| `Ctrl+Left/Right` | Switch terminal tabs |
| `Ctrl+S` | Split horizontal |
| `Ctrl+Shift+S` | Split vertical |
| `Ctrl+Shift+W` | Close split |
| `Ctrl+Tab` | Toggle split focus |
| `Shift+PageUp/Down` | Scroll history |

### Editor (Vim Mode - Default)

#### Normal Mode
| Key | Action |
|-----|--------|
| `i` | Enter Insert mode |
| `a` | Append after cursor |
| `v` | Enter Visual mode |
| `:` | Enter Command mode |
| `h/j/k/l` | Move cursor |
| `0` / `$` | Line start/end |
| `w` / `b` | Word forward/back |
| `g` / `G` | Buffer start/end |
| `x` | Delete character |
| `u` | Undo |
| `Ctrl+R` | Redo |
| `Ctrl+S` | Save |

#### Insert Mode
| Key | Action |
|-----|--------|
| `Esc` | Return to Normal mode |
| `Backspace` | Delete before cursor |
| `Enter` | New line |
| `Tab` | Insert spaces |

### Editor (Emacs Mode)
| Key | Action |
|-----|--------|
| `Ctrl+B/F/P/N` | Move left/right/up/down |
| `Ctrl+A/E` | Line start/end |
| `Alt+F/B` | Word forward/back |
| `Ctrl+D` | Delete character |
| `Ctrl+K` | Kill to end of line |
| `Ctrl+/` | Undo |
| `Ctrl+X` | Save |

### Editor (Default Mode)
| Key | Action |
|-----|--------|
| Arrow keys | Move cursor |
| `Home/End` | Line start/end |
| `Ctrl+Left/Right` | Word navigation |
| `Ctrl+Z/Y` | Undo/Redo |
| `Ctrl+S` | Save |

## Configuration

Ratterm reads configuration from `~/.ratrc` on startup.

```bash
# Keybinding mode: vim, emacs, or default
mode = vim

# Custom keybindings (optional)
# quit = ctrl+q
# copy = ctrl+shift+c
# paste = ctrl+v
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RATTERM_NO_UPDATE` | Disable auto-update checks |
| `RATTERM_INSTALL_DIR` | Custom install directory |

## Requirements

- **Windows**: Windows 10 1809+ (ConPTY support)
- **Linux/macOS**: Standard POSIX PTY support
- **Rust**: 1.85+ (for building from source, Rust 2024 Edition)

## Development

```bash
# Run tests
cargo test

# Build release
cargo build --release

# Run with logging
RUST_LOG=debug cargo run
```

## License

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

See [LICENSE](LICENSE) for the full license text.

## Documentation

Detailed documentation is available in the `docs/` folder:

| Document | Description |
|----------|-------------|
| [hotkeys.md](docs/hotkeys.md) | Complete keyboard shortcut reference |
| [ratrc_docs.md](docs/ratrc_docs.md) | Configuration file reference |
| [command_palette.md](docs/command_palette.md) | Command palette commands |
| [architecture.md](docs/architecture.md) | System architecture overview |
| [extension_system.md](docs/extension_system.md) | Extension REST API reference |
| [extensions.md](docs/extensions.md) | Extension development guide |
| [testing.md](docs/testing.md) | Testing and CI guide |

## Acknowledgments

- [ratatui](https://github.com/ratatui-org/ratatui) - TUI framework
- [portable-pty](https://github.com/wez/wezterm/tree/main/pty) - Cross-platform PTY
- [ropey](https://github.com/cessen/ropey) - Rope data structure
