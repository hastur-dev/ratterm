# Dependencies

This document lists all dependencies used by Ratatui Full IDE.

## Core Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `ratatui` | 0.29 | TUI framework for rendering widgets |
| `crossterm` | 0.28 | Terminal backend, event handling |
| `tokio` | 1.41 | Async runtime |

## Terminal Emulation

| Crate | Version | Purpose |
|-------|---------|---------|
| `portable-pty` | 0.8 | Cross-platform PTY spawning |
| `vte` | 0.13 | ANSI/VT100 escape sequence parsing |

## Text Editing

| Crate | Version | Purpose |
|-------|---------|---------|
| `ropey` | 1.6 | Efficient rope data structure for text buffers |
| `unicode-width` | 0.2 | Character width calculation |
| `unicode-segmentation` | 1.12 | Unicode text segmentation |

## Syntax Highlighting (Planned)

| Crate | Version | Purpose |
|-------|---------|---------|
| `tree-sitter` | 0.24 | Incremental parsing for syntax highlighting |
| `tree-sitter-rust` | 0.23 | Rust grammar |
| `tree-sitter-python` | 0.23 | Python grammar |
| `tree-sitter-javascript` | 0.23 | JavaScript grammar |

## LSP (Planned)

| Crate | Version | Purpose |
|-------|---------|---------|
| `lsp-types` | 0.97 | LSP protocol types |

## Serialization

| Crate | Version | Purpose |
|-------|---------|---------|
| `serde` | 1.0 | Serialization framework |
| `serde_json` | 1.0 | JSON serialization for LSP |
| `serde_yaml` | 0.9 | YAML for configuration files |

## Utilities

| Crate | Version | Purpose |
|-------|---------|---------|
| `dirs` | 5.0 | Platform-specific directories |
| `fuzzy-matcher` | 0.3 | Fuzzy string matching for file picker |
| `thiserror` | 2.0 | Error type derivation |
| `tracing` | 0.1 | Structured logging |
| `tracing-subscriber` | 0.3 | Log subscriber with env filter |

## Dev Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `proptest` | 1.5 | Property-based testing |
| `tempfile` | 3.14 | Temporary files for tests |
| `pretty_assertions` | 1.4 | Better assertion diffs |

## Installation

### Rust

Ensure you have Rust 1.75+ installed:

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Or on Windows, download from https://rustup.rs
```

### Build from Source

```bash
# Clone and build
git clone https://github.com/your-username/ratatui-full-ide
cd ratatui-full-ide
cargo build --release
```

### Development Build

```bash
# Install with all features
cargo install --path .

# Or run directly
cargo run
```

## Platform-Specific Notes

### Windows

The `portable-pty` crate uses ConPTY on Windows 10 1809+. Ensure you have:
- Windows 10 version 1809 or later
- Visual Studio Build Tools (for compilation)

### Linux

Requires standard development libraries:

```bash
# Ubuntu/Debian
sudo apt install build-essential

# Fedora
sudo dnf groupinstall "Development Tools"
```

### macOS

Requires Xcode command line tools:

```bash
xcode-select --install
```

## Optional: Language Servers

For LSP features (when implemented), install language servers:

```bash
# Rust
rustup component add rust-analyzer

# Python
pip install pyright

# TypeScript/JavaScript
npm install -g typescript-language-server typescript

# Go
go install golang.org/x/tools/gopls@latest
```
