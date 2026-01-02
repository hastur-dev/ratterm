//! Docker API layer for programmatic access and testing.
//!
//! Provides a simple API for remote Docker management that can be used
//! for testing, extensions, and CLI tools.
//!
//! # Host Management API
//!
//! The `DockerHostManager` provides methods to directly set and test Docker hosts,
//! bypassing the UI for testing and automation purposes.

use super::container::{DockerContainer, DockerHost, DockerImage, DockerItemList};
use super::discovery::{DockerDiscovery, DockerDiscoveryResult};
use std::process::{Command, Stdio};

/// Host manager for programmatic Docker host manipulation.
///
/// This allows setting the Docker host directly without going through the UI,
/// useful for testing, automation, and debugging.
pub struct DockerHostManager<'a> {
    items: &'a mut DockerItemList,
}

impl<'a> DockerHostManager<'a> {
    /// Creates a new host manager for the given item list.
    #[must_use]
    pub fn new(items: &'a mut DockerItemList) -> Self {
        Self { items }
    }

    /// Gets the currently selected host.
    #[must_use]
    pub fn current_host(&self) -> &DockerHost {
        &self.items.selected_host
    }

    /// Sets the host to local Docker.
    pub fn set_local(&mut self) {
        self.items.set_selected_host(DockerHost::Local);
    }

    /// Sets the host to a remote Docker via SSH (without password).
    ///
    /// This is useful for SSH key-based authentication.
    pub fn set_remote(&mut self, host_id: u32, hostname: &str, port: u16, username: &str) {
        let host = DockerHost::remote(
            host_id,
            hostname.to_string(),
            port,
            username.to_string(),
            None,
        );
        self.items.set_selected_host(host);
    }

    /// Sets the host to a remote Docker via SSH with password.
    ///
    /// This is the full authentication method for password-based SSH.
    pub fn set_remote_with_password(
        &mut self,
        host_id: u32,
        hostname: &str,
        port: u16,
        username: &str,
        password: &str,
        display_name: Option<&str>,
    ) {
        let host = DockerHost::remote_with_password(
            host_id,
            hostname.to_string(),
            port,
            username.to_string(),
            display_name.map(String::from),
            Some(password.to_string()),
        );
        self.items.set_selected_host(host);
    }

    /// Tests the current host configuration by attempting discovery.
    ///
    /// Returns diagnostic information about the host.
    pub fn test_current_host(&self) -> Vec<String> {
        let mut results = Vec::new();
        let host = &self.items.selected_host;

        results.push("=== Testing Current Docker Host ===".to_string());
        results.push(format!(
            "Host Type: {}",
            if host.is_local() { "Local" } else { "Remote" }
        ));
        results.push(format!("Display Name: {}", host.display_name()));

        match host {
            DockerHost::Local => {
                results.push("Storage Key: local".to_string());
            }
            DockerHost::Remote {
                host_id,
                hostname,
                port,
                username,
                password,
                ..
            } => {
                results.push(format!("Host ID: {}", host_id));
                results.push(format!("Hostname: {}", hostname));
                results.push(format!("Port: {}", port));
                results.push(format!("Username: {}", username));
                results.push(format!("Has Password: {}", password.is_some()));
                results.push(format!("Storage Key: {}", host.storage_key()));

                if let Some(ssh_args) = host.ssh_args() {
                    results.push(format!("SSH Args: {:?}", ssh_args));
                }
            }
        }

        results.push("\n--- Discovery Test ---".to_string());
        let discovery_result = DockerDiscovery::discover_all_for_host(host);

        results.push(format!(
            "Docker Available: {}",
            discovery_result.docker_available
        ));
        results.push(format!("Availability: {:?}", discovery_result.availability));
        results.push(format!(
            "Running Containers: {}",
            discovery_result.running_containers.len()
        ));
        results.push(format!(
            "Stopped Containers: {}",
            discovery_result.stopped_containers.len()
        ));
        results.push(format!("Images: {}", discovery_result.images.len()));

        if let Some(err) = discovery_result.error {
            results.push(format!("Error: {}", err));
        }

        results.push("=== Test Complete ===".to_string());
        results
    }

