//! Running the `docker` CLI, locally as a child process and remotely over a
//! pooled SSH session.
//!
//! This is the legacy transport. New code should prefer [`super::client`],
//! which talks the Docker API through bollard and needs no text parsing; the
//! CLI path stays because `docker search`, `docker pull` progress and the
//! Docker Desktop start logic have no equivalent that works everywhere.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use super::host::DockerHost;

/// Timeout for Docker commands in milliseconds.
pub(super) const COMMAND_TIMEOUT_MS: u64 = 2000;

/// Timeout for remote Docker commands in milliseconds (longer for SSH overhead).
pub(super) const REMOTE_TIMEOUT_MS: u64 = 5000;

/// Quick timeout for availability check in milliseconds.
pub(super) const QUICK_TIMEOUT_MS: u64 = 1000;

/// Quick timeout for remote availability check in milliseconds.
pub(super) const REMOTE_QUICK_TIMEOUT_MS: u64 = 3000;

/// Timeout for image pull operations (10 minutes).
pub(super) const PULL_TIMEOUT_MS: u64 = 600_000;

/// Poll interval for checking if process completed.
const POLL_INTERVAL_MS: u64 = 50;

/// Returns the docker command name for the current platform.
#[must_use]
pub(super) fn docker_cmd() -> &'static str {
    if cfg!(target_os = "windows") {
        "docker.exe"
    } else {
        "docker"
    }
}

/// Builds an `ExitStatus` from a raw exit code.
///
/// `std::process::ExitStatus` has no portable constructor, but a remote command
/// still has an exit code, and the discovery code is written against
/// `std::process::Output`.
pub(super) fn exit_status_from(code: i32) -> std::process::ExitStatus {
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

/// Quotes an argument the remote shell would otherwise mangle, such as the
/// `{{json .}}` format strings.
///
/// Pure, so the quoting rule is testable without a host to run it on.
#[must_use]
pub(super) fn quote_remote_arg(arg: &str) -> String {
    if arg.contains('{') || arg.contains('}') || arg.contains(' ') {
        format!("'{}'", arg.replace('\'', "'\\''"))
    } else {
        arg.to_string()
    }
}

/// Assembles the command line sent to a remote shell.
#[must_use]
pub(super) fn remote_docker_command(docker_args: &[&str]) -> String {
    let quoted: Vec<String> = docker_args.iter().map(|a| quote_remote_arg(a)).collect();
    format!("docker {}", quoted.join(" "))
}

/// Runs a command with a timeout.
///
/// Returns None if the command times out or fails to start. The process is
/// killed and reaped on timeout, so a hung `docker` does not leak a child.
pub(super) fn run_with_timeout(cmd: &mut Command, timeout_ms: u64) -> Option<Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return None,
    };

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = drain(child.stdout.take());
                let stderr = drain(child.stderr.take());
                return Some(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(_) => {
                let _ = child.kill();
                return None;
            }
        }
    }
}

/// Reads a captured pipe to the end, treating a read error as no output.
fn drain<R: std::io::Read>(pipe: Option<R>) -> Vec<u8> {
    pipe.map(|mut s| {
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut s, &mut buf).ok();
        buf
    })
    .unwrap_or_default()
}

/// Runs a Docker command on a remote host through the pooled SSH session.
///
/// This used to build a shell string and spawn `ssh`, `sshpass` or `plink` per
/// call, which put the password in the process list, disabled host-key
/// checking, and paid for a TCP and authentication handshake every time. The
/// command now travels over an already-authenticated session; `timeout_ms` is
/// unused because the session carries its own I/O timeout.
pub(super) fn run_remote_with_timeout(
    host: &DockerHost,
    docker_args: &[&str],
    _timeout_ms: u64,
) -> Option<Output> {
    let host_id = host.host_id()?;
    let docker_cmd = remote_docker_command(docker_args);

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn exit_statuses_round_trip_through_the_conversion() {
        assert!(exit_status_from(0).success());
        assert!(!exit_status_from(1).success());
        assert_eq!(exit_status_from(3).code(), Some(3));
    }

    #[test]
    fn a_plain_argument_is_left_alone() {
        assert_eq!(quote_remote_arg("ps"), "ps");
        assert_eq!(quote_remote_arg("--all"), "--all");
    }

    #[test]
    fn a_format_string_is_quoted_for_the_remote_shell() {
        assert_eq!(quote_remote_arg("{{json .}}"), "'{{json .}}'");
        assert_eq!(quote_remote_arg("two words"), "'two words'");
    }

    #[test]
    fn an_embedded_single_quote_is_escaped_rather_than_ending_the_quote() {
        // The shell has no escape inside single quotes, so the only correct
        // form closes, escapes and reopens.
        assert_eq!(quote_remote_arg("a'b c"), r#"'a'\''b c'"#);
    }

    #[test]
    fn a_remote_command_is_assembled_with_the_docker_prefix() {
        let command = remote_docker_command(&["ps", "-a", "--format", "{{json .}}"]);
        assert_eq!(command, "docker ps -a --format '{{json .}}'");
    }

    #[test]
    fn the_docker_binary_matches_the_platform() {
        if cfg!(target_os = "windows") {
            assert_eq!(docker_cmd(), "docker.exe");
        } else {
            assert_eq!(docker_cmd(), "docker");
        }
    }

    #[test]
    fn a_command_that_cannot_start_reports_nothing_rather_than_hanging() {
        let mut cmd = Command::new("ratterm-no-such-binary-9d1f");
        assert!(run_with_timeout(&mut cmd, 500).is_none());
    }

    #[test]
    fn a_remote_command_on_an_unregistered_host_reports_failure() {
        // No target has been published for this id, so the executor refuses
        // rather than reaching for a subprocess. Before this change the same
        // call would have spawned `ssh` and hung on a password prompt.
        let host = DockerHost::remote(876_543);
        assert!(run_remote_with_timeout(&host, &["ps"], 1000).is_none());
    }

    #[test]
    fn a_local_host_has_no_remote_path() {
        assert!(run_remote_with_timeout(&DockerHost::Local, &["ps"], 1000).is_none());
    }
}
