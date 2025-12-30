//! Docker container and image discovery via CLI.
//!
//! Discovers running containers and available images by executing
//! Docker CLI commands and parsing JSON output.

use super::container::{DockerContainer, DockerImage};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Maximum number of items to parse from Docker output.
const MAX_PARSE_ITEMS: usize = 500;

/// Timeout for Docker commands in milliseconds.
const COMMAND_TIMEOUT_MS: u64 = 2000;

/// Quick timeout for availability check in milliseconds.
const QUICK_TIMEOUT_MS: u64 = 1000;

/// Poll interval for checking if process completed.
const POLL_INTERVAL_MS: u64 = 50;

/// Docker availability status.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DockerAvailability {
    /// Docker status not yet checked.
    #[default]
    Unknown,
    /// Docker CLI is not installed on the system.
    NotInstalled,
    /// Docker is installed but the daemon is not running.
    NotRunning,
    /// Docker daemon returned an error (API issue, etc.).
    DaemonError(String),
    /// Docker is available and running.
    Available,
}

impl DockerAvailability {
    /// Returns true if Docker is available and running.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Returns true if Docker is installed (but may not be running).
    #[must_use]
    pub fn is_installed(&self) -> bool {
        matches!(
            self,
            Self::Available | Self::NotRunning | Self::DaemonError(_)
        )
    }

    /// Returns the error message if there's a daemon error.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::DaemonError(msg) => Some(msg.as_str()),
            _ => None,
        }
    }
}

/// Result of Docker discovery operation.
#[derive(Debug, Clone, Default)]
pub struct DockerDiscoveryResult {
    /// Running containers.
    pub running_containers: Vec<DockerContainer>,
    /// Stopped containers.
    pub stopped_containers: Vec<DockerContainer>,
    /// Available images.
    pub images: Vec<DockerImage>,
    /// Whether Docker CLI is available.
    pub docker_available: bool,
    /// Docker availability status (more detailed).
    pub availability: DockerAvailability,
    /// Error message if discovery failed.
    pub error: Option<String>,
}

impl DockerDiscoveryResult {
    /// Creates a result indicating Docker is not installed.
    #[must_use]
    pub fn not_installed() -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::NotInstalled,
            error: Some("Docker is not installed on this system.".to_string()),
            ..Default::default()
        }
    }

    /// Creates a result indicating Docker is not running.
    #[must_use]
    pub fn not_running() -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::NotRunning,
            error: Some("Docker is not currently running.".to_string()),
            ..Default::default()
        }
    }

    /// Creates a result indicating Docker daemon has an error.
    #[must_use]
    pub fn daemon_error(error: String) -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::DaemonError(error.clone()),
            error: Some(error),
            ..Default::default()
        }
    }

    /// Creates a result indicating Docker is not available.
    #[must_use]
    pub fn not_available(error: String) -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::Unknown,
            error: Some(error),
            ..Default::default()
        }
    }

    /// Returns total count of all items.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.running_containers.len() + self.stopped_containers.len() + self.images.len()
    }

    /// Returns true if any containers or images were found.
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.running_containers.is_empty()
            || !self.stopped_containers.is_empty()
            || !self.images.is_empty()
    }
}

/// Docker discovery service.
pub struct DockerDiscovery;

