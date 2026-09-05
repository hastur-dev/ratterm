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

Every top-level module, and what it is for. `tests/docs_tests.rs` fails if this
list and `src/lib.rs` disagree, which is how the previous version of this
section came to name three modules that did not exist.

| Module | Purpose |
|---|---|
| `api` | Control API: named pipe, Unix socket or loopback TCP, with per-session token authentication |
| `app` | Application state and the orchestration between every other module |
| `cli` | Command-line parsing for headless runs, scenarios and endpoint selection |
| `clipboard` | Copy and paste, with an OSC 52 path for remote shells |
| `completion` | Autocomplete: keyword fallback, caching and debouncing |
| `config` | `.ratrc` parsing, keybinding modes, platform key differences |
| `daemon` | Push metric collection: deployment, and the receiver remote hosts report to |
| `debugger` | Debug Adapter Protocol sessions and breakpoints |
| `docker` | Container, image and host management |
| `docker_logs` | Container log streaming, storage and search |
| `editor` | Text buffer, cursor, viewport, per-tab state, Vim and Emacs behaviour |
| `extension` | Extension loading, approval and lifecycle |
| `filebrowser` | File and directory browsing with fuzzy search |
| `fixtures` | Fixture state for scripted runs, in place of the user's real configuration |
| `git` | Status, gutter marks, blame, diff and the Git dashboard |
| `hosts` | The one host registry: identity, capabilities and reachability |
| `k8s` | Kubernetes: contexts, typed resource views, actions and pod logs |
| `logging` | Tracing setup and log retention |
| `lsp` | Language servers: diagnostics, hover, definitions, symbols, formatting |
| `remote` | Persistent SSH sessions, the session pool, port forwards and remote execution |
| `scenario` | Scripted interface runs and their assertions |
| `secrets` | Credential storage: the OS keychain, and an encrypted file where there is none |
| `session` | Session persistence across restarts |
| `ssh` | Host list, credentials, network scanning and metric collection over SSH |
| `store` | The durable SQLite store: metrics, events, alerts, retention and downsampling |
| `telemetry` | The one ingest path both metric collectors write to |
| `terminal` | PTY, ANSI parsing, the cell grid and the multiplexer |
| `theme` | Colour themes and their persistence |
| `ui` | Every widget. Presentation only — logic lives in the module it belongs to |
| `updater` | Update checking and installation |

Two binaries are built from these: `rat`, the application, and `rat-agent`, the
metric reporter a fleet machine runs (`src/bin/rat-agent.rs`).

---

## Core Components

### Panels and their state (`src/app/panel.rs`)

Every list in the interface — references, code actions, symbols, diagnostics,
hosts, containers, pods — used to carry three fields on `App`: the items, a
selected index, and a scroll offset. Nineteen of them were the language server
alone. Each was navigated by its own hand-written code, so a fix to one did not
reach the others, and an index could outlive the list it pointed into.

`ListPanel<T>` is that state once, with selection bounded by construction. A
panel holding no items has no selection; replacing the items moves the
selection into the new list rather than leaving it past the end. `None` items
means closed, which is deliberately not the same as open-and-empty — "no
references found" and "the references panel is not open" are different answers,
and conflating them is why an empty result used to render as a blank box.

A few panels draw more rows than they hold items: a references panel holds one
item per file and draws one row per location. Those use the `_in(rows)` methods,
which take the row count rather than inferring it.

`Panel` is what the application asks of a panel — a title, whether it is open,
what a key did, and how to draw itself — so `App` can treat them alike instead
of naming each one in every match. `K8sManager` implements it.

Three states group what used to be loose fields:

| Type | File | Replaces |
|---|---|---|
| `LspUiState` | `src/app/lsp_state.rs` | 19 `lsp_*` fields |
| `GitUiState` | `src/app/side_state.rs` | `git_gutter`, `git_blame_active`, `git_blame_data` |
| `DebugUiState` | `src/app/side_state.rs` | `debug_session`, `breakpoint_store`, `debug_panel_visible` |

`GitUiState` is the clearest case: blame was a flag and a vector that could
disagree, so a failed load could leave the previous file's blame beside the
current file's text. It is one `Option<Vec<BlameLine>>` now, and that state is
unrepresentable.

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
