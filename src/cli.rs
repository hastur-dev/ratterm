//! Command-line options.
//!
//! Parsed by hand rather than with a derive macro, to match the rest of the
//! entry point and to keep the dependency list short. The flags that matter
//! here are the ones that make the application drivable without a terminal:
//! `--headless`, `--scenario`, `--fixtures`, and the endpoint selection.

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::api::ApiEndpoint;

/// Default frame size for a headless run.
pub const DEFAULT_HEADLESS_SIZE: (u16, u16) = (120, 40);

/// Everything the entry point needs to know from the command line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOptions {
    /// Run with no terminal, at this frame size.
    pub headless: Option<(u16, u16)>,
    /// Scenario file to run.
    pub scenario: Option<PathBuf>,
    /// Directory of scenario files to run.
    pub scenario_dir: Option<PathBuf>,
    /// Where to write snapshots and reports.
    pub results_dir: Option<PathBuf>,
    /// Explicit control-API endpoint.
    pub api_endpoint: Option<ApiEndpoint>,
    /// Do not start the control API at all.
    pub no_api: bool,
    /// Do not require a token on the control API.
    ///
    /// Only for a scenario run on a machine the user already trusts; the flag
    /// exists so a test harness does not have to read the token file.
    pub api_no_auth: bool,
    /// Load hosts, Docker items and metrics from this directory instead of
    /// the user's real configuration.
    pub fixtures: Option<PathBuf>,
    /// Skip the update check.
    pub no_update: bool,
    /// Enable the F1/F2/F3 test keys.
    pub test_keys: bool,
    /// File to open.
    pub file: Option<String>,
}

impl CliOptions {
    /// Returns true if this invocation runs scenarios rather than a session.
    #[must_use]
    pub const fn is_scenario_run(&self) -> bool {
        self.scenario.is_some() || self.scenario_dir.is_some()
    }

    /// Returns true if no real terminal should be used.
    #[must_use]
    pub const fn is_headless(&self) -> bool {
        self.headless.is_some() || self.is_scenario_run()
    }

    /// Returns the frame size for a headless run.
    #[must_use]
    pub fn headless_size(&self) -> (u16, u16) {
        self.headless.unwrap_or(DEFAULT_HEADLESS_SIZE)
    }
}

/// Parses the arguments after the program name.
///
/// # Errors
/// Returns a message naming the flag that could not be understood.
pub fn parse(args: &[String]) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        index += 1;

        match arg {
            "--headless" => {
                // The size is optional: `--headless` alone is the default
                // frame, `--headless 120x40` is explicit.
                match args.get(index) {
                    Some(next) if !next.starts_with('-') => {
                        options.headless = Some(parse_size(next)?);
                        index += 1;
                    }
                    _ => options.headless = Some(DEFAULT_HEADLESS_SIZE),
                }
            }
            "--scenario" => {
                options.scenario = Some(PathBuf::from(value_for(arg, args, &mut index)?));
            }
            "--scenario-dir" => {
                options.scenario_dir = Some(PathBuf::from(value_for(arg, args, &mut index)?));
            }
            "--results-dir" => {
                options.results_dir = Some(PathBuf::from(value_for(arg, args, &mut index)?));
            }
            "--api-socket" => {
                let value = value_for(arg, args, &mut index)?;
                options.api_endpoint = Some(ApiEndpoint::Local(value));
            }
            "--api-tcp" => {
                let value = value_for(arg, args, &mut index)?;
                options.api_endpoint = Some(ApiEndpoint::Tcp(parse_tcp_addr(&value)?));
            }
            "--no-api" => options.no_api = true,
            "--api-no-auth" => options.api_no_auth = true,
            "--fixtures" => {
                options.fixtures = Some(PathBuf::from(value_for(arg, args, &mut index)?));
            }
            "--no-update" => options.no_update = true,
            "--test-keys" => options.test_keys = true,
            other if other.starts_with('-') => {
                // Unknown flags are left alone: the entry point handles
                // --version, --update and friends before this runs.
            }
            other => {
                if options.file.is_none() {
                    options.file = Some(other.to_string());
                }
            }
        }
    }

    if options.no_api && options.api_endpoint.is_some() {
        return Err("--no-api and an explicit endpoint cannot both be given".to_string());
    }

    Ok(options)
}

