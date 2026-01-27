//! Daemon deployment to remote SSH hosts.
//!
//! Uses SFTP/SSH to deploy, manage, and stop the metrics
//! collection daemon on remote Linux hosts.

use tracing::{debug, error, info, warn};

use super::script::{deploy_daemon_command, status_daemon_command, stop_daemon_command};
use super::types::DaemonError;
use crate::remote::SftpClient;

/// Result of a daemon deployment check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonStatus {
    /// Daemon is running with the given PID.
    Running(u32),
    /// Daemon is not running.
    NotRunning,
    /// Could not determine status.
    Unknown,
}

/// Deploys and manages daemons on remote hosts.
pub struct DaemonDeployer;

impl DaemonDeployer {
    /// Deploys the daemon to a remote host.
    ///
    /// This will:
    /// 1. Write the daemon script to ~/.ratterm/daemon.sh
    /// 2. Make it executable
    /// 3. Start it in the background with nohup
    ///
    /// # Arguments
    /// * `sftp` - Connected SFTP client
    /// * `host_id` - Unique identifier for this host (passed to daemon as HOST_ID env var)
    ///
    /// # Errors
    /// Returns error if deployment fails.
    pub fn deploy(sftp: &SftpClient, host_id: u32) -> Result<(), DaemonError> {
        assert!(host_id > 0, "host_id must be positive");

        info!(
            "Deploying daemon to {} (host_id={})",
            sftp.context().hostname,
            host_id
        );

        // Check if daemon is already running
        match Self::status(sftp)? {
            DaemonStatus::Running(pid) => {
                info!(
                    "Daemon already running on {} with PID {}",
                    sftp.context().hostname,
                    pid
                );
                return Ok(());
            }
            DaemonStatus::NotRunning => {
                debug!("No existing daemon found, proceeding with deployment");
            }
            DaemonStatus::Unknown => {
                warn!("Could not determine daemon status, attempting deployment anyway");
            }
        }

        // Deploy and start the daemon
        let deploy_cmd = deploy_daemon_command(host_id);
        debug!("Deploy command length: {} bytes", deploy_cmd.len());

        let output = sftp
            .exec_command(&deploy_cmd)
            .map_err(|e| DaemonError::DeployFailed(e.to_string()))?;

        debug!("Deploy output: {}", output);

        if output.contains("DAEMON_STARTED_") {
            // Extract PID from output
            if let Some(pid_str) = output
                .split("DAEMON_STARTED_")
                .nth(1)
                .and_then(|s| s.lines().next())
            {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    info!(
                        "Daemon deployed successfully to {} with PID {}",
                        sftp.context().hostname,
                        pid
                    );
                    return Ok(());
                }
            }
            // Even if we can't parse PID, deployment likely succeeded
            info!("Daemon deployed to {}", sftp.context().hostname);
            return Ok(());
        }

        // Verify by checking status
        std::thread::sleep(std::time::Duration::from_millis(500));

        match Self::status(sftp)? {
            DaemonStatus::Running(pid) => {
                info!(
                    "Daemon verified running on {} with PID {}",
                    sftp.context().hostname,
                    pid
                );
                Ok(())
            }
            DaemonStatus::NotRunning => {
                error!(
                    "Daemon failed to start on {}. Output: {}",
                    sftp.context().hostname,
                    output
                );
                Err(DaemonError::DeployFailed(
                    "Daemon did not start after deployment".to_string(),
                ))
            }
            DaemonStatus::Unknown => {
                warn!("Could not verify daemon status after deployment");
                Ok(()) // Assume success
            }
        }
    }

    /// Stops the daemon on a remote host.
    ///
    /// # Errors
    /// Returns error if the stop command fails.
    pub fn stop(sftp: &SftpClient) -> Result<(), DaemonError> {
        info!("Stopping daemon on {}", sftp.context().hostname);

        let output = sftp
            .exec_command(stop_daemon_command())
            .map_err(|e| DaemonError::StopFailed(e.to_string()))?;

        debug!("Stop output: {}", output);

        if output.contains("DAEMON_STOPPED") {
            info!("Daemon stopped on {}", sftp.context().hostname);
            Ok(())
        } else if output.contains("DAEMON_NOT_FOUND") {
            debug!("No daemon was running on {}", sftp.context().hostname);
            Ok(())
        } else {
            warn!("Unexpected stop output: {}", output);
            Ok(()) // Don't fail, the daemon might be stopped anyway
        }
    }

    /// Checks if the daemon is running on a remote host.
    ///
    /// # Errors
    /// Returns error if the status check fails.
    pub fn status(sftp: &SftpClient) -> Result<DaemonStatus, DaemonError> {
        debug!("Checking daemon status on {}", sftp.context().hostname);

        let output = sftp
            .exec_command(status_daemon_command())
            .map_err(|e| DaemonError::StatusCheckFailed(e.to_string()))?;

        debug!("Status output: {}", output);

        if output.contains("DAEMON_RUNNING") {
            // Try to extract PID from the first line
            if let Some(pid_line) = output.lines().next() {
                if let Ok(pid) = pid_line.trim().parse::<u32>() {
                    return Ok(DaemonStatus::Running(pid));
                }
            }
            // Running but couldn't get PID
            Ok(DaemonStatus::Running(0))
        } else if output.contains("DAEMON_NOT_RUNNING") {
            Ok(DaemonStatus::NotRunning)
        } else {
            Ok(DaemonStatus::Unknown)
        }
    }

    /// Checks if the daemon is running (simplified boolean check).
    ///
    /// # Errors
    /// Returns error if the status check fails.
    pub fn is_running(sftp: &SftpClient) -> Result<bool, DaemonError> {
        match Self::status(sftp)? {
            DaemonStatus::Running(_) => Ok(true),
            DaemonStatus::NotRunning | DaemonStatus::Unknown => Ok(false),
        }
    }

    /// Restarts the daemon on a remote host.
    ///
    /// Stops any existing daemon and deploys a fresh instance.
    ///
    /// # Errors
    /// Returns error if restart fails.
    pub fn restart(sftp: &SftpClient, host_id: u32) -> Result<(), DaemonError> {
        info!("Restarting daemon on {}", sftp.context().hostname);

        // Stop existing daemon (ignore errors)
        let _ = Self::stop(sftp);

        // Wait for process to fully terminate
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Deploy fresh
        Self::deploy(sftp, host_id)
    }

    /// Gets the daemon logs from a remote host.
    ///
    /// # Errors
    /// Returns error if log retrieval fails.
    pub fn get_logs(sftp: &SftpClient) -> Result<String, DaemonError> {
        sftp.exec_command("tail -100 ~/.ratterm/daemon.log 2>/dev/null || echo 'NO_LOGS'")
            .map_err(|e| DaemonError::StatusCheckFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_status_enum() {
        assert_eq!(DaemonStatus::Running(123), DaemonStatus::Running(123));
        assert_ne!(DaemonStatus::Running(1), DaemonStatus::NotRunning);
        assert_ne!(DaemonStatus::NotRunning, DaemonStatus::Unknown);
    }

    #[test]
    fn test_daemon_status_debug() {
        let status = DaemonStatus::Running(12345);
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("Running"));
        assert!(debug_str.contains("12345"));
    }
}
