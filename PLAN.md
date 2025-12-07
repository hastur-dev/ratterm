# Ratatui Full IDE - Architecture Plan

## Overview

A split-terminal TUI application:
- **Left Pane**: Full PTY terminal emulator (bash/zsh/vim compatible)
- **Right Pane**: VSCode-like editor using Language Server Protocol (LSP)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            ratatui-full-ide                             │
├─────────────────────────────────────────────────────────────────────────┤
│  src/                                                                   │
│  ├── main.rs                 # Entry point, event loop                 │
│  ├── app.rs                  # Application state & orchestration       │
│  ├── config.rs               # Configuration loading (config.yaml)     │
│  │                                                                      │
│  ├── terminal/               # Left pane - PTY terminal                │
│  │   ├── mod.rs                                                        │
│  │   ├── pty.rs              # PTY spawning via portable-pty           │
│  │   ├── parser.rs           # ANSI/VTE escape sequence parsing        │
│  │   ├── grid.rs             # Terminal grid/cell buffer               │
│  │   └── input.rs            # Keyboard input handling for PTY         │
│  │                                                                      │
│  ├── editor/                 # Right pane - Code editor                │
│  │   ├── mod.rs                                                        │
│  │   ├── buffer.rs           # Text buffer (using ropey)               │
│  │   ├── cursor.rs           # Cursor position & movement              │
│  │   ├── view.rs             # Viewport/scroll management              │
│  │   ├── highlight.rs        # Tree-sitter syntax highlighting         │
│  │   ├── input.rs            # Editor keybindings                      │
│  │   └── file_picker.rs      # File search/picker UI                   │
│  │                                                                      │
│  ├── lsp/                    # Language Server Protocol client         │
│  │   ├── mod.rs                                                        │
│  │   ├── client.rs           # LSP client implementation               │
│  │   ├── transport.rs        # JSON-RPC stdio transport                │
│  │   ├── capabilities.rs     # Server capability negotiation           │
│  │   ├── completion.rs       # Autocomplete handling                   │
│  │   ├── diagnostics.rs      # Error/warning display                   │
│  │   ├── hover.rs            # Hover information                       │
│  │   ├── goto.rs             # Go-to-definition/references             │
│  │   └── registry.rs         # Language server discovery/config        │
│  │                                                                      │
│  ├── ui/                     # UI rendering                            │
│  │   ├── mod.rs                                                        │
│  │   ├── layout.rs           # Split pane layout management            │
│  │   ├── terminal_widget.rs  # Render PTY terminal                     │
│  │   ├── editor_widget.rs    # Render code editor                      │
│  │   ├── completion_menu.rs  # Autocomplete dropdown                   │
│  │   ├── diagnostics_panel.rs # Error/warning list                     │
│  │   ├── file_picker_widget.rs # File search UI                        │
│  │   ├── statusbar.rs        # Status bar                              │
│  │   └── theme.rs            # Color themes                            │
│  │                                                                      │
│  └── utils/                  # Shared utilities                        │
│      ├── mod.rs                                                        │
│      ├── event.rs            # Event types & channels                  │
│      └── keybindings.rs      # Keybinding configuration                │
│                                                                         │
│  tests/                      # Integration tests                       │
│  examples/                   # Usage examples                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Core Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| `ratatui` | TUI framework | 0.29 |
| `crossterm` | Terminal backend | 0.28 |
| `portable-pty` | Cross-platform PTY | 0.8 |
| `vte` | ANSI escape sequence parsing | 0.13 |
| `ropey` | Efficient text buffer (rope data structure) | 1.6 |
| `tree-sitter` | Syntax highlighting | 0.24 |
| `tree-sitter-*` | Language grammars | various |
| `lsp-types` | LSP protocol types | 0.97 |
| `tokio` | Async runtime | 1.41 |
| `serde` | Serialization | 1.0 |
| `serde_json` | JSON for LSP | 1.0 |
| `serde_yaml` | Config parsing | 0.9 |
| `dirs` | Platform directories | 5.0 |
| `fuzzy-matcher` | File picker fuzzy search | 0.3 |
| `thiserror` | Error handling | 2.0 |
| `tracing` | Logging/observability | 0.1 |

---

