//! Access checker for Docker container logs.
//!
//! Verifies that the current user/environment can access a container's
//! log stream, and provides platform-specific guidance when access fails.

use super::types::AccessStatus;

/// Checks and reports on Docker log access for containers.
#[derive(Debug)]
pub struct AccessChecker;

impl AccessChecker {
    /// Returns platform-specific instructions for gaining Docker access.
    #[must_use]
    pub fn access_instructions() -> &'static str {
        if cfg!(windows) {
            "Ensure Docker Desktop is running and your user has access.\n\
             Try running as Administrator if access is denied."
        } else if cfg!(target_os = "macos") {
            "Ensure Docker Desktop is running.\n\
             If using Docker CLI directly, ensure you're in the 'docker' group."
        } else {
            "Ensure your user is in the 'docker' group:\n\
             sudo usermod -aG docker $USER\n\
             Then log out and back in."
        }
    }

    /// Returns a human-readable summary of an access status.
    #[must_use]
    pub fn status_summary(status: &AccessStatus) -> &'static str {
        match status {
            AccessStatus::Unknown => "Not checked",
            AccessStatus::Accessible => "Accessible",
            AccessStatus::Denied(_) => "Access denied",
            AccessStatus::NotFound => "Container not found",
            AccessStatus::Error(_) => "Error",
        }
    }

    /// Returns the color for an access status indicator.
    #[must_use]
    pub fn status_color(status: &AccessStatus) -> ratatui::style::Color {
        use ratatui::style::Color;
        match status {
            AccessStatus::Unknown => Color::Gray,
            AccessStatus::Accessible => Color::Green,
            AccessStatus::Denied(_) => Color::Red,
            AccessStatus::NotFound => Color::Yellow,
            AccessStatus::Error(_) => Color::Red,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn test_access_instructions_not_empty() {
        let instructions = AccessChecker::access_instructions();
        assert!(!instructions.is_empty());
    }

    #[test]
    fn test_status_summary_all_variants() {
        assert_eq!(
            AccessChecker::status_summary(&AccessStatus::Unknown),
            "Not checked"
        );
        assert_eq!(
            AccessChecker::status_summary(&AccessStatus::Accessible),
            "Accessible"
        );
        assert_eq!(
            AccessChecker::status_summary(&AccessStatus::Denied("x".to_string())),
            "Access denied"
        );
        assert_eq!(
            AccessChecker::status_summary(&AccessStatus::NotFound),
            "Container not found"
        );
        assert_eq!(
            AccessChecker::status_summary(&AccessStatus::Error("x".to_string())),
            "Error"
        );
    }

    #[test]
    fn test_status_color_all_variants() {
        assert_eq!(
            AccessChecker::status_color(&AccessStatus::Unknown),
            Color::Gray
        );
        assert_eq!(
            AccessChecker::status_color(&AccessStatus::Accessible),
            Color::Green
        );
        assert_eq!(
            AccessChecker::status_color(&AccessStatus::Denied("x".to_string())),
            Color::Red
        );
        assert_eq!(
            AccessChecker::status_color(&AccessStatus::NotFound),
            Color::Yellow
        );
        assert_eq!(
            AccessChecker::status_color(&AccessStatus::Error("x".to_string())),
            Color::Red
        );
    }
}
