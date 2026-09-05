//! Persistent authenticated SSH sessions.
//!
//! What this replaces: every remote action used to build a shell string and
//! spawn a fresh client — `sshpass -p '<password>' ssh -o
//! StrictHostKeyChecking=no ...` on Linux, `plink -pw ...` on Windows, with a
//! WSL fallback. That meant a TCP and authentication handshake per action, the
//! password visible in the process list, host-key checking switched off, and a
//! runtime dependency on three external programs.
//!
//! A [`RemoteSession`] authenticates once and stays open. It can run commands,
//! open SFTP, and forward a local port, which is what the Docker and Kubernetes
//! managers use to reach a remote daemon or API server with an ordinary
//! client.
//!
//! Host keys are checked against `~/.ssh/known_hosts`. The default policy
//! pins a key the first time a host is seen and refuses a key that has
//! changed — the case that matters, because a changed key is what an
//! interception looks like.
//!
//! `ProxyJump` chains are built from nested port forwards rather than
//! command-line flags: each hop forwards the next hop's address to a loopback
//! port, and the next session connects to that.

use std::io::Read;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ssh2::{CheckResult, KnownHostFileKind, KnownHostKeyFormat, Session, Sftp};
use thiserror::Error;
use tracing::{debug, warn};
use zeroize::Zeroizing;

use super::forward::{ForwardError, PortForward};

/// TCP connect timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Read and write timeout for session traffic.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest `ProxyJump` chain accepted.
///
/// Each hop costs a session and a forward, and a chain longer than this is
/// almost always a configuration loop.
pub const MAX_JUMP_HOPS: usize = 8;

/// Maximum bytes captured from a command's output streams.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

/// Errors raised by remote sessions.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The address could not be resolved.
    #[error("could not resolve {0}")]
    Resolve(String),

    /// The TCP connection failed.
    #[error("could not reach {target}: {source}")]
    Connect {
        /// The endpoint that was tried.
        target: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The SSH handshake failed.
    #[error("SSH handshake with {target} failed: {message}")]
    Handshake {
        /// The endpoint that was tried.
        target: String,
        /// Underlying message.
        message: String,
    },

    /// Authentication failed.
    #[error("authentication as {username}@{target} failed: {message}")]
    Auth {
        /// The user that was tried.
        username: String,
        /// The endpoint that was tried.
        target: String,
        /// Underlying message.
        message: String,
    },

    /// The host key is not in `known_hosts`.
    #[error("host key for {host} is not known (fingerprint {fingerprint})")]
    UnknownHostKey {
        /// The host as written in `known_hosts`.
        host: String,
        /// SHA-256 fingerprint, base64, as OpenSSH prints it.
        fingerprint: String,
    },

    /// The host key does not match the pinned one.
    #[error(
        "host key for {host} has CHANGED (now {fingerprint}); refusing to connect. \
         Remove the old entry from known_hosts only if you know why it changed."
    )]
    HostKeyMismatch {
        /// The host as written in `known_hosts`.
        host: String,
        /// The key now offered.
        fingerprint: String,
    },

    /// A command could not be run.
    #[error("remote command failed: {0}")]
    Command(String),

    /// SFTP could not be started.
    #[error("could not open SFTP: {0}")]
    Sftp(String),

    /// A forward could not be created.
    #[error(transparent)]
    Forward(#[from] ForwardError),

    /// The jump chain was too long or looped.
    #[error("proxy jump chain is longer than {MAX_JUMP_HOPS} hops")]
    JumpChainTooLong,
}

/// What to do about a host key that is not already pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostKeyPolicy {
    /// Pin a key the first time a host is seen; refuse a changed key.
    ///
    /// This is what OpenSSH does with `StrictHostKeyChecking=accept-new`.
    #[default]
    AcceptNew,
    /// Refuse anything not already in `known_hosts`.
    Strict,
    /// Report an unknown key to the caller so it can ask the user.
    Prompt,
    /// Accept anything. Only for tests and for hosts the user has told us to
    /// trust unconditionally.
    AcceptAny,
}