## Component Details

### 1. Terminal Emulator (Left Pane)

**PTY Management (`terminal/pty.rs`)**
- Spawn shell process via `portable-pty`
- Read/write to PTY master fd
- Handle window resize (SIGWINCH equivalent)

**ANSI Parser (`terminal/parser.rs`)**
- Use `vte` crate for parsing escape sequences
- Support SGR (colors/styles), cursor movement, scrolling
- Handle alternate screen buffer (for vim/less)

**Terminal Grid (`terminal/grid.rs`)**
- 2D grid of cells with character + style
- Scrollback buffer (configurable size)
- Damage tracking for efficient redraws

**Key Features:**
- Full ANSI/VT100/VT220 compatibility
- 256-color and true color support
- Mouse event passthrough
- Scrollback history
- Copy/paste support

### 2. Code Editor (Right Pane)

**Text Buffer (`editor/buffer.rs`)**
- Use `ropey` rope data structure
- O(log n) insertions/deletions
- Line-based access for rendering
- Undo/redo history

**Syntax Highlighting (`editor/highlight.rs`)**
- Tree-sitter for parsing
- Incremental re-parsing on edits
- Language grammar loading
- Highlight queries for theming

**Viewport (`editor/view.rs`)**
- Scroll position management
- Line wrapping (optional)
- Line numbers
- Git gutter (future)

**File Picker (`editor/file_picker.rs`)**
- Fuzzy file search
- Recent files list
- Directory browsing
- Integration with left terminal for file ops

### 3. LSP Client

**Transport (`lsp/transport.rs`)**
- JSON-RPC over stdio
- Message framing (Content-Length headers)
- Request/response correlation
- Async message handling

**Client (`lsp/client.rs`)**
- Initialize/shutdown lifecycle
- Capability negotiation
- Document synchronization
- Request multiplexing

**Features Supported:**
| Feature | LSP Method |
|---------|------------|
| Autocomplete | `textDocument/completion` |
| Diagnostics | `textDocument/publishDiagnostics` |
| Hover | `textDocument/hover` |
| Go to Definition | `textDocument/definition` |
| Find References | `textDocument/references` |
| Signature Help | `textDocument/signatureHelp` |
| Code Actions | `textDocument/codeAction` |
| Formatting | `textDocument/formatting` |

**Language Server Registry (`lsp/registry.rs`)**
```yaml
# config.yaml example
language_servers:
  rust:
    command: rust-analyzer
    args: []
    root_patterns: ["Cargo.toml"]
  python:
    command: pyright-langserver
    args: ["--stdio"]
    root_patterns: ["pyproject.toml", "setup.py"]
  typescript:
    command: typescript-language-server
    args: ["--stdio"]
    root_patterns: ["package.json", "tsconfig.json"]
```

### 4. UI/Layout

**Split Layout (`ui/layout.rs`)**
- Horizontal split (terminal | editor)
- Resizable split position
- Focus management (Alt+Left/Right)
- Fullscreen toggle for either pane

**Keybindings:**
| Key | Action |
|-----|--------|
| `Alt+Left/Right` | Switch focus between panes |
| `Alt+[/]` | Resize pane split |
| `Ctrl+P` | Open file picker (editor) |
| `Ctrl+Space` | Trigger autocomplete (editor) |
| `F2` | Go to definition |
| `Shift+F2` | Find references |
| `Ctrl+.` | Code actions |
| `Ctrl+S` | Save file |
| `Escape` | Close popups/menus |

---

## Event Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Main Event Loop                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Terminal   │  │    Editor    │  │     LSP      │          │
│  │   Events     │  │    Events    │  │   Events     │          │
│  │   (PTY)      │  │  (keyboard)  │  │  (async)     │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│         └────────────────┴─────────────────┘                   │
│                          │                                      │
│                    ┌─────▼─────┐                                │
│                    │  App::    │                                │
│                    │  update() │                                │
│                    └─────┬─────┘                                │
│                          │                                      │
│                    ┌─────▼─────┐                                │
│                    │  render() │                                │
│                    └───────────┘                                │
└─────────────────────────────────────────────────────────────────┘
```

**Event Types:**
```rust
enum AppEvent {
    // Input events
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),

    // PTY events
    PtyOutput(Vec<u8>),
    PtyExit(i32),

    // LSP events
    LspResponse { id: i64, result: Value },
    LspNotification { method: String, params: Value },
    LspDiagnostics { uri: Url, diagnostics: Vec<Diagnostic> },

    // Editor events
    FileOpened { path: PathBuf },
    FileSaved { path: PathBuf },

    // System
    Tick,
    Quit,
}
```

---

## Configuration (config.yaml)

```yaml
# Theme
theme: "vscode-dark"

