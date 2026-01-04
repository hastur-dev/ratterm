//! Add-on installer for background script execution.
//!
//! Handles downloading and executing install scripts using the
//! existing BackgroundManager.

use super::types::{AddonError, ScriptType};
use crate::terminal::BackgroundManager;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Maximum script size (1MB).
const MAX_SCRIPT_SIZE: usize = 1_048_576;

/// Common executable names for detecting installed technologies.
/// Maps addon_id to possible executable names.
const DETECTION_COMMANDS: &[(&str, &[&str])] = &[
    ("vim", &["vim", "nvim", "gvim"]),
    ("neovim", &["nvim"]),
    ("emacs", &["emacs", "emacsclient"]),
    ("nodejs", &["node", "npm", "npx"]),
    ("javascript", &["node", "npm", "npx"]),
    ("npm", &["npm", "npx", "node"]),
    ("python", &["python", "python3", "py"]),
    ("rust", &["rustc", "cargo"]),
    ("go", &["go"]),
    ("java", &["java", "javac"]),
    ("dotnet", &["dotnet"]),
    ("ruby", &["ruby"]),
    ("php", &["php"]),
    ("perl", &["perl"]),
    ("git", &["git"]),
    ("docker", &["docker"]),
    ("kubectl", &["kubectl"]),
    ("terraform", &["terraform"]),
    ("ansible", &["ansible"]),
    ("cmake", &["cmake"]),
    ("make", &["make"]),
    ("gcc", &["gcc"]),
    ("clang", &["clang"]),
    ("llvm", &["llc", "clang"]),
    ("gpustat", &["gpustat", "nvidia-smi"]),
];

/// Installation phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPhase {
    /// Downloading script from GitHub.
    Downloading,
    /// Running install script.
    Installing,
    /// Installation complete.
    Completed,
    /// Installation failed.
    Failed,
}

impl InstallPhase {
    /// Returns true if this phase represents completion (success or failure).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    /// Returns true if this phase represents an error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Failed)
    }

    /// Returns a display string for this phase.
    #[must_use]
    pub fn display(&self) -> &'static str {
        match self {
            Self::Downloading => "Downloading...",
            Self::Installing => "Installing...",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }
}

/// Progress of an ongoing addon installation.
#[derive(Debug, Clone)]
pub struct InstallProgress {
    /// Addon being installed.
    pub addon_id: String,
    /// Current phase.
    pub phase: InstallPhase,
    /// Progress percentage (0-100).
    pub progress: u8,
    /// Error message if failed.
    pub error: Option<String>,
    /// Background process ID.
    pub process_id: Option<u64>,
}

impl InstallProgress {
    /// Creates a new install progress tracker.
    #[must_use]
    pub fn new(addon_id: String) -> Self {
        assert!(!addon_id.is_empty(), "Addon ID must not be empty");

        Self {
            addon_id,
            phase: InstallPhase::Downloading,
            progress: 0,
            error: None,
            process_id: None,
        }
    }

    /// Sets the phase to downloading.
    pub fn set_downloading(&mut self) {
        self.phase = InstallPhase::Downloading;
        self.progress = 10;
    }

    /// Sets the phase to installing with process ID.
    pub fn set_installing(&mut self, process_id: u64) {
        self.phase = InstallPhase::Installing;
        self.progress = 50;
        self.process_id = Some(process_id);
    }

    /// Sets the phase to completed.
    pub fn set_completed(&mut self) {
        self.phase = InstallPhase::Completed;
        self.progress = 100;
    }

    /// Sets the phase to failed with error message.
    pub fn set_failed(&mut self, error: String) {
        self.phase = InstallPhase::Failed;
        self.error = Some(error);
    }

    /// Returns true if installation is complete (success or failure).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.phase.is_finished()
    }
}

/// Add-on installer that manages script execution.
pub struct AddonInstaller {
    /// Directory for temporary scripts.
    temp_dir: PathBuf,
}

impl AddonInstaller {
    /// Creates a new addon installer.
    #[must_use]
    pub fn new() -> Self {
        let temp_dir = env::temp_dir().join("ratterm-addons");
        Self { temp_dir }
    }

