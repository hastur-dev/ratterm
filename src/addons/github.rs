//! GitHub API client for addon discovery.
//!
//! Fetches addon listings from a GitHub repository.

use super::types::{current_os_directory, Addon, AddonError, AddonMetadata, ScriptType};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Cache expiration time (5 minutes).
const CACHE_EXPIRATION_SECS: u64 = 300;

/// Maximum number of addons to fetch.
const MAX_ADDONS: usize = 100;

/// Maximum number of files to check per addon.
const MAX_FILES_PER_ADDON: usize = 20;

/// Default directory containing addon scripts.
const SCRIPTS_DIRECTORY: &str = "scripts";

/// GitHub API response for directory contents.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubEntry {
    /// Entry name (file or directory name).
    pub name: String,
    /// Entry path relative to repo root.
    pub path: String,
    /// Entry type ("file" or "dir").
    #[serde(rename = "type")]
    pub entry_type: String,
    /// File size in bytes (for files only).
    #[serde(default)]
    pub size: u64,
    /// Download URL (for files only).
    #[serde(default)]
    pub download_url: Option<String>,
}

impl GitHubEntry {
    /// Returns true if this entry is a directory.
    #[must_use]
    pub fn is_directory(&self) -> bool {
        self.entry_type == "dir"
    }

    /// Returns true if this entry is a file.
    #[must_use]
    pub fn is_file(&self) -> bool {
        self.entry_type == "file"
    }
}

/// Cached addon list with expiration.
struct CachedAddons {
    /// List of addons.
    addons: Vec<Addon>,
    /// When the cache was last updated.
    cached_at: Instant,
}

/// GitHub API client for addon discovery.
pub struct AddonGitHubClient {
    /// HTTP client.
    client: reqwest::blocking::Client,
    /// Repository in "owner/repo" format.
    repository: String,
    /// Branch to fetch from.
    branch: String,
    /// Cached addon list.
    cache: Arc<RwLock<Option<CachedAddons>>>,
}

impl AddonGitHubClient {
    /// Creates a new GitHub client.
    #[must_use]
    pub fn new(repository: &str, branch: &str) -> Self {
        assert!(!repository.is_empty(), "Repository must not be empty");
        assert!(
            repository.contains('/'),
            "Repository must be in owner/repo format"
        );
        assert!(!branch.is_empty(), "Branch must not be empty");

        let client = reqwest::blocking::Client::builder()
            .user_agent("ratterm-addon-manager")
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            client,
            repository: repository.to_string(),
            branch: branch.to_string(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Updates the repository and branch.
    pub fn set_repository(&mut self, repository: &str, branch: &str) {
        assert!(!repository.is_empty(), "Repository must not be empty");
        assert!(
            repository.contains('/'),
            "Repository must be in owner/repo format"
        );
        assert!(!branch.is_empty(), "Branch must not be empty");

        self.repository = repository.to_string();
        self.branch = branch.to_string();

        // Clear cache when repository changes
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }
    }

    /// Fetches the list of available addons.
    ///
    /// Uses cached results if available and not expired.
    pub fn fetch_addons(&self, force_refresh: bool) -> Result<Vec<Addon>, AddonError> {
        info!("[ADDON] fetch_addons called, force_refresh={}", force_refresh);

        // Check cache first
        if !force_refresh {
            if let Ok(cache) = self.cache.read() {
                if let Some(ref cached) = *cache {
                    let elapsed = cached.cached_at.elapsed();
                    if elapsed < Duration::from_secs(CACHE_EXPIRATION_SECS) {
                        info!("[ADDON] Returning {} cached addons", cached.addons.len());
                        return Ok(cached.addons.clone());
                    }
                }
            }
        }

        // Fetch from GitHub
        info!("[ADDON] Fetching addons from GitHub API (BLOCKING CALL)...");
        let start = Instant::now();
        let addons = self.fetch_addons_from_api()?;
        info!("[ADDON] GitHub API fetch completed in {:?}, found {} addons", start.elapsed(), addons.len());

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(CachedAddons {
                addons: addons.clone(),
                cached_at: Instant::now(),
            });
        }

        Ok(addons)
    }

    /// Fetches addons from the GitHub API.
    ///
    /// Uses the new directory structure: scripts/{os}/{technology}/
    fn fetch_addons_from_api(&self) -> Result<Vec<Addon>, AddonError> {
        let os_dir = current_os_directory();
        let os_path = format!("{}/{}", SCRIPTS_DIRECTORY, os_dir);

        info!("[ADDON] fetch_addons_from_api: looking in '{}' directory", os_path);

        // Get contents of the OS-specific scripts directory
        let entries = self.fetch_contents(&os_path)?;
        debug!("[ADDON] Found {} entries in {} directory", entries.len(), os_dir);

        // Filter for directories (each directory is a technology/addon)
        let directories: Vec<_> = entries
            .into_iter()
            .filter(|e| e.is_directory())
            .take(MAX_ADDONS)
            .collect();

        info!("[ADDON] Found {} addon directories", directories.len());

        let mut addons = Vec::with_capacity(directories.len());

        // Check each directory for required scripts
        for dir in &directories {
            debug!("[ADDON] Checking addon directory: {}", dir.name);
            let addon_path = format!("{}/{}", os_path, dir.name);
            match self.check_addon_directory(&addon_path, &dir.name) {
                Ok(addon) => {
                    info!("[ADDON] Found valid addon: {} (install={})",
                        addon.id, addon.has_install);
                    addons.push(addon);
                }
                Err(e) => {
                    warn!("[ADDON] Skipping invalid addon '{}': {}", dir.name, e);
                    continue;
                }
            }
        }

        info!("[ADDON] Total valid addons: {}", addons.len());
        Ok(addons)
    }

