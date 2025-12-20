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
- Split terminals horizontally (`Ctrl+S`) or vertically (`Ctrl+Shift+S`)
- ANSI/VT100 escape sequence parsing
- 256-color and true color support
- Scrollback history (`Shift+PageUp/Down`)
- Alternate screen buffer support

### Code Editor
- **Four keybinding modes**: Vim, Emacs, VSCode, and Default (configurable in `~/.ratrc`)
- Modal editing (Normal, Insert, Visual, Command modes in Vim mode)
- VSCode-style editing features: multi-cursor support, line operations, smart comments
- Undo/redo support
- File browser (`Ctrl+O`)
- Search in file (`Ctrl+F`)
- Multiple file tabs (`Alt+Shift+Left/Right` to switch)

### General
- **Command Palette** (`Ctrl+Shift+P`) for quick access to all commands
- **Mode Switcher** (`Ctrl+Shift+Tab`) to cycle between Vim/Emacs/Default/VSCode editor modes
- Resizable split panes (`Alt+[` / `Alt+]`)
- Clipboard support (`Ctrl+Shift+C` to copy, `Ctrl+V` to paste)
- Save confirmation on exit
- Auto-updates (checks on startup)

### Extensions
- **Theme Extensions** - Custom TOML-based color schemes
- **WASM Plugins** - Sandboxed, portable extensions
- **Native Plugins** - Full-access compiled plugins (.dll/.so/.dylib)
- **GitHub Installation** - `rat ext install user/repo`
- See [docs/extensions.md](docs/extensions.md) for full documentation

## Keybindings

### Global
| Key | Action |
|-----|--------|
| `Ctrl+Shift+P` | Open Command Palette |
| `Ctrl+Shift+Tab` | Switch Editor Mode (cycles through Vim/Emacs/Default/VSCode) |
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

### Editor (VSCode Mode)

VSCode mode provides a familiar editing experience for VSCode users with standard keyboard shortcuts.

#### Navigation
| Key | Action |
|-----|--------|
| Arrow keys | Move cursor |
| `Home/End` | Line start/end |
| `Ctrl+Home/End` | Buffer start/end |
| `Ctrl+Left/Right` | Word navigation |
| `PageUp/Down` | Page navigation |

#### Selection
| Key | Action |
|-----|--------|
| `Shift+Arrow` | Extend selection |
| `Shift+Home/End` | Select to line start/end |
| `Ctrl+Shift+Left/Right` | Select word |
| `Ctrl+A` | Select all |
| `Ctrl+L` | Select line |

#### Editing
| Key | Action |
|-----|--------|
| `Ctrl+Z` | Undo |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo |
| `Ctrl+S` | Save |
| `Ctrl+D` | Duplicate line |
| `Ctrl+Shift+K` | Delete line |
| `Alt+Up/Down` | Move line up/down |
| `Ctrl+/` | Toggle comment |
| `Ctrl+]` / `Ctrl+[` | Indent/Outdent |
| `Tab` | Indent |
| `Shift+Tab` | Outdent |

## Configuration

Ratterm reads configuration from `~/.ratrc` on startup.

```bash
# Keybinding mode: vim, emacs, vscode, or default
mode = vim

# For VSCode mode, use:
# mode = vscode

# Custom keybindings (optional)
# quit = ctrl+q
# copy = ctrl+shift+c
# paste = ctrl+v
```

### VSCode Settings Import

When using VSCode mode, Ratterm can automatically import settings from your existing VSCode installation. The following settings are supported:

| VSCode Setting | Description |
|----------------|-------------|
| `editor.tabSize` | Number of spaces per tab |
| `editor.insertSpaces` | Use spaces instead of tabs |
| `editor.wordWrap` | Word wrap mode (off/on/bounded) |
| `editor.cursorStyle` | Cursor style (line/block/underline) |
| `editor.lineNumbers` | Line numbers mode (off/on/relative) |
| `files.autoSave` | Auto-save mode |
| `files.trimTrailingWhitespace` | Trim trailing whitespace on save |
| `files.insertFinalNewline` | Insert final newline on save |

Settings are loaded from:
- **Windows**: `%APPDATA%\Code\User\settings.json`
- **macOS**: `~/Library/Application Support/Code/User/settings.json`
- **Linux**: `~/.config/Code/User/settings.json`

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

## Acknowledgments

- [ratatui](https://github.com/ratatui-org/ratatui) - TUI framework
- [portable-pty](https://github.com/wez/wezterm/tree/main/pty) - Cross-platform PTY
- [ropey](https://github.com/cessen/ropey) - Rope data structure
