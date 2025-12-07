//! Extension installer.
//!
//! Handles downloading, extracting, and installing extensions.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use super::manifest::{ExtensionManifest, load_manifest};
use super::registry::{GitHubRegistry, RegistryProvider};
use super::{ExtensionError, extensions_dir};

/// Extension installer.
pub struct Installer {
    /// GitHub registry client.
    registry: GitHubRegistry,
}

impl Default for Installer {
    fn default() -> Self {
        Self::new()
    }
}

impl Installer {
    /// Creates a new installer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: GitHubRegistry::new(),
        }
    }

    /// Installs an extension from a GitHub repository.
    pub fn install_from_github(&self, repo_ref: &str) -> Result<ExtensionManifest, ExtensionError> {
        let (owner, repo, version) = GitHubRegistry::parse_repo_ref(repo_ref)
            .ok_or_else(|| ExtensionError::Registry(format!("Invalid repo: {}", repo_ref)))?;

        // Get extension info
        let info = self.registry.get(repo_ref)?;
        let version_to_install = version.unwrap_or_else(|| format!("v{}", info.version));

        // Ensure extensions directory exists
        let ext_dir = extensions_dir().ok_or_else(|| {
            ExtensionError::Registry("Could not determine extensions directory".to_string())
        })?;
        fs::create_dir_all(&ext_dir)?;

        let install_dir = ext_dir.join(&info.name);
        if install_dir.exists() {
            return Err(ExtensionError::AlreadyInstalled(info.name));
        }

        // Download the archive
        let archive_path =
            self.registry
                .download_release_archive(&owner, &repo, &version_to_install)?;

        // Extract and install
        self.extract_and_install(&archive_path, &install_dir)?;

        // Clean up downloaded archive
        let _ = fs::remove_file(&archive_path);

        // Load and return the manifest
        let manifest_path = install_dir.join("extension.toml");
        load_manifest(&manifest_path)
    }

    /// Installs an extension from a local directory (for development).
    pub fn install_from_local(&self, source: &Path) -> Result<ExtensionManifest, ExtensionError> {
        // Validate source has extension.toml
        let manifest_path = source.join("extension.toml");
        if !manifest_path.exists() {
            return Err(ExtensionError::Manifest(
                "Source directory missing extension.toml".to_string(),
            ));
        }

        let manifest = load_manifest(&manifest_path)?;

        // Ensure extensions directory exists
        let ext_dir = extensions_dir().ok_or_else(|| {
            ExtensionError::Registry("Could not determine extensions directory".to_string())
        })?;
        fs::create_dir_all(&ext_dir)?;

        let install_dir = ext_dir.join(&manifest.extension.name);
        if install_dir.exists() {
            return Err(ExtensionError::AlreadyInstalled(
                manifest.extension.name.clone(),
            ));
        }

        // Copy the directory
        copy_dir_recursive(source, &install_dir)?;

        Ok(manifest)
    }

    /// Extracts a zip archive to the installation directory.
    fn extract_and_install(&self, archive: &Path, dest: &Path) -> Result<(), ExtensionError> {
        let file = File::open(archive)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| ExtensionError::Registry(format!("Failed to open archive: {}", e)))?;

        // Create destination directory
        fs::create_dir_all(dest)?;

        // Find the root directory in the archive (GitHub zips have a top-level folder)
        let root_prefix = if archive.len() > 0 {
            let first = archive.by_index(0).map_err(|e| {
                ExtensionError::Registry(format!("Failed to read archive entry: {}", e))
            })?;
            let name = first.name();
            if name.contains('/') {
                Some(name.split('/').next().unwrap_or("").to_string())
            } else {
                None
            }
        } else {
            None
        };

        // Extract files
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                ExtensionError::Registry(format!("Failed to read archive entry: {}", e))
            })?;

            let mut outpath = PathBuf::new();

            // Strip root prefix if present
            let name = file.name();
            let relative_path = if let Some(ref prefix) = root_prefix {
                if let Some(stripped) = name.strip_prefix(prefix) {
                    stripped.trim_start_matches('/')
                } else {
                    name
                }
            } else {
                name
            };

            if relative_path.is_empty() {
                continue;
            }

            outpath.push(dest);
            outpath.push(relative_path);

            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
            }
        }

        // Verify extension.toml exists
        if !dest.join("extension.toml").exists() {
            fs::remove_dir_all(dest)?;
            return Err(ExtensionError::Manifest(
                "Archive does not contain extension.toml".to_string(),
            ));
        }

        Ok(())
    }

    /// Updates an installed extension.
    pub fn update(&self, name: &str) -> Result<ExtensionManifest, ExtensionError> {
        let ext_dir = extensions_dir().ok_or_else(|| {
            ExtensionError::Registry("Could not determine extensions directory".to_string())
        })?;

        let install_dir = ext_dir.join(name);
        if !install_dir.exists() {
            return Err(ExtensionError::NotFound(name.to_string()));
        }

        // Load current manifest to get the source repo
        let manifest_path = install_dir.join("extension.toml");
        let manifest = load_manifest(&manifest_path)?;

        // Get the homepage to determine the repo
        let homepage = &manifest.extension.homepage;
        let repo_ref = if homepage.starts_with("https://github.com/") {
            homepage.strip_prefix("https://github.com/").unwrap_or(name)
        } else {
            return Err(ExtensionError::Registry(
                "Cannot determine source repository for update".to_string(),
            ));
        };

        // Remove old version
        fs::remove_dir_all(&install_dir)?;

        // Install new version
        self.install_from_github(repo_ref)
    }
}

/// Recursively copies a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installer_new() {
        let installer = Installer::new();
        // Just verify it creates without panic
        drop(installer);
    }
}
