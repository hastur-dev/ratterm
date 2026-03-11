//! Optional log-forwarding daemon for Docker containers.
//!
//! Provides a `LogDaemonManager` that can deploy a log-forwarding helper
//! process via `docker exec`. The system works without the daemon using
//! `docker logs --follow` as a fallback.

/// Status of the log daemon for a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    /// Daemon has not been installed for this container.
    NotInstalled,
    /// Daemon is being installed.
    Installing,
    /// Daemon is running and forwarding logs.
    Running,
    /// Daemon installation or execution failed.
    Failed,
    /// User declined daemon installation.
    UserDeclined,
}

impl DaemonStatus {
    /// Returns a display label for the status.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::NotInstalled => "Not Installed",
            Self::Installing => "Installing...",
            Self::Running => "Running",
            Self::Failed => "Failed",
            Self::UserDeclined => "Declined",
        }
    }

    /// Returns true if the daemon is in a terminal state.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Running | Self::Failed | Self::UserDeclined)
    }

    /// Returns true if the daemon is actively running.
    #[must_use]
    pub fn is_running(self) -> bool {
        self == Self::Running
    }
}

/// Manages log daemon deployment for containers.
///
/// The daemon approach is optional — the system falls back to
/// `docker logs --follow` (via bollard) when no daemon is present.
#[derive(Debug, Clone)]
pub struct LogDaemonManager {
    /// Per-container daemon status.
    statuses: Vec<(String, DaemonStatus)>,
}

impl LogDaemonManager {
    /// Creates a new daemon manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            statuses: Vec::new(),
        }
    }

    /// Returns the daemon status for a container.
    #[must_use]
    pub fn status(&self, container_id: &str) -> DaemonStatus {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        self.statuses
            .iter()
            .find(|(id, _)| id == container_id)
            .map(|(_, s)| *s)
            .unwrap_or(DaemonStatus::NotInstalled)
    }

    /// Sets the daemon status for a container.
    pub fn set_status(&mut self, container_id: &str, status: DaemonStatus) {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        if let Some(entry) = self.statuses.iter_mut().find(|(id, _)| id == container_id)
        {
            entry.1 = status;
        } else {
            self.statuses
                .push((container_id.to_string(), status));
        }
    }

    /// Records that the user declined daemon installation.
    pub fn decline(&mut self, container_id: &str) {
        self.set_status(container_id, DaemonStatus::UserDeclined);
    }

    /// Returns the number of containers with active daemons.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.statuses
            .iter()
            .filter(|(_, s)| *s == DaemonStatus::Running)
            .count()
    }

    /// Clears all daemon statuses.
    pub fn clear(&mut self) {
        self.statuses.clear();
    }
}

impl Default for LogDaemonManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_status_is_not_installed() {
        let manager = LogDaemonManager::new();
        assert_eq!(manager.status("abc"), DaemonStatus::NotInstalled);
    }

    #[test]
    fn test_set_and_get_status() {
        let mut manager = LogDaemonManager::new();
        manager.set_status("c1", DaemonStatus::Installing);
        assert_eq!(manager.status("c1"), DaemonStatus::Installing);

        manager.set_status("c1", DaemonStatus::Running);
        assert_eq!(manager.status("c1"), DaemonStatus::Running);
    }

    #[test]
    fn test_multiple_containers() {
        let mut manager = LogDaemonManager::new();
        manager.set_status("c1", DaemonStatus::Running);
        manager.set_status("c2", DaemonStatus::Failed);
        manager.set_status("c3", DaemonStatus::UserDeclined);

        assert_eq!(manager.status("c1"), DaemonStatus::Running);
        assert_eq!(manager.status("c2"), DaemonStatus::Failed);
        assert_eq!(manager.status("c3"), DaemonStatus::UserDeclined);
    }

    #[test]
    fn test_decline() {
        let mut manager = LogDaemonManager::new();
        manager.decline("c1");
        assert_eq!(manager.status("c1"), DaemonStatus::UserDeclined);
    }

    #[test]
    fn test_active_count() {
        let mut manager = LogDaemonManager::new();
        assert_eq!(manager.active_count(), 0);

        manager.set_status("c1", DaemonStatus::Running);
        manager.set_status("c2", DaemonStatus::Failed);
        manager.set_status("c3", DaemonStatus::Running);
        assert_eq!(manager.active_count(), 2);
    }

    #[test]
    fn test_clear() {
        let mut manager = LogDaemonManager::new();
        manager.set_status("c1", DaemonStatus::Running);
        manager.set_status("c2", DaemonStatus::Running);
        assert_eq!(manager.active_count(), 2);

        manager.clear();
        assert_eq!(manager.active_count(), 0);
        assert_eq!(manager.status("c1"), DaemonStatus::NotInstalled);
    }

    #[test]
    fn test_daemon_status_labels() {
        assert_eq!(DaemonStatus::NotInstalled.label(), "Not Installed");
        assert_eq!(DaemonStatus::Installing.label(), "Installing...");
        assert_eq!(DaemonStatus::Running.label(), "Running");
        assert_eq!(DaemonStatus::Failed.label(), "Failed");
        assert_eq!(DaemonStatus::UserDeclined.label(), "Declined");
    }

    #[test]
    fn test_is_terminal() {
        assert!(!DaemonStatus::NotInstalled.is_terminal());
        assert!(!DaemonStatus::Installing.is_terminal());
        assert!(DaemonStatus::Running.is_terminal());
        assert!(DaemonStatus::Failed.is_terminal());
        assert!(DaemonStatus::UserDeclined.is_terminal());
    }

    #[test]
    fn test_is_running() {
        assert!(DaemonStatus::Running.is_running());
        assert!(!DaemonStatus::NotInstalled.is_running());
        assert!(!DaemonStatus::Failed.is_running());
    }

    #[test]
    fn test_default() {
        let manager = LogDaemonManager::default();
        assert_eq!(manager.active_count(), 0);
    }
}
