//! Whether a host is up, and since when.
//!
//! Reachability used to be tracked in three places — `App::host_statuses`, the
//! health dashboard's per-host metrics, and the background status checker —
//! which could and did disagree on screen. This is the single representation
//! all three now share.

use std::time::{Duration, SystemTime};

/// What is known about a host's reachability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Reachability {
    /// Never checked.
    #[default]
    Unknown,
    /// A check is in flight.
    Checking,
    /// Answered on the last check.
    Online,
    /// Did not answer on the last check.
    Offline,
    /// Answered, but refused the credentials we have.
    ///
    /// Worth distinguishing from offline: retrying will not help, and the fix
    /// is a different one.
    AuthFailed,
}

impl Reachability {
    /// Returns true if the host answered.
    #[must_use]
    pub const fn is_up(self) -> bool {
        matches!(self, Self::Online)
    }

    /// Returns true if the host is known not to be usable.
    #[must_use]
    pub const fn is_down(self) -> bool {
        matches!(self, Self::Offline | Self::AuthFailed)
    }

    /// Returns a one-word label for the UI.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Checking => "checking",
            Self::Online => "online",
            Self::Offline => "offline",
            Self::AuthFailed => "auth failed",
        }
    }

    /// Returns the single character used in dense lists.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Unknown => '?',
            Self::Checking => '~',
            Self::Online => '+',
            Self::Offline => '-',
            Self::AuthFailed => '!',
        }
    }
}

/// Reachability plus the timing that makes it useful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostStatus {
    /// Current reachability.
    pub reachability: Reachability,
    /// When the current reachability was first observed.
    ///
    /// This is what "offline since 14:02" is derived from; it is deliberately
    /// not updated by a repeated observation of the same state.
    pub since: SystemTime,
    /// The last time the host answered at all.
    pub last_seen: Option<SystemTime>,
    /// The last error, for the detail pane.
    pub last_error: Option<String>,
}

impl Default for HostStatus {
    fn default() -> Self {
        Self {
            reachability: Reachability::Unknown,
            since: SystemTime::now(),
            last_seen: None,
            last_error: None,
        }
    }
}

impl HostStatus {
    /// Creates a status in a known state as of now.
    #[must_use]
    pub fn new(reachability: Reachability) -> Self {
        Self {
            reachability,
            since: SystemTime::now(),
            last_seen: if reachability.is_up() {
                Some(SystemTime::now())
            } else {
                None
            },
            last_error: None,
        }
    }

    /// Records an observation.
    ///
    /// `since` only moves when the state actually changes, so "offline since"
    /// means what it says.
    pub fn observe(&mut self, reachability: Reachability, error: Option<String>) {
        if reachability != self.reachability {
            self.reachability = reachability;
            self.since = SystemTime::now();
        }
        if reachability.is_up() {
            self.last_seen = Some(SystemTime::now());
            self.last_error = None;
        } else {
            self.last_error = error;
        }
    }

    /// Returns how long the host has been in its current state.
    #[must_use]
    pub fn in_state_for(&self) -> Option<Duration> {
        SystemTime::now().duration_since(self.since).ok()
    }

    /// Returns how long since the host last answered.
    #[must_use]
    pub fn offline_for(&self) -> Option<Duration> {
        if self.reachability.is_up() {
            return None;
        }
        let last = self.last_seen?;
        SystemTime::now().duration_since(last).ok()
    }

    /// Renders the duration in the current state as a short string.
    #[must_use]
    pub fn since_label(&self) -> String {
        match self.in_state_for() {
            None => "just now".to_string(),
            Some(d) => format_duration(d),
        }
    }
}

/// Formats a duration the way a status line wants it: one unit, no decimals.
#[must_use]
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_default_status_is_unknown() {
        let status = HostStatus::default();
        assert_eq!(status.reachability, Reachability::Unknown);
        assert!(status.last_seen.is_none());
        assert!(status.last_error.is_none());
    }

    #[test]
    fn reachability_classifies_up_and_down() {
        assert!(Reachability::Online.is_up());
        assert!(!Reachability::Online.is_down());
        assert!(Reachability::Offline.is_down());
        assert!(Reachability::AuthFailed.is_down());
        assert!(!Reachability::Unknown.is_up());
        assert!(!Reachability::Unknown.is_down());
        assert!(!Reachability::Checking.is_up());
        assert!(!Reachability::Checking.is_down());
    }

    #[test]
    fn every_reachability_has_a_label_and_a_glyph() {
        for r in [
            Reachability::Unknown,
            Reachability::Checking,
            Reachability::Online,
            Reachability::Offline,
            Reachability::AuthFailed,
        ] {
            assert!(!r.label().is_empty());
            assert!(!r.glyph().is_whitespace());
        }
    }

    #[test]
    fn glyphs_are_distinct() {
        let glyphs: Vec<char> = [
            Reachability::Unknown,
            Reachability::Checking,
            Reachability::Online,
            Reachability::Offline,
            Reachability::AuthFailed,
        ]
        .iter()
        .map(|r| r.glyph())
        .collect();
        let mut unique = glyphs.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), glyphs.len(), "{glyphs:?}");
    }

    #[test]
    fn going_online_records_a_last_seen_time() {
        let mut status = HostStatus::default();
        status.observe(Reachability::Online, None);
        assert!(status.last_seen.is_some());
        assert!(status.offline_for().is_none());
    }

    #[test]
    fn repeating_a_state_does_not_reset_the_since_time() {
        let mut status = HostStatus::new(Reachability::Offline);
        let first = status.since;
        std::thread::sleep(Duration::from_millis(5));
        status.observe(Reachability::Offline, Some("still down".to_string()));
        assert_eq!(status.since, first, "since must track state changes only");
    }

    #[test]
    fn changing_state_moves_the_since_time() {
        let mut status = HostStatus::new(Reachability::Online);
        let first = status.since;
        std::thread::sleep(Duration::from_millis(5));
        status.observe(Reachability::Offline, Some("timeout".to_string()));
        assert!(status.since > first);
        assert_eq!(status.last_error.as_deref(), Some("timeout"));
    }

    #[test]
    fn coming_back_online_clears_the_error() {
        let mut status = HostStatus::new(Reachability::Offline);
        status.observe(Reachability::Offline, Some("timeout".to_string()));
        status.observe(Reachability::Online, None);
        assert!(status.last_error.is_none());
    }

    #[test]
    fn offline_for_needs_a_previous_sighting() {
        let mut status = HostStatus::default();
        status.observe(Reachability::Offline, None);
        assert!(
            status.offline_for().is_none(),
            "a host never seen has no offline duration"
        );

        status.observe(Reachability::Online, None);
        status.observe(Reachability::Offline, None);
        assert!(status.offline_for().is_some());
    }

    #[test]
    fn durations_render_in_one_unit() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_secs(59)), "59s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(3599)), "59m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(format_duration(Duration::from_secs(86_399)), "23h");
        assert_eq!(format_duration(Duration::from_secs(86_400)), "1d");
        assert_eq!(format_duration(Duration::from_secs(200_000)), "2d");
    }

    #[test]
    fn a_fresh_status_reports_a_short_since_label() {
        let status = HostStatus::new(Reachability::Online);
        assert_eq!(status.since_label(), "0s");
    }
}
