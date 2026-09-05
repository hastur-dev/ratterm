//! Choosing how to reach a Docker daemon.
//!
//! Three transports exist:
//!
//! - a Unix domain socket, `/var/run/docker.sock`, on Unix;
//! - a Windows named pipe, `//./pipe/docker_engine`, which is what Docker
//!   Desktop exposes;
//! - a TCP endpoint on loopback that an SSH port forward carries to a remote
//!   daemon.
//!
//! The decision is a pure function, [`choose_transport`], over a
//! [`TransportProbe`] describing what was found. Probing touches the
//! filesystem and the environment; deciding does not. That split is what lets
//! the rule be tested for every platform on every platform — a Windows CI box
//! can check the Unix branch and vice versa — and it is why the platform is a
//! field on the probe rather than a `cfg` inside the decision.

use std::fmt;

/// Default Unix socket the Docker daemon listens on.
pub const DEFAULT_UNIX_SOCKET: &str = "/var/run/docker.sock";

/// Default named pipe Docker Desktop exposes on Windows.
pub const DEFAULT_NAMED_PIPE: &str = "//./pipe/docker_engine";

/// The port a remote daemon is assumed to serve when it is reached by forward.
pub const DEFAULT_REMOTE_TCP_PORT: u16 = 2375;

/// The address the forward connects to on the remote side.
///
/// Loopback on the remote host, so the daemon does not need to be exposed to
/// its own network for this to work.
pub const REMOTE_LOOPBACK: &str = "127.0.0.1";

/// Which operating system the local endpoint would live on.
///
/// Carried on the probe rather than read from `cfg!` inside the decision, so
/// the decision can be exercised for both platforms in one test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Linux, macOS, BSD: a Unix domain socket.
    Unix,
    /// Windows: a named pipe.
    Windows,
}

impl Platform {
    /// The platform this build runs on.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }

    /// The endpoint path this platform's local daemon uses.
    #[must_use]
    pub const fn default_endpoint(self) -> &'static str {
        match self {
            Self::Unix => DEFAULT_UNIX_SOCKET,
            Self::Windows => DEFAULT_NAMED_PIPE,
        }
    }
}

/// What was observed about a candidate host, before any decision is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportProbe {
    /// `None` for the local machine, `Some(id)` for an SSH host.
    pub host_id: Option<u32>,
    /// The platform the local endpoint would be on.
    pub platform: Platform,
    /// Whether the platform-local socket or pipe exists.
    pub local_endpoint_present: bool,
    /// `DOCKER_HOST`, if the environment sets it.
    pub docker_host_env: Option<String>,
    /// The port the remote daemon is expected to serve.
    pub remote_port: u16,
}

impl TransportProbe {
    /// A probe for the local machine on this platform.
    #[must_use]
    pub fn local(local_endpoint_present: bool) -> Self {
        Self {
            host_id: None,
            platform: Platform::current(),
            local_endpoint_present,
            docker_host_env: None,
            remote_port: DEFAULT_REMOTE_TCP_PORT,
        }
    }

    /// A probe for a remote SSH host.
    #[must_use]
    pub fn remote(host_id: u32) -> Self {
        Self {
            host_id: Some(host_id),
            platform: Platform::current(),
            local_endpoint_present: false,
            docker_host_env: None,
            remote_port: DEFAULT_REMOTE_TCP_PORT,
        }
    }

    /// Sets the platform, for tests and for cross-platform reasoning.
    #[must_use]
    pub const fn on(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }

    /// Records a `DOCKER_HOST` value.
    #[must_use]
    pub fn with_env(mut self, value: Option<String>) -> Self {
        self.docker_host_env = value.filter(|v| !v.trim().is_empty());
        self
    }

    /// Sets the port the remote daemon is expected to serve.
    #[must_use]
    pub const fn with_remote_port(mut self, port: u16) -> Self {
        self.remote_port = port;
        self
    }
}

/// Why no transport could be chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreachable {
    /// The local endpoint is absent and there is no host to forward through.
    NoLocalEndpoint {
        /// The path that was looked for.
        probed: String,
    },
}

/// The decision: how this host should be reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportChoice {
    /// Connect to a Unix domain socket at this path.
    UnixSocket(String),
    /// Connect to a Windows named pipe at this path.
    NamedPipe(String),
    /// Connect to the endpoint `DOCKER_HOST` names.
    Environment(String),
    /// Open an SSH port forward and connect to the local end of it.
    SshForward {
        /// SSH host id to forward through.
        host_id: u32,
        /// Address on the remote side, always loopback.
        remote_host: &'static str,
        /// Port on the remote side.
        remote_port: u16,
    },
    /// Nothing usable was found.
    Unavailable(Unreachable),
}

