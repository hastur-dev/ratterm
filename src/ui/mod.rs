//! User interface module.
//!
//! Provides widgets and layout for the TUI.

pub mod editor_tabs;
pub mod editor_widget;
pub mod file_picker;
pub mod layout;
pub mod popup;
pub mod statusbar;
pub mod terminal_tabs;
pub mod terminal_widget;

pub use layout::{FocusedPane, LayoutAreas, SplitLayout};
pub use popup::{Popup, PopupKind, PopupWidget};