/// Where and how to connect.
#[derive(Clone, Default)]
pub struct RemoteTarget {
    /// SSH host id this target came from, when it came from the host list.
    pub host_id: Option<u32>,
    /// Hostname or address.
    pub hostname: String,
    /// Port.
    pub port: u16,
    /// User to authenticate as.
    pub username: String,
    /// Password, if password authentication is to be tried.
    pub password: Option<Zeroizing<String>>,
    /// Private key file, if key authentication is to be tried.
    pub key_path: Option<PathBuf>,
    /// Passphrase for the private key.
    pub key_passphrase: Option<Zeroizing<String>>,
    /// Host to jump through first.
    pub jump: Option<Box<RemoteTarget>>,
}

impl std::fmt::Debug for RemoteTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteTarget")
            .field("host_id", &self.host_id)
            .field("endpoint", &self.endpoint())
            .field("username", &self.username)
            .field("has_password", &self.password.is_some())
            .field("key_path", &self.key_path)
            .field("jump", &self.jump)
            .finish()
    }
}

impl RemoteTarget {
    /// Creates a target with the standard SSH port.
    #[must_use]
    pub fn new(hostname: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            host_id: None,
            hostname: hostname.into(),
            port: 22,
            username: username.into(),
            password: None,
            key_path: None,
            key_passphrase: None,
            jump: None,
        }
    }

    /// Sets the port.
    #[must_use]
    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the password.
    #[must_use]
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(Zeroizing::new(password.into()));
        self
    }

    /// Sets the private key file.
    #[must_use]
    pub fn with_key(mut self, key_path: impl Into<PathBuf>) -> Self {
        self.key_path = Some(key_path.into());
        self
    }

    /// Sets the host to jump through.
    #[must_use]
    pub fn with_jump(mut self, jump: Self) -> Self {
        self.jump = Some(Box::new(jump));
        self
    }

    /// Returns `host:port`.
    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.hostname, self.port)
    }

    /// Returns the hops to make, outermost jump first, this target last.
    ///
    /// # Errors
    /// Returns [`SessionError::JumpChainTooLong`] for a chain longer than
    /// [`MAX_JUMP_HOPS`], which is also what a configuration loop looks like.
    pub fn hop_chain(&self) -> Result<Vec<&Self>, SessionError> {
        let mut hops = Vec::new();
        let mut current = Some(self);

        for _ in 0..=MAX_JUMP_HOPS {
            let Some(target) = current else {
                hops.reverse();
                return Ok(hops);
            };
            hops.push(target);
            current = target.jump.as_deref();
        }

        Err(SessionError::JumpChainTooLong)
    }
}

/// The result of running a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Exit status.
    pub exit_status: i32,
}

impl CommandOutput {
    /// Returns true if the command exited zero.
    #[must_use]
    pub const fn success(&self) -> bool {
        self.exit_status == 0
    }

    /// Returns the trimmed standard output.
    #[must_use]
    pub fn trimmed(&self) -> &str {
        self.stdout.trim()
    }

    /// Turns a non-zero exit into an error, keeping stderr for the message.
    ///
    /// # Errors
    /// Returns [`SessionError::Command`] when the exit status is non-zero.
    pub fn ok(self) -> Result<Self, SessionError> {
        if self.success() {
            Ok(self)
        } else {
            Err(SessionError::Command(format!(
                "exit status {}: {}",
                self.exit_status,
                self.stderr.trim()
            )))
        }
    }
}

/// One authenticated SSH session, kept open for reuse.
pub struct RemoteSession {
    session: Session,
    target: RemoteTarget,
    opened: Instant,
    last_used: Instant,
    /// Forwards that carry the jump chain; dropped with the session.
    _jump_forwards: Vec<PortForward>,
}

impl std::fmt::Debug for RemoteSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSession")
            .field("endpoint", &self.target.endpoint())
            .field("username", &self.target.username)
            .field("age", &self.opened.elapsed())
            .field("idle", &self.last_used.elapsed())
            .finish()
    }
}