    /// Checks if a technology is already installed on the system.
    ///
    /// Uses the `where` command on Windows or `which` on Unix to detect
    /// if the technology's executable is available in PATH.
    ///
    /// # Returns
    /// `Some(path)` if found, `None` if not installed.
    #[must_use]
    pub fn detect_installed(addon_id: &str) -> Option<String> {
        info!("[ADDON-DETECT] Checking if '{}' is installed...", addon_id);

        // Find the detection commands for this addon
        let addon_id_lower = addon_id.to_lowercase();
        let fallback: [&str; 1] = [addon_id];
        let commands = DETECTION_COMMANDS
            .iter()
            .find(|(id, _)| *id == addon_id_lower)
            .map(|(_, cmds)| *cmds)
            .unwrap_or(&fallback);

        for cmd in commands {
            debug!("[ADDON-DETECT] Looking for executable: {}", cmd);

            #[cfg(windows)]
            let check = std::process::Command::new("where")
                .arg(cmd)
                .output();

            #[cfg(not(windows))]
            let check = std::process::Command::new("which")
                .arg(cmd)
                .output();

            match check {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if !path.is_empty() {
                        info!("[ADDON-DETECT] Found '{}' at: {}", cmd, path);
                        return Some(path);
                    }
                }
                Ok(_) => {
                    debug!("[ADDON-DETECT] '{}' not found in PATH", cmd);
                }
                Err(e) => {
                    warn!("[ADDON-DETECT] Error checking for '{}': {}", cmd, e);
                }
            }
        }

        info!("[ADDON-DETECT] '{}' is NOT installed", addon_id);
        None
    }

    /// Checks if a technology is installed and returns version info.
    ///
    /// # Returns
    /// `Some((path, version))` if found with version, `Some((path, ""))` if found without version.
    #[must_use]
    pub fn detect_installed_with_version(addon_id: &str) -> Option<(String, String)> {
        let path = Self::detect_installed(addon_id)?;

        // Try to get version
        let version_args: &[&str] = match addon_id.to_lowercase().as_str() {
            "vim" | "neovim" => &["--version"],
            "nodejs" | "node" => &["--version"],
            "python" => &["--version"],
            "rust" | "rustc" => &["--version"],
            "go" => &["version"],
            "java" => &["-version"],
            "dotnet" => &["--version"],
            "ruby" => &["--version"],
            "php" => &["--version"],
            "git" => &["--version"],
            "docker" => &["--version"],
            _ => &["--version"],
        };

        // Get the command name from the path
        let cmd = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(addon_id);

        debug!("[ADDON-DETECT] Getting version for '{}' with {:?}", cmd, version_args);

        let version_check = std::process::Command::new(cmd)
            .args(version_args)
            .output();

        let version = match version_check {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Some tools output version to stderr (like java -version)
                let combined = if stdout.trim().is_empty() {
                    stderr.to_string()
                } else {
                    stdout.to_string()
                };
                // Get first line as version
                combined.lines().next().unwrap_or("").trim().to_string()
            }
            Err(_) => String::new(),
        };

        debug!("[ADDON-DETECT] Version: '{}'", version);
        Some((path, version))
    }

    /// Ensures required directories exist.
    fn ensure_dirs(&self) -> Result<(), AddonError> {
        fs::create_dir_all(&self.temp_dir)
            .map_err(|e| AddonError::ConfigError(format!("Failed to create temp dir: {}", e)))?;
        Ok(())
    }

    /// Checks if an installation is complete.
    ///
    /// # Returns
    /// Updated progress if the process has finished, None if still running.
    pub fn check_install_complete(
        &self,
        progress: &InstallProgress,
        background_manager: &BackgroundManager,
    ) -> Option<InstallProgress> {
        let process_id = progress.process_id?;

        let info = background_manager.get_info(process_id)?;

        if !info.status.is_finished() {
            debug!("[ADDON-INSTALL] Process {} still running", process_id);
            return None;
        }

        info!("[ADDON-INSTALL] Process {} finished, status: {:?}", process_id, info.status);

        let mut updated = progress.clone();

        if info.status.is_error() {
            let output = background_manager
                .get_output(process_id)
                .unwrap_or_default();
            let error_msg = info
                .error_message
                .unwrap_or_else(|| "Installation failed".to_string());
            let full_error = if output.is_empty() {
                error_msg.clone()
            } else {
                // Truncate output for error message
                let truncated: String = output.chars().take(500).collect();
                format!("{}: {}", error_msg, truncated)
            };
            warn!("[ADDON-INSTALL] Installation failed: {}", full_error);
            updated.set_failed(full_error);
        } else {
            info!("[ADDON-INSTALL] Installation completed successfully!");
            updated.set_completed();
        }

        Some(updated)
    }

    /// Writes a script to a temporary file.
    fn write_temp_script(&self, addon_id: &str, content: &str) -> Result<PathBuf, AddonError> {
        let filename = format!("{}_{}", addon_id, ScriptType::Install.filename());
        let path = self.temp_dir.join(filename);

        let mut file = fs::File::create(&path)
            .map_err(|e| AddonError::ExecutionFailed(format!("Failed to create script: {}", e)))?;

        file.write_all(content.as_bytes())
            .map_err(|e| AddonError::ExecutionFailed(format!("Failed to write script: {}", e)))?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)
                .map_err(|e| AddonError::ExecutionFailed(format!("Failed to get perms: {}", e)))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms)
                .map_err(|e| AddonError::ExecutionFailed(format!("Failed to set perms: {}", e)))?;
        }

        Ok(path)
    }

    /// Builds the command to execute a script.
    fn build_command(&self, script_path: &std::path::Path) -> String {
        let path_str = script_path.display().to_string();

        #[cfg(windows)]
        {
            // Use -Command with & operator for better path handling through cmd /C
            // Escape single quotes in path and wrap with single quotes for PowerShell
            let escaped_path = path_str.replace('\'', "''");
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -Command \"& '{}'\"",
                escaped_path
            )
        }

        #[cfg(not(windows))]
        {
            format!("bash \"{}\"", path_str)
        }
    }

    /// Starts an addon installation with pre-fetched content.
    ///
    /// This is the non-blocking version that receives content from the background fetcher.
    pub fn start_install_with_content(
        &self,
        addon_id: &str,
        script_content: &str,
        background_manager: &mut BackgroundManager,
    ) -> Result<InstallProgress, AddonError> {
        assert!(!addon_id.is_empty(), "Addon ID must not be empty");

        info!("[ADDON-INSTALL] Starting installation with pre-fetched content: '{}'", addon_id);

        self.ensure_dirs()?;

        let mut progress = InstallProgress::new(addon_id.to_string());

        // Validate script size
        if script_content.len() > MAX_SCRIPT_SIZE {
            warn!("[ADDON-INSTALL] Script too large: {} bytes", script_content.len());
            return Err(AddonError::ExecutionFailed(
                "Script exceeds maximum size".to_string(),
            ));
        }

        // Write script to temp file
        let script_path = self.write_temp_script(addon_id, script_content)?;
        info!("[ADDON-INSTALL] Script written to: {:?}", script_path);

        // Build command based on platform
        let command = self.build_command(&script_path);
        info!("[ADDON-INSTALL] Executing command: {}", command);

        // Start background process
        let process_id = background_manager
            .start(&command)
            .map_err(AddonError::ExecutionFailed)?;

        info!("[ADDON-INSTALL] Background process started with ID: {}", process_id);
        progress.set_installing(process_id);

        Ok(progress)
    }
}

