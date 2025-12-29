//! SSH Manager UI components.
//!
//! This module provides the SSH Manager popup widget for managing SSH connections.

mod selector;
mod selector_scan;
mod types;
mod widget;
mod widget_forms;
mod widget_render;

pub use selector::SSHManagerSelector;
pub use types::{
    AddHostField, CredentialField, MAX_DISPLAY_HOSTS, SSHHostDisplay, SSHManagerMode,
    ScanCredentialField,
};
pub use widget::SSHManagerWidget;
