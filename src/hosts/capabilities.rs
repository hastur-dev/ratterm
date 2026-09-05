//! What a host can do.
//!
//! Detected once per session with a single round trip and then cached, so the
//! Docker manager can answer "which of these six machines runs Docker" without
//! probing every one of them every time a dashboard opens.

use std::time::{Duration, SystemTime};

/// One probe: the tool to look for and the capability it proves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    /// Executable name looked for on `PATH`.
    pub program: &'static str,
    /// Marker printed when the program is present.
    pub marker: &'static str,
}

/// The probes run in one round trip.
///
/// `command -v` is POSIX and works in every shell we are likely to meet,
/// unlike `which`, which is not always installed.
pub const PROBES: &[Probe] = &[
    Probe {
        program: "docker",
        marker: "has-docker",
    },
    Probe {
        program: "docker-compose",
        marker: "has-compose-v1",
    },
    Probe {
        program: "kubectl",
        marker: "has-kubectl",
    },
    Probe {
        program: "nvidia-smi",
        marker: "has-nvidia-smi",
    },
    Probe {
        program: "podman",
        marker: "has-podman",
    },
    Probe {
        program: "systemctl",
        marker: "has-systemd",
    },
];

/// Marker printed when `docker compose` (the plugin form) works.
pub const COMPOSE_V2_MARKER: &str = "has-compose-v2";

/// How long a capability answer is trusted before it is probed again.
pub const CAPABILITY_TTL: Duration = Duration::from_secs(3600);

/// What one host was found to support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCapabilities {
    /// A Docker CLI is on `PATH`.
    pub docker: bool,
    /// `docker compose` (plugin) works.
    pub compose_v2: bool,
    /// The standalone `docker-compose` binary is present.
    pub compose_v1: bool,
    /// `kubectl` is on `PATH`.
    pub kubectl: bool,
    /// `nvidia-smi` is on `PATH`, so GPU metrics are worth collecting.
    pub nvidia_smi: bool,
    /// `podman` is on `PATH`.
    pub podman: bool,
    /// systemd is available, so the metrics agent can be installed as a unit.
    pub systemd: bool,
    /// When this was detected.
    pub detected_at: SystemTime,
}

impl Default for HostCapabilities {
    fn default() -> Self {
        Self {
            docker: false,
            compose_v2: false,
            compose_v1: false,
            kubectl: false,
            nvidia_smi: false,
            podman: false,
            systemd: false,
            detected_at: SystemTime::UNIX_EPOCH,
        }
    }
}

impl HostCapabilities {
    /// Returns the shell command that produces the capability report.
    ///
    /// One command rather than one per tool: on a link with 40 ms of latency
    /// six probes cost a quarter of a second serially and nothing in parallel.
    #[must_use]
    pub fn probe_command() -> String {
        let mut parts: Vec<String> = PROBES
            .iter()
            .map(|probe| {
                format!(
                    "command -v {} >/dev/null 2>&1 && echo {}",
                    probe.program, probe.marker
                )
            })
            .collect();
        // The Compose plugin is a docker subcommand, not a binary on PATH.
        parts.push(format!(
            "docker compose version >/dev/null 2>&1 && echo {COMPOSE_V2_MARKER}"
        ));
        parts.join("; ")
    }

    /// Reads the markers out of the probe command's output.
    #[must_use]
    pub fn from_probe_output(output: &str) -> Self {
        let has = |marker: &str| output.lines().any(|line| line.trim() == marker);

        Self {
            docker: has("has-docker"),
            compose_v2: has(COMPOSE_V2_MARKER),
            compose_v1: has("has-compose-v1"),
            kubectl: has("has-kubectl"),
            nvidia_smi: has("has-nvidia-smi"),
            podman: has("has-podman"),
            systemd: has("has-systemd"),
            detected_at: SystemTime::now(),
        }
    }