impl Default for AddonInstaller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_phase() {
        assert!(!InstallPhase::Downloading.is_finished());
        assert!(!InstallPhase::Installing.is_finished());
        assert!(InstallPhase::Completed.is_finished());
        assert!(InstallPhase::Failed.is_finished());

        assert!(!InstallPhase::Completed.is_error());
        assert!(InstallPhase::Failed.is_error());
    }

    #[test]
    fn test_install_progress() {
        let mut progress = InstallProgress::new("test".to_string());
        assert_eq!(progress.phase, InstallPhase::Downloading);
        assert!(!progress.is_finished());

        progress.set_installing(123);
        assert_eq!(progress.phase, InstallPhase::Installing);
        assert_eq!(progress.process_id, Some(123));

        progress.set_completed();
        assert!(progress.is_finished());
        assert!(progress.error.is_none());
    }

    #[test]
    fn test_install_progress_failure() {
        let mut progress = InstallProgress::new("test".to_string());
        progress.set_failed("Something went wrong".to_string());

        assert!(progress.is_finished());
        assert_eq!(progress.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_installer_creation() {
        let installer = AddonInstaller::new();
        assert!(installer.temp_dir.to_string_lossy().contains("ratterm-addons"));
    }

    #[test]
    fn test_detect_installed_rust() {
        // Rust should be installed since we're running cargo tests
        let result = AddonInstaller::detect_installed("rust");
        assert!(result.is_some(), "Rust should be detected since we're running cargo");
        let path = result.unwrap();
        assert!(!path.is_empty(), "Path should not be empty");
        // Path should contain rustc or cargo
        let path_lower = path.to_lowercase();
        assert!(
            path_lower.contains("rustc") || path_lower.contains("cargo"),
            "Path should contain rustc or cargo: {}",
            path
        );
    }

    #[test]
    fn test_detect_nonexistent_addon() {
        // An addon that definitely doesn't exist
        let result = AddonInstaller::detect_installed("zzznonexistent12345zzz");
        assert!(result.is_none(), "Nonexistent addon should not be detected");
    }

    #[test]
    fn test_detect_installed_python() {
        // Python is usually installed on dev machines
        let result = AddonInstaller::detect_installed("python");
        // Just check it runs without panicking
        // The result depends on the system
        if let Some(path) = result {
            assert!(!path.is_empty(), "If detected, path should not be empty");
            let path_lower = path.to_lowercase();
            assert!(
                path_lower.contains("python") || path_lower.contains("py"),
                "Path should contain python: {}",
                path
            );
        }
    }
}