impl RemoteSession {
    /// Opens a session, walking any `ProxyJump` chain first.
    ///
    /// # Errors
    /// Returns an error if any hop cannot be reached, authenticated, or
    /// verified against `known_hosts`.
    pub fn connect(target: &RemoteTarget, policy: HostKeyPolicy) -> Result<Self, SessionError> {
        let hops = target.hop_chain()?;
        let mut forwards: Vec<PortForward> = Vec::new();

        for (index, hop) in hops.iter().enumerate() {
            // The first hop is reached directly; later hops through the
            // forward opened by the previous one.
            let address = match forwards.last() {
                Some(forward) => forward.local_addr().to_string(),
                None => hop.endpoint(),
            };

            let session = open_session(&address, hop, policy)?;

            if index + 1 < hops.len() {
                let next = hops[index + 1];
                forwards.push(PortForward::start(session, &next.hostname, next.port)?);
            } else {
                let now = Instant::now();
                return Ok(Self {
                    session,
                    target: (*hop).clone(),
                    opened: now,
                    last_used: now,
                    _jump_forwards: forwards,
                });
            }
        }

        // `hop_chain` always yields at least the target itself.
        Err(SessionError::Command("empty connection chain".to_string()))
    }

    /// Returns the target this session is connected to.
    #[must_use]
    pub const fn target(&self) -> &RemoteTarget {
        &self.target
    }

    /// Returns how long the session has been open.
    #[must_use]
    pub fn age(&self) -> Duration {
        self.opened.elapsed()
    }

    /// Returns how long since the session was last used.
    #[must_use]
    pub fn idle_for(&self) -> Duration {
        self.last_used.elapsed()
    }

    /// Marks the session as used now.
    pub fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    /// Returns true if the session still answers.
    ///
    /// Sends a keepalive rather than trusting the authenticated flag, which
    /// stays true after the peer has gone away.
    pub fn is_alive(&mut self) -> bool {
        if !self.session.authenticated() {
            return false;
        }
        match self.session.keepalive_send() {
            Ok(_) => true,
            Err(e) => {
                debug!("session to {} is gone: {}", self.target.endpoint(), e);
                false
            }
        }
    }

    /// Runs a command and captures both output streams.
    ///
    /// # Errors
    /// Returns an error if the channel cannot be opened or the output cannot
    /// be read.
    pub fn exec(&mut self, command: &str) -> Result<CommandOutput, SessionError> {
        if command.is_empty() {
            return Err(SessionError::Command(
                "command must not be empty".to_string(),
            ));
        }

        self.touch();

        let mut channel = self
            .session
            .channel_session()
            .map_err(|e| SessionError::Command(format!("could not open a channel: {e}")))?;

        channel
            .exec(command)
            .map_err(|e| SessionError::Command(format!("could not run {command:?}: {e}")))?;

        // `Read::take` consumes its receiver, so read through a borrow: the
        // channel is still needed for stderr and for the exit status.
        let mut stdout = String::new();
        Read::by_ref(&mut channel)
            .take(MAX_OUTPUT_BYTES as u64)
            .read_to_string(&mut stdout)
            .map_err(|e| SessionError::Command(format!("could not read stdout: {e}")))?;

        let mut stderr = String::new();
        let mut err_stream = channel.stderr();
        Read::by_ref(&mut err_stream)
            .take(MAX_OUTPUT_BYTES as u64)
            .read_to_string(&mut stderr)
            .map_err(|e| SessionError::Command(format!("could not read stderr: {e}")))?;
        drop(err_stream);

        channel
            .wait_close()
            .map_err(|e| SessionError::Command(format!("could not close the channel: {e}")))?;

        let exit_status = channel.exit_status().unwrap_or(-1);

        Ok(CommandOutput {
            stdout,
            stderr,
            exit_status,
        })
    }

    /// Opens the SFTP subsystem.
    ///
    /// # Errors
    /// Returns an error if SFTP cannot be started.
    pub fn sftp(&mut self) -> Result<Sftp, SessionError> {
        self.touch();
        self.session
            .sftp()
            .map_err(|e| SessionError::Sftp(e.to_string()))
    }

    /// Forwards a fresh loopback port to `remote_host:remote_port`.
    ///
    /// The forward opens its own SSH connection to the same target, because a
    /// libssh2 session must not be driven from two threads. Reconnecting is
    /// cheap next to the alternative of serialising every call behind a lock.
    ///
    /// # Errors
    /// Returns an error if the extra session cannot be opened or the local
    /// port cannot be bound.
    pub fn forward_local_port(
        &mut self,
        remote_host: &str,
        remote_port: u16,
        policy: HostKeyPolicy,
    ) -> Result<PortForward, SessionError> {
        self.touch();
        let session = open_session(&self.target.endpoint(), &self.target, policy)?;
        Ok(PortForward::start(session, remote_host, remote_port)?)
    }

