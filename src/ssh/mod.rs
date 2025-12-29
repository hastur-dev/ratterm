//! SSH connection management module.
//!
//! Provides functionality for:
//! - Managing saved SSH hosts and credentials
//! - Scanning local network for SSH-capable hosts
//! - Secure credential storage with multiple storage modes
//!
//! # Storage Modes
//!
//! - **Plaintext**: Credentials stored in plain text (convenient, less secure)
//! - **MasterPassword**: Encrypted with AES-256-GCM using a master password
//! - **ExternalManager**: Future integration with password managers

pub mod host;
pub mod scanner;
pub mod storage;

pub use host::{ConnectionStatus, SSHCredentials, SSHHost, SSHHostList};
pub use scanner::{NetworkInterface, NetworkScanner, ScanResult};
pub use storage::{SSHStorage, StorageError, StorageMode};
