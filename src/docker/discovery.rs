//! Docker container and image discovery via CLI.
//!
//! Discovers running containers and available images by executing
//! Docker CLI commands and parsing JSON output. Supports both local
//! Docker and remote Docker via SSH.

use super::container::{DockerContainer, DockerHost, DockerImage, DockerSearchResult};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Maximum number of items to parse from Docker output.
const MAX_PARSE_ITEMS: usize = 500;

/// Timeout for Docker commands in milliseconds.
const COMMAND_TIMEOUT_MS: u64 = 2000;

/// Timeout for remote Docker commands in milliseconds (longer for SSH overhead).
const REMOTE_TIMEOUT_MS: u64 = 5000;

/// Quick timeout for availability check in milliseconds.
const QUICK_TIMEOUT_MS: u64 = 1000;

/// Quick timeout for remote availability check in milliseconds.
const REMOTE_QUICK_TIMEOUT_MS: u64 = 3000;

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

/// Builds an `ExitStatus` from a raw exit code.
///
/// `std::process::ExitStatus` has no portable constructor, but a remote command
/// still has an exit code, and the discovery code is written against
/// `std::process::Output`.
fn exit_status_from(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        // A Unix wait status puts the exit code in the high byte.
        std::process::ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
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

    /// Runs a Docker command on a remote host through the pooled SSH session.
    ///
    /// This used to build a shell string and spawn `ssh`, `sshpass` or
    /// `plink` per call, which put the password in the process list, disabled
    /// host-key checking, and paid for a TCP and authentication handshake
    /// every time. The command now travels over an already-authenticated
    /// session; `timeout_ms` is unused because the session carries its own
    /// I/O timeout.
    fn run_remote_with_timeout(
        host: &DockerHost,
        docker_args: &[&str],
        _timeout_ms: u64,
    ) -> Option<Output> {
        let host_id = host.host_id()?;

        // Quote arguments the remote shell would otherwise mangle, such as
        // the `{{json .}}` format strings.
        let quoted_args: Vec<String> = docker_args
            .iter()
            .map(|arg| {
                if arg.contains('{') || arg.contains('}') || arg.contains(' ') {
                    format!("'{}'", arg.replace('\'', "'\\''"))
                } else {
                    (*arg).to_string()
                }
            })
            .collect();
        let docker_cmd = format!("docker {}", quoted_args.join(" "));

        match crate::remote::exec_on_host(host_id, &docker_cmd) {
            Ok(result) => Some(Output {
                status: exit_status_from(result.exit_status),
                stdout: result.stdout.into_bytes(),
                stderr: result.stderr.into_bytes(),
            }),
            Err(e) => {
                tracing::warn!("remote docker command failed on host {host_id}: {e}");
                None
            }
        }
    }

    /// Checks if Docker is available on a remote host via SSH.
    #[must_use]
    pub fn is_docker_available_remote(host: &DockerHost) -> bool {
        assert!(host.is_remote(), "host must be remote");

        Self::run_remote_with_timeout(
            host,
            &["version", "--format", "{{.Server.Version}}"],
            REMOTE_QUICK_TIMEOUT_MS,
        )
        .map(|o| o.status.success())
        .unwrap_or(false)
    }

    /// Checks Docker availability on a specific host.
    #[must_use]
    pub fn check_availability_for_host(host: &DockerHost) -> DockerAvailability {
        match host {
            DockerHost::Local => Self::check_availability(),
            DockerHost::Remote { .. } => Self::check_availability_remote(host),
        }
    }

    /// Checks Docker availability on a remote host via SSH.
    #[must_use]
    pub fn check_availability_remote(host: &DockerHost) -> DockerAvailability {
        assert!(host.is_remote(), "host must be remote");

        // Check if SSH connection works and docker CLI exists
        let output = Self::run_remote_with_timeout(host, &["--version"], REMOTE_QUICK_TIMEOUT_MS);

        let cli_exists = output.as_ref().map(|o| o.status.success()).unwrap_or(false);

        if !cli_exists {
            // Could be SSH failure or docker not installed
            let err_msg = output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                .unwrap_or_else(|| "SSH connection failed or timed out".to_string());

            if err_msg.contains("not found") || err_msg.contains("command not found") {
                return DockerAvailability::NotInstalled;
            }
            return DockerAvailability::DaemonError(err_msg);
        }

        // Docker CLI exists, check if daemon is running
        let output =
            Self::run_remote_with_timeout(host, &["ps", "-q", "--no-trunc"], REMOTE_TIMEOUT_MS);

        match output {
            Some(o) if o.status.success() => DockerAvailability::Available,
            Some(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let stderr_trimmed = stderr.trim();

                if stderr_trimmed.contains("Cannot connect to the Docker daemon")
                    || stderr_trimmed.contains("Is the docker daemon running")
                {
                    DockerAvailability::NotRunning
                } else if !stderr_trimmed.is_empty() {
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
            None => DockerAvailability::DaemonError("Remote command timed out".to_string()),
        }
    }

    /// Performs discovery on a specific host (local or remote).
    #[must_use]
    pub fn discover_all_for_host(host: &DockerHost) -> DockerDiscoveryResult {
        match host {
            DockerHost::Local => Self::discover_all(),
            DockerHost::Remote { .. } => Self::discover_all_remote(host),
        }
    }

    /// Performs full discovery on a remote host via SSH.
    #[must_use]
    pub fn discover_all_remote(host: &DockerHost) -> DockerDiscoveryResult {
        assert!(host.is_remote(), "host must be remote");

        // Check availability first
        let availability = Self::check_availability_remote(host);

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
                    "Unable to determine Docker status on remote host.".to_string(),
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
        match Self::discover_all_containers_remote(host) {
            Ok((running, stopped)) => {
                result.running_containers = running;
                result.stopped_containers = stopped;
            }
            Err(e) => {
                result.error = Some(format!("Container discovery failed: {}", e));
            }
        }

        // Discover images
        match Self::discover_images_remote(host) {
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

    /// Discovers all containers on a remote host.
    pub fn discover_all_containers_remote(
        host: &DockerHost,
    ) -> Result<(Vec<DockerContainer>, Vec<DockerContainer>), String> {
        assert!(host.is_remote(), "host must be remote");

        let output = Self::run_remote_with_timeout(
            host,
            &["ps", "-a", "--format", "{{json .}}"],
            REMOTE_TIMEOUT_MS,
        )
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

    /// Discovers all images on a remote host.
    pub fn discover_images_remote(host: &DockerHost) -> Result<Vec<DockerImage>, String> {
        assert!(host.is_remote(), "host must be remote");

        let output = Self::run_remote_with_timeout(
            host,
            &["images", "--format", "{{json .}}"],
            REMOTE_TIMEOUT_MS,
        )
        .ok_or_else(|| "Docker command timed out".to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker images failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let images = Self::parse_images_json(&stdout);

        Ok(images)
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

            if let Some(container) = Self::parse_container_line(line)
                && (!running_only || container.status.is_running())
            {
                containers.push(container);
                parse_count += 1;
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
        Self::start_container_on_host(container_id, &DockerHost::Local)
    }

    /// Starts a container on the specified host.
    pub fn start_container_on_host(container_id: &str, host: &DockerHost) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        match host {
            DockerHost::Local => {
                let output = Command::new(docker_cmd)
                    .args(["start", container_id])
                    .output()
                    .map_err(|e| format!("Failed to start container: {}", e))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker start failed: {}", stderr));
                }
            }
            DockerHost::Remote { .. } => {
                let output = Self::run_remote_with_timeout(
                    host,
                    &["start", container_id],
                    REMOTE_TIMEOUT_MS,
                )
                .ok_or("Remote command timed out")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker start failed: {}", stderr));
                }
            }
        }

        Ok(())
    }

    /// Stops a container (local only - use `stop_container_on_host` for remote).
    pub fn stop_container(container_id: &str) -> Result<(), String> {
        Self::stop_container_on_host(container_id, &DockerHost::Local)
    }

    /// Stops a container on the specified host.
    pub fn stop_container_on_host(container_id: &str, host: &DockerHost) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        match host {
            DockerHost::Local => {
                let output = Command::new(docker_cmd)
                    .args(["stop", container_id])
                    .output()
                    .map_err(|e| format!("Failed to stop container: {}", e))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker stop failed: {}", stderr));
                }
            }
            DockerHost::Remote { .. } => {
                let output =
                    Self::run_remote_with_timeout(host, &["stop", container_id], REMOTE_TIMEOUT_MS)
                        .ok_or("Remote command timed out")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker stop failed: {}", stderr));
                }
            }
        }

        Ok(())
    }

    /// Removes a container (local only - use `remove_container_on_host` for remote).
    pub fn remove_container(container_id: &str, force: bool) -> Result<(), String> {
        Self::remove_container_on_host(container_id, force, &DockerHost::Local)
    }

    /// Removes a container on the specified host.
    pub fn remove_container_on_host(
        container_id: &str,
        force: bool,
        host: &DockerHost,
    ) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let mut args = vec!["rm"];
        if force {
            args.push("-f");
        }
        args.push(container_id);

        match host {
            DockerHost::Local => {
                let output = Command::new(docker_cmd)
                    .args(&args)
                    .output()
                    .map_err(|e| format!("Failed to remove container: {}", e))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker rm failed: {}", stderr));
                }
            }
            DockerHost::Remote { .. } => {
                let args_str: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
                let output = Self::run_remote_with_timeout(host, &args_str, REMOTE_TIMEOUT_MS)
                    .ok_or("Remote command timed out")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker rm failed: {}", stderr));
                }
            }
        }

        Ok(())
    }

    /// Removes an image (local only - use `remove_image_on_host` for remote).
    pub fn remove_image(image_id: &str, force: bool) -> Result<(), String> {
        Self::remove_image_on_host(image_id, force, &DockerHost::Local)
    }

    /// Removes an image on the specified host.
    pub fn remove_image_on_host(
        image_id: &str,
        force: bool,
        host: &DockerHost,
    ) -> Result<(), String> {
        assert!(!image_id.is_empty(), "image_id must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let mut args = vec!["rmi"];
        if force {
            args.push("-f");
        }
        args.push(image_id);

        match host {
            DockerHost::Local => {
                let output = Command::new(docker_cmd)
                    .args(&args)
                    .output()
                    .map_err(|e| format!("Failed to remove image: {}", e))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker rmi failed: {}", stderr));
                }
            }
            DockerHost::Remote { .. } => {
                let args_str: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
                let output = Self::run_remote_with_timeout(host, &args_str, REMOTE_TIMEOUT_MS)
                    .ok_or("Remote command timed out")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("docker rmi failed: {}", stderr));
                }
            }
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

    // =========================================================================
    // Docker Hub Search and Image Management
    // =========================================================================

    /// Timeout for image pull operations (10 minutes).
    const PULL_TIMEOUT_MS: u64 = 600_000;

    /// Searches Docker Hub for images matching the given term.
    ///
    /// Executes `docker search` on the specified host (local or remote via SSH).
    ///
    /// # Arguments
    /// * `host` - The Docker host (local or remote)
    /// * `search_term` - The term to search for
    /// * `limit` - Maximum number of results (capped at 100)
    ///
    /// # Returns
    /// `Ok(Vec<DockerSearchResult>)` on success, `Err(String)` on failure.
    pub fn search_docker_hub(
        host: &DockerHost,
        search_term: &str,
        limit: usize,
    ) -> Result<Vec<DockerSearchResult>, String> {
        assert!(!search_term.is_empty(), "search_term must not be empty");

        let limit_capped = limit.min(100);
        let limit_arg = format!("--limit={}", limit_capped);
        let format_arg = "--format={{json .}}";

        tracing::info!(
            "Searching Docker Hub for '{}' with limit {} on {:?}",
            search_term,
            limit_capped,
            host
        );

        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(Self::docker_cmd());
                cmd.args(["search", &limit_arg, format_arg, search_term]);
                Self::run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => Self::run_remote_with_timeout(
                host,
                &["search", &limit_arg, format_arg, search_term],
                REMOTE_TIMEOUT_MS,
            ),
        };

        let output = output.ok_or_else(|| "Docker search command timed out".to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("docker search failed: {}", stderr);
            return Err(format!("docker search failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = Self::parse_search_results(&stdout);

        tracing::info!("Found {} search results", results.len());
        Ok(results)
    }

    /// Parses Docker Hub search results from JSON output.
    fn parse_search_results(output: &str) -> Vec<DockerSearchResult> {
        let mut results = Vec::new();

        for line in output.lines().take(MAX_PARSE_ITEMS) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(result) = Self::parse_search_line(line) {
                results.push(result);
            }
        }

        results
    }

    /// Parses a single search result JSON line.
    fn parse_search_line(json_line: &str) -> Option<DockerSearchResult> {
        // Extract fields from JSON
        let name = Self::extract_json_field(json_line, "Name")?;

        let description = Self::extract_json_field(json_line, "Description").unwrap_or_default();

        // StarCount can be a number or string
        let stars_str = Self::extract_json_field(json_line, "StarCount").unwrap_or_default();
        let stars = stars_str.parse::<u32>().unwrap_or(0);

        // IsOfficial can be "[OK]" or "true" or empty
        let official = Self::extract_json_field(json_line, "IsOfficial")
            .map(|s| s == "[OK]" || s.to_lowercase() == "true")
            .unwrap_or(false);

        Some(DockerSearchResult {
            name,
            description,
            stars,
            official,
        })
    }

    /// Checks if an image exists on the specified host.
    ///
    /// Executes `docker images -q <image>` - returns true if output is non-empty.
    ///
    /// # Arguments
    /// * `host` - The Docker host (local or remote)
    /// * `image_name` - The image name to check (e.g., "nginx", "ubuntu:20.04")
    ///
    /// # Returns
    /// `Ok(true)` if image exists, `Ok(false)` if not, `Err(String)` on failure.
    pub fn image_exists_on_host(host: &DockerHost, image_name: &str) -> Result<bool, String> {
        assert!(!image_name.is_empty(), "image_name must not be empty");

        tracing::info!("Checking if image '{}' exists on {:?}", image_name, host);

        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(Self::docker_cmd());
                cmd.args(["images", "-q", image_name]);
                Self::run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => Self::run_remote_with_timeout(
                host,
                &["images", "-q", image_name],
                REMOTE_TIMEOUT_MS,
            ),
        };

        let output = output.ok_or_else(|| "Docker images command timed out".to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("docker images check failed: {}", stderr);
            return Err(format!("docker images check failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let exists = !stdout.trim().is_empty();

        tracing::info!("Image '{}' exists: {}", image_name, exists);
        Ok(exists)
    }

    /// Pulls an image on the specified host.
    ///
    /// Executes `docker pull <image>` with a 10-minute timeout for large images.
    ///
    /// # Arguments
    /// * `host` - The Docker host (local or remote)
    /// * `image_name` - The image to pull (e.g., "nginx:latest", "ubuntu")
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(String)` on failure.
    pub fn pull_image_on_host(host: &DockerHost, image_name: &str) -> Result<(), String> {
        assert!(!image_name.is_empty(), "image_name must not be empty");

        tracing::info!("Pulling image '{}' on {:?}", image_name, host);

        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(Self::docker_cmd());
                cmd.args(["pull", image_name]);
                Self::run_with_timeout(&mut cmd, Self::PULL_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => {
                Self::run_remote_with_timeout(host, &["pull", image_name], Self::PULL_TIMEOUT_MS)
            }
        };

        let output = output.ok_or_else(|| {
            "Image pull timed out (10 minute limit). The image may be very large.".to_string()
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("docker pull failed: {}", stderr);
            return Err(format!("docker pull failed: {}", stderr.trim()));
        }

        tracing::info!("Successfully pulled image '{}'", image_name);
        Ok(())
    }

    /// Builds the docker run command for container creation.
    ///
    /// Creates an interactive container with optional volume mounts and startup command.
    ///
    /// # Arguments
    /// * `image` - The image to run
    /// * `volume_mounts` - Volume mounts as "host:container" strings
    /// * `startup_command` - Optional command to run (appended at end)
    #[must_use]
    pub fn build_create_container_command(
        image: &str,
        volume_mounts: &[String],
        startup_command: Option<&str>,
    ) -> String {
        assert!(!image.is_empty(), "image must not be empty");

        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let mut parts = vec![
            docker_cmd.to_string(),
            "run".to_string(),
            "-it".to_string(),
            "--rm".to_string(),
        ];

        // Add volume mounts
        for mount in volume_mounts {
            parts.push("-v".to_string());
            parts.push(mount.clone());
        }

        // Add image
        parts.push(image.to_string());

        // Add startup command if provided
        if let Some(cmd) = startup_command {
            let cmd_trimmed = cmd.trim();
            if !cmd_trimmed.is_empty() {
                for arg in cmd_trimmed.split_whitespace() {
                    parts.push(arg.to_string());
                }
            }
        }

        parts.join(" ")
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

    #[test]
    fn a_remote_command_on_an_unregistered_host_reports_failure() {
        // No target has been published for this id, so the executor refuses
        // rather than reaching for a subprocess. Before this change the same
        // call would have spawned `ssh` and hung on a password prompt.
        let host = DockerHost::remote(876_543);
        let output = DockerDiscovery::run_remote_with_timeout(&host, &["ps"], 1000);
        assert!(output.is_none());
    }

    #[test]
    fn a_local_host_has_no_remote_path() {
        let output = DockerDiscovery::run_remote_with_timeout(&DockerHost::Local, &["ps"], 1000);
        assert!(output.is_none());
    }

    #[test]
    fn exit_statuses_round_trip_through_the_conversion() {
        assert!(exit_status_from(0).success());
        assert!(!exit_status_from(1).success());
        assert_eq!(exit_status_from(3).code(), Some(3));
    }
}
