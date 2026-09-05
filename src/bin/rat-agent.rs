//! Metric reporter for a machine in the fleet.
//!
//! The deployed daemon was a Bash script that read `/proc` and posted with
//! `curl` or `wget`. That excluded Windows and macOS entirely, and any Linux
//! host without one of those two programs. This is the same reporter as one
//! binary that runs everywhere ratterm builds.
//!
//! It posts the same `POST /metrics` payload to the same receiver, so a fleet
//! can run both during a migration.
//!
//! ```sh
//! rat-agent --host-id 3 --endpoint http://127.0.0.1:19999/metrics
//! rat-agent --host-id 3 --once            # one sample, for a cron entry
//! ```

use std::process::ExitCode;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ratterm::daemon::DEFAULT_RECEIVER_PORT;
use ratterm::telemetry::agent::{Agent, DEFAULT_INTERVAL, to_daemon_metrics};

/// How long a POST is given before it is abandoned.
///
/// Short: a reporter that blocks on an unreachable collector stops sampling,
/// and the next interval is only seconds away.
const POST_TIMEOUT: Duration = Duration::from_secs(5);

/// Command line for the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentArgs {
    /// Host id to report as, matching the id in ratterm's host list.
    host_id: u32,
    /// Where to post samples.
    endpoint: String,
    /// Seconds between samples.
    interval: Duration,
    /// Take one sample and exit.
    once: bool,
    /// Print samples instead of posting them.
    dry_run: bool,
}

impl Default for AgentArgs {
    fn default() -> Self {
        Self {
            host_id: 0,
            endpoint: format!("http://127.0.0.1:{DEFAULT_RECEIVER_PORT}/metrics"),
            interval: DEFAULT_INTERVAL,
            once: false,
            dry_run: false,
        }
    }
}

/// Usage text, printed for `--help` and for a bad argument.
fn help_text() -> String {
    format!(
        "rat-agent - report this machine's metrics to ratterm\n\
         \n\
         Usage: rat-agent --host-id <ID> [options]\n\
         \n\
         Options:\n\
           --host-id <ID>       Host id to report as (required)\n\
           --endpoint <URL>     Where to post   [default: http://127.0.0.1:{DEFAULT_RECEIVER_PORT}/metrics]\n\
           --interval <SECS>    Seconds between samples [default: {}]\n\
           --once               Take one sample and exit\n\
           --dry-run            Print the payload instead of posting it\n\
           --help               Show this message\n\
         \n\
         The endpoint is usually a reverse tunnel back to the machine running\n\
         ratterm: ssh -R {DEFAULT_RECEIVER_PORT}:127.0.0.1:{DEFAULT_RECEIVER_PORT} <collector>\n",
        DEFAULT_INTERVAL.as_secs()
    )
}

/// Parses the command line.
///
/// Returns the message to print when the arguments do not describe a run,
/// which covers both `--help` and a mistake.
fn parse_args<I>(args: I) -> Result<AgentArgs, String>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = AgentArgs::default();
    let mut host_id_given = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Err(help_text()),
            "--once" => parsed.once = true,
            "--dry-run" => parsed.dry_run = true,
            "--host-id" => {
                let value = args.next().ok_or("--host-id needs a value")?;
                parsed.host_id = value
                    .parse()
                    .map_err(|_| format!("--host-id must be a number, got '{value}'"))?;
                host_id_given = true;
            }
            "--endpoint" => {
                let value = args.next().ok_or("--endpoint needs a value")?;
                if !value.starts_with("http://") && !value.starts_with("https://") {
                    return Err(format!("--endpoint must be an http(s) URL, got '{value}'"));
                }
                parsed.endpoint = value;
            }
            "--interval" => {
                let value = args.next().ok_or("--interval needs a value")?;
                let seconds: u64 = value
                    .parse()
                    .map_err(|_| format!("--interval must be a number, got '{value}'"))?;
                if seconds == 0 {
                    return Err("--interval must be at least 1 second".to_string());
                }
                parsed.interval = Duration::from_secs(seconds);
            }
            other => return Err(format!("unknown argument '{other}'\n\n{}", help_text())),
        }
    }

    if !host_id_given {
        return Err(format!("--host-id is required\n\n{}", help_text()));
    }

    Ok(parsed)
}