    /// Checks an addon directory for required scripts.
    ///
    /// # Arguments
    /// * `addon_path` - Full path to addon in repo (e.g., "scripts/vim")
    /// * `addon_id` - Just the addon name (e.g., "vim")
    fn check_addon_directory(&self, addon_path: &str, addon_id: &str) -> Result<Addon, AddonError> {
        assert!(!addon_path.is_empty(), "Addon path must not be empty");
        assert!(!addon_id.is_empty(), "Addon ID must not be empty");

        debug!("[ADDON] check_addon_directory: path='{}', id='{}'", addon_path, addon_id);

        let entries = self.fetch_contents(addon_path)?;
        debug!("[ADDON] Found {} files in addon directory", entries.len());

        let file_names: HashMap<&str, &GitHubEntry> =
            entries.iter().map(|e| (e.name.as_str(), e)).collect();

        // Check for platform-specific install script
        let install_filename = ScriptType::Install.filename();

        debug!("[ADDON] Looking for install='{}'", install_filename);

        let has_install = file_names.contains_key(install_filename);

        debug!("[ADDON] has_install={}", has_install);

        // Get description from README if present
        let description = self.fetch_readme_description(addon_path, &file_names);

        // Fetch and parse config.yaml if present
        let metadata = self.fetch_addon_metadata(&file_names);

        let mut addon = Addon::new(addon_id.to_string())
            .with_description(description)
            .with_install(has_install);

        if let Some(meta) = metadata {
            addon = addon.with_metadata(meta);
        }

        Ok(addon)
    }

