//! Add-on Manager UI module.
//!
//! Provides widgets and state management for the add-on manager popup.
//!
//! # Components
//!
//! - `AddonManagerSelector`: State management for the popup
//! - `AddonManagerWidget`: Main rendering widget
//! - `types`: Enums and display types

mod selector;
mod types;
mod widget;
mod widget_render;

pub use selector::AddonManagerSelector;
pub use types::{AddonDisplay, AddonListSection, AddonManagerMode, MAX_DISPLAY_ADDONS};
pub use widget::AddonManagerWidget;