impl TransportChoice {
    /// A short name for the transport, for logs and error messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::UnixSocket(path) => format!("unix socket {path}"),
            Self::NamedPipe(path) => format!("named pipe {path}"),
            Self::Environment(endpoint) => format!("DOCKER_HOST {endpoint}"),
            Self::SshForward {
                host_id,
                remote_host,
                remote_port,
            } => format!("ssh forward to host {host_id} ({remote_host}:{remote_port})"),
            Self::Unavailable(Unreachable::NoLocalEndpoint { probed }) => {
                format!("nothing usable (looked for {probed})")
            }
        }
    }

    /// Why this transport was chosen over the others.
    ///
    /// Shown in the fleet view so a surprising choice can be understood
    /// without reading the source.
    #[must_use]
    pub fn rationale(&self) -> &'static str {
        match self {
            Self::UnixSocket(_) => "the local daemon socket is present",
            Self::NamedPipe(_) => "Docker Desktop's named pipe is present",
            Self::Environment(_) => "DOCKER_HOST names an endpoint",
            Self::SshForward { .. } => "the daemon is on another machine, reached over SSH",
            Self::Unavailable(_) => "no local endpoint and no SSH host to forward through",
        }
    }
}

/// Decides how to reach the daemon described by `probe`.
///
/// The order is deliberate. A remote host always uses the forward: its daemon
/// is on the other machine, and a local socket that happens to exist belongs
/// to a different daemon entirely. For the local machine the platform endpoint
/// wins when it is there, `DOCKER_HOST` is the documented override when it is
/// not, and anything else is a failure with the path that was looked for.
#[must_use]
pub fn choose_transport(probe: &TransportProbe) -> TransportChoice {
    if let Some(host_id) = probe.host_id {
        return TransportChoice::SshForward {
            host_id,
            remote_host: REMOTE_LOOPBACK,
            remote_port: probe.remote_port,
        };
    }

    let endpoint = probe.platform.default_endpoint().to_string();

    if probe.local_endpoint_present {
        return match probe.platform {
            Platform::Unix => TransportChoice::UnixSocket(endpoint),
            Platform::Windows => TransportChoice::NamedPipe(endpoint),
        };
    }

    if let Some(env) = probe.docker_host_env.as_ref() {
        return TransportChoice::Environment(env.clone());
    }

    TransportChoice::Unavailable(Unreachable::NoLocalEndpoint { probed: endpoint })
}

/// A transport that was actually opened.
///
/// Distinct from [`TransportChoice`] because a forward carries a live tunnel:
/// the choice says "forward through host 3", this says "and it landed on
/// 127.0.0.1:54321".
#[derive(Clone, PartialEq, Eq)]
pub enum Transport {
    /// A Unix domain socket at this path.
    UnixSocket(String),
    /// A Windows named pipe at this path.
    NamedPipe(String),
    /// A TCP or HTTP endpoint named by the environment.
    Environment(String),
    /// A loopback port carried to a remote daemon over SSH.
    Forwarded {
        /// SSH host id the tunnel runs through.
        host_id: u32,
        /// The loopback address the client connects to.
        local: String,
        /// The remote endpoint, as `host:port`.
        remote: String,
    },
}

impl Transport {
    /// The address bollard was pointed at.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        match self {
            Self::UnixSocket(path) | Self::NamedPipe(path) | Self::Environment(path) => path,
            Self::Forwarded { local, .. } => local,
        }
    }

    /// True when this transport is a tunnel that has to stay open.
    #[must_use]
    pub const fn is_forwarded(&self) -> bool {
        matches!(self, Self::Forwarded { .. })
    }
}