/// Reads the value that follows a flag.
fn value_for(flag: &str, args: &[String], index: &mut usize) -> Result<String, String> {
    let value = args
        .get(*index)
        .ok_or_else(|| format!("{flag} needs a value"))?;
    if value.starts_with('-') {
        return Err(format!("{flag} needs a value, found {value:?}"));
    }
    *index += 1;
    Ok(value.clone())
}

/// Parses `WIDTHxHEIGHT`.
fn parse_size(text: &str) -> Result<(u16, u16), String> {
    crate::scenario::step::parse_size(text)
}

/// Parses a loopback address, accepting a bare port.
fn parse_tcp_addr(text: &str) -> Result<SocketAddr, String> {
    if let Ok(port) = text.parse::<u16>() {
        if port == 0 {
            return Err("--api-tcp needs a non-zero port".to_string());
        }
        return Ok(SocketAddr::from(([127, 0, 0, 1], port)));
    }

    let addr: SocketAddr = text
        .parse()
        .map_err(|_| format!("--api-tcp value {text:?} is not an address or a port"))?;

    if !addr.ip().is_loopback() {
        return Err(format!(
            "--api-tcp {addr} is not a loopback address; the control endpoint is never exposed to the network"
        ));
    }

    Ok(addr)
}

/// Returns the help text for the flags parsed here.
#[must_use]
pub fn help_text() -> &'static str {
    "\
Automation options:
  --headless [WxH]        Run with no terminal, rendering to an off-screen
                          buffer (default 120x40)
  --scenario <file>       Run a scenario file and exit with its verdict
  --scenario-dir <dir>    Run every scenario in a directory
  --results-dir <dir>     Where snapshots are written (default test-results)
  --api-socket <name>     Named pipe (Windows) or socket path (Unix) for the
                          control API
  --api-tcp <addr|port>   Loopback TCP endpoint for the control API, which
                          forwards over ssh -L; a token is always required
  --api-no-auth           Do not require a token (local, trusted runs only)
  --no-api                Do not start the control API
  --fixtures <dir>        Load hosts, Docker items and metrics from a fixture
                          directory instead of the real configuration, so a
                          run never touches real machines
