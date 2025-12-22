//! Extension process manager.
//!
//! Manages the lifecycle of API extension processes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::extension::manifest::{ExtensionManifest, ProcessConfig};

/// Status of an extension process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    /// Process is starting.
    Starting,
    /// Process is running.
    Running,
    /// Process has stopped.
    Stopped,
    /// Process crashed.
    Crashed,
    /// Process failed to start.
    Failed,
}

/// An extension process.
pub struct ExtensionProcess {
    /// Extension name.
    pub name: String,
    /// Extension version.
    pub version: String,
    /// Process handle.
    process: Option<Child>,
    /// Process status.
    status: ProcessStatus,
    /// Number of restarts.
    restart_count: u32,
    /// Last restart time.
    last_restart: Option<Instant>,
    /// Extension directory.
    ext_dir: PathBuf,
    /// Process configuration.
    config: ProcessConfig,
}

impl ExtensionProcess {
    /// Creates a new extension process (not yet started).
    fn new(name: String, version: String, ext_dir: PathBuf, config: ProcessConfig) -> Self {
        Self {
            name,
            version,
            process: None,
            status: ProcessStatus::Stopped,
            restart_count: 0,
            last_restart: None,
            ext_dir,
            config,
        }
    }

    /// Starts the extension process.
    fn start(&mut self, api_url: &str, api_token: &str) -> Result<(), String> {
        if self.process.is_some() {
            return Err("Process already running".to_string());
        }

        self.status = ProcessStatus::Starting;

        // Expand placeholders in command and args
        let ext_dir_str = self.ext_dir.to_string_lossy();
        let command = self.config.command.replace("{ext_dir}", &ext_dir_str);
        let args: Vec<String> = self
            .config
            .args
            .iter()
            .map(|arg| arg.replace("{ext_dir}", &ext_dir_str))
            .collect();

        // Determine working directory
        let cwd = self
            .config
            .cwd
            .as_ref()
            .map(|c| c.replace("{ext_dir}", &ext_dir_str))
            .unwrap_or_else(|| ext_dir_str.to_string());

        // Build command
        let mut cmd = Command::new(&command);
        cmd.args(&args);
        cmd.current_dir(&cwd);

        // Set environment variables
        cmd.env("RATTERM_API_URL", api_url);
        cmd.env("RATTERM_API_TOKEN", api_token);
        cmd.env("RATTERM_EXTENSION_NAME", &self.name);
        cmd.env("RATTERM_EXTENSION_DIR", &*ext_dir_str);

        // Add custom environment variables from config
        for (key, value) in &self.config.env {
            let expanded = value.replace("{ext_dir}", &ext_dir_str);
            cmd.env(key, expanded);
        }

        // Don't inherit stdin, capture stdout/stderr for logging
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn the process
        match cmd.spawn() {
            Ok(child) => {
                self.process = Some(child);
                self.status = ProcessStatus::Running;
                self.last_restart = Some(Instant::now());
                tracing::info!("Started extension process: {}", self.name);
                Ok(())
            }
            Err(e) => {
                self.status = ProcessStatus::Failed;
                Err(format!("Failed to start process: {}", e))
            }
        }
    }

    /// Stops the extension process.
    fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            // Try to kill the process
            let _ = child.kill();
            let _ = child.wait();
            self.status = ProcessStatus::Stopped;
            tracing::info!("Stopped extension process: {}", self.name);
        }
    }

    /// Checks if the process is still running.
    fn check_status(&mut self) -> ProcessStatus {
        if let Some(ref mut child) = self.process {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process has exited
                    self.process = None;
                    self.status = if status.success() {
                        ProcessStatus::Stopped
                    } else {
                        ProcessStatus::Crashed
                    };
                }
                Ok(None) => {
                    // Still running
                    self.status = ProcessStatus::Running;
                }
                Err(_) => {
                    self.status = ProcessStatus::Failed;
                }
            }
        }
        self.status
    }

    /// Returns whether the process should be restarted.
    fn should_restart(&self) -> bool {
        if !self.config.restart_on_crash {
            return false;
        }

        if self.restart_count >= self.config.max_restarts {
            return false;
        }

        matches!(self.status, ProcessStatus::Crashed | ProcessStatus::Failed)
    }

    /// Returns the restart delay.
    fn restart_delay(&self) -> Duration {
        // Exponential backoff
        let base_delay = Duration::from_millis(self.config.restart_delay_ms);
        base_delay * 2u32.saturating_pow(self.restart_count)
    }

    /// Returns the current status.
    pub fn status(&self) -> ProcessStatus {
        self.status
    }
}

