//! Auto-update module for ratterm.
//!
//! Checks for updates on startup and downloads new versions automatically.

use std::env;
use std::fs;
use std::io::{self, Write};

/// Current version of ratterm.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub repository for updates.
const REPO: &str = "hastur-dev/ratterm";

/// Update check result.
#[derive(Debug)]
pub enum UpdateStatus {
    /// No update available.
    UpToDate,
    /// Update available with version string.
    Available(String),
    /// Update check failed.
    Failed(String),
    /// Updates disabled.
    Disabled,
}

/// Updater configuration.
pub struct Updater {
    /// Whether auto-update is enabled.
    enabled: bool,
    /// GitHub repository.
    repo: String,
    /// Current version.
    current_version: String,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

impl Updater {
    /// Creates a new updater.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: env::var("RATTERM_NO_UPDATE").is_err(),
            repo: REPO.to_string(),
            current_version: VERSION.to_string(),
        }
    }

    /// Checks for updates.
    pub fn check(&self) -> UpdateStatus {
        if !self.enabled {
            return UpdateStatus::Disabled;
        }

        match self.fetch_latest_version() {
            Ok(latest) => {
                if self.is_newer(&latest) {
                    UpdateStatus::Available(latest)
                } else {
                    UpdateStatus::UpToDate
                }
            }
            Err(e) => UpdateStatus::Failed(e),
        }
    }

    /// Fetches the latest version from GitHub.
    fn fetch_latest_version(&self) -> Result<String, String> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);

        // Use a simple blocking HTTP request
        // In production, you might want to use reqwest or ureq
        let output = std::process::Command::new("curl")
            .args([
                "-fsSL",
                "-H",
                "Accept: application/vnd.github.v3+json",
                &url,
            ])
            .output()
            .map_err(|e| format!("Failed to run curl: {}", e))?;

        if !output.status.success() {
            return Err("Failed to fetch release info".to_string());
        }

        let body = String::from_utf8_lossy(&output.stdout);

        // Simple JSON parsing for tag_name
        for line in body.lines() {
            if line.contains("\"tag_name\"") {
                if let Some(start) = line.find(": \"v") {
                    if let Some(end) = line[start + 4..].find('"') {
                        return Ok(line[start + 4..start + 4 + end].to_string());
                    }
                }
                if let Some(start) = line.find(": \"") {
                    if let Some(end) = line[start + 3..].find('"') {
                        let version = &line[start + 3..start + 3 + end];
                        return Ok(version.trim_start_matches('v').to_string());
                    }
                }
            }
        }

        Err("Could not parse version from response".to_string())
    }

    /// Checks if the given version is newer than current.
    fn is_newer(&self, other: &str) -> bool {
        let current = parse_version(&self.current_version);
        let other = parse_version(other);

        other > current
    }

    /// Downloads and installs the update.
    pub fn update(&self, new_version: &str) -> Result<(), String> {
        let asset_name = self.get_asset_name();
        let url = format!(
            "https://github.com/{}/releases/download/v{}/{}",
            self.repo, new_version, asset_name
        );

        // Get current executable path
        let current_exe =
            env::current_exe().map_err(|e| format!("Failed to get current exe path: {}", e))?;

        let backup_path = current_exe.with_extension("old");
        let temp_path = current_exe.with_extension("new");

        // Download new version
        eprintln!("Downloading ratterm v{}...", new_version);

        let output = std::process::Command::new("curl")
            .args(["-fsSL", "-o", temp_path.to_str().unwrap_or(""), &url])
            .output()
            .map_err(|e| format!("Failed to download: {}", e))?;

        if !output.status.success() {
            return Err("Download failed".to_string());
        }

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&temp_path)
                .map_err(|e| format!("Failed to get permissions: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&temp_path, perms)
                .map_err(|e| format!("Failed to set permissions: {}", e))?;
        }

        // Backup current executable
        if current_exe.exists() {
            fs::rename(&current_exe, &backup_path)
                .map_err(|e| format!("Failed to backup current exe: {}", e))?;
        }

        // Move new executable into place
        fs::rename(&temp_path, &current_exe).map_err(|e| {
            // Try to restore backup
            let _ = fs::rename(&backup_path, &current_exe);
            format!("Failed to install new exe: {}", e)
        })?;

        // Remove backup
        let _ = fs::remove_file(&backup_path);

        eprintln!("Updated to v{}. Please restart ratterm.", new_version);

        Ok(())
    }

    /// Gets the asset name for the current platform.
    fn get_asset_name(&self) -> String {
        let os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };

        let arch = if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x86_64" // fallback
        };

        if cfg!(target_os = "windows") {
            format!("rat-{}-{}.exe", os, arch)
        } else {
            format!("rat-{}-{}", os, arch)
        }
    }
}

/// Parses a version string into comparable parts.
fn parse_version(version: &str) -> (u32, u32, u32) {
    let parts: Vec<u32> = version
        .trim_start_matches('v')
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    )
}

/// Checks for updates and prompts user.
/// Returns true if user wants to update now.
pub fn check_for_updates() -> bool {
    let updater = Updater::new();

    match updater.check() {
        UpdateStatus::Available(version) => {
            eprintln!();
            eprintln!("╔════════════════════════════════════════╗");
            eprintln!("║  A new version of ratterm is available ║");
            eprintln!("║  Current: v{:<28}║", VERSION);
            eprintln!("║  Latest:  v{:<28}║", version);
            eprintln!("╚════════════════════════════════════════╝");
            eprintln!();

            // Check if running interactively
            if atty::is(atty::Stream::Stdin) {
                eprint!("Update now? [Y/n] ");
                let _ = io::stderr().flush();

                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_ok() {
                    let input = input.trim().to_lowercase();
                    if input.is_empty() || input == "y" || input == "yes" {
                        if let Err(e) = updater.update(&version) {
                            eprintln!("Update failed: {}", e);
                        } else {
                            return true;
                        }
                    }
                }
            } else {
                eprintln!("Run 'rat --update' to update.");
            }
        }
        UpdateStatus::UpToDate => {
            // Silent - no message needed
        }
        UpdateStatus::Failed(_e) => {
            // Silent failure - don't bother user
        }
        UpdateStatus::Disabled => {
            // Updates disabled via env var
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("v1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("0.1.0"), (0, 1, 0));
    }

    #[test]
    fn test_is_newer() {
        let updater = Updater {
            enabled: true,
            repo: "test/test".to_string(),
            current_version: "0.1.0".to_string(),
        };

        assert!(updater.is_newer("0.2.0"));
        assert!(updater.is_newer("0.1.1"));
        assert!(updater.is_newer("1.0.0"));
        assert!(!updater.is_newer("0.1.0"));
        assert!(!updater.is_newer("0.0.9"));
    }
}
