//! Add-ons system for Ratterm.
//!
//! This module provides functionality for discovering, installing, and
//! managing technology add-ons from a GitHub repository.
//!
//! # Architecture
//!
//! - **types**: Core data structures (`Addon`, `InstalledAddon`, `AddonConfig`)
//! - **github**: GitHub API client for fetching addon listings
//! - **fetcher**: Background fetcher for non-blocking HTTP operations
//! - **installer**: Background script execution using `BackgroundManager`
//! - **storage**: Persistence to `.ratrc` configuration file
//!
//! # Usage
//!
//! ```ignore
//! use ratterm::addons::{AddonConfig, BackgroundFetcher};
//!
//! let config = AddonConfig::new();
//! let fetcher = BackgroundFetcher::new(&config.repository, &config.branch);
//! fetcher.request_addon_list(false);
//! // ... poll for results in event loop
//! ```

mod fetcher;
mod github;
mod installer;
mod storage;
mod types;

pub use fetcher::{BackgroundFetcher, FetchResult, FetcherStatus};
pub use github::{AddonGitHubClient, GitHubEntry};
pub use installer::{AddonInstaller, InstallPhase, InstallProgress};
pub use storage::AddonStorage;
pub use types::{
    Addon, AddonConfig, AddonError, AddonMetadata, InstalledAddon, ScriptType,
    MAX_INSTALLED_ADDONS,
};
