//! Auto-update module for ratterm.
//!
//! Checks for updates on startup and downloads new versions automatically.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

/// Current version of ratterm.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub repository for updates.
const REPO: &str = "hastur-dev/ratterm";

/// Maximum retry attempts for HTTP requests.
const MAX_HTTP_RETRIES: usize = 3;

/// Token printed by `--verify` to confirm a valid binary.
pub const VERIFY_TOKEN: &str = "verify-ok";

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
    /// Whether running in development mode (via cargo run).
    is_dev_mode: bool,
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
        let is_dev_mode = Self::detect_dev_mode();
        Self {
            enabled: env::var("RATTERM_NO_UPDATE").is_err(),
            is_dev_mode,
            repo: REPO.to_string(),
            current_version: VERSION.to_string(),
        }
    }

    /// Detects if running from a cargo target directory (dev mode).
    fn detect_dev_mode() -> bool {
        if let Ok(exe_path) = env::current_exe() {
            let path_str = exe_path.to_string_lossy();
            // Check if running from cargo's target directory
            path_str.contains("target\\debug")
                || path_str.contains("target/debug")
                || path_str.contains("target\\release")
                || path_str.contains("target/release")
        } else {
            false
        }
    }

    /// Returns true if running in development mode.
    #[must_use]
    pub fn is_dev_mode(&self) -> bool {
        self.is_dev_mode
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

    /// Fetches the latest version from GitHub using reqwest.
    fn fetch_latest_version(&self) -> Result<String, String> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);

        // Retry loop with bounded iterations
        for attempt in 0..MAX_HTTP_RETRIES {
            match self.fetch_version_attempt(&url) {
                Ok(version) => return Ok(version),
                Err(_) if attempt < MAX_HTTP_RETRIES - 1 => {
                    // Wait before retry (exponential backoff)
                    std::thread::sleep(std::time::Duration::from_millis(100 * (1 << attempt)));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err("Max retries exceeded".to_string())
    }

    /// Single attempt to fetch version from GitHub API.
    fn fetch_version_attempt(&self, url: &str) -> Result<String, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("ratterm-updater")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

        let response = client
            .get(url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| format!("Failed to fetch release info: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("GitHub API returned status {}", response.status()));
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| format!("Failed to parse JSON: {e}"))?;

        let tag_name = json
            .get("tag_name")
            .and_then(|v| v.as_str())
            .ok_or("No tag_name in response")?;

        Ok(tag_name.trim_start_matches('v').to_string())
    }

    /// Checks if the given version is newer than current.
    fn is_newer(&self, other: &str) -> bool {
        let current = parse_version(&self.current_version);
        let other = parse_version(other);

        other > current
    }

    /// Downloads and installs the update.
    /// Returns Ok(true) if an actual update was performed, Ok(false) if already up to date.
    pub fn update(&self, new_version: &str) -> Result<bool, String> {
        // Double-check version before downloading to avoid unnecessary work
        if !self.is_newer(new_version) {
            eprintln!(
                "Already running v{} (requested v{}).",
                self.current_version, new_version
            );
            return Ok(false);
        }

        let asset_name = self.get_asset_name();
        let url = format!(
            "https://github.com/{}/releases/download/v{}/{}",
            self.repo, new_version, asset_name
        );

        // Get current executable path
        let current_exe =
            env::current_exe().map_err(|e| format!("Failed to get current exe path: {e}"))?;

        let temp_path = current_exe.with_extension("new");

        // Download new version using reqwest
        eprintln!("Downloading ratterm v{new_version}...");
        self.download_file(&url, &temp_path)?;

        // Verify download actually produced a file
        let temp_meta = fs::metadata(&temp_path)
            .map_err(|_| "Download completed but file not found".to_string())?;

        if temp_meta.len() == 0 {
            let _ = fs::remove_file(&temp_path);
            return Err("Downloaded file is empty".to_string());
        }

        // Make executable on Unix before verification
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&temp_path)
                .map_err(|e| format!("Failed to get permissions: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&temp_path, perms)
                .map_err(|e| format!("Failed to set permissions: {e}"))?;
        }

        // Verify the downloaded binary is a valid ratterm executable
        // before replacing the current one
        eprintln!("Verifying downloaded binary...");
        if let Err(e) = self.verify_binary(&temp_path, new_version) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "Downloaded binary failed verification: {e}. Update aborted."
            ));
        }
        eprintln!("Verification passed.");

        // Platform-specific installation
        #[cfg(windows)]
        {
            self.install_windows_update(&current_exe, &temp_path, new_version)?;
        }

        #[cfg(not(windows))]
        {
            self.install_unix_update(&current_exe, &temp_path, new_version)?;
        }

        Ok(true)
    }

    /// Downloads a file from a URL to a local path using reqwest.
    fn download_file(&self, url: &str, dest: &Path) -> Result<(), String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("ratterm-updater")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

        let response = client
            .get(url)
            .send()
            .map_err(|e| format!("Failed to download: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Download failed with status {} - release asset may not exist for this platform",
                response.status()
            ));
        }

        let bytes = response
            .bytes()
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        fs::write(dest, &bytes).map_err(|e| format!("Failed to write file: {e}"))?;

        Ok(())
    }

    /// Verifies a downloaded binary is a valid ratterm executable.
    ///
    /// Runs the binary with `--verify` and checks that:
    /// 1. The process exits with code 0
    /// 2. The output contains the expected version and verify token
    ///
    /// Returns Ok(()) if verification passes, Err with details if it fails.
    fn verify_binary(&self, binary_path: &Path, expected_version: &str) -> Result<(), String> {
        assert!(binary_path.exists(), "Binary path must exist");
        assert!(
            !expected_version.is_empty(),
            "Expected version must not be empty"
        );

        let output = Command::new(binary_path)
            .arg("--verify")
            .output()
            .map_err(|e| format!("Failed to execute downloaded binary for verification: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Verification failed: binary exited with {} (stderr: {})",
                output.status,
                stderr.trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        validate_verify_output(stdout.trim(), expected_version)
    }
}

/// Validates the output from a `--verify` invocation.
///
/// Expected format: "ratterm v{version} verify-ok"
/// Returns Ok(()) if the output matches, Err with details if not.
fn validate_verify_output(output: &str, expected_version: &str) -> Result<(), String> {
    let expected = format!(
        "ratterm v{} {}",
        expected_version.trim_start_matches('v'),
        VERIFY_TOKEN
    );

    if output != expected {
        return Err(format!(
            "Verification failed: expected '{}', got '{}'",
            expected, output
        ));
    }

    Ok(())
}

impl Updater {
    /// Installs update on Windows using a helper script.
    /// Windows cannot replace a running executable directly.
    #[cfg(windows)]
    fn install_windows_update(
        &self,
        current_exe: &Path,
        temp_path: &Path,
        new_version: &str,
    ) -> Result<(), String> {
        let script_path = current_exe.with_extension("update.bat");
        let current_exe_str = current_exe.to_string_lossy();
        let temp_path_str = temp_path.to_string_lossy();
        let backup_path = current_exe.with_extension("old");
        let backup_path_str = backup_path.to_string_lossy();

        // Create a batch script that:
        // 1. Waits for the current process to exit
        // 2. Replaces the executable
        // 3. Verifies the new binary works (--verify)
        // 4. If verification fails, reverts to backup
        // 5. If verification passes, starts the new binary
        // 6. Cleans up
        let verify_expected = format!(
            "ratterm v{} {}",
            new_version.trim_start_matches('v'),
            VERIFY_TOKEN
        );
        let script_content = format!(
            r#"@echo off
setlocal EnableDelayedExpansion
echo Updating ratterm to v{new_version}...
set RETRIES=30
:WAIT_LOOP
if !RETRIES! LEQ 0 (
    echo ERROR: Timed out waiting for ratterm to exit.
    goto :REVERT
)
tasklist /FI "PID eq %~1" 2>NUL | find /I /N "%~1" >NUL
if "!ERRORLEVEL!"=="0" (
    timeout /t 1 /nobreak >NUL
    set /a RETRIES=!RETRIES!-1
    goto :WAIT_LOOP
)
echo Process exited, installing update...
if exist "{backup_path_str}" del /f /q "{backup_path_str}"
move /y "{current_exe_str}" "{backup_path_str}"
if errorlevel 1 (
    echo ERROR: Failed to create backup of current binary.
    goto :REVERT
)
move /y "{temp_path_str}" "{current_exe_str}"
if errorlevel 1 (
    echo ERROR: Failed to install new binary.
    goto :REVERT
)
echo Verifying new binary...
for /f "delims=" %%i in ('"{current_exe_str}" --verify 2^>NUL') do set "VERIFY_OUT=%%i"
if "!VERIFY_OUT!" NEQ "{verify_expected}" (
    echo ERROR: Verification failed!
    echo   Expected: {verify_expected}
    echo   Got:      !VERIFY_OUT!
    echo Reverting to previous version...
    del /f /q "{current_exe_str}" 2>NUL
    goto :REVERT
)
echo Verification passed!
del /f /q "{backup_path_str}" 2>NUL
echo Update to v{new_version} complete! Starting ratterm...
start "" "{current_exe_str}"
del /f /q "%~f0"
exit /b 0
:REVERT
echo Restoring previous version...
if exist "{backup_path_str}" (
    if exist "{current_exe_str}" del /f /q "{current_exe_str}"
    move /y "{backup_path_str}" "{current_exe_str}"
    if errorlevel 1 (
        echo CRITICAL: Failed to restore backup! Your binary may be at:
        echo   {backup_path_str}
        echo Please manually rename it to:
        echo   {current_exe_str}
    ) else (
        echo Previous version restored successfully.
    )
)
if exist "{temp_path_str}" del /f /q "{temp_path_str}"
echo.
echo Update failed. Press any key to close.
pause >NUL
del /f /q "%~f0"
exit /b 1
"#
        );

        fs::write(&script_path, &script_content)
            .map_err(|e| format!("Failed to create update script: {e}"))?;

        // Launch the script with current PID as argument
        let pid = std::process::id();
        Command::new("cmd")
            .args([
                "/c",
                "start",
                "/min",
                "",
                script_path.to_str().unwrap_or(""),
                &pid.to_string(),
            ])
            .spawn()
            .map_err(|e| format!("Failed to launch update script: {e}"))?;

        eprintln!("Update prepared. Application will restart automatically...");

        Ok(())
    }

    /// Installs update on Unix systems.
    #[cfg(not(windows))]
    fn install_unix_update(
        &self,
        current_exe: &Path,
        temp_path: &Path,
        new_version: &str,
    ) -> Result<(), String> {
        let backup_path = current_exe.with_extension("old");

        // Backup current executable
        if current_exe.exists() {
            fs::rename(current_exe, &backup_path)
                .map_err(|e| format!("Failed to backup current exe: {e}"))?;
        }

        // Move new executable into place
        fs::rename(temp_path, current_exe).map_err(|e| {
            // Try to restore backup
            let _ = fs::rename(&backup_path, current_exe);
            format!("Failed to install new exe: {e}")
        })?;

        // Remove backup
        let _ = fs::remove_file(&backup_path);

        eprintln!("Updated to v{new_version}.");

        Ok(())
    }

    /// Performs update and triggers application restart.
    /// On Windows, the restart happens automatically via batch script.
    /// On Unix, this returns true to signal the caller to restart.
    pub fn update_and_restart(&self, new_version: &str) -> Result<bool, String> {
        let updated = self.update(new_version)?;

        if !updated {
            return Ok(false);
        }

        // On Windows, the batch script handles restart
        // On Unix, we signal the caller to restart
        Ok(true)
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

/// Restarts the application by spawning a new process and exiting.
/// On Windows, this is handled by the update batch script.
/// On Unix, we exec the new binary directly.
#[allow(clippy::expect_used)] // Fatal error in divergent function is acceptable
pub fn restart_application() -> ! {
    let exe = env::current_exe().expect("Failed to get current executable path");
    let args: Vec<String> = env::args().skip(1).collect();

    eprintln!("Restarting ratterm...");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // exec replaces the current process
        let err = Command::new(&exe).args(&args).exec();
        eprintln!("Failed to restart: {err}");
        std::process::exit(1);
    }

    #[cfg(windows)]
    {
        // On Windows, spawn a new process and exit
        let _ = Command::new(&exe).args(&args).spawn();
        std::process::exit(0);
    }

    #[cfg(not(any(unix, windows)))]
    {
        eprintln!("Restart not supported on this platform. Please restart manually.");
        std::process::exit(0);
    }
}

/// Result of checking for updates at startup.
#[derive(Debug, Clone)]
pub enum StartupUpdateResult {
    /// No update check performed or up to date.
    None,
    /// In dev mode, update available but skipped.
    DevModeUpdateAvailable { current: String, latest: String },
    /// In dev mode, running latest version.
    DevModeUpToDate { current: String },
    /// In dev mode, check failed.
    DevModeCheckFailed { current: String, error: String },
    /// Update available, user declined.
    UpdateAvailable { current: String, latest: String },
    /// Update was performed, need restart.
    UpdatePerformed { version: String },
}

/// Checks for updates and prompts user.
/// Returns the result for the app to display.
pub fn check_for_updates() -> StartupUpdateResult {
    let updater = Updater::new();

    // Skip auto-update prompts in development mode
    if updater.is_dev_mode() {
        match updater.check() {
            UpdateStatus::Available(version) => {
                return StartupUpdateResult::DevModeUpdateAvailable {
                    current: VERSION.to_string(),
                    latest: version,
                };
            }
            UpdateStatus::UpToDate => {
                return StartupUpdateResult::DevModeUpToDate {
                    current: VERSION.to_string(),
                };
            }
            UpdateStatus::Failed(e) => {
                return StartupUpdateResult::DevModeCheckFailed {
                    current: VERSION.to_string(),
                    error: e,
                };
            }
            UpdateStatus::Disabled => {
                return StartupUpdateResult::None;
            }
        }
    }

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
                        match updater.update_and_restart(&version) {
                            Ok(true) => {
                                // Update performed - signal caller to exit/restart
                                return StartupUpdateResult::UpdatePerformed {
                                    version: version.clone(),
                                };
                            }
                            Ok(false) => {
                                // Already up to date - continue normally
                                eprintln!("Continuing with current version...");
                            }
                            Err(e) => {
                                eprintln!("Update failed: {e}");
                                eprintln!("Continuing with current version...");
                            }
                        }
                    }
                }
                // User declined
                StartupUpdateResult::UpdateAvailable {
                    current: VERSION.to_string(),
                    latest: version,
                }
            } else {
                eprintln!("Run 'rat --update' to update.");
                StartupUpdateResult::UpdateAvailable {
                    current: VERSION.to_string(),
                    latest: version,
                }
            }
        }
        UpdateStatus::UpToDate => StartupUpdateResult::None,
        UpdateStatus::Failed(_e) => StartupUpdateResult::None,
        UpdateStatus::Disabled => StartupUpdateResult::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper to create an Updater with a specific version for testing.
    fn test_updater(current_version: &str) -> Updater {
        Updater {
            enabled: true,
            is_dev_mode: false,
            repo: "test/test".to_string(),
            current_version: current_version.to_string(),
        }
    }

    // ── parse_version tests ──

    #[test]
    fn test_parse_version_standard() {
        assert_eq!(parse_version("1.2.3"), (1, 2, 3));
    }

    #[test]
    fn test_parse_version_with_v_prefix() {
        assert_eq!(parse_version("v1.2.3"), (1, 2, 3));
    }

    #[test]
    fn test_parse_version_zero() {
        assert_eq!(parse_version("0.1.0"), (0, 1, 0));
    }

    #[test]
    fn test_parse_version_missing_parts() {
        assert_eq!(parse_version("1"), (1, 0, 0));
        assert_eq!(parse_version("1.2"), (1, 2, 0));
    }

    #[test]
    fn test_parse_version_empty() {
        assert_eq!(parse_version(""), (0, 0, 0));
    }

    // ── is_newer tests ──

    #[test]
    fn test_is_newer_major_bump() {
        let updater = test_updater("0.1.0");
        assert!(updater.is_newer("1.0.0"));
    }

    #[test]
    fn test_is_newer_minor_bump() {
        let updater = test_updater("0.1.0");
        assert!(updater.is_newer("0.2.0"));
    }

    #[test]
    fn test_is_newer_patch_bump() {
        let updater = test_updater("0.1.0");
        assert!(updater.is_newer("0.1.1"));
    }

    #[test]
    fn test_is_newer_same_version() {
        let updater = test_updater("0.1.0");
        assert!(!updater.is_newer("0.1.0"));
    }

    #[test]
    fn test_is_newer_older_version() {
        let updater = test_updater("0.1.0");
        assert!(!updater.is_newer("0.0.9"));
    }

    #[test]
    fn test_is_newer_with_v_prefix() {
        let updater = test_updater("0.1.0");
        assert!(updater.is_newer("v0.2.0"));
    }

    // ── get_asset_name tests ──

    #[test]
    fn test_get_asset_name_format() {
        let updater = test_updater("0.1.0");
        let name = updater.get_asset_name();

        // Should contain "rat-" prefix
        assert!(
            name.starts_with("rat-"),
            "Asset name should start with 'rat-'"
        );

        // Should contain architecture
        assert!(
            name.contains("x86_64") || name.contains("aarch64"),
            "Asset name should contain architecture"
        );

        // Platform-specific checks
        if cfg!(target_os = "windows") {
            assert!(name.ends_with(".exe"), "Windows asset should end with .exe");
            assert!(
                name.contains("windows"),
                "Windows asset should contain 'windows'"
            );
        } else if cfg!(target_os = "macos") {
            assert!(
                !name.ends_with(".exe"),
                "Unix asset should not end with .exe"
            );
            assert!(name.contains("macos"), "macOS asset should contain 'macos'");
        } else {
            assert!(
                !name.ends_with(".exe"),
                "Unix asset should not end with .exe"
            );
            assert!(name.contains("linux"), "Linux asset should contain 'linux'");
        }
    }

    // ── validate_verify_output tests ──

    #[test]
    fn test_validate_verify_output_correct() {
        let result = validate_verify_output("ratterm v0.2.0 verify-ok", "0.2.0");
        assert!(
            result.is_ok(),
            "Correct output should pass: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_validate_verify_output_with_v_prefix() {
        // Expected version has v prefix - should still work
        let result = validate_verify_output("ratterm v0.2.0 verify-ok", "v0.2.0");
        assert!(
            result.is_ok(),
            "Version with v prefix should pass: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_validate_verify_output_wrong_version() {
        let result = validate_verify_output("ratterm v0.1.0 verify-ok", "0.2.0");
        assert!(result.is_err(), "Wrong version should fail");
        let err = result.err().unwrap_or_default();
        assert!(err.contains("Verification failed"), "Error: {err}");
        assert!(
            err.contains("v0.2.0"),
            "Error should mention expected version: {err}"
        );
    }

    #[test]
    fn test_validate_verify_output_missing_token() {
        let result = validate_verify_output("ratterm v0.2.0", "0.2.0");
        assert!(result.is_err(), "Missing verify token should fail");
    }

    #[test]
    fn test_validate_verify_output_wrong_token() {
        let result = validate_verify_output("ratterm v0.2.0 bad-token", "0.2.0");
        assert!(result.is_err(), "Wrong verify token should fail");
    }

    #[test]
    fn test_validate_verify_output_empty() {
        let result = validate_verify_output("", "0.2.0");
        assert!(result.is_err(), "Empty output should fail");
    }

    #[test]
    fn test_validate_verify_output_garbage() {
        let result = validate_verify_output("<!DOCTYPE html>", "0.2.0");
        assert!(result.is_err(), "HTML content should fail");
    }

    #[test]
    fn test_validate_verify_output_wrong_binary_name() {
        let result = validate_verify_output("othertool v0.2.0 verify-ok", "0.2.0");
        assert!(result.is_err(), "Wrong binary name should fail");
    }

    // ── verify_binary integration tests ──

    #[test]
    fn test_verify_binary_nonexistent_binary_panics() {
        let updater = test_updater("0.1.0");
        let fake_path = Path::new("this_binary_does_not_exist_12345");

        // Should panic because of the assert
        let result = std::panic::catch_unwind(|| updater.verify_binary(fake_path, "0.1.0"));
        assert!(
            result.is_err(),
            "Should panic on nonexistent binary (assertion)"
        );
    }

    #[allow(clippy::expect_used)] // expect is acceptable in test setup code
    #[test]
    fn test_verify_binary_invalid_executable() {
        // Create a temporary file that is not a valid executable
        let temp_dir = env::temp_dir();
        let fake_exe = temp_dir.join("ratterm_test_fake_binary.exe");
        {
            let mut f = fs::File::create(&fake_exe).expect("test setup: create temp file");
            f.write_all(b"this is not an executable")
                .expect("test setup: write to temp file");
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&fake_exe)
                .expect("test setup: get metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&fake_exe, perms).expect("test setup: set permissions");
        }

        let updater = test_updater("0.1.0");
        let result = updater.verify_binary(&fake_exe, "0.1.0");

        // Clean up
        let _ = fs::remove_file(&fake_exe);

        assert!(
            result.is_err(),
            "Invalid executable should fail verification"
        );
    }

    // ── VERIFY_TOKEN constant test ──

    #[test]
    fn test_verify_token_is_stable() {
        // The verify token must remain stable across versions
        assert_eq!(VERIFY_TOKEN, "verify-ok");
    }

    // ── UpdateStatus tests ──

    #[test]
    fn test_check_returns_disabled_when_not_enabled() {
        let updater = Updater {
            enabled: false,
            is_dev_mode: false,
            repo: "test/test".to_string(),
            current_version: "0.1.0".to_string(),
        };
        assert!(
            matches!(updater.check(), UpdateStatus::Disabled),
            "Should return Disabled when not enabled"
        );
    }
}