impl Drop for ExtensionProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Manager for API extension processes.
pub struct ApiExtensionManager {
    /// Extension processes by name.
    processes: HashMap<String, ExtensionProcess>,
    /// API URL for extensions to connect to.
    api_url: String,
    /// API token for authentication.
    api_token: String,
}

impl ApiExtensionManager {
    /// Creates a new API extension manager.
    #[must_use]
    pub fn new(api_url: String, api_token: String) -> Self {
        Self {
            processes: HashMap::new(),
            api_url,
            api_token,
        }
    }

    /// Loads an API extension from a manifest.
    pub fn load(&mut self, manifest: &ExtensionManifest, ext_dir: &Path) -> Result<(), String> {
        let Some(ref process_config) = manifest.process else {
            return Err("Missing [process] section in manifest".to_string());
        };

        let name = manifest.extension.name.clone();
        let version = manifest.extension.version.clone();

        // Check if already loaded
        if self.processes.contains_key(&name) {
            return Err(format!("Extension '{}' already loaded", name));
        }

        // Create process entry
        let mut process = ExtensionProcess::new(
            name.clone(),
            version,
            ext_dir.to_path_buf(),
            process_config.clone(),
        );

        // Start the process
        process.start(&self.api_url, &self.api_token)?;

        self.processes.insert(name, process);
        Ok(())
    }

    /// Unloads an extension.
    pub fn unload(&mut self, name: &str) -> Result<(), String> {
        let mut process = self
            .processes
            .remove(name)
            .ok_or_else(|| format!("Extension '{}' not loaded", name))?;

        process.stop();
        Ok(())
    }

    /// Updates all processes (check status, restart if needed).
    pub fn update(&mut self) {
        let names: Vec<String> = self.processes.keys().cloned().collect();

        for name in names {
            if let Some(process) = self.processes.get_mut(&name) {
                process.check_status();

                if process.should_restart() {
                    // Check restart delay
                    let should_restart = process
                        .last_restart
                        .map(|t| t.elapsed() >= process.restart_delay())
                        .unwrap_or(true);

                    if should_restart {
                        tracing::info!("Restarting extension: {}", name);
                        process.restart_count += 1;
                        let _ = process.start(&self.api_url, &self.api_token);
                    }
                }
            }
        }
    }

    /// Returns the number of loaded extensions.
    #[must_use]
    pub fn count(&self) -> usize {
        self.processes.len()
    }

    /// Returns the names of all loaded extensions.
    #[must_use]
    pub fn loaded_extensions(&self) -> Vec<&str> {
        self.processes.keys().map(String::as_str).collect()
    }

    /// Returns the status of a specific extension.
    #[must_use]
    pub fn get_status(&self, name: &str) -> Option<ProcessStatus> {
        self.processes.get(name).map(|p| p.status())
    }

    /// Stops all extensions.
    pub fn stop_all(&mut self) {
        for process in self.processes.values_mut() {
            process.stop();
        }
    }
}

impl Drop for ApiExtensionManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_status() {
        assert_eq!(ProcessStatus::Running, ProcessStatus::Running);
        assert_ne!(ProcessStatus::Running, ProcessStatus::Stopped);
    }

    #[test]
    fn test_manager_new() {
        let manager = ApiExtensionManager::new(
            "http://127.0.0.1:7878".to_string(),
            "test-token".to_string(),
        );
        assert_eq!(manager.count(), 0);
    }
}
