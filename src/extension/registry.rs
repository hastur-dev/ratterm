//! Extension registry and GitHub integration.
//!
//! Provides an interface for discovering and downloading extensions from GitHub.

use std::path::PathBuf;

use serde::Deserialize;

use super::ExtensionError;

/// Registry provider trait for extensibility.
pub trait RegistryProvider: Send + Sync {
    /// Searches for extensions matching a query.
    fn search(&self, query: &str) -> Result<Vec<ExtensionInfo>, ExtensionError>;

    /// Gets information about a specific extension.
    fn get(&self, name: &str) -> Result<ExtensionInfo, ExtensionError>;

    /// Downloads an extension archive.
    fn download(&self, name: &str, version: &str) -> Result<PathBuf, ExtensionError>;
}

/// Extension information from the registry.
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    /// Extension name.
    pub name: String,
    /// Latest version.
    pub version: String,
    /// Description.
    pub description: String,
    /// Author.
    pub author: String,
    /// Download URL.
    pub download_url: String,
    /// Homepage URL.
    pub homepage: String,
}

/// GitHub release information.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

/// GitHub release asset.
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// GitHub repository info.
#[derive(Debug, Deserialize)]
struct GitHubRepo {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    html_url: String,
}

/// GitHub-based extension registry.
pub struct GitHubRegistry {
    /// HTTP client for API requests.
    client: reqwest::blocking::Client,
}

impl Default for GitHubRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubRegistry {
    /// Creates a new GitHub registry client.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("ratterm-extension-manager")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self { client }
    }

    /// Parses a GitHub repo reference (user/repo or user/repo@version).
    #[must_use]
    pub fn parse_repo_ref(input: &str) -> Option<(String, String, Option<String>)> {
        let (repo_part, version) = if let Some(idx) = input.find('@') {
            let (repo, ver) = input.split_at(idx);
            (repo, Some(ver[1..].to_string()))
        } else if let Some(idx) = input.find('#') {
            // Branch reference
            let (repo, branch) = input.split_at(idx);
            (repo, Some(branch[1..].to_string()))
        } else {
            (input, None)
        };

        let parts: Vec<&str> = repo_part.split('/').collect();
        if parts.len() != 2 {
            return None;
        }

        Some((parts[0].to_string(), parts[1].to_string(), version))
    }

    /// Gets the latest release for a repository.
    pub fn get_latest_release(&self, owner: &str, repo: &str) -> Result<GitHubRelease, ExtensionError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            owner, repo
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| ExtensionError::Registry(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ExtensionError::Registry(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        response
            .json::<GitHubRelease>()
            .map_err(|e| ExtensionError::Registry(format!("Failed to parse response: {}", e)))
    }

    /// Gets repository information.
    pub fn get_repo_info(&self, owner: &str, repo: &str) -> Result<GitHubRepo, ExtensionError> {
        let url = format!("https://api.github.com/repos/{}/{}", owner, repo);

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| ExtensionError::Registry(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ExtensionError::Registry(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        response
            .json::<GitHubRepo>()
            .map_err(|e| ExtensionError::Registry(format!("Failed to parse response: {}", e)))
    }

    /// Downloads a file from a URL.
    pub fn download_file(&self, url: &str, dest: &PathBuf) -> Result<(), ExtensionError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|e| ExtensionError::Registry(format!("Download failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ExtensionError::Registry(format!(
                "Download failed: {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .map_err(|e| ExtensionError::Registry(format!("Failed to read response: {}", e)))?;

        std::fs::write(dest, &bytes)?;

        Ok(())
    }

    /// Downloads the source archive for a release.
    pub fn download_release_archive(
        &self,
        owner: &str,
        repo: &str,
        version: &str,
    ) -> Result<PathBuf, ExtensionError> {
        let cache = super::cache_dir()
            .ok_or_else(|| ExtensionError::Registry("Could not determine cache directory".to_string()))?;

        std::fs::create_dir_all(&cache)?;

        let filename = format!("{}-{}-{}.zip", owner, repo, version);
        let dest = cache.join(&filename);

        // Use zipball URL
        let url = format!(
            "https://github.com/{}/{}/archive/refs/tags/{}.zip",
            owner, repo, version
        );

        self.download_file(&url, &dest)?;

        Ok(dest)
    }
}

impl RegistryProvider for GitHubRegistry {
    fn search(&self, _query: &str) -> Result<Vec<ExtensionInfo>, ExtensionError> {
        // GitHub doesn't have a ratterm-specific search
        // For now, return empty - future: use GitHub search API with topic filter
        Ok(Vec::new())
    }

    fn get(&self, name: &str) -> Result<ExtensionInfo, ExtensionError> {
        let (owner, repo, _version) = Self::parse_repo_ref(name)
            .ok_or_else(|| ExtensionError::Registry(format!("Invalid repo reference: {}", name)))?;

        let repo_info = self.get_repo_info(&owner, &repo)?;
        let release = self.get_latest_release(&owner, &repo)?;

        // Find the extension zip asset
        let download_url = release
            .assets
            .iter()
            .find(|a| a.name.ends_with(".zip") || a.name == "extension.zip")
            .map(|a| a.browser_download_url.clone())
            .unwrap_or_else(|| {
                format!(
                    "https://github.com/{}/{}/archive/refs/tags/{}.zip",
                    owner, repo, release.tag_name
                )
            });

        Ok(ExtensionInfo {
            name: repo.clone(),
            version: release.tag_name.trim_start_matches('v').to_string(),
            description: repo_info.description.unwrap_or_default(),
            author: owner,
            download_url,
            homepage: repo_info.html_url,
        })
    }

    fn download(&self, name: &str, version: &str) -> Result<PathBuf, ExtensionError> {
        let (owner, repo, _) = Self::parse_repo_ref(name)
            .ok_or_else(|| ExtensionError::Registry(format!("Invalid repo reference: {}", name)))?;

        self.download_release_archive(&owner, &repo, version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_ref_simple() {
        let (owner, repo, version) = GitHubRegistry::parse_repo_ref("user/repo").unwrap();
        assert_eq!(owner, "user");
        assert_eq!(repo, "repo");
        assert!(version.is_none());
    }

    #[test]
    fn test_parse_repo_ref_with_version() {
        let (owner, repo, version) = GitHubRegistry::parse_repo_ref("user/repo@v1.0.0").unwrap();
        assert_eq!(owner, "user");
        assert_eq!(repo, "repo");
        assert_eq!(version, Some("v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_repo_ref_with_branch() {
        let (owner, repo, version) = GitHubRegistry::parse_repo_ref("user/repo#main").unwrap();
        assert_eq!(owner, "user");
        assert_eq!(repo, "repo");
        assert_eq!(version, Some("main".to_string()));
    }

    #[test]
    fn test_parse_repo_ref_invalid() {
        assert!(GitHubRegistry::parse_repo_ref("invalid").is_none());
        assert!(GitHubRegistry::parse_repo_ref("too/many/parts").is_none());
    }
}