    /// Returns true if this answer is old enough to be worth re-checking.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.age().is_none_or(|age| age > CAPABILITY_TTL)
    }

    /// Returns how long ago the capabilities were detected.
    #[must_use]
    pub fn age(&self) -> Option<Duration> {
        SystemTime::now().duration_since(self.detected_at).ok()
    }

    /// Returns true if the host can run Compose stacks either way.
    #[must_use]
    pub const fn has_compose(&self) -> bool {
        self.compose_v2 || self.compose_v1
    }

    /// Returns the Compose command prefix to use on this host.
    #[must_use]
    pub const fn compose_command(&self) -> Option<&'static str> {
        if self.compose_v2 {
            Some("docker compose")
        } else if self.compose_v1 {
            Some("docker-compose")
        } else {
            None
        }
    }

    /// Returns short labels for the capabilities present, for the fleet view.
    #[must_use]
    pub fn labels(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.docker {
            out.push("docker");
        }
        if self.has_compose() {
            out.push("compose");
        }
        if self.kubectl {
            out.push("kubectl");
        }
        if self.podman {
            out.push("podman");
        }
        if self.nvidia_smi {
            out.push("gpu");
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_command_covers_every_probe() {
        let command = HostCapabilities::probe_command();
        for probe in PROBES {
            assert!(
                command.contains(probe.program),
                "{} missing from {command}",
                probe.program
            );
            assert!(command.contains(probe.marker), "{} missing", probe.marker);
        }
        assert!(command.contains(COMPOSE_V2_MARKER));
    }

    #[test]
    fn the_probe_command_is_one_shell_invocation() {
        let command = HostCapabilities::probe_command();
        assert!(!command.contains('\n'), "must be a single line");
        assert!(command.contains("command -v"), "{command}");
    }

    #[test]
    fn an_empty_report_means_no_capabilities() {
        let caps = HostCapabilities::from_probe_output("");
        assert!(!caps.docker);
        assert!(!caps.kubectl);
        assert!(!caps.has_compose());
        assert!(caps.labels().is_empty());
        assert_eq!(caps.compose_command(), None);
    }

    #[test]
    fn markers_are_read_out_of_the_report() {
        let caps = HostCapabilities::from_probe_output(
            "has-docker\nhas-compose-v2\nhas-kubectl\nhas-nvidia-smi\n",
        );
        assert!(caps.docker);
        assert!(caps.compose_v2);
        assert!(caps.kubectl);
        assert!(caps.nvidia_smi);
        assert!(!caps.podman);
        assert_eq!(caps.compose_command(), Some("docker compose"));
    }

    #[test]
    fn surrounding_whitespace_and_noise_are_tolerated() {
        // A login shell often prints a banner before the probe output.
        let caps = HostCapabilities::from_probe_output(
            "Welcome to Ubuntu\n  has-docker  \nLast login: today\nhas-kubectl\n",
        );
        assert!(caps.docker);
        assert!(caps.kubectl);
    }

    #[test]
    fn a_marker_inside_another_word_does_not_count() {
        let caps = HostCapabilities::from_probe_output("nothas-dockerhere\n");
        assert!(!caps.docker, "substring matches must not be accepted");
    }

    #[test]
    fn compose_v1_is_used_when_the_plugin_is_absent() {
        let caps = HostCapabilities::from_probe_output("has-docker\nhas-compose-v1\n");
        assert!(caps.has_compose());
        assert_eq!(caps.compose_command(), Some("docker-compose"));
    }

    #[test]
    fn the_plugin_wins_when_both_are_present() {
        let caps = HostCapabilities::from_probe_output("has-compose-v1\nhas-compose-v2\n");
        assert_eq!(caps.compose_command(), Some("docker compose"));
    }

    #[test]
    fn labels_list_only_what_is_present() {
        let caps = HostCapabilities::from_probe_output("has-docker\nhas-nvidia-smi\n");
        assert_eq!(caps.labels(), vec!["docker", "gpu"]);
    }

    #[test]
    fn a_default_answer_is_stale() {
        assert!(HostCapabilities::default().is_stale());
    }

    #[test]
    fn a_fresh_answer_is_not_stale() {
        let caps = HostCapabilities::from_probe_output("has-docker");
        assert!(!caps.is_stale());
        assert!(caps.age().expect("an age") < Duration::from_secs(5));
    }
}
