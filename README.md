# Ratterm

#
### "I don't want to use more than 1 window to do everything and I'm doing most things in the terminal anyway" -- by some guy who will later consider all this code to be trash
#

A split-terminal TUI application with a PTY-based terminal emulator and code editor.

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
- Vim, Emacs, and Default keybinding modes (configurable in `~/.ratrc`)
- Modal editing (Normal, Insert, Visual, Command modes in Vim mode)
- Undo/redo support
- File browser (`Ctrl+O`)
- Search in file (`Ctrl+F`)
- Multiple file tabs (`Alt+Shift+Left/Right` to switch)

### General
- Resizable split panes (`Alt+[` / `Alt+]`)
- Clipboard support (`Ctrl+Shift+C` to copy, `Ctrl+V` to paste)
- Save confirmation on exit
- Auto-updates (checks on startup)

## Keybindings

### Global
| Key | Action |
|-----|--------|
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
- **Rust**: 1.75+ (for building from source)

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

MIT

## Acknowledgments

- [ratatui](https://github.com/ratatui-org/ratatui) - TUI framework
- [portable-pty](https://github.com/wez/wezterm/tree/main/pty) - Cross-platform PTY
- [ropey](https://github.com/cessen/ropey) - Rope data structure
