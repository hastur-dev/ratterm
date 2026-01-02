//! Docker container and image management module.
//!
//! Provides functionality for:
//! - Discovering running Docker containers and available images
//! - Managing quick-connect hotkeys for Docker items
//! - Connecting to containers via `docker exec`
//! - Running images as new containers
//! - Remote Docker management via SSH
//!
//! # Features
//!
//! - **Discovery**: Scans local and remote systems for containers and images via Docker CLI
//! - **Quick Connect**: Assign Ctrl+Alt+1-9 hotkeys to frequently used items (per-host)
//! - **Container Stats**: View real-time container resource usage
//! - **Container Logs**: Stream container logs in split terminal pane
//! - **Remote Management**: Manage Docker on remote hosts via SSH
//! - **API Layer**: Programmatic access for testing and extensions

pub mod api;
pub mod container;
pub mod discovery;
pub mod storage;

pub use api::{DockerApi, DockerHostManager};
pub use container::{
    ContainerCreationState, DockerContainer, DockerHost, DockerImage, DockerItemList,
    DockerItemType, DockerQuickConnectItem, DockerRunOptions, DockerSearchResult, DockerStatus,
    MAX_SEARCH_RESULTS, QuickConnectSlots, VolumeMountConfig,
};
pub use discovery::{DockerAvailability, DockerDiscovery, DockerDiscoveryResult};
pub use storage::{DockerStorage, DockerStorageError};