    /// Returns detailed debug info about the host configuration.
    #[must_use]
    pub fn debug_info(&self) -> String {
        let host = &self.items.selected_host;
        match host {
            DockerHost::Local => "DockerHost::Local".to_string(),
            DockerHost::Remote {
                host_id,
                hostname,
                port,
                username,
                display_name,
                password,
            } => {
                format!(
                    "DockerHost::Remote {{ host_id: {}, hostname: {}, port: {}, username: {}, display_name: {:?}, has_password: {} }}",
                    host_id,
                    hostname,
                    port,
                    username,
                    display_name,
                    password.is_some()
                )
            }
        }
    }
}

/// Docker API for programmatic access to Docker management.
pub struct DockerApi;

impl DockerApi {
    /// Tests SSH connectivity to a remote host.
    ///
    /// Returns Ok(()) if SSH connection succeeds, Err with message otherwise.
    /// Note: This requires SSH key authentication (no password prompt).
    pub fn test_ssh_connection(
        hostname: &str,
        port: u16,
        username: &str,
    ) -> Result<String, String> {
        let ssh_cmd = if cfg!(target_os = "windows") {
            "ssh.exe"
        } else {
            "ssh"
        };

        let mut cmd = Command::new(ssh_cmd);

        // Add port if not default
        if port != 22 {
            cmd.arg("-p").arg(port.to_string());
        }

        // Add connection timeout and options
        cmd.arg("-o").arg("ConnectTimeout=10");
        cmd.arg("-o").arg("StrictHostKeyChecking=no");
        cmd.arg("-o").arg("UserKnownHostsFile=/dev/null");
        // Note: BatchMode=yes disables password prompts, so we don't use it
        // This allows SSH agent or key-based auth to work

        // User@host
        cmd.arg(format!("{}@{}", username, hostname));

        // Simple command to test connection
        cmd.arg("echo").arg("SSH_OK");

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute SSH: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().contains("SSH_OK") {
                Ok(format!(
                    "SSH connection to {}@{}:{} successful",
                    username, hostname, port
                ))
            } else {
                Ok(format!("SSH connected, output: {}", stdout.trim()))
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "SSH failed: {}. Make sure you have SSH key auth set up or use the main app with password.",
                stderr.trim()
            ))
        }
    }

    /// Tests if Docker is available on a remote host.
    ///
    /// Returns Ok with Docker version if available, Err otherwise.
    pub fn test_remote_docker(hostname: &str, port: u16, username: &str) -> Result<String, String> {
        let host = DockerHost::remote(
            0, // Temporary ID
            hostname.to_string(),
            port,
            username.to_string(),
            None,
        );

        let available = DockerDiscovery::is_docker_available_remote(&host);
        if available {
            // Try to get Docker version
            let version_cmd =
                DockerDiscovery::build_remote_docker_command(&host, "docker --version");

            let output = Command::new(if cfg!(target_os = "windows") {
                "cmd"
            } else {
                "sh"
            })
            .args(if cfg!(target_os = "windows") {
                vec!["/C", &version_cmd]
            } else {
                vec!["-c", &version_cmd]
            })
            .output()
            .map_err(|e| format!("Failed to get Docker version: {}", e))?;

            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                Ok(format!(
                    "Docker available on {}@{}: {}",
                    username,
                    hostname,
                    version.trim()
                ))
            } else {
                Ok(format!(
                    "Docker available on {}@{} (version check failed)",
                    username, hostname
                ))
            }
        } else {
            Err(format!("Docker not available on {}@{}", username, hostname))
        }
    }

    /// Lists containers on a remote host.
    ///
    /// Returns a list of containers (running and stopped).
    pub fn list_remote_containers(
        hostname: &str,
        port: u16,
        username: &str,
    ) -> Result<Vec<DockerContainer>, String> {
        let host = DockerHost::remote(0, hostname.to_string(), port, username.to_string(), None);

        let result = DockerDiscovery::discover_all_for_host(&host);

        if !result.docker_available {
            return Err(result
                .error
                .unwrap_or_else(|| "Docker not available".to_string()));
        }

        let mut all_containers = result.running_containers;
        all_containers.extend(result.stopped_containers);
        Ok(all_containers)
    }

    /// Lists images on a remote host.
    pub fn list_remote_images(
        hostname: &str,
        port: u16,
        username: &str,
    ) -> Result<Vec<DockerImage>, String> {
        let host = DockerHost::remote(0, hostname.to_string(), port, username.to_string(), None);

        let result = DockerDiscovery::discover_all_for_host(&host);

        if !result.docker_available {
            return Err(result
                .error
                .unwrap_or_else(|| "Docker not available".to_string()));
        }

        Ok(result.images)
    }

    /// Performs full discovery on a remote host.
    pub fn discover_remote(hostname: &str, port: u16, username: &str) -> DockerDiscoveryResult {
        let host = DockerHost::remote(0, hostname.to_string(), port, username.to_string(), None);

        DockerDiscovery::discover_all_for_host(&host)
    }

    /// Builds an SSH command for executing Docker commands on a remote host.
    ///
    /// This is useful for debugging - you can see exactly what command would be run.
    pub fn build_ssh_command(
        hostname: &str,
        port: u16,
        username: &str,
        docker_command: &str,
    ) -> String {
        let host = DockerHost::remote(0, hostname.to_string(), port, username.to_string(), None);

        DockerDiscovery::build_remote_docker_command(&host, docker_command)
    }

    /// Executes a raw Docker command on a remote host.
    ///
    /// Returns (stdout, stderr, exit_code).
    pub fn exec_remote_docker(
        hostname: &str,
        port: u16,
        username: &str,
        docker_args: &str,
    ) -> Result<(String, String, i32), String> {
        let host = DockerHost::remote(0, hostname.to_string(), port, username.to_string(), None);

        let full_cmd = DockerDiscovery::build_remote_docker_command(&host, docker_args);

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", &full_cmd]).output()
        } else {
            Command::new("sh").args(["-c", &full_cmd]).output()
        };

        let output = output.map_err(|e| format!("Failed to execute command: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }

    /// Returns diagnostic information about remote Docker connectivity.
    pub fn diagnose_remote(hostname: &str, port: u16, username: &str) -> Vec<String> {
        let mut results = Vec::new();

        results.push(format!(
            "=== Diagnosing Docker on {}@{}:{} ===",
            username, hostname, port
        ));

        // Test 1: SSH connectivity
        results.push("\n[1] Testing SSH connectivity...".to_string());
        match Self::test_ssh_connection(hostname, port, username) {
            Ok(msg) => results.push(format!("    ✓ {}", msg)),
            Err(msg) => {
                results.push(format!("    ✗ {}", msg));
                results.push("    → Cannot proceed without SSH connection".to_string());
                return results;
            }
        }

        // Test 2: Show the SSH command that would be used
        results.push("\n[2] SSH command format:".to_string());
        let cmd = Self::build_ssh_command(hostname, port, username, "docker ps");
        results.push(format!("    {}", cmd));

        // Test 3: Docker availability
        results.push("\n[3] Testing Docker availability...".to_string());
        match Self::test_remote_docker(hostname, port, username) {
            Ok(msg) => results.push(format!("    ✓ {}", msg)),
            Err(msg) => {
                results.push(format!("    ✗ {}", msg));
                results.push("    → Docker may not be installed or not in PATH".to_string());
                return results;
            }
        }

        // Test 4: List containers
        results.push("\n[4] Discovering containers...".to_string());
        match Self::list_remote_containers(hostname, port, username) {
            Ok(containers) => {
                results.push(format!("    ✓ Found {} containers", containers.len()));
                for c in containers.iter().take(5) {
                    let status = if c.is_running() { "running" } else { "stopped" };
                    results.push(format!("      - {} ({}) [{}]", c.name, c.image, status));
                }
                if containers.len() > 5 {
                    results.push(format!("      ... and {} more", containers.len() - 5));
                }
            }
            Err(msg) => results.push(format!("    ✗ {}", msg)),
        }

        // Test 5: List images
        results.push("\n[5] Discovering images...".to_string());
        match Self::list_remote_images(hostname, port, username) {
            Ok(images) => {
                results.push(format!("    ✓ Found {} images", images.len()));
                for img in images.iter().take(5) {
                    results.push(format!("      - {}:{}", img.repository, img.tag));
                }
                if images.len() > 5 {
                    results.push(format!("      ... and {} more", images.len() - 5));
                }
            }
            Err(msg) => results.push(format!("    ✗ {}", msg)),
        }

        results.push("\n=== Diagnosis complete ===".to_string());
        results
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ssh_command() {
        let cmd = DockerApi::build_ssh_command("example.com", 22, "user", "docker ps");
        assert!(cmd.contains("ssh"));
        assert!(cmd.contains("user@example.com"));
        assert!(cmd.contains("docker ps"));
    }

    #[test]
    fn test_build_ssh_command_custom_port() {
        let cmd = DockerApi::build_ssh_command("example.com", 2222, "admin", "docker images");
        assert!(cmd.contains("ssh"));
        assert!(cmd.contains("-p 2222"));
        assert!(cmd.contains("admin@example.com"));
        assert!(cmd.contains("docker images"));
    }

    #[test]
    fn test_discovery_with_password() {
        // Test that discovery works with a host that has a password
        let host = DockerHost::remote_with_password(
            11,
            "10.0.0.18".to_string(),
            22,
            "hastur".to_string(),
            Some("Desk Rock5c".to_string()),
            Some("Im408839!".to_string()),
        );

        // Verify password is set
        assert!(host.password().is_some());
        assert_eq!(host.password().unwrap(), "Im408839!");

        // Build the command and verify it generates a valid SSH command
        let cmd = DockerDiscovery::build_remote_docker_command(&host, "docker ps");
        println!("Generated command: {}", cmd);

        // On Windows with plink available, should use plink with -pw
        // On Windows without plink, falls back to ssh.exe
        // On Linux/Mac with sshpass, should use sshpass
        // On Linux/Mac without sshpass, falls back to ssh
        if cfg!(target_os = "windows") {
            if cmd.contains("plink") {
                // Plink is available - verify password flag is used
                assert!(cmd.contains("-pw"), "Expected -pw flag in plink command");
                assert!(
                    cmd.contains("hastur@10.0.0.18"),
                    "Expected user@host in command"
                );
            } else {
                // Plink not available - should fall back to ssh.exe
                assert!(
                    cmd.contains("ssh"),
                    "Expected ssh fallback when plink unavailable"
                );
                assert!(
                    cmd.contains("hastur@10.0.0.18"),
                    "Expected user@host in command"
                );
            }
        } else {
            // Linux/Mac - should use ssh or sshpass
            assert!(
                cmd.contains("ssh") || cmd.contains("sshpass"),
                "Expected ssh or sshpass in command"
            );
        }
    }

    #[test]
    #[ignore] // This test requires network access to 10.0.0.18
    fn test_real_discovery_desk_rock5c() {
        // Integration test: discovers Docker containers from a remote host via SSH/plink
        let host = DockerHost::remote_with_password(
            11,
            "10.0.0.18".to_string(),
            22,
            "hastur".to_string(),
            Some("Desk Rock5c".to_string()),
            Some("Im408839!".to_string()),
        );

        // Verify SSH auth tools are available
        assert!(
            DockerDiscovery::check_ssh_auth_tools().is_ok(),
            "SSH auth tools (plink on Windows, sshpass on Linux) must be available"
        );

        let result = DockerDiscovery::discover_all_for_host(&host);

        assert!(
            result.docker_available,
            "Docker should be available on Desk Rock5c"
        );
        assert!(
            !result.running_containers.is_empty(),
            "Should find at least one running container"
        );

        // Verify we found the expected container
        let web_server = result
            .running_containers
            .iter()
            .find(|c| c.name == "web-server");
        assert!(
            web_server.is_some(),
            "Should find the 'web-server' container"
        );
    }
}