/// Named rather than derived, so a log line reads as prose and the forward
/// shows both ends.
impl fmt::Debug for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnixSocket(path) => write!(f, "unix socket {path}"),
            Self::NamedPipe(path) => write!(f, "named pipe {path}"),
            Self::Environment(endpoint) => write!(f, "DOCKER_HOST {endpoint}"),
            Self::Forwarded {
                host_id,
                local,
                remote,
            } => write!(f, "ssh forward host {host_id}: {local} -> {remote}"),
        }
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_present_unix_socket_is_chosen_on_unix() {
        let probe = TransportProbe::local(true).on(Platform::Unix);
        assert_eq!(
            choose_transport(&probe),
            TransportChoice::UnixSocket(DEFAULT_UNIX_SOCKET.to_string())
        );
    }

    #[test]
    fn a_present_named_pipe_is_chosen_on_windows() {
        let probe = TransportProbe::local(true).on(Platform::Windows);
        assert_eq!(
            choose_transport(&probe),
            TransportChoice::NamedPipe(DEFAULT_NAMED_PIPE.to_string())
        );
    }

    #[test]
    fn docker_host_is_used_when_the_platform_endpoint_is_missing() {
        let probe = TransportProbe::local(false)
            .on(Platform::Unix)
            .with_env(Some("tcp://10.0.0.5:2375".to_string()));
        assert_eq!(
            choose_transport(&probe),
            TransportChoice::Environment("tcp://10.0.0.5:2375".to_string())
        );
    }

    #[test]
    fn the_platform_endpoint_wins_over_docker_host_when_both_are_there() {
        let probe = TransportProbe::local(true)
            .on(Platform::Windows)
            .with_env(Some("tcp://10.0.0.5:2375".to_string()));
        assert_eq!(
            choose_transport(&probe),
            TransportChoice::NamedPipe(DEFAULT_NAMED_PIPE.to_string())
        );
    }

    #[test]
    fn an_empty_docker_host_is_treated_as_unset() {
        let probe = TransportProbe::local(false)
            .on(Platform::Unix)
            .with_env(Some("   ".to_string()));
        assert!(probe.docker_host_env.is_none());
        assert!(matches!(
            choose_transport(&probe),
            TransportChoice::Unavailable(_)
        ));
    }

    #[test]
    fn a_remote_host_always_forwards_even_when_a_local_socket_exists() {
        // The local socket belongs to this machine's daemon, not the remote
        // one; using it would silently show the wrong containers.
        let probe = TransportProbe::remote(4).on(Platform::Unix);
        let mut with_local = probe.clone();
        with_local.local_endpoint_present = true;

        for candidate in [probe, with_local] {
            assert_eq!(
                choose_transport(&candidate),
                TransportChoice::SshForward {
                    host_id: 4,
                    remote_host: REMOTE_LOOPBACK,
                    remote_port: DEFAULT_REMOTE_TCP_PORT,
                }
            );
        }
    }

    #[test]
    fn a_remote_host_can_be_pointed_at_another_port() {
        let probe = TransportProbe::remote(4).with_remote_port(2376);
        assert_eq!(
            choose_transport(&probe),
            TransportChoice::SshForward {
                host_id: 4,
                remote_host: REMOTE_LOOPBACK,
                remote_port: 2376,
            }
        );
    }

    #[test]
    fn nothing_available_reports_the_path_it_looked_for() {
        for (platform, expected) in [
            (Platform::Unix, DEFAULT_UNIX_SOCKET),
            (Platform::Windows, DEFAULT_NAMED_PIPE),
        ] {
            let probe = TransportProbe::local(false).on(platform);
            match choose_transport(&probe) {
                TransportChoice::Unavailable(Unreachable::NoLocalEndpoint { probed }) => {
                    assert_eq!(probed, expected);
                }
                other => panic!("expected Unavailable, got {other:?}"),
            }
        }
    }

    #[test]
    fn every_choice_describes_itself_and_says_why() {
        let choices = vec![
            TransportChoice::UnixSocket(DEFAULT_UNIX_SOCKET.to_string()),
            TransportChoice::NamedPipe(DEFAULT_NAMED_PIPE.to_string()),
            TransportChoice::Environment("tcp://host:2375".to_string()),
            TransportChoice::SshForward {
                host_id: 2,
                remote_host: REMOTE_LOOPBACK,
                remote_port: 2375,
            },
            TransportChoice::Unavailable(Unreachable::NoLocalEndpoint {
                probed: "/var/run/docker.sock".to_string(),
            }),
        ];
        for choice in choices {
            assert!(!choice.describe().is_empty());
            assert!(!choice.rationale().is_empty());
        }
    }

    #[test]
    fn the_current_platform_matches_the_build() {
        let platform = Platform::current();
        if cfg!(windows) {
            assert_eq!(platform, Platform::Windows);
            assert_eq!(platform.default_endpoint(), DEFAULT_NAMED_PIPE);
        } else {
            assert_eq!(platform, Platform::Unix);
            assert_eq!(platform.default_endpoint(), DEFAULT_UNIX_SOCKET);
        }
    }

    #[test]
    fn a_transport_debug_names_the_transport_and_both_ends_of_a_forward() {
        let forwarded = Transport::Forwarded {
            host_id: 7,
            local: "127.0.0.1:54321".to_string(),
            remote: "127.0.0.1:2375".to_string(),
        };
        let rendered = format!("{forwarded:?}");
        assert!(rendered.contains("ssh forward"), "{rendered}");
        assert!(rendered.contains("127.0.0.1:54321"), "{rendered}");
        assert!(rendered.contains("host 7"), "{rendered}");
        assert!(forwarded.is_forwarded());
        assert_eq!(forwarded.endpoint(), "127.0.0.1:54321");

        let socket = Transport::UnixSocket(DEFAULT_UNIX_SOCKET.to_string());
        assert_eq!(
            format!("{socket:?}"),
            format!("unix socket {DEFAULT_UNIX_SOCKET}")
        );
        assert_eq!(
            socket.to_string(),
            format!("unix socket {DEFAULT_UNIX_SOCKET}")
        );
        assert!(!socket.is_forwarded());

        let pipe = Transport::NamedPipe(DEFAULT_NAMED_PIPE.to_string());
        assert!(format!("{pipe:?}").contains("named pipe"));
        assert_eq!(pipe.endpoint(), DEFAULT_NAMED_PIPE);

        let env = Transport::Environment("tcp://h:2375".to_string());
        assert!(format!("{env:?}").contains("DOCKER_HOST"));
        assert_eq!(env.endpoint(), "tcp://h:2375");
    }
}
