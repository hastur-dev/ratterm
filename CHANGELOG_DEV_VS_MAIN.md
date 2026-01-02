# Changelog: dev vs main

This document compares the `dev` branch against `main`, detailing all features added, fixes applied, and breaking changes.

**Comparison Date:** 2026-01-02
**Commits ahead of main:** 14
**Files changed:** 66
**Lines added:** +47,347
**Lines removed:** -834

---

## Summary of Major Changes

| Category | Description |
|----------|-------------|
| **New Feature** | Docker Manager - Full container/image management |
| **New Feature** | Inline Autocomplete with LSP support |
| **New Feature** | Windows 11 Keybinding Notification |
| **Fix** | Windows 11 hotkey conflicts (Ctrl+Shift+P) |
| **Removed** | VSCode editor mode and extension system |
| **Fix** | Character width calculation for `\r` |

---

## New Features

### 1. Docker Manager (PR #30)

A comprehensive Docker container and image management system integrated into Ratterm.

**Hotkey:** `Ctrl+Shift+D` to open Docker Manager

#### Capabilities

- **Container Discovery**: Scans local and remote systems for running/stopped containers
- **Image Management**: View and run available Docker images
- **Quick Connect**: Assign `Ctrl+Alt+1-9` hotkeys to frequently used containers (per-host)
- **Container Creation**: Create new containers with customizable options:
  - Port mappings
  - Volume mounts
  - Environment variables
  - Network settings
  - Resource limits
- **Remote Docker**: Manage Docker on remote hosts via SSH (uses SSH Manager credentials)
- **Container Exec**: Open interactive terminal sessions inside containers

#### New Files Added

```
src/docker/
├── mod.rs           # Module exports
├── api.rs           # Docker API layer (542 lines)
├── container.rs     # Container/Image data structures (1,205 lines)
├── discovery.rs     # Container/Image discovery (1,782 lines)
└── storage.rs       # Persistent storage for settings (267 lines)

src/app/
├── docker_connect.rs     # Docker connection logic (308 lines)
├── docker_ops.rs         # Docker operations (491 lines)
├── input_docker.rs       # Docker input handling (874 lines)
└── input_docker_create.rs # Container creation input (475 lines)

src/ui/docker_manager/
├── mod.rs              # Module exports
├── selector.rs         # Selection state management (1,293 lines)
├── types.rs            # UI type definitions (442 lines)
├── widget.rs           # Main widget (543 lines)
├── widget_create.rs    # Creation form widget (638 lines)
├── widget_forms.rs     # Form components (465 lines)
└── widget_render.rs    # Rendering helpers (185 lines)
```

#### Docker Manager Hotkeys

| Hotkey | Action |
|--------|--------|
| `Ctrl+Shift+D` | Open Docker Manager |
| `Esc` | Close Docker Manager |
| `Up` / `k` | Previous item |
| `Down` / `j` | Next item |
| `Tab` | Switch section (Running → Stopped → Images) |
| `Enter` | Exec into container / Run image |
| `Shift+H` | Change host (local/remote) |
| `Shift+R` | Jump to Running Containers |
| `Shift+S` | Jump to Stopped Containers |
| `Shift+I` | Jump to Images |
| `Ctrl+Alt+1-9` | Quick connect to assigned item |

#### Fixes Applied to Docker Manager

1. **Fixed `docker.exe` → `docker` for remote hosts** (discovery.rs:252-265)
   - Was incorrectly using Windows command on remote Linux hosts
   - Added proper quoting for arguments containing `{{json .}}`

2. **Added direct plink execution** (discovery.rs:279-383)
   - Bypasses cmd.exe quoting issues
   - Auto-accepts SSH host key prompts
   - Proper password authentication support

3. **Fixed bash brace expansion issue**
   - `{{json .}}` format template was being interpreted by bash
   - Arguments with `{`, `}`, or spaces now wrapped in single quotes

---

### 2. Inline Autocomplete System (PR #29)

A full-featured code completion system with ghost text suggestions.

**Hotkey:** `Ctrl+Space` to accept suggestion

#### Features

- **Ghost Text Display**: Grayed-out italic text appears at cursor position
- **300ms Debounce**: Completions trigger after typing pauses
- **LSP-First**: Uses Language Server Protocol when available
- **Keyword Fallback**: Buffer words + language keywords when no LSP
- **Cross-Platform**: Windows, Mac, and Linux support
- **Background Async**: Non-blocking via Tokio channels

#### Supported Languages (via LSP)

- Rust (rust-analyzer)
- Python (pylsp, pyright)
- JavaScript/TypeScript (typescript-language-server)
- Java (jdtls)
- C# (omnisharp)
- PHP (intelephense)
- SQL (sql-language-server)
- HTML/CSS (vscode-html-languageserver)

#### New Files Added

```
src/completion/
├── mod.rs        # Main completion module (478 lines)
├── cache.rs      # Completion caching (372 lines)
├── debounce.rs   # Input debouncing (229 lines)
├── keyword.rs    # Keyword provider (1,461 lines)
├── provider.rs   # Provider trait/types (421 lines)
└── lsp/
    ├── mod.rs      # LSP module exports
    ├── client.rs   # LSP client (651 lines)
    ├── config.rs   # LSP configuration (376 lines)
    └── manager.rs  # LSP lifecycle management (293 lines)

src/ui/
└── ghost_text.rs  # Ghost text rendering (273 lines)

tests/
└── completion_tests.rs  # Integration tests (468 lines)
```

