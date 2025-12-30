//! Docker container and image management module.
//!
//! Provides functionality for:
//! - Discovering running Docker containers and available images
//! - Managing quick-connect hotkeys for Docker items
//! - Connecting to containers via `docker exec`
//! - Running images as new containers
//!
//! # Features
//!
//! - **Discovery**: Scans local system for containers and images via Docker CLI
//! - **Quick Connect**: Assign Ctrl+Alt+1-9 hotkeys to frequently used items
//! - **Container Stats**: View real-time container resource usage
//! - **Container Logs**: Stream container logs in split terminal pane

pub mod container;
pub mod discovery;
pub mod storage;

pub use container::{
    DockerContainer, DockerImage, DockerItemList, DockerItemType, DockerQuickConnectItem,
    DockerRunOptions, DockerStatus,
};
pub use discovery::{DockerAvailability, DockerDiscovery, DockerDiscoveryResult};
pub use storage::{DockerStorage, DockerStorageError};
