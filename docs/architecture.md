# Ratterm Architecture

This document provides a high-level overview of Ratterm's architecture and how its components interact.

---

## Overview

Ratterm is a split-terminal TUI application built with Rust. It combines:

- **Terminal Emulator**: Full PTY-based terminal with ANSI/VT100 support
- **Code Editor**: Multi-mode editor with Vim, Emacs, and Default keybindings
- **File Browser**: Fuzzy-search file navigation
- **SSH Manager**: Manage and connect to SSH hosts
- **Docker Manager**: Manage and connect to Docker containers
- **Extension System**: REST API-based plugin architecture

---

## Module Structure

```
src/
+-- main.rs                 # Entry point, event loop
+-- lib.rs                  # Library exports
+-- app/
|   +-- mod.rs              # Core App state & orchestration
|   +-- input.rs            # Global input routing
|   +-- input_editor.rs     # Editor-specific input handling
|   +-- input_health.rs     # Health dashboard input
|   +-- popup_ops.rs        # Popup operation handlers
|   +-- keymap.rs           # Key-to-bytes conversion
+-- config/
|   +-- mod.rs              # Configuration loading (.ratrc)
|   +-- keybindings.rs      # Keybinding mode definitions
+-- editor/
|   +-- mod.rs              # Editor core with modes
|   +-- buffer.rs           # Text buffer (ropey-based)
|   +-- cursor.rs           # Cursor position & movement
|   +-- edit.rs             # Edit operations
|   +-- find.rs             # Search functionality
|   +-- view.rs             # Viewport management
+-- terminal/
|   +-- mod.rs              # Terminal core
|   +-- pty.rs              # PTY spawning & management
|   +-- parser.rs           # ANSI/VTE escape parsing
|   +-- grid.rs             # Terminal cell grid
|   +-- multiplexer/        # Multi-tab & split management
|   +-- cell.rs             # Cell representation
|   +-- action.rs           # Terminal actions
|   +-- style.rs            # ANSI style handling
+-- ui/
|   +-- mod.rs              # UI module exports
|   +-- layout.rs           # Split pane layout manager
|   +-- editor_widget.rs    # Editor rendering
|   +-- terminal_widget.rs  # Terminal rendering
|   +-- editor_tabs.rs      # Editor tab bar
|   +-- terminal_tabs.rs    # Terminal tab bar
|   +-- statusbar.rs        # Status bar
|   +-- file_picker.rs      # File browser UI
|   +-- popup.rs            # Popup dialogs
+-- filebrowser/
|   +-- mod.rs              # File browser logic
|   +-- entry.rs            # File entry representation
+-- ssh/
|   +-- mod.rs              # SSH manager
|   +-- hosts.rs            # Host storage
|   +-- scanner.rs          # Network scanning
|   +-- health.rs           # Health monitoring
+-- docker/
|   +-- mod.rs              # Docker manager
|   +-- discovery.rs        # Container discovery
|   +-- remote.rs           # Remote host support
+-- completion/
|   +-- mod.rs              # Completion system
|   +-- lsp.rs              # LSP integration
|   +-- keywords.rs         # Keyword fallback
+-- extensions/
|   +-- mod.rs              # Extension loader
|   +-- api.rs              # REST API server
|   +-- manager.rs          # Extension lifecycle
+-- clipboard.rs            # Clipboard operations
+-- updater.rs              # Auto-update checking
```

---

## Core Components

### App (`src/app/mod.rs`)

The central state manager that orchestrates all components:

```rust
pub struct App {
    terminals: Option<TerminalMultiplexer>,  // Terminal tabs & splits
    editor: Editor,                          // Code editor
    file_browser: FileBrowser,               // File navigation
    layout: SplitLayout,                     // Pane layout
    mode: AppMode,                           // Current UI mode
    popup: Popup,                            // Active popup
    config: Config,                          // User configuration
    // ... SSH, Docker, extensions
}
```

**Responsibilities:**
- Route input events to appropriate handlers
- Coordinate state between components
- Manage UI modes and popups
- Handle file operations

### Terminal Multiplexer (`src/terminal/multiplexer/`)

Manages multiple terminal tabs with optional grid splitting:

```
Tab 1 (current)    Tab 2              Tab 3
+---+---+          +-------+          +-------+
| A | B |          |       |          |       |
+---+---+          |   X   |          |   Y   |
| C | D |          |       |          |       |
+---+---+          +-------+          +-------+
(2x2 grid)         (single)           (single)
```

**Features:**
- Up to 4 terminals per tab (2x2 grid)
- Independent focus per pane
- PTY management for each terminal

### Editor (`src/editor/`)

Modal text editor supporting multiple keybinding modes:

```
Keybinding Modes:
+-- Vim (default)
|   +-- Normal mode
|   +-- Insert mode
|   +-- Visual mode
|   +-- Command mode
+-- Emacs
+-- Default (arrow keys)
```

**Components:**
- **Buffer**: Rope-based text storage (ropey)
- **Cursor**: Position and selection management
- **View**: Viewport and scroll handling
- **Edit**: Text manipulation operations

### Layout (`src/ui/layout.rs`)

Manages the split between terminal and editor panes:

```
+------------------+------------------+
|                  |                  |
|    Terminal      |     Editor       |
|      Pane        |      Pane        |
|                  |                  |
+------------------+------------------+
       ^                   ^
       |                   |
    Alt+Left            Alt+Right

<-- Alt+[ shrink    Alt+] expand -->
```

