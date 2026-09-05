//! What to ask the API server for when reading a pod's logs.

use kube::api::LogParams;

/// How many lines a one-shot fetch returns by default.
pub(super) const DEFAULT_TAIL_LINES: i64 = 500;

/// What to ask the API server for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PodLogOptions {
    /// Container to read. Required when the pod has more than one.
    pub container: Option<String>,
    /// Keep reading as new lines are written.
    pub follow: bool,
    /// Number of lines from the end to start with.
    pub tail_lines: Option<i64>,
    /// Only lines written in the last this-many seconds.
    pub since_seconds: Option<i64>,
    /// Read the previous container instance instead of the running one. This
    /// is how the logs of a crashed container are recovered.
    pub previous: bool,
}

impl PodLogOptions {
    /// Options for a one-shot read of the last [`DEFAULT_TAIL_LINES`] lines.
    #[must_use]
    pub fn tail() -> Self {
        Self {
            tail_lines: Some(DEFAULT_TAIL_LINES),
            ..Self::default()
        }
    }

    /// Reads a specific container.
    #[must_use]
    pub fn container(mut self, name: impl Into<String>) -> Self {
        self.container = Some(name.into());
        self
    }

    /// Turns following on.
    #[must_use]
    pub const fn following(mut self) -> Self {
        self.follow = true;
        self
    }

    /// Sets how many lines from the end to start with.
    #[must_use]
    pub const fn with_tail_lines(mut self, lines: i64) -> Self {
        self.tail_lines = Some(lines);
        self
    }

    /// Limits the read to recent lines.
    #[must_use]
    pub const fn since_seconds(mut self, seconds: i64) -> Self {
        self.since_seconds = Some(seconds);
        self
    }

    /// Reads the previous container instance.
    #[must_use]
    pub const fn previous_instance(mut self) -> Self {
        self.previous = true;
        self
    }

    /// Builds the API request parameters.
    ///
    /// Timestamps are always requested: they are what lets a follower tell a
    /// new line from one it has already delivered, and they are stripped from
    /// the message before it reaches the buffer.
    ///
    /// `follow` is deliberately not passed through. The follower polls, so
    /// each individual request must return rather than hang open.
    #[must_use]
    pub fn to_params(&self) -> LogParams {
        LogParams {
            container: self.container.clone(),
            follow: false,
            previous: self.previous,
            since_seconds: self.since_seconds,
            tail_lines: self.tail_lines,
            timestamps: true,
            ..LogParams::default()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn options_map_onto_the_api_parameters() {
        let options = PodLogOptions::tail()
            .container("sidecar")
            .following()
            .with_tail_lines(20)
            .since_seconds(300)
            .previous_instance();
        let params = options.to_params();

        assert_eq!(params.container.as_deref(), Some("sidecar"));
        assert_eq!(params.tail_lines, Some(20));
        assert_eq!(params.since_seconds, Some(300));
        assert!(params.previous);
        assert!(params.timestamps, "timestamps drive the follower's cursor");
        assert!(!params.follow, "each poll must return");
        assert!(options.follow, "the caller's intent is kept on the options");
    }

    #[test]
    fn default_options_ask_for_nothing_in_particular() {
        let params = PodLogOptions::default().to_params();
        assert!(params.container.is_none());
        assert!(params.tail_lines.is_none());
        assert!(params.since_seconds.is_none());
        assert!(!params.previous);
        assert!(params.timestamps);
    }

    #[test]
    fn the_tail_preset_asks_for_the_default_line_count() {
        assert_eq!(
            PodLogOptions::tail().to_params().tail_lines,
            Some(DEFAULT_TAIL_LINES)
        );
        assert!(!PodLogOptions::tail().follow);
    }
}