/// Seconds since the Unix epoch, or zero if the clock is before it.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Samples and reports until interrupted.
fn run(args: &AgentArgs) -> ExitCode {
    let mut agent = Agent::new(args.host_id);
    let client = match reqwest::blocking::Client::builder()
        .timeout(POST_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("rat-agent: could not build an HTTP client: {e}");
            return ExitCode::FAILURE;
        }
    };

    // The first CPU reading has no previous observation to compare against, so
    // it always reads zero. Discard it rather than reporting a false idle.
    let _ = agent.sample();
    thread::sleep(args.interval.min(DEFAULT_INTERVAL));

    loop {
        let metrics = agent.sample();
        let payload = to_daemon_metrics(&metrics, unix_now());

        if args.dry_run {
            match serde_json::to_string_pretty(&payload) {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("rat-agent: could not render the sample: {e}"),
            }
        } else if let Err(e) = client.post(&args.endpoint).json(&payload).send() {
            // A collector that is down is normal — it is a TUI somebody
            // closed. Keep sampling and say so once per attempt.
            eprintln!("rat-agent: could not post to {}: {e}", args.endpoint);
        }

        if args.once {
            return ExitCode::SUCCESS;
        }

        thread::sleep(args.interval);
    }
}

fn main() -> ExitCode {
    match parse_args(std::env::args().skip(1)) {
        Ok(args) => run(&args),
        Err(message) => {
            // `--help` is a success; everything else that lands here is not.
            if message.starts_with("rat-agent - ") {
                println!("{message}");
                ExitCode::SUCCESS
            } else {
                eprintln!("rat-agent: {message}");
                ExitCode::FAILURE
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Parses a borrowed argument list, as a caller would type it.
    fn parse(args: &[&str]) -> Result<AgentArgs, String> {
        parse_args(args.iter().map(|s| (*s).to_string()))
    }

    #[test]
    fn a_host_id_is_required() {
        let error = parse(&[]).expect_err("no host id");
        assert!(error.contains("--host-id is required"), "{error}");
    }

    #[test]
    fn the_defaults_point_at_the_local_receiver() {
        let args = parse(&["--host-id", "3"]).expect("valid");
        assert_eq!(args.host_id, 3);
        assert_eq!(
            args.endpoint,
            format!("http://127.0.0.1:{DEFAULT_RECEIVER_PORT}/metrics")
        );
        assert_eq!(args.interval, DEFAULT_INTERVAL);
        assert!(!args.once);
        assert!(!args.dry_run);
    }

    #[test]
    fn every_option_is_accepted() {
        let args = parse(&[
            "--host-id",
            "12",
            "--endpoint",
            "http://10.0.0.5:9000/metrics",
            "--interval",
            "30",
            "--once",
            "--dry-run",
        ])
        .expect("valid");

        assert_eq!(args.host_id, 12);
        assert_eq!(args.endpoint, "http://10.0.0.5:9000/metrics");
        assert_eq!(args.interval, Duration::from_secs(30));
        assert!(args.once);
        assert!(args.dry_run);
    }

    #[test]
    fn help_is_reported_as_the_usage_text() {
        let message = parse(&["--help"]).expect_err("help is not a run");
        assert!(message.starts_with("rat-agent - "), "{message}");
        assert!(message.contains("--host-id"));
        assert!(message.contains("--dry-run"));
    }

    #[test]
    fn a_non_numeric_host_id_is_refused() {
        let error = parse(&["--host-id", "gpu-box"]).expect_err("not a number");
        assert!(error.contains("must be a number"), "{error}");
    }

    #[test]
    fn a_missing_value_is_refused_rather_than_defaulted() {
        assert!(parse(&["--host-id"]).is_err());
        assert!(parse(&["--host-id", "1", "--endpoint"]).is_err());
        assert!(parse(&["--host-id", "1", "--interval"]).is_err());
    }

    #[test]
    fn a_zero_interval_is_refused() {
        let error = parse(&["--host-id", "1", "--interval", "0"]).expect_err("zero");
        assert!(error.contains("at least 1 second"), "{error}");
    }

    #[test]
    fn an_endpoint_that_is_not_a_url_is_refused() {
        // A bare host:port is the most likely mistake, and it would fail at
        // the first POST with a much less helpful message.
        let error =
            parse(&["--host-id", "1", "--endpoint", "10.0.0.5:9000"]).expect_err("not a URL");
        assert!(error.contains("http(s) URL"), "{error}");
    }

    #[test]
    fn https_endpoints_are_accepted() {
        let args =
            parse(&["--host-id", "1", "--endpoint", "https://collector/metrics"]).expect("valid");
        assert_eq!(args.endpoint, "https://collector/metrics");
    }

    #[test]
    fn an_unknown_argument_is_refused_with_the_usage_text() {
        let error = parse(&["--host-id", "1", "--verbose"]).expect_err("unknown");
        assert!(error.contains("unknown argument '--verbose'"), "{error}");
        assert!(error.contains("Usage:"), "{error}");
    }

    #[test]
    fn the_clock_helper_returns_a_plausible_time() {
        let now = unix_now();
        assert!(now > 1_577_836_800, "{now}");
    }

    #[test]
    fn the_help_text_names_the_reverse_tunnel_that_makes_it_work() {
        // The endpoint default is only reachable through a tunnel; a user who
        // does not know that sees timeouts and no explanation.
        assert!(help_text().contains("ssh -R"));
    }
}