    /// Fetches and parses config.yaml for an addon.
    fn fetch_addon_metadata(&self, files: &HashMap<&str, &GitHubEntry>) -> Option<AddonMetadata> {
        // Look for config.yaml or config.yml
        let config_names = ["config.yaml", "config.yml"];

        for name in config_names {
            if let Some(entry) = files.get(name) {
                if let Some(ref url) = entry.download_url {
                    match self.fetch_raw_content(url) {
                        Ok(content) => {
                            match serde_yaml::from_str::<AddonMetadata>(&content) {
                                Ok(metadata) => {
                                    debug!("[ADDON] Parsed config.yaml: {:?}", metadata);
                                    return Some(metadata);
                                }
                                Err(e) => {
                                    warn!("[ADDON] Failed to parse config.yaml: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("[ADDON] Failed to fetch config.yaml: {}", e);
                        }
                    }
                }
            }
        }

        None
    }

    /// Fetches the first line of README as description.
    fn fetch_readme_description(
        &self,
        addon_id: &str,
        files: &HashMap<&str, &GitHubEntry>,
    ) -> String {
        // Try common README filenames
        let readme_names = ["README.md", "readme.md", "README.txt", "readme.txt"];

        for name in readme_names {
            if let Some(entry) = files.get(name) {
                if let Some(ref url) = entry.download_url {
                    if let Ok(content) = self.fetch_raw_content(url) {
                        // Get first non-empty line that's not a heading marker
                        for line in content.lines().take(10) {
                            let line = line.trim();
                            if !line.is_empty() && !line.starts_with('#') {
                                let desc = line.chars().take(200).collect::<String>();
                                return desc;
                            }
                        }
                    }
                }
            }
        }

        format!("Add-on: {}", addon_id)
    }

    /// Fetches script content for an addon.
    ///
    /// Uses the new directory structure: scripts/{os}/{technology}/install.ext
    pub fn fetch_script(&self, addon_id: &str, script_type: ScriptType) -> Result<String, AddonError> {
        assert!(!addon_id.is_empty(), "Addon ID must not be empty");

        let os_dir = current_os_directory();
        let addon_path = format!("{}/{}/{}", SCRIPTS_DIRECTORY, os_dir, addon_id);
        let filename = script_type.filename();

        info!("[ADDON] fetch_script: addon='{}', type={:?}, path='{}'",
            addon_id, script_type, addon_path);

        let entries = self.fetch_contents(&addon_path)?;
        debug!("[ADDON] Found {} entries in addon directory", entries.len());

        let file = entries
            .iter()
            .find(|e| e.name == filename)
            .ok_or_else(|| {
                warn!("[ADDON] Script not found: {} in {}", filename, addon_path);
                AddonError::ScriptNotFound(addon_id.to_string(), script_type)
            })?;

        let url = file
            .download_url
            .as_ref()
            .ok_or_else(|| {
                warn!("[ADDON] No download URL for script: {}", filename);
                AddonError::ScriptNotFound(addon_id.to_string(), script_type)
            })?;

        info!("[ADDON] Downloading script from: {}", url);
        self.fetch_raw_content(url)
    }

    /// Fetches repository contents at a path.
    fn fetch_contents(&self, path: &str) -> Result<Vec<GitHubEntry>, AddonError> {
        let url = if path.is_empty() {
            format!(
                "https://api.github.com/repos/{}/contents?ref={}",
                self.repository, self.branch
            )
        } else {
            format!(
                "https://api.github.com/repos/{}/contents/{}?ref={}",
                self.repository, path, self.branch
            )
        };

        debug!("[ADDON] fetch_contents: GET {}", url);
        let start = Instant::now();

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| {
                warn!("[ADDON] HTTP request failed: {}", e);
                AddonError::NetworkError(e.to_string())
            })?;

        let elapsed = start.elapsed();
        let status = response.status();
        debug!("[ADDON] Response: {} in {:?}", status, elapsed);

        if status == reqwest::StatusCode::NOT_FOUND {
            warn!("[ADDON] Repository or path not found: {}", path);
            return Err(AddonError::RepositoryNotFound);
        }

        if status == reqwest::StatusCode::FORBIDDEN {
            // Check for rate limit
            if let Some(remaining) = response.headers().get("x-ratelimit-remaining") {
                if remaining.to_str().unwrap_or("1") == "0" {
                    warn!("[ADDON] GitHub rate limit exceeded!");
                    return Err(AddonError::RateLimitExceeded);
                }
            }
        }

        if !status.is_success() {
            warn!("[ADDON] GitHub API error: {}", status);
            return Err(AddonError::NetworkError(format!(
                "GitHub API error: {}",
                status
            )));
        }

        let entries: Vec<GitHubEntry> = response
            .json()
            .map_err(|e| {
                warn!("[ADDON] Failed to parse JSON: {}", e);
                AddonError::NetworkError(format!("Failed to parse response: {}", e))
            })?;

        debug!("[ADDON] Parsed {} entries from response", entries.len());

        // Limit entries to prevent abuse
        Ok(entries.into_iter().take(MAX_FILES_PER_ADDON).collect())
    }

    /// Fetches raw content from a URL.
    fn fetch_raw_content(&self, url: &str) -> Result<String, AddonError> {
        assert!(!url.is_empty(), "URL must not be empty");

        debug!("[ADDON] fetch_raw_content: GET {}", url);
        let start = Instant::now();

        let response = self
            .client
            .get(url)
            .send()
            .map_err(|e| {
                warn!("[ADDON] HTTP request failed: {}", e);
                AddonError::NetworkError(e.to_string())
            })?;

        let elapsed = start.elapsed();
        let status = response.status();
        debug!("[ADDON] Response: {} in {:?}", status, elapsed);

        if !status.is_success() {
            warn!("[ADDON] Failed to fetch content: {}", status);
            return Err(AddonError::NetworkError(format!(
                "Failed to fetch content: {}",
                status
            )));
        }

        let content = response
            .text()
            .map_err(|e| AddonError::NetworkError(format!("Failed to read content: {}", e)))?;

        info!("[ADDON] Downloaded {} bytes in {:?}", content.len(), elapsed);
        Ok(content)
    }

    /// Clears the addon cache.
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }
    }

    /// Returns the current repository.
    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// Returns the current branch.
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_entry() {
        let dir_entry = GitHubEntry {
            name: "nodejs".to_string(),
            path: "nodejs".to_string(),
            entry_type: "dir".to_string(),
            size: 0,
            download_url: None,
        };

        assert!(dir_entry.is_directory());
        assert!(!dir_entry.is_file());

        let file_entry = GitHubEntry {
            name: "install.ps1".to_string(),
            path: "scripts/windows/nodejs/install.ps1".to_string(),
            entry_type: "file".to_string(),
            size: 1024,
            download_url: Some("https://raw.githubusercontent.com/...".to_string()),
        };

        assert!(file_entry.is_file());
        assert!(!file_entry.is_directory());
    }

    #[test]
    fn test_client_creation() {
        let client = AddonGitHubClient::new("hastur-dev/installer-repo", "main");
        assert_eq!(client.repository(), "hastur-dev/installer-repo");
        assert_eq!(client.branch(), "main");
    }

    #[test]
    fn test_set_repository() {
        let mut client = AddonGitHubClient::new("hastur-dev/installer-repo", "main");
        client.set_repository("other/repo", "dev");
        assert_eq!(client.repository(), "other/repo");
        assert_eq!(client.branch(), "dev");
    }

    #[test]
    #[should_panic(expected = "Repository must be in owner/repo format")]
    fn test_invalid_repository() {
        let _ = AddonGitHubClient::new("invalid-repo", "main");
    }
}