**Modes:**
- **Terminal-first** (default): IDE hidden until needed
- **IDE-always**: Both panes always visible

---

## Data Flow

### Input Event Flow

```
crossterm::Event
       |
       v
  main.rs (event loop)
       |
       v
  App::handle_event()
       |
       +-- Mode == Popup? --> Popup handler
       |
       +-- Mode == FileBrowser? --> FileBrowser handler
       |
       +-- Focus == Terminal? --> Terminal input
       |
       +-- Focus == Editor? --> Editor input (by keybinding mode)
                                   |
                                   +-- Vim handler
                                   +-- Emacs handler
                                   +-- Default handler
```

### Render Flow

```
main.rs (render loop)
       |
       v
  App::render()
       |
       +-- SplitLayout::render()
       |       |
       |       +-- TerminalWidget (left pane)
       |       +-- EditorWidget (right pane)
       |
       +-- StatusBar::render()
       |
       +-- Popup::render() (if active)
```

---

## Extension Architecture

Extensions run as external processes communicating via REST API:

```
+-------------+          HTTP (127.0.0.1:7878)          +-------------+
|   Ratterm   | <-------------------------------------> |  Extension  |
|   (Host)    |          JSON REST API                  |  (Process)  |
+-------------+                                         +-------------+
      |                                                       |
      v                                                       v
  - Event stream (SSE)                                  - Any language
  - Terminal operations                                 - Own runtime
  - Editor operations                                   - User approval
  - File system access                                  - Sandboxed
```

**API Categories:**
- `/api/v1/terminal/*` - Terminal buffer, input, scrolling
- `/api/v1/editor/*` - Editor content, cursor, operations
- `/api/v1/fs/*` - File system operations
- `/api/v1/layout/*` - Pane focus and sizing
- `/api/v1/system/*` - Config, themes, notifications
- `/api/v1/events/stream` - Real-time event stream (SSE)

---

## Configuration Flow

```
~/.ratrc (user config)
       |
       v
  Config::load()
       |
       +-- Parse key=value pairs
       |
       +-- Apply keybinding mode
       |
       +-- Apply theme
       |
       +-- Apply custom keybindings
       |
       v
  App::new(config)
```

**Config priorities:**
1. Command-line arguments (highest)
2. Environment variables
3. `.ratrc` file
4. Built-in defaults (lowest)

---

## PTY Architecture

Each terminal pane connects to a pseudo-terminal:

```
+-------------+     +-------------+     +-------------+
|  Terminal   | --> |    PTY      | --> |    Shell    |
|   Widget    |     |  (conpty/   |     | (bash/pwsh) |
|             | <-- |   unix)     | <-- |             |
+-------------+     +-------------+     +-------------+
      |                   |                   |
   Render             Read/Write          Execute
   ANSI              stdin/stdout        Commands
```

**Platform support:**
- **Windows**: ConPTY (Windows 10 1809+)
- **Linux/macOS**: Unix PTY (openpty)

---

## Theme System

Themes define colors for all UI components:

```
Theme Definition
       |
       +-- Terminal colors (fg, bg, cursor, selection)
       +-- Editor colors (syntax, gutter, line numbers)
       +-- Status bar colors (mode indicators)
       +-- Tab bar colors (active, inactive)
       +-- Popup colors (borders, selections)
       +-- File browser colors (directories, files)
```

**Built-in themes:** Dark, Light, Dracula, Gruvbox, Nord

---

## Completion System

Code completion uses LSP when available, with keyword fallback:

```
User types
    |
    v
Debounce (300ms)
    |
    v
+-- LSP available? --> LSP request --> LSP completions
|
+-- No LSP --> Keyword completions (buffer + language keywords)
    |
    v
Ghost text display
    |
    v
Ctrl+Space --> Accept completion
```

**Supported LSP servers:**
- Rust (rust-analyzer)
- Python (pylsp, pyright)
- JavaScript/TypeScript (tsserver)
- Go (gopls)
- C/C++ (clangd)

---

## Error Handling Strategy

1. **User-facing errors**: Display in status bar or popup
2. **Recoverable errors**: Log and continue
3. **Fatal errors**: Clean shutdown with error message

All components use `Result<T, E>` for error propagation.

---

## Performance Considerations

- **Rope buffer**: O(log n) text operations
- **Virtual scrolling**: Only render visible content
- **Debounced completion**: Reduce LSP requests
- **Cached PTY output**: Efficient terminal updates
- **Event batching**: Combine rapid input events

---

## Security Model

- **Extensions**: Require user approval, run in separate processes
- **SSH credentials**: Optional master password encryption
- **API authentication**: Bearer token for extension API
- **Localhost only**: API binds to 127.0.0.1

---

## Dependencies

| Dependency | Purpose |
|------------|---------|
| `ratatui` | TUI framework |
| `crossterm` | Terminal I/O |
| `portable-pty` | Cross-platform PTY |
| `vte` | ANSI escape parsing |
| `ropey` | Rope data structure for text |
| `tree-sitter` | Syntax highlighting |
| `tokio` | Async runtime |
| `serde` | Serialization |
| `tracing` | Logging |

See [DEPENDENCIES.md](../DEPENDENCIES.md) for complete list.
