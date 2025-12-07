//! Plugin API traits and types.
//!
//! Defines the interface that plugins implement and the host API they can access.

use std::path::PathBuf;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// Plugin capability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCapability {
    /// Can render content in the status bar.
    StatusWidget,
    /// Can provide command palette commands.
    Commands,
    /// Can decorate tab titles.
    TabDecorator,
    /// Can render content in the editor gutter.
    EditorGutter,
    /// Can overlay content on the terminal.
    TerminalOverlay,
}

impl PluginCapability {
    /// Parse capability from string.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<PluginCapability> {
        match s.to_lowercase().as_str() {
            "status_widget" | "statuswidget" => Some(PluginCapability::StatusWidget),
            "commands" => Some(PluginCapability::Commands),
            "tab_decorator" | "tabdecorator" => Some(PluginCapability::TabDecorator),
            "editor_gutter" | "editorgutter" => Some(PluginCapability::EditorGutter),
            "terminal_overlay" | "terminaloverlay" => Some(PluginCapability::TerminalOverlay),
            _ => None,
        }
    }
}

/// Plugin type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    /// Theme-only plugin.
    Theme,
    /// WASM plugin.
    Wasm,
    /// Native plugin.
    Native,
}

/// Plugin metadata.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Plugin type.
    pub plugin_type: PluginType,
    /// Plugin capabilities.
    pub capabilities: Vec<PluginCapability>,
}

/// Error type for plugin operations.
#[derive(Debug)]
pub enum PluginError {
    /// Initialization failed.
    Init(String),
    /// Command execution failed.
    Command(String),
    /// Render failed.
    Render(String),
    /// Generic error.
    Other(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::Init(e) => write!(f, "Init error: {}", e),
            PluginError::Command(e) => write!(f, "Command error: {}", e),
            PluginError::Render(e) => write!(f, "Render error: {}", e),
            PluginError::Other(e) => write!(f, "Plugin error: {}", e),
        }
    }
}

impl std::error::Error for PluginError {}

/// A single cell of widget output.
#[derive(Debug, Clone)]
pub struct WidgetCell {
    /// Character to display.
    pub ch: char,
    /// Style for the cell.
    pub style: Style,
}

impl WidgetCell {
    /// Creates a new widget cell.
    #[must_use]
    pub fn new(ch: char) -> Self {
        Self {
            ch,
            style: Style::default(),
        }
    }

    /// Sets the foreground color.
    #[must_use]
    pub fn fg(mut self, color: Color) -> Self {
        self.style = self.style.fg(color);
        self
    }

    /// Sets the background color.
    #[must_use]
    pub fn bg(mut self, color: Color) -> Self {
        self.style = self.style.bg(color);
        self
    }
}

/// Plugin interface that both WASM and native plugins implement.
pub trait RattermPlugin: Send + Sync {
    /// Returns plugin metadata.
    fn info(&self) -> PluginInfo;

    /// Called when the plugin is loaded.
    fn on_load(&mut self, host: &dyn PluginHost) -> Result<(), PluginError>;

    /// Called when the plugin is unloaded.
    fn on_unload(&mut self);

    /// Executes a command (if plugin provides commands).
    fn execute_command(&mut self, cmd: &str, args: &[&str]) -> Result<(), PluginError>;

    /// Renders widget content for the given area.
    fn render_widget(&self, area: Rect) -> Option<Vec<WidgetCell>>;

    /// Returns commands provided by this plugin.
    fn commands(&self) -> Vec<PluginCommand> {
        Vec::new()
    }
}

/// A command provided by a plugin.
#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Command identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description.
    pub description: String,
}

/// Host API exposed to plugins.
pub trait PluginHost: Send + Sync {
    /// Gets the current theme name.
    fn theme_name(&self) -> &str;

    /// Gets terminal content as lines.
    fn terminal_lines(&self) -> Vec<String>;

    /// Gets editor content.
    fn editor_content(&self) -> Option<String>;

    /// Gets the current file path.
    fn current_file(&self) -> Option<PathBuf>;

    /// Shows a notification to the user.
    fn notify(&self, message: &str);

    /// Reads a configuration value.
    fn get_config(&self, key: &str) -> Option<String>;

    /// Gets the current working directory.
    fn current_dir(&self) -> PathBuf;
}

/// Stub implementation for testing.
#[derive(Default)]
pub struct StubPluginHost;

impl PluginHost for StubPluginHost {
    fn theme_name(&self) -> &str {
        "dark"
    }

    fn terminal_lines(&self) -> Vec<String> {
        Vec::new()
    }

    fn editor_content(&self) -> Option<String> {
        None
    }

    fn current_file(&self) -> Option<PathBuf> {
        None
    }

    fn notify(&self, _message: &str) {}

    fn get_config(&self, _key: &str) -> Option<String> {
        None
    }

    fn current_dir(&self) -> PathBuf {
        std::env::current_dir().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_capability_from_str() {
        assert_eq!(
            PluginCapability::from_str("status_widget"),
            Some(PluginCapability::StatusWidget)
        );
        assert_eq!(
            PluginCapability::from_str("commands"),
            Some(PluginCapability::Commands)
        );
        assert_eq!(PluginCapability::from_str("invalid"), None);
    }

    #[test]
    fn test_widget_cell() {
        let cell = WidgetCell::new('X').fg(Color::Red).bg(Color::Blue);
        assert_eq!(cell.ch, 'X');
    }

    #[test]
    fn test_stub_host() {
        let host = StubPluginHost;
        assert_eq!(host.theme_name(), "dark");
        assert!(host.terminal_lines().is_empty());
    }
}