# Terminal settings
terminal:
  shell: null  # null = use $SHELL or default
  scrollback: 10000
  font_size: 14

# Editor settings
editor:
  tab_size: 4
  insert_spaces: true
  word_wrap: false
  line_numbers: true

# LSP settings
lsp:
  auto_start: true
  format_on_save: true

# Language servers
language_servers:
  rust:
    command: "rust-analyzer"
  python:
    command: "pyright-langserver"
    args: ["--stdio"]
  javascript:
    command: "typescript-language-server"
    args: ["--stdio"]
  go:
    command: "gopls"

# Keybindings (override defaults)
keybindings:
  switch_pane: "Alt+Tab"
  file_picker: "Ctrl+P"
```

---

## Implementation Phases

### Phase 1: Foundation
1. Project scaffold with Cargo workspace
2. Basic ratatui app with split layout
3. Event loop architecture
4. Configuration loading

### Phase 2: Terminal Emulator
1. PTY spawning with portable-pty
2. VTE parser integration
3. Terminal grid and rendering
4. Input handling and passthrough

### Phase 3: Basic Editor
1. Text buffer with ropey
2. Cursor movement and editing
3. File loading/saving
4. Basic viewport scrolling

### Phase 4: Syntax Highlighting
1. Tree-sitter integration
2. Language grammar loading
3. Highlight query application
4. Theme system

### Phase 5: LSP Integration
1. JSON-RPC transport
2. LSP client lifecycle
3. Document synchronization
4. Autocomplete

### Phase 6: Advanced LSP Features
1. Diagnostics display
2. Hover information
3. Go-to-definition
4. Code actions

### Phase 7: Polish
1. File picker with fuzzy search
2. Status bar
3. Error handling improvements
4. Performance optimization

---

## Safety Rules Compliance

| Rule | Implementation |
|------|----------------|
| Simple Control Flow | No recursion; iterative algorithms with explicit stacks |
| Bounded Loops | All loops have MAX_ITERATIONS constants |
| No Runtime Allocation | Pre-allocated buffers for terminal grid, text chunks |
| Function Length ≤60 lines | Enforced via clippy lint |
| Assertion Density ≥2/fn | Debug assertions for invariants |
| Minimal Scope | Variables declared at point of use |
| Checked Returns | All Results propagated with `?` or explicit handling |
| Limited Macros | Only derive macros and logging |
| Single Dereference | References only, no raw pointers |
| Zero Warnings | `RUSTFLAGS="-D warnings"` in CI |

---

## Test Strategy

| Component | Test Type | Coverage Target |
|-----------|-----------|-----------------|
| terminal/parser | Unit (property-based) | ANSI sequence parsing |
| terminal/grid | Unit | Cell operations, scrolling |
| editor/buffer | Unit (property-based) | Rope operations |
| editor/cursor | Unit | Movement, bounds |
| lsp/transport | Unit | Message framing |
| lsp/client | Integration | Request/response flow |
| ui/* | Snapshot | Render output |
| Full app | Integration | User workflows |

---

## File Line Counts (Target)

Each file should stay under 500 lines per the /enforce-5-steps rule. Complex components are split:

- `lsp/client.rs` → `client.rs` + `capabilities.rs` + `sync.rs`
- `terminal/parser.rs` → Uses `vte` crate, thin wrapper
- `editor/buffer.rs` → `buffer.rs` + `history.rs` (undo/redo)

---

## Next Steps

Upon approval of this plan:
1. Write comprehensive test files (Step 1)
2. Implement and verify (Step 2)
3. Generate README.md (Step 3)
4. Generate DEPENDENCIES.md (Step 4)
5. Verify all files ≤500 lines (Step 5)