impl DockerDiscovery {
    /// Returns the docker command name for the current platform.
    fn docker_cmd() -> &'static str {
        if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        }
    }

    /// Runs a command with a timeout.
    /// Returns None if the command times out or fails to start.
    /// Properly kills the process if it times out.
    fn run_with_timeout(cmd: &mut Command, timeout_ms: u64) -> Option<Output> {
        // Configure to capture output
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Spawn the process
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => return None,
        };

        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);

        // Poll for completion with timeout
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process completed - collect output
                    let stdout = child
                        .stdout
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();

                    let stderr = child
                        .stderr
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();

                    return Some(Output {
                        status,
                        stdout,
                        stderr,
                    });
                }
                Ok(None) => {
                    // Still running - check timeout
                    if start.elapsed() >= timeout {
                        // Timeout - kill the process
                        let _ = child.kill();
                        let _ = child.wait(); // Reap the zombie
                        return None;
                    }
                    // Sleep briefly before polling again
                    std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                }
                Err(_) => {
                    // Error checking status
                    let _ = child.kill();
                    return None;
                }
            }
        }
    }

    /// Checks if Docker CLI is available on the system.
    #[must_use]
    pub fn is_docker_available() -> bool {
        let mut cmd = Command::new(Self::docker_cmd());
        cmd.arg("version")
            .arg("--format")
            .arg("{{.Server.Version}}");

        Self::run_with_timeout(&mut cmd, QUICK_TIMEOUT_MS)
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Returns the Docker version string, or None if not available.
    #[must_use]
    pub fn docker_version() -> Option<String> {
        let mut cmd = Command::new(Self::docker_cmd());
        cmd.arg("version")
            .arg("--format")
            .arg("{{.Server.Version}}");

        let output = Self::run_with_timeout(&mut cmd, QUICK_TIMEOUT_MS)?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if version.is_empty() {
                None
            } else {
                Some(version)
            }
        } else {
            None
        }
    }

    /// Checks Docker availability with detailed status.
    #[must_use]
    pub fn check_availability() -> DockerAvailability {
        // First check if docker CLI exists by trying to get version (quick check)
        let mut cmd = Command::new(Self::docker_cmd());
        cmd.arg("--version");

        let cli_exists = Self::run_with_timeout(&mut cmd, QUICK_TIMEOUT_MS)
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !cli_exists {
            return DockerAvailability::NotInstalled;
        }

        // CLI exists, now check if daemon is running using a simple ps command
        // This is faster than 'docker info' which can be slow
        let mut cmd = Command::new(Self::docker_cmd());
        cmd.args(["ps", "-q", "--no-trunc"]);

        match Self::run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS) {
            Some(output) if output.status.success() => DockerAvailability::Available,
            Some(output) => {
                // Command failed - analyze the error
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stderr_trimmed = stderr.trim();

                // Check for common error patterns
                if stderr_trimmed.contains("Cannot connect to the Docker daemon")
                    || stderr_trimmed.contains("Is the docker daemon running")
                    || stderr_trimmed.contains("docker daemon is not running")
                {
                    DockerAvailability::NotRunning
                } else if !stderr_trimmed.is_empty() {
                    // Docker returned an error (API error, internal error, etc.)
                    // Truncate long error messages
                    let msg = if stderr_trimmed.len() > 200 {
                        format!("{}...", &stderr_trimmed[..200])
                    } else {
                        stderr_trimmed.to_string()
                    };
                    DockerAvailability::DaemonError(msg)
                } else {
                    DockerAvailability::NotRunning
                }
            }
            None => {
                // Command timed out - daemon likely not responding
                DockerAvailability::DaemonError("Docker command timed out".to_string())
            }
        }
    }

    /// Starts Docker Desktop (Windows/macOS).
    /// Returns Ok(()) if started successfully, Err with message otherwise.
    pub fn start_docker_desktop() -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            // Try to start Docker Desktop on Windows
            let result = Command::new("cmd")
                .args(["/C", "start", "", "Docker Desktop"])
                .spawn();

            match result {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to start Docker Desktop: {}", e)),
            }
        }

        #[cfg(target_os = "macos")]
        {
            let result = Command::new("open").args(["-a", "Docker"]).spawn();

            match result {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to start Docker Desktop: {}", e)),
            }
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, Docker typically runs as a service
            let result = Command::new("systemctl").args(["start", "docker"]).output();

            match result {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("Failed to start Docker service: {}", stderr))
                }
                Err(e) => Err(format!("Failed to start Docker service: {}", e)),
            }
        }
    }

    /// Discovers all running containers.
    ///
    /// # Returns
    /// Ok with list of running containers, or Err with error message.
    pub fn discover_running_containers() -> Result<Vec<DockerContainer>, String> {
        let mut cmd = Command::new(Self::docker_cmd());
        cmd.args(["ps", "--format", "{{json .}}"]);

        let output = Self::run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            .ok_or_else(|| "Docker command timed out".to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker ps failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers = Self::parse_containers_json(&stdout, true);

        Ok(containers)
    }

    /// Discovers all containers (including stopped).
    ///
    /// # Returns
    /// Tuple of (running containers, stopped containers).
    pub fn discover_all_containers() -> Result<(Vec<DockerContainer>, Vec<DockerContainer>), String>
    {
        let mut cmd = Command::new(Self::docker_cmd());
        cmd.args(["ps", "-a", "--format", "{{json .}}"]);

        let output = Self::run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            .ok_or_else(|| "Docker command timed out".to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker ps -a failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let all_containers = Self::parse_containers_json(&stdout, false);

        let mut running = Vec::new();
        let mut stopped = Vec::new();

        for container in all_containers {
            if container.status.is_running() {
                running.push(container);
            } else {
                stopped.push(container);
            }
        }

        Ok((running, stopped))
    }

    /// Discovers all available images.
    pub fn discover_images() -> Result<Vec<DockerImage>, String> {
        let mut cmd = Command::new(Self::docker_cmd());
        cmd.args(["images", "--format", "{{json .}}"]);

        let output = Self::run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            .ok_or_else(|| "Docker command timed out".to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker images failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let images = Self::parse_images_json(&stdout);

        Ok(images)
    }

    /// Performs full discovery of containers and images.
    #[must_use]
    pub fn discover_all() -> DockerDiscoveryResult {
        // Check Docker availability first with detailed status
        let availability = Self::check_availability();

        match &availability {
            DockerAvailability::NotInstalled => {
                return DockerDiscoveryResult::not_installed();
            }
            DockerAvailability::NotRunning => {
                return DockerDiscoveryResult::not_running();
            }
            DockerAvailability::DaemonError(msg) => {
                return DockerDiscoveryResult::daemon_error(msg.clone());
            }
            DockerAvailability::Unknown => {
                return DockerDiscoveryResult::not_available(
                    "Unable to determine Docker status.".to_string(),
                );
            }
            DockerAvailability::Available => {
                // Continue with discovery
            }
        }

        let mut result = DockerDiscoveryResult {
            docker_available: true,
            availability: DockerAvailability::Available,
            ..Default::default()
        };

        // Discover containers
        match Self::discover_all_containers() {
            Ok((running, stopped)) => {
                result.running_containers = running;
                result.stopped_containers = stopped;
            }
            Err(e) => {
                result.error = Some(format!("Container discovery failed: {}", e));
            }
        }

        // Discover images (continue even if container discovery failed)
        match Self::discover_images() {
            Ok(images) => {
                result.images = images;
            }
            Err(e) => {
                let err_msg = format!("Image discovery failed: {}", e);
                if let Some(ref mut existing) = result.error {
                    existing.push_str("; ");
                    existing.push_str(&err_msg);
                } else {
                    result.error = Some(err_msg);
                }
            }
        }

        result
    }

    /// Parses JSON output from `docker ps --format {{json .}}`.
    fn parse_containers_json(output: &str, running_only: bool) -> Vec<DockerContainer> {
        let mut containers = Vec::new();
        let mut parse_count = 0;

        for line in output.lines() {
            if parse_count >= MAX_PARSE_ITEMS {
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(container) = Self::parse_container_line(line) {
                if !running_only || container.status.is_running() {
                    containers.push(container);
                    parse_count += 1;
                }
            }
        }

        containers
    }

    /// Parses a single JSON line from docker ps output.
    fn parse_container_line(json_line: &str) -> Option<DockerContainer> {
        // Simple JSON parsing without external crate
        // Expected format: {"ID":"abc123","Names":"my-container",...}

        let id = Self::extract_json_field(json_line, "ID")?;
        let names = Self::extract_json_field(json_line, "Names").unwrap_or_default();
        let image = Self::extract_json_field(json_line, "Image").unwrap_or_default();
        let status = Self::extract_json_field(json_line, "Status").unwrap_or_default();
        let ports = Self::extract_json_field(json_line, "Ports").unwrap_or_default();
        let created = Self::extract_json_field(json_line, "CreatedAt").unwrap_or_default();

        // Clean up names (remove leading slashes)
        let name = names.trim_start_matches('/').to_string();

        let mut container = DockerContainer::new(id, name, image, status);
        container.created = created;

        // Parse ports (comma-separated)
        if !ports.is_empty() {
            container.ports = ports
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
        }

        Some(container)
    }

    /// Parses JSON output from `docker images --format {{json .}}`.
    fn parse_images_json(output: &str) -> Vec<DockerImage> {
        let mut images = Vec::new();
        let mut parse_count = 0;

        for line in output.lines() {
            if parse_count >= MAX_PARSE_ITEMS {
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(image) = Self::parse_image_line(line) {
                // Skip intermediate images without repository
                if image.repository != "<none>" {
                    images.push(image);
                    parse_count += 1;
                }
            }
        }

        images
    }

    /// Parses a single JSON line from docker images output.
    fn parse_image_line(json_line: &str) -> Option<DockerImage> {
        let id = Self::extract_json_field(json_line, "ID")?;
        let repository = Self::extract_json_field(json_line, "Repository").unwrap_or_default();
        let tag = Self::extract_json_field(json_line, "Tag").unwrap_or_default();
        let size = Self::extract_json_field(json_line, "Size").unwrap_or_default();
        let created = Self::extract_json_field(json_line, "CreatedAt").unwrap_or_default();

        let mut image = DockerImage::new(id, repository, tag);
        image.size = size;
        image.created = created;

        Some(image)
    }

    /// Extracts a field value from a JSON object string.
    ///
    /// Handles simple JSON format: {"field":"value",...}
    fn extract_json_field(json: &str, field: &str) -> Option<String> {
        // Look for "field":"value" pattern
        let pattern = format!("\"{}\":\"", field);
        let start = json.find(&pattern)?;
        let value_start = start + pattern.len();

        // Find the closing quote (handle escaped quotes)
        let rest = &json[value_start..];
        let mut value_end = 0;
        let chars = rest.chars().peekable();
        let mut in_escape = false;

        for c in chars {
            if in_escape {
                in_escape = false;
                value_end += c.len_utf8();
            } else if c == '\\' {
                in_escape = true;
                value_end += 1;
            } else if c == '"' {
                break;
            } else {
                value_end += c.len_utf8();
            }

            // Safety bound
            if value_end > 1000 {
                break;
            }
        }

        let value = &rest[..value_end];

        // Unescape common sequences
        let unescaped = value
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
            .replace("\\/", "/")
            .replace("\\n", "\n")
            .replace("\\t", "\t");

        Some(unescaped)
    }

    /// Starts a container and returns its ID.
    pub fn start_container(container_id: &str) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let output = Command::new(cmd)
            .args(["start", container_id])
            .output()
            .map_err(|e| format!("Failed to start container: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker start failed: {}", stderr));
        }

        Ok(())
    }

    /// Stops a container.
    pub fn stop_container(container_id: &str) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let output = Command::new(cmd)
            .args(["stop", container_id])
            .output()
            .map_err(|e| format!("Failed to stop container: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker stop failed: {}", stderr));
        }

        Ok(())
    }

    /// Removes a container.
    pub fn remove_container(container_id: &str, force: bool) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let mut args = vec!["rm"];
        if force {
            args.push("-f");
        }
        args.push(container_id);

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to remove container: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker rm failed: {}", stderr));
        }

        Ok(())
    }

    /// Removes an image.
    pub fn remove_image(image_id: &str, force: bool) -> Result<(), String> {
        assert!(!image_id.is_empty(), "image_id must not be empty");

        let cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let mut args = vec!["rmi"];
        if force {
            args.push("-f");
        }
        args.push(image_id);

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to remove image: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker rmi failed: {}", stderr));
        }

        Ok(())
    }

    /// Builds the command to exec into a container.
    #[must_use]
    pub fn build_exec_command(container_id: &str, shell: &str) -> String {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        assert!(!shell.is_empty(), "shell must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        format!("{} exec -it {} {}", docker_cmd, container_id, shell)
    }

    /// Builds the command to run an image as a new container.
    #[must_use]
    pub fn build_run_command(image: &str, shell: &str) -> String {
        assert!(!image.is_empty(), "image must not be empty");
        assert!(!shell.is_empty(), "shell must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        format!("{} run -it --rm {} {}", docker_cmd, image, shell)
    }

    /// Builds the command to show container stats.
    #[must_use]
    pub fn build_stats_command(container_id: &str) -> String {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        format!("{} stats {}", docker_cmd, container_id)
    }

    /// Builds the command to show container logs.
    #[must_use]
    pub fn build_logs_command(container_id: &str) -> String {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        format!("{} logs -f {}", docker_cmd, container_id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_field() {
        let json = r#"{"ID":"abc123","Names":"/my-container","Image":"nginx:latest"}"#;

        assert_eq!(
            DockerDiscovery::extract_json_field(json, "ID"),
            Some("abc123".to_string())
        );
        assert_eq!(
            DockerDiscovery::extract_json_field(json, "Names"),
            Some("/my-container".to_string())
        );
        assert_eq!(
            DockerDiscovery::extract_json_field(json, "Image"),
            Some("nginx:latest".to_string())
        );
        assert_eq!(DockerDiscovery::extract_json_field(json, "Missing"), None);
    }

    #[test]
    fn test_parse_container_line() {
        let json = r#"{"ID":"abc123def456","Names":"/my-nginx","Image":"nginx:latest","Status":"Up 5 minutes","Ports":"0.0.0.0:8080->80/tcp","CreatedAt":"2024-01-01 10:00:00"}"#;

        let container = DockerDiscovery::parse_container_line(json).unwrap();

        assert_eq!(container.id, "abc123def456");
        assert_eq!(container.name, "my-nginx");
        assert_eq!(container.image, "nginx:latest");
        assert!(container.is_running());
        assert_eq!(container.ports.len(), 1);
        assert_eq!(container.ports[0], "0.0.0.0:8080->80/tcp");
    }

    #[test]
    fn test_parse_image_line() {
        let json = r#"{"ID":"sha256:abc123","Repository":"nginx","Tag":"latest","Size":"150MB","CreatedAt":"2024-01-01"}"#;

        let image = DockerDiscovery::parse_image_line(json).unwrap();

        assert_eq!(image.id, "sha256:abc123");
        assert_eq!(image.repository, "nginx");
        assert_eq!(image.tag, "latest");
        assert_eq!(image.size, "150MB");
        assert_eq!(image.full_name(), "nginx:latest");
    }

    #[test]
    fn test_build_exec_command() {
        let cmd = DockerDiscovery::build_exec_command("abc123", "/bin/bash");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("exec -it"));
        assert!(cmd.contains("abc123"));
        assert!(cmd.contains("/bin/bash"));
    }

    #[test]
    fn test_build_run_command() {
        let cmd = DockerDiscovery::build_run_command("nginx:latest", "/bin/sh");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("run -it --rm"));
        assert!(cmd.contains("nginx:latest"));
        assert!(cmd.contains("/bin/sh"));
    }

    #[test]
    fn test_build_stats_command() {
        let cmd = DockerDiscovery::build_stats_command("abc123");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("stats"));
        assert!(cmd.contains("abc123"));
    }

    #[test]
    fn test_build_logs_command() {
        let cmd = DockerDiscovery::build_logs_command("abc123");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("logs -f"));
        assert!(cmd.contains("abc123"));
    }

    #[test]
    fn test_discovery_result_not_available() {
        let result = DockerDiscoveryResult::not_available("Docker not found".to_string());
        assert!(!result.docker_available);
        assert!(result.error.is_some());
        assert!(!result.has_items());
    }
}