    /// Returns the SHA-256 fingerprint of the peer's host key.
    #[must_use]
    pub fn fingerprint(&self) -> Option<String> {
        self.session.host_key().map(|(key, _)| fingerprint_of(key))
    }
}

/// Opens and authenticates one session against `address`.
///
/// `address` is where to connect; `target` says who to authenticate as and
/// which host name the key belongs to. They differ for a jump hop, where the
/// address is a loopback forward.
fn open_session(
    address: &str,
    target: &RemoteTarget,
    policy: HostKeyPolicy,
) -> Result<Session, SessionError> {
    let socket = resolve(address)?;

    let tcp = TcpStream::connect_timeout(&socket, CONNECT_TIMEOUT).map_err(|source| {
        SessionError::Connect {
            target: address.to_string(),
            source,
        }
    })?;
    let _ = tcp.set_read_timeout(Some(IO_TIMEOUT));
    let _ = tcp.set_write_timeout(Some(IO_TIMEOUT));
    let _ = tcp.set_nodelay(true);

    let mut session = Session::new().map_err(|e| SessionError::Handshake {
        target: address.to_string(),
        message: e.to_string(),
    })?;
    session.set_timeout(u32::try_from(IO_TIMEOUT.as_millis()).unwrap_or(u32::MAX));
    session.set_tcp_stream(tcp);
    session.handshake().map_err(|e| SessionError::Handshake {
        target: address.to_string(),
        message: e.to_string(),
    })?;

    verify_host_key(&session, target, policy)?;
    authenticate(&session, target, address)?;

    Ok(session)
}

/// Resolves `host:port` to a single socket address.
fn resolve(address: &str) -> Result<SocketAddr, SessionError> {
    address
        .to_socket_addrs()
        .map_err(|_| SessionError::Resolve(address.to_string()))?
        .next()
        .ok_or_else(|| SessionError::Resolve(address.to_string()))
}

/// Checks the peer's host key against `known_hosts` and applies `policy`.
fn verify_host_key(
    session: &Session,
    target: &RemoteTarget,
    policy: HostKeyPolicy,
) -> Result<(), SessionError> {
    if policy == HostKeyPolicy::AcceptAny {
        return Ok(());
    }

    let Some((key, key_type)) = session.host_key() else {
        return Err(SessionError::Handshake {
            target: target.endpoint(),
            message: "the server offered no host key".to_string(),
        });
    };

    let mut known = session.known_hosts().map_err(|e| SessionError::Handshake {
        target: target.endpoint(),
        message: format!("could not read known hosts: {e}"),
    })?;

    let path = known_hosts_path();
    if let Some(path) = path.as_ref() {
        // A missing file is normal on a fresh machine.
        let _ = known.read_file(path, KnownHostFileKind::OpenSSH);
    }

    match known.check_port(&target.hostname, target.port, key) {
        CheckResult::Match => Ok(()),
        CheckResult::Mismatch => Err(SessionError::HostKeyMismatch {
            host: target.hostname.clone(),
            fingerprint: fingerprint_of(key),
        }),
        CheckResult::Failure => {
            // The database could not be consulted at all. Pinning now would
            // record a key that cannot be compared later, so accept the
            // connection but say so.
            warn!(
                "could not check the known-hosts database for {}",
                target.hostname
            );
            match policy {
                HostKeyPolicy::Strict | HostKeyPolicy::Prompt => {
                    Err(SessionError::UnknownHostKey {
                        host: target.hostname.clone(),
                        fingerprint: fingerprint_of(key),
                    })
                }
                HostKeyPolicy::AcceptAny | HostKeyPolicy::AcceptNew => Ok(()),
            }
        }
        CheckResult::NotFound => match policy {
            HostKeyPolicy::AcceptAny | HostKeyPolicy::AcceptNew => {
                pin_host_key(&mut known, target, key, key_type, path.as_deref());
                Ok(())
            }
            HostKeyPolicy::Strict | HostKeyPolicy::Prompt => Err(SessionError::UnknownHostKey {
                host: target.hostname.clone(),
                fingerprint: fingerprint_of(key),
            }),
        },
    }
}

