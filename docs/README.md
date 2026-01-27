# Ratterm Documentation

Welcome to the Ratterm documentation. This folder contains detailed guides for all aspects of the application.

## Quick Links

### For Users

| Document | Description |
|----------|-------------|
| [hotkeys.md](hotkeys.md) | Complete keyboard shortcut reference |
| [ratrc_docs.md](ratrc_docs.md) | Configuration file (`.ratrc`) reference |
| [command_palette.md](command_palette.md) | All command palette commands |

### For Developers

| Document | Description |
|----------|-------------|
| [architecture.md](architecture.md) | System architecture overview |
| [extension_system.md](extension_system.md) | Extension REST API reference |
| [extensions.md](extensions.md) | How to create extensions |
| [testing.md](testing.md) | Running tests and CI |

---

## Getting Started

### Basic Usage

1. **Launch Ratterm**: Run `rat` or `rat <file>` to open with a file
2. **Terminal-First Mode**: By default, only the terminal is visible
3. **Open Editor**: Type `open` in terminal or press `Ctrl+I`
4. **Command Palette**: Press `Ctrl+Shift+P` (or `F1` on Windows 11)

### Essential Hotkeys

| Action | Hotkey |
|--------|--------|
| Open Command Palette | `Ctrl+Shift+P` / `F1` |
| Toggle IDE | `Ctrl+I` |
| New Terminal Tab | `Ctrl+T` |
| Open File Browser | `Ctrl+O` |
| Switch Editor Mode | `Ctrl+Shift+Tab` |
| Quit | `Ctrl+Q` |

### Configuration

Create `~/.ratrc` to customize Ratterm:

```
# Set editor mode
mode = vim

# Default shell
shell = bash

# Theme
theme = dracula

# Always show IDE pane
ide-always = false
```

See [ratrc_docs.md](ratrc_docs.md) for all options.

---

## Document Overview

### hotkeys.md
Complete reference of all keyboard shortcuts organized by context:
- Global hotkeys
- Terminal hotkeys
- Editor hotkeys (by mode: Vim, Emacs, Default)
- File browser hotkeys
- SSH Manager hotkeys
- Docker Manager hotkeys
- Mouse support

### ratrc_docs.md
Configuration file reference covering:
- Shell configuration
- IDE settings
- Keybinding modes
- Theme customization (presets and custom colors)
- Custom keybindings
- SSH Manager settings
- Docker Manager settings
- Logging configuration
- Custom addon commands

### command_palette.md
All commands accessible via the command palette:
- File operations
- Edit operations
- Search operations
- View/layout commands
- Terminal commands
- SSH commands
- Docker commands
- Theme commands
- Extension commands

### architecture.md
Technical overview for contributors:
- Module structure
- Core components
- Data flow diagrams
- Extension architecture
- Configuration flow
- PTY architecture
- Theme system
- Completion system

### extension_system.md
REST API reference for extension developers:
- API endpoints
- Authentication
- Terminal operations
- Editor operations
- File system operations
- Layout operations
- Event streaming (SSE)
- Example extensions

### extensions.md
Guide for creating extensions:
- Extension manifest format
- Theme extensions
- WASM plugins
- Native plugins
- Plugin API
- Publishing extensions

### testing.md
Testing guide for contributors:
- Running tests locally
- Docker-based CI testing
- Test categories
- Writing tests
- Code coverage

---

## Contributing

When adding new features, please update the relevant documentation:

1. **New hotkeys**: Add to `hotkeys.md`
2. **New config options**: Add to `ratrc_docs.md`
3. **New commands**: Add to `command_palette.md`
4. **Architecture changes**: Update `architecture.md`
5. **API changes**: Update `extension_system.md`

Documentation should include:
- What the feature does
- How to use it (steps or examples)
- Any configuration options
- Platform-specific behavior (if applicable)
