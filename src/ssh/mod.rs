//! SSH connection management module.
//!
//! Provides functionality for:
//! - Managing saved SSH hosts and credentials
//! - Scanning local network for SSH-capable hosts
//! - Secure credential storage with multiple storage modes
//! - Health monitoring with metrics collection
//!
//! # Storage Modes
//!
//! - **Plaintext**: Credentials stored in plain text (convenient, less secure)
//! - **MasterPassword**: Encrypted with AES-256-GCM using a master password
//! - **ExternalManager**: Future integration with password managers

pub mod collector;
pub mod host;
pub mod metrics;
pub mod scanner;
pub mod storage;

pub use collector::{build_collection_info, HostCollectionInfo, MetricsCollector};
pub use host::{ConnectionStatus, JumpHostInfo, SSHCredentials, SSHHost, SSHHostList};
pub use metrics::{DeviceMetrics, GpuMetrics, GpuType, MetricStatus};
pub use scanner::{NetworkInterface, NetworkScanner, ScanResult};
pub use storage::{SSHStorage, StorageError, StorageMode};
