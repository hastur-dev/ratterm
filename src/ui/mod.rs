//! User interface module.
//!
//! Provides widgets and layout for the TUI.

pub mod debug_panel;
pub mod docker_manager;
pub mod editor_tabs;
pub mod editor_widget;
pub mod file_picker;
pub mod ghost_text;
pub mod git_dashboard;
pub mod health_dashboard;
pub mod hotkey_overlay;
pub mod key_hint_bar;
pub mod layout;
pub mod lsp_actions;
pub mod lsp_diagnostics;
pub mod lsp_hover;
pub mod lsp_references;
pub mod lsp_signature;
pub mod lsp_symbols;
pub mod manager_footer;
pub mod popup;
pub mod ssh_manager;
pub mod statusbar;
pub mod terminal_tabs;
pub mod terminal_widget;
pub mod window_position;

pub use docker_manager::{
    DockerListSection, DockerManagerMode, DockerManagerSelector, DockerManagerWidget,
};
pub use health_dashboard::{DashboardMode, HealthDashboard, HealthDashboardWidget};
pub use key_hint_bar::{KeyHint, KeyHintBar, KeyHintStyle};
pub use layout::{FocusedPane, LayoutAreas, SplitLayout};
pub use manager_footer::ManagerFooter;
pub use popup::{
    Command, CommandPalette, ModeSwitcher, ModeSwitcherWidget, Popup, PopupKind, PopupWidget,
    ShellInstallPrompt, ShellInstallPromptWidget, ShellSelector, ShellSelectorWidget,
};
pub use ssh_manager::{SSHManagerMode, SSHManagerSelector, SSHManagerWidget};