#### Autocomplete Hotkeys

| Hotkey | Action |
|--------|--------|
| `Ctrl+Space` | Accept current suggestion |
| `Esc` | Dismiss suggestion |

---

### 3. Windows 11 Keybinding Notification (PR #27)

Windows 11 introduced a system-wide `Ctrl+Shift+P` shortcut that conflicts with the Command Palette.

#### Changes

- **New Hotkey**: `F1` now opens Command Palette on Windows 11
- **Notification Popup**: First-time users on Windows 11 see a notification explaining the change
- **Platform Detection**: Added `src/config/platform.rs` for Windows 11 detection

#### New Files Added

```
src/config/platform.rs                    # Windows 11 detection (106 lines)
src/ui/popup/keybinding_notification.rs   # Notification widget (149 lines)
```

---

## Bug Fixes

### Windows 11 Hotkey Issue (PR #27)

- **Problem**: Windows 11 uses `Ctrl+Shift+P` for system command palette in terminals
- **Solution**: Changed default Command Palette hotkey to `F1` on Windows 11
- **Commit**: `5363b8f`

### Character Width Calculation (PR #29)

- **Problem**: Carriage return (`\r`) characters were causing rendering issues
- **Solution**: Treat `\r` as width 0 (same as `\n`) and skip during rendering
- **Commit**: `794ad79`

---

## Removed Features

### VSCode Editor Mode (PR #28)

The VSCode keybinding mode and VSCode extension integration have been removed.

**Reason**: Simplifying the codebase by focusing on Vim, Emacs, and Default modes.

#### Files Removed

```
src/config/vscode.rs  # VSCode keybinding configuration (-498 lines)
```

#### Changes

- `Ctrl+Shift+Tab` mode cycling now only includes: Vim → Emacs → Default
- VSCode-style keybindings removed from documentation
- Extension system no longer attempts VSCode compatibility

---

## API/Protocol Additions

New API handlers and protocol messages for Docker integration:

```
src/api/handler.rs   # +250 lines - Docker API handlers
src/api/protocol.rs  # +131 lines - Docker protocol messages
```

---

## Documentation Updates

### docs/hotkeys.md

- Added Docker Manager hotkeys section (+122 lines)
- Added Autocomplete section
- Updated Command Palette section for Windows 11
- Removed VSCode Mode section (-46 lines)
- Updated mode switcher description

### docs/ratrc_docs.md

- Added Docker configuration options (+37 lines)

---

## Configuration Changes

### New `.ratrc` Options

```ini
# Docker Manager settings
docker_default_host = local    # Default: local, or SSH host ID

# Windows 11 notification (auto-managed)
win11_notification_shown = true
```

---

## Breaking Changes

1. **VSCode Mode Removed**: Users who were using VSCode keybindings need to switch to Vim, Emacs, or Default mode

2. **Command Palette Hotkey (Windows 11)**:
   - Old: `Ctrl+Shift+P`
   - New: `F1`
   - Note: `Ctrl+P` and `Ctrl+Shift+P` still work on non-Windows 11 systems

---

## Commit History

| Commit | Description |
|--------|-------------|
| `b034a4b` | Merge PR #30 - Docker Manager |
| `57ea3b9` | Merge origin/dev - conflict resolution |
| `9c7e7b8` | Merge dev - initial docker + win11 merge |
| `585a63c` | Container execution via SSH terminals |
| `d62c3cb` | Docker remote host fixes (plink, quoting) |
| `a09def7` | Merge PR #29 - Editor upgrades |
| `794ad79` | Character width fix for `\r` |
| `632e7b5` | Completion system fixes |
| `27097ae` | Inline ghost-text suggestions |
| `58d54b5` | Merge PR #28 - Remove VSCode |
| `e7bfa37` | Removed VSCode stuff |
| `237a313` | Merge PR #27 - Windows 11 update |
| `5363b8f` | Windows 11 hotkey fix |
| `dc6bdf9` | First Docker Manager commit |

---

## Migration Guide

### For Users on Windows 11

1. The Command Palette is now opened with `F1` instead of `Ctrl+Shift+P`
2. You will see a one-time notification about this change
3. `Ctrl+P` still works as an alternative

### For Users Using VSCode Mode

1. VSCode mode has been removed
2. Switch to one of the remaining modes:
   - **Vim** (default): Modal editing with hjkl navigation
   - **Emacs**: Ctrl-key based navigation and editing
   - **Default**: Arrow key navigation, standard editing
3. Use `Ctrl+Shift+Tab` to cycle through available modes

### For Extension Developers

1. The VSCode extension compatibility layer has been removed
2. Use the native Ratterm extension API instead
3. Docker operations are now available via the API

---

## Statistics

| Metric | Value |
|--------|-------|
| New Rust files | 31 |
| Modified files | 35 |
| Total lines added | 47,347 |
| Total lines removed | 834 |
| Net change | +46,513 lines |
| New modules | 2 (docker, completion) |
| New UI components | 9 |
| New tests | 468 lines |