"
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn no_arguments_means_an_ordinary_session() {
        let options = parse(&[]).expect("parse");
        assert!(!options.is_headless());
        assert!(!options.is_scenario_run());
        assert!(options.file.is_none());
        assert!(options.api_endpoint.is_none());
    }

    #[test]
    fn a_bare_file_argument_is_the_file_to_open() {
        let options = parse(&args(&["src/main.rs"])).expect("parse");
        assert_eq!(options.file.as_deref(), Some("src/main.rs"));
    }

    #[test]
    fn only_the_first_bare_argument_is_taken_as_the_file() {
        let options = parse(&args(&["a.txt", "b.txt"])).expect("parse");
        assert_eq!(options.file.as_deref(), Some("a.txt"));
    }

    #[test]
    fn headless_defaults_to_a_usable_frame() {
        let options = parse(&args(&["--headless"])).expect("parse");
        assert_eq!(options.headless, Some(DEFAULT_HEADLESS_SIZE));
        assert!(options.is_headless());
    }

    #[test]
    fn headless_accepts_an_explicit_size() {
        let options = parse(&args(&["--headless", "80x24"])).expect("parse");
        assert_eq!(options.headless, Some((80, 24)));
    }

    #[test]
    fn headless_followed_by_another_flag_uses_the_default_size() {
        let options = parse(&args(&["--headless", "--no-update"])).expect("parse");
        assert_eq!(options.headless, Some(DEFAULT_HEADLESS_SIZE));
        assert!(options.no_update);
    }

    #[test]
    fn a_bad_headless_size_is_reported() {
        assert!(parse(&args(&["--headless", "wide"])).is_err());
        assert!(parse(&args(&["--headless", "0x40"])).is_err());
    }

    #[test]
    fn a_scenario_run_is_headless_by_implication() {
        let options = parse(&args(&["--scenario", "tests/scenarios/open.yaml"])).expect("parse");
        assert!(options.is_scenario_run());
        assert!(options.is_headless());
        assert_eq!(options.headless_size(), DEFAULT_HEADLESS_SIZE);
    }

    #[test]
    fn a_scenario_directory_is_accepted() {
        let options = parse(&args(&["--scenario-dir", "tests/scenarios"])).expect("parse");
        assert_eq!(options.scenario_dir, Some(PathBuf::from("tests/scenarios")));
    }

    #[test]
    fn a_flag_without_its_value_is_reported() {
        let err = parse(&args(&["--scenario"])).expect_err("must fail");
        assert!(err.contains("--scenario"), "{err}");

        let err = parse(&args(&["--scenario", "--headless"])).expect_err("must fail");
        assert!(err.contains("--scenario"), "{err}");
    }

    #[test]
    fn a_named_endpoint_is_accepted() {
        let options = parse(&args(&["--api-socket", "/tmp/ratterm-1.sock"])).expect("parse");
        match options.api_endpoint {
            Some(ApiEndpoint::Local(name)) => assert_eq!(name, "/tmp/ratterm-1.sock"),
            other => panic!("expected a local endpoint, got {other:?}"),
        }
    }

    #[test]
    fn a_tcp_port_becomes_a_loopback_address() {
        let options = parse(&args(&["--api-tcp", "47113"])).expect("parse");
        match options.api_endpoint {
            Some(ApiEndpoint::Tcp(addr)) => {
                assert!(addr.ip().is_loopback());
                assert_eq!(addr.port(), 47_113);
            }
            other => panic!("expected a TCP endpoint, got {other:?}"),
        }
    }

    #[test]
    fn a_full_loopback_address_is_accepted() {
        let options = parse(&args(&["--api-tcp", "127.0.0.1:9000"])).expect("parse");
        match options.api_endpoint {
            Some(ApiEndpoint::Tcp(addr)) => assert_eq!(addr.port(), 9000),
            other => panic!("expected a TCP endpoint, got {other:?}"),
        }
    }

    #[test]
    fn a_public_tcp_address_is_refused() {
        let err = parse(&args(&["--api-tcp", "0.0.0.0:9000"])).expect_err("must fail");
        assert!(err.contains("loopback"), "{err}");
    }

    #[test]
    fn a_nonsense_tcp_value_is_refused() {
        assert!(parse(&args(&["--api-tcp", "not-an-address"])).is_err());
        assert!(parse(&args(&["--api-tcp", "0"])).is_err());
    }

    #[test]
    fn no_api_conflicts_with_an_explicit_endpoint() {
        let err = parse(&args(&["--no-api", "--api-tcp", "9000"])).expect_err("must fail");
        assert!(err.contains("--no-api"), "{err}");
    }

    #[test]
    fn fixtures_and_results_directories_are_captured() {
        let options = parse(&args(&[
            "--fixtures",
            "tests/fixtures/fleet",
            "--results-dir",
            "out",
        ]))
        .expect("parse");
        assert_eq!(
            options.fixtures,
            Some(PathBuf::from("tests/fixtures/fleet"))
        );
        assert_eq!(options.results_dir, Some(PathBuf::from("out")));
    }

    #[test]
    fn unknown_flags_are_left_for_the_entry_point() {
        // `--version` and friends are handled before this parser runs; an
        // unknown flag must not be mistaken for the file to open.
        let options = parse(&args(&["--version"])).expect("parse");
        assert!(options.file.is_none());
    }

    #[test]
    fn the_help_text_mentions_every_automation_flag() {
        let help = help_text();
        for flag in [
            "--headless",
            "--scenario",
            "--scenario-dir",
            "--results-dir",
            "--api-socket",
            "--api-tcp",
            "--api-no-auth",
            "--no-api",
            "--fixtures",
        ] {
            assert!(help.contains(flag), "{flag} missing from the help text");
        }
    }
}