/// Adds a host key to `known_hosts` and writes the file back.
fn pin_host_key(
    known: &mut ssh2::KnownHosts,
    target: &RemoteTarget,
    key: &[u8],
    key_type: ssh2::HostKeyType,
    path: Option<&Path>,
) {
    let host = if target.port == 22 {
        target.hostname.clone()
    } else {
        format!("[{}]:{}", target.hostname, target.port)
    };

    let format = match key_type {
        ssh2::HostKeyType::Rsa => KnownHostKeyFormat::SshRsa,
        ssh2::HostKeyType::Dss => KnownHostKeyFormat::SshDss,
        ssh2::HostKeyType::Ecdsa256 => KnownHostKeyFormat::Ecdsa256,
        ssh2::HostKeyType::Ecdsa384 => KnownHostKeyFormat::Ecdsa384,
        ssh2::HostKeyType::Ecdsa521 => KnownHostKeyFormat::Ecdsa521,
        ssh2::HostKeyType::Ed25519 => KnownHostKeyFormat::Ed25519,
        ssh2::HostKeyType::Unknown => {
            warn!("not pinning an unrecognised host key type for {host}");
            return;
        }
    };

    if let Err(e) = known.add(&host, key, "added by ratterm", format) {
        warn!("could not record the host key for {host}: {e}");
        return;
    }

    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = known.write_file(path, KnownHostFileKind::OpenSSH) {
        warn!("could not write {}: {e}", path.display());
    } else {
        debug!("pinned the host key for {host}");
    }
}

/// Returns the path of the user's `known_hosts` file.
#[must_use]
pub fn known_hosts_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("RATTERM_KNOWN_HOSTS") {
        return Some(PathBuf::from(explicit));
    }
    dirs::home_dir().map(|home| home.join(".ssh").join("known_hosts"))
}

/// Formats a host key the way OpenSSH prints a fingerprint.
#[must_use]
pub fn fingerprint_of(key: &[u8]) -> String {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(key);
    let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest);
    format!("SHA256:{encoded}")
}

