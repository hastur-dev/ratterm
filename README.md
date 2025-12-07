# Ratatui Full IDE

A split-terminal TUI application with a PTY-based terminal emulator on the left and an LSP-powered code editor on the right.

## Features

### Terminal Emulator (Left Pane)
- Full PTY (pseudo-terminal) support via `portable-pty`
- ANSI/VT100 escape sequence parsing via `vte`
- Supports interactive programs (bash, zsh, vim, etc.)
- 256-color and true color support
- Alternate screen buffer support
- Scrollback history

### Code Editor (Right Pane)
- Efficient text buffer using `ropey` rope data structure
- Vim-like keybindings (Normal, Insert, Visual modes)
- Undo/redo support
- Syntax highlighting (via Tree-sitter - planned)
- LSP integration for autocomplete, diagnostics (planned)

### UI
- Resizable split panes
- Focus management with `Alt+Left/Right` or `Alt+Tab`
- Status bar with mode, cursor position, file info

## Prerequisites

- Rust 1.75 or later
- A terminal emulator

### Windows
- Windows 10 1809 or later (for ConPTY support)

### Linux/macOS
- Standard POSIX PTY support

## Installation

```bash
# Clone the repository
git clone https://github.com/your-username/ratatui-full-ide
cd ratatui-full-ide

# Build in release mode
cargo build --release

# Run
cargo run --release
```

## Usage

### Keybindings

#### Global
| Key | Action |
|-----|--------|
| `Alt+Left` | Focus terminal pane |
| `Alt+Right` | Focus editor pane |
| `Alt+Tab` | Toggle between panes |
| `Alt+[` | Move split left |
| `Alt+]` | Move split right |
| `Ctrl+Q` | Quit |

#### Editor - Normal Mode
| Key | Action |
|-----|--------|
| `i` | Enter Insert mode |
| `a` | Enter Insert mode after cursor |
| `v` | Enter Visual mode |
| `h/j/k/l` or arrows | Move cursor |
| `0` | Go to line start |
| `$` or `End` | Go to line end |
| `w` | Move to next word |
| `b` | Move to previous word |
| `g` | Go to buffer start |
| `G` | Go to buffer end |
| `x` | Delete character |
| `u` | Undo |
| `Ctrl+R` | Redo |
| `Ctrl+S` | Save file |
| `PageUp/Down` | Page navigation |

#### Editor - Insert Mode
| Key | Action |
|-----|--------|
| `Esc` | Return to Normal mode |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |
| `Enter` | Insert newline |
| `Tab` | Insert 4 spaces |
| arrows | Move cursor |
| `Ctrl+S` | Save file |

#### Editor - Visual Mode
| Key | Action |
|-----|--------|
| `Esc` | Cancel selection, return to Normal mode |
| `h/l` or arrows | Extend selection |
| `d` or `x` | Delete selection |

## Architecture

```
src/
├── main.rs                 # Entry point
├── lib.rs                  # Library root
├── app.rs                  # Application state and event loop
├── terminal/               # Terminal emulator
│   ├── mod.rs              # Terminal orchestration
│   ├── pty.rs              # PTY spawning via portable-pty
│   ├── parser.rs           # ANSI escape sequence parser
│   ├── grid.rs             # Terminal cell grid
│   └── style.rs            # Colors and attributes
├── editor/                 # Code editor
│   ├── mod.rs              # Editor orchestration
│   ├── buffer.rs           # Text buffer with ropey
│   ├── cursor.rs           # Cursor and selection
│   └── view.rs             # Viewport management
└── ui/                     # User interface
    ├── mod.rs              # UI module root
    ├── layout.rs           # Split pane layout
    ├── terminal_widget.rs  # Terminal rendering
    ├── editor_widget.rs    # Editor rendering
    └── statusbar.rs        # Status bar
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run library tests only
cargo test --lib

# Run with verbose output
cargo test -- --nocapture
```

### Code Quality

```bash
# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --all-targets -- -D warnings

# Build with all warnings as errors
RUSTFLAGS="-D warnings" cargo build
```

## Planned Features

- [ ] LSP integration for language servers
- [ ] Syntax highlighting via Tree-sitter
- [ ] File picker with fuzzy search
- [ ] Multiple editor tabs
- [ ] Configuration file support
- [ ] Theming

## License

MIT

## Acknowledgments

- [ratatui](https://github.com/ratatui-org/ratatui) - TUI framework
- [portable-pty](https://github.com/wez/wezterm/tree/main/pty) - Cross-platform PTY
- [vte](https://github.com/alacritty/vte) - ANSI parser
- [ropey](https://github.com/cessen/ropey) - Rope data structure
