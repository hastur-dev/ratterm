//! Docker Manager UI module.
//!
//! Provides the Docker manager popup widget and selector state
//! for browsing and connecting to Docker containers and images.

mod fleet_nav;
mod fleet_view;
mod selector;
mod selector_hosts;
mod selector_run;
mod types;
mod widget;
mod widget_create;
mod widget_create_volumes;
mod widget_forms;
mod widget_modes;
mod widget_render;

pub use fleet_nav::{EVENT_PANE_ROWS, FleetAction, FleetViewState, handle_key as handle_fleet_key};
pub use fleet_view::{FleetViewWidget, event_lines, filter_hint, header_lines};
pub use selector::DockerManagerSelector;
pub use types::{
    CreationField, DockerHostDisplay, DockerItemDisplay, DockerListSection, DockerManagerMode,
    HostCredentialField, MAX_DISPLAY_HOSTS, RunOptionsField,
};
pub use widget::DockerManagerWidget;
