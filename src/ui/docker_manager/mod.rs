//! Docker Manager UI module.
//!
//! Provides the Docker manager popup widget and selector state
//! for browsing and connecting to Docker containers and images.

mod selector;
mod types;
mod widget;
mod widget_forms;
mod widget_render;

pub use selector::DockerManagerSelector;
pub use types::{DockerItemDisplay, DockerListSection, DockerManagerMode, RunOptionsField};
pub use widget::DockerManagerWidget;
