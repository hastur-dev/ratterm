//! User interface module.
//!
//! Provides widgets and layout for the TUI.

pub mod editor_tabs;
pub mod editor_widget;
pub mod file_picker;
pub mod layout;
pub mod popup;
pub mod ssh_manager;
pub mod statusbar;
pub mod terminal_tabs;
pub mod terminal_widget;

pub use layout::{FocusedPane, LayoutAreas, SplitLayout};
pub use popup::{
    Command, CommandPalette, ModeSwitcher, ModeSwitcherWidget, Popup, PopupKind, PopupWidget,
    ShellInstallPrompt, ShellInstallPromptWidget, ShellSelector, ShellSelectorWidget,
};
pub use ssh_manager::{SSHManagerMode, SSHManagerSelector, SSHManagerWidget};