/// Authenticates a handshaken session.
///
/// Order matters: an agent or a key file is preferred over a password, so a
/// password is the exception rather than the default path.
fn authenticate(
    session: &Session,
    target: &RemoteTarget,
    address: &str,
) -> Result<(), SessionError> {
    let mut attempts: Vec<String> = Vec::new();

    if session.userauth_agent(&target.username).is_ok() && session.authenticated() {
        return Ok(());
    }
    attempts.push("ssh-agent".to_string());

    if let Some(key_path) = target.key_path.as_ref() {
        if key_path.exists() {
            let passphrase = target.key_passphrase.as_ref().map(|p| p.as_str());
            let result = session.userauth_pubkey_file(&target.username, None, key_path, passphrase);
            if result.is_ok() && session.authenticated() {
                return Ok(());
            }
            attempts.push(format!("key {}", key_path.display()));
        } else {
            attempts.push(format!("key {} (missing)", key_path.display()));
        }
    }

    if let Some(password) = target.password.as_ref() {
        let result = session.userauth_password(&target.username, password);
        if result.is_ok() && session.authenticated() {
            return Ok(());
        }
        attempts.push("password".to_string());
    }

    Err(SessionError::Auth {
        username: target.username.clone(),
        target: address.to_string(),
        message: format!("tried {}", attempts.join(", ")),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn target(name: &str) -> RemoteTarget {
        RemoteTarget::new(name, "user")
    }

    #[test]
    fn a_target_defaults_to_port_22() {
        assert_eq!(target("host").port, 22);
        assert_eq!(target("host").endpoint(), "host:22");
    }

    #[test]
    fn builders_set_every_field() {
        let t = target("host")
            .with_port(2222)
            .with_password("secret")
            .with_key("/tmp/id_ed25519");
        assert_eq!(t.port, 2222);
        assert_eq!(t.endpoint(), "host:2222");
        assert!(t.password.is_some());
        assert_eq!(t.key_path, Some(PathBuf::from("/tmp/id_ed25519")));
    }

    #[test]
    fn debug_output_does_not_print_the_password() {
        let t = target("host").with_password("hunter2");
        let rendered = format!("{t:?}");
        assert!(!rendered.contains("hunter2"), "{rendered}");
        assert!(rendered.contains("has_password: true"), "{rendered}");
    }

    #[test]
    fn a_target_with_no_jump_is_a_single_hop() {
        let t = target("host");
        let hops = t.hop_chain().expect("chain");
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0].hostname, "host");
    }

    #[test]
    fn a_jump_chain_is_ordered_outermost_first() {
        let t = target("target").with_jump(target("inner").with_jump(target("outer")));
        let hops = t.hop_chain().expect("chain");
        let names: Vec<&str> = hops.iter().map(|h| h.hostname.as_str()).collect();
        assert_eq!(names, vec!["outer", "inner", "target"]);
    }

    #[test]
    fn an_over_long_jump_chain_is_refused() {
        let mut t = target("h0");
        for i in 1..=(MAX_JUMP_HOPS + 1) {
            t = target(&format!("h{i}")).with_jump(t);
        }
        assert!(matches!(t.hop_chain(), Err(SessionError::JumpChainTooLong)));
    }

    #[test]
    fn a_chain_exactly_at_the_limit_is_accepted() {
        let mut t = target("h0");
        for i in 1..MAX_JUMP_HOPS {
            t = target(&format!("h{i}")).with_jump(t);
        }
        let hops = t.hop_chain().expect("chain");
        assert_eq!(hops.len(), MAX_JUMP_HOPS);
    }

    #[test]
    fn command_output_reports_success_and_failure() {
        let ok = CommandOutput {
            stdout: " hello \n".to_string(),
            stderr: String::new(),
            exit_status: 0,
        };
        assert!(ok.success());
        assert_eq!(ok.trimmed(), "hello");
        assert!(ok.clone().ok().is_ok());

        let bad = CommandOutput {
            stdout: String::new(),
            stderr: "no such file\n".to_string(),
            exit_status: 2,
        };
        assert!(!bad.success());
        let err = bad.ok().expect_err("must fail");
        assert!(err.to_string().contains("no such file"), "{err}");
        assert!(err.to_string().contains('2'), "{err}");
    }

    #[test]
    fn fingerprints_match_the_openssh_format() {
        let fp = fingerprint_of(b"some key bytes");
        assert!(fp.starts_with("SHA256:"), "{fp}");
        // Base64 of a 32-byte digest without padding is 43 characters.
        assert_eq!(fp.len(), "SHA256:".len() + 43, "{fp}");
        assert!(!fp.ends_with('='), "no padding: {fp}");
    }

    #[test]
    fn fingerprints_differ_for_different_keys() {
        assert_ne!(fingerprint_of(b"key-a"), fingerprint_of(b"key-b"));
        assert_eq!(fingerprint_of(b"key-a"), fingerprint_of(b"key-a"));
    }

    #[test]
    fn the_known_hosts_path_can_be_overridden() {
        // Guards the hook the tests and the fixtures mode rely on.
        let previous = std::env::var_os("RATTERM_KNOWN_HOSTS");
        // SAFETY: single-threaded test setting a variable it also restores.
        unsafe { std::env::set_var("RATTERM_KNOWN_HOSTS", "/tmp/custom_known_hosts") };
        assert_eq!(
            known_hosts_path(),
            Some(PathBuf::from("/tmp/custom_known_hosts"))
        );
        unsafe {
            match previous {
                Some(v) => std::env::set_var("RATTERM_KNOWN_HOSTS", v),
                None => std::env::remove_var("RATTERM_KNOWN_HOSTS"),
            }
        }
    }

    #[test]
    fn resolving_a_loopback_address_works() {
        let addr = resolve("127.0.0.1:22").expect("resolve");
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 22);
    }

    #[test]
    fn resolving_nonsense_is_an_error() {
        assert!(matches!(
            resolve("not a host name at all"),
            Err(SessionError::Resolve(_))
        ));
    }

    #[test]
    fn connecting_to_a_closed_port_reports_the_endpoint() {
        // Bind and release so the port is almost certainly free.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
        let port = probe.local_addr().expect("addr").port();
        drop(probe);

        let t = target("127.0.0.1").with_port(port);
        let err = RemoteSession::connect(&t, HostKeyPolicy::AcceptAny).expect_err("must fail");
        match err {
            SessionError::Connect { target, .. } => {
                assert!(target.contains(&port.to_string()), "{target}");
            }
            other => panic!("expected a connect error, got {other}"),
        }
    }

    #[test]
    fn the_default_policy_pins_new_keys() {
        assert_eq!(HostKeyPolicy::default(), HostKeyPolicy::AcceptNew);
    }
}
