# Ratterm Extension System

Ratterm supports a powerful extension system that allows you to customize and extend its functionality. Extensions can provide:

- **Custom Themes** - TOML-based color schemes
- **WASM Plugins** - Sandboxed, portable plugins
- **Native Plugins** - Full-access compiled plugins (.dll/.so/.dylib)

## Quick Start

### Installing Extensions

Install extensions from GitHub repositories:

```bash
# Install from GitHub
rat ext install user/repo

# Install a specific version
rat ext install user/repo@v1.0.0

# Install from a specific branch
rat ext install user/repo#branch
```

### Managing Extensions

```bash
# List installed extensions
rat ext list

# Update all extensions
rat ext update

# Update a specific extension
rat ext update extension-name

# Remove an extension
rat ext remove extension-name

# Get help
rat ext help
```

### Command Palette

You can also access extension commands through the command palette (`Ctrl+Shift+P`):

- **Extension: List Installed** - Show installed extensions
- **Extension: Install from GitHub** - Shows CLI command to install
- **Extension: Update All** - Shows CLI command to update
- **Extension: Remove Extension** - Shows CLI command to remove

## Directory Structure

Extensions are stored in your Ratterm data directory:

```
~/.ratterm/
├── extensions/           # Installed extensions
│   ├── my-theme/
│   │   ├── extension.toml
│   │   └── theme.toml
│   └── git-widget/
│       ├── extension.toml
│       └── plugin.wasm
├── themes/               # User's custom themes (not extensions)
│   └── my-custom.toml
└── cache/
    └── downloads/        # Temporary download cache
```

## Creating Extensions

### Extension Manifest

Every extension requires an `extension.toml` manifest file:

```toml
[extension]
name = "my-extension"
version = "1.0.0"
description = "A custom extension for Ratterm"
author = "Your Name"
license = "MIT"
homepage = "https://github.com/user/my-extension"
type = "theme"  # theme | widget | command | native

[compatibility]
ratterm = ">=0.1.0"  # Minimum Ratterm version
```

### Theme Extensions

Theme extensions provide custom color schemes:

```toml
# extension.toml
[extension]
name = "my-cool-theme"
version = "1.0.0"
type = "theme"

[theme]
file = "theme.toml"
```

```toml
# theme.toml
[theme]
name = "My Cool Theme"
description = "A beautiful custom theme"
base = "dark"  # Optional: inherit from built-in theme

[palette]
# Define reusable colors
bg = "#1a1b26"
fg = "#c0caf5"
accent = "#7aa2f7"
comment = "#565f89"

[colors]
# Terminal colors
terminal.background = "$bg"
terminal.foreground = "$fg"
terminal.cursor = "$accent"
terminal.selection = "#283457"

# Editor colors
editor.background = "$bg"
editor.foreground = "$fg"
editor.cursor = "$accent"
editor.gutter = "#3b4261"
editor.lineNumber = "$comment"

# Status bar
statusbar.background = "#16161e"
statusbar.foreground = "$fg"
statusbar.mode = "$accent"

# Tabs
tabs.active.background = "$bg"
tabs.active.foreground = "$fg"
tabs.inactive.background = "#16161e"
tabs.inactive.foreground = "$comment"

# File browser
filebrowser.directory = "$accent"
filebrowser.file = "$fg"
filebrowser.selected = "$accent"

# Popup dialogs
popup.background = "$bg"
popup.border = "$accent"
popup.title = "$accent"
```

### WASM Plugins

WASM plugins run in a sandboxed environment:

```toml
# extension.toml
[extension]
name = "git-status"
version = "0.1.0"
type = "widget"

[wasm]
file = "plugin.wasm"
capabilities = ["status_widget", "commands"]
```

#### WASM Capabilities

- `status_widget` - Render content in the status bar
- `commands` - Provide command palette commands
- `tab_decorator` - Decorate tab titles
- `editor_gutter` - Render in the editor gutter
- `terminal_overlay` - Overlay content on the terminal

#### Building WASM Plugins

WASM plugins can be built from Rust:

```bash
cargo build --target wasm32-unknown-unknown --release
```

The plugin must export these functions:
- `ratterm_plugin_init() -> *mut RattermPlugin`
- `ratterm_plugin_info() -> PluginInfo`

### Native Plugins

Native plugins have full system access and require user confirmation:

```toml
# extension.toml
[extension]
name = "native-lsp"
version = "1.0.0"
type = "native"

[native]
windows = "plugin.dll"
linux = "plugin.so"
macos = "plugin.dylib"
trusted = false  # Requires user confirmation
```

#### Security Warning

Native plugins have **full system access**. They can:
- Read and write any files
- Access the network
- Execute system commands
- Access all memory

**Only install native plugins from sources you trust completely.**

## Plugin API

### RattermPlugin Trait

All plugins implement the `RattermPlugin` trait:

```rust
pub trait RattermPlugin: Send + Sync {
    /// Returns plugin metadata.
    fn info(&self) -> PluginInfo;

    /// Called when the plugin is loaded.
    fn on_load(&mut self, host: &dyn PluginHost) -> Result<(), PluginError>;

    /// Called when the plugin is unloaded.
    fn on_unload(&mut self);

    /// Execute a command provided by this plugin.
    fn execute_command(&mut self, cmd: &str, args: &[&str]) -> Result<(), PluginError>;

    /// Render widget content (if this plugin provides widgets).
    fn render_widget(&self, area: Rect) -> Option<Vec<WidgetCell>>;
}
```

### PluginHost API

Plugins can access Ratterm functionality through the host API:

```rust
pub trait PluginHost: Send + Sync {
    /// Get the current theme name.
    fn theme_name(&self) -> &str;

    /// Get terminal content (read-only).
    fn terminal_lines(&self) -> Vec<String>;

    /// Get editor content (read-only).
    fn editor_content(&self) -> Option<String>;

    /// Get the current file path.
    fn current_file(&self) -> Option<PathBuf>;

    /// Show a notification to the user.
    fn notify(&self, message: &str);

    /// Read a configuration value.
    fn get_config(&self, key: &str) -> Option<String>;

    /// Get the current working directory.
    fn current_dir(&self) -> PathBuf;
}
```

## Publishing Extensions

### GitHub Releases

To make your extension installable, create a GitHub release:

1. Tag your repository with a version (e.g., `v1.0.0`)
2. Create a release with the source code
3. Users can then install with: `rat ext install user/repo`

### Release Structure

The release archive should contain:
- `extension.toml` at the root
- All required files (theme.toml, plugin.wasm, etc.)

## Custom Themes (Non-Extension)

You can also create custom themes without packaging them as extensions:

1. Create a `.toml` file in `~/.ratterm/themes/`
2. Follow the theme.toml format above
3. The theme will appear in the theme selector

Example: `~/.ratterm/themes/my-theme.toml`

## Troubleshooting

### Extension not loading

1. Check the manifest is valid TOML
2. Verify all required fields are present
3. Check the ratterm logs for errors

### WASM plugin crashes

1. Ensure the plugin was built for `wasm32-unknown-unknown`
2. Check memory limits aren't exceeded
3. Verify exported functions match the expected signatures

### Native plugin security warning

Native plugins require explicit user confirmation. To skip this (not recommended), you can set `trusted = true` in the manifest, but this should only be done for plugins you've personally verified.

## Future Features

The extension system is designed to support:

- Central extension registry
- Extension signing and verification
- Automatic updates
- Extension dependencies
- More plugin capabilities (LSP integration, syntax highlighting, etc.)
