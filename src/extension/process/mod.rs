//! Extension process management.
//!
//! Handles spawning, monitoring, and lifecycle management of API extensions.

pub mod manager;

pub use manager::{ApiExtensionManager, ExtensionProcess, ProcessStatus};
