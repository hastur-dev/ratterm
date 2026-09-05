//! Canned state for scenario runs.
//!
//! A fixture directory stands in for the user's `~/.ratterm`, so a scripted
//! run has hosts, Docker entries and metrics to look at without connecting to
//! anything. Loading fixtures also clears the remote executor's target table,
//! which is what guarantees the run cannot reach a real machine: a remote call
//! then fails with "unknown host" instead of dialling out.
//!
//! Layout:
//!
//! ```text
//! fixtures/fleet/
//!   ssh_hosts.toml     hosts and credentials, same format as the real file
//!   docker_items.toml  quick-connect slots and the selected host
//!   metrics.json       one entry per host, seeding the health dashboard
//! ```
//!
//! Every file is optional; a directory with only `ssh_hosts.toml` is valid.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use crate::ssh::SSHHostList;

/// File names inside a fixture directory.
pub const HOSTS_FILE: &str = "ssh_hosts.toml";
/// Docker entries file name.
pub const DOCKER_FILE: &str = "docker_items.toml";
/// Metrics file name.
pub const METRICS_FILE: &str = "metrics.json";

/// Largest fixture file that will be read.
const MAX_FIXTURE_BYTES: u64 = 4 * 1024 * 1024;

/// Errors raised while loading fixtures.
#[derive(Debug, Error)]
pub enum FixtureError {
    /// The directory does not exist or is not a directory.
    #[error("fixture directory {0} does not exist")]
    MissingDirectory(PathBuf),

    /// A file could not be read.
    #[error("could not read {path}: {source}")]
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },

    /// A file was larger than [`MAX_FIXTURE_BYTES`].
    #[error("fixture file {0} is too large")]
    TooLarge(PathBuf),

    /// A file could not be parsed.
    #[error("could not parse {path}: {message}")]
    Parse {
        /// The file that could not be parsed.
        path: PathBuf,
        /// Parser message.
        message: String,
    },
}

/// One host's canned metrics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FixtureMetrics {
    /// Host id these metrics belong to.
    pub host_id: u32,
    /// One-minute load average.
    #[serde(default)]
    pub cpu_load1: Option<f64>,
    /// CPU use as a percentage.
    #[serde(default)]
    pub cpu_percent: Option<f64>,
    /// Memory used, in bytes.
    #[serde(default)]
    pub mem_used_bytes: Option<u64>,
    /// Memory total, in bytes.
    #[serde(default)]
    pub mem_total_bytes: Option<u64>,
    /// Disk used, in bytes.
    #[serde(default)]
    pub disk_used_bytes: Option<u64>,
    /// Disk total, in bytes.
    #[serde(default)]
    pub disk_total_bytes: Option<u64>,
    /// GPU use as a percentage.
    #[serde(default)]
    pub gpu_util_percent: Option<f64>,
    /// Whether the host should appear reachable.
    #[serde(default = "default_true")]
    pub online: bool,
}

fn default_true() -> bool {
    true
}

/// Everything a fixture directory provides.
#[derive(Debug, Default)]
pub struct Fixtures {
    /// Where the fixtures came from.
    pub source: PathBuf,
    /// Hosts and credentials.
    pub hosts: SSHHostList,
    /// Raw Docker entries, parsed by the Docker storage layer.
    pub docker_items_toml: Option<String>,
    /// Canned metrics, keyed by host id.
    pub metrics: HashMap<u32, FixtureMetrics>,
}

impl Fixtures {
    /// Loads a fixture directory.
    ///
    /// # Errors
    /// Returns an error if the directory is missing or a file is unreadable or
    /// malformed. A missing individual file is not an error.
    pub fn load(dir: &Path) -> Result<Self, FixtureError> {
        if !dir.is_dir() {
            return Err(FixtureError::MissingDirectory(dir.to_path_buf()));
        }

        let hosts = match read_optional(&dir.join(HOSTS_FILE))? {
            None => SSHHostList::new(),
            Some(text) => parse_hosts(&dir.join(HOSTS_FILE), &text)?,
        };

        let docker_items_toml = read_optional(&dir.join(DOCKER_FILE))?;

        let metrics = match read_optional(&dir.join(METRICS_FILE))? {
            None => HashMap::new(),
            Some(text) => {
                let path = dir.join(METRICS_FILE);
                let list: Vec<FixtureMetrics> =
                    serde_json::from_str(&text).map_err(|e| FixtureError::Parse {
                        path: path.clone(),
                        message: e.to_string(),
                    })?;
                list.into_iter().map(|m| (m.host_id, m)).collect()
            }
        };

        info!(
            "loaded fixtures from {}: {} hosts, {} metric entries",
            dir.display(),
            hosts.len(),
            metrics.len()
        );

        Ok(Self {
            source: dir.to_path_buf(),
            hosts,
            docker_items_toml,
            metrics,
        })
    }

    /// Returns how many hosts the fixtures describe.
    #[must_use]
    pub fn host_count(&self) -> usize {
        self.hosts.len()
    }

    /// Returns the metrics for a host, if any.
    #[must_use]
    pub fn metrics_for(&self, host_id: u32) -> Option<&FixtureMetrics> {
        self.metrics.get(&host_id)
    }

    /// Clears the shared remote executor.
    ///
    /// Called after loading so nothing can dial a real machine: every host id
    /// becomes unknown to the executor, and a remote call fails immediately
    /// with a message saying so.
    pub fn isolate_remote_access() {
        crate::remote::with_shared(|executor| {
            executor.close_all();
            executor.set_targets(HashMap::new());
        });
    }
}

/// Reads a file, or returns `None` if it does not exist.
fn read_optional(path: &Path) -> Result<Option<String>, FixtureError> {
    if !path.exists() {
        return Ok(None);
    }

    let size = std::fs::metadata(path)
        .map_err(|source| FixtureError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    if size > MAX_FIXTURE_BYTES {
        return Err(FixtureError::TooLarge(path.to_path_buf()));
    }

    std::fs::read_to_string(path)
        .map(Some)
        .map_err(|source| FixtureError::Io {
            path: path.to_path_buf(),
            source,
        })
}

/// Parses a host list, tolerating the `[settings]` block the real file has.
fn parse_hosts(path: &Path, text: &str) -> Result<SSHHostList, FixtureError> {
    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(flatten)]
        hosts: SSHHostList,
    }

    let wrapper: Wrapper = toml::from_str(text).map_err(|e| FixtureError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    Ok(wrapper.hosts)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).expect("write fixture");
    }

    const HOSTS: &str = r#"
next_id = 3

[[hosts]]
id = 1
hostname = "10.0.0.10"
port = 22
display_name = "fixture-a"
connection_count = 0

[[hosts]]
id = 2
hostname = "10.0.0.11"
port = 2222
display_name = "fixture-b"
connection_count = 0

[credentials.1]
username = "alice"
password = "not-a-real-password"
save = true

[credentials.2]
username = "bob"
save = true
"#;

    #[test]
    fn a_missing_directory_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("absent");
        assert!(matches!(
            Fixtures::load(&missing),
            Err(FixtureError::MissingDirectory(_))
        ));
    }

    #[test]
    fn an_empty_directory_loads_as_empty_fixtures() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fixtures = Fixtures::load(dir.path()).expect("load");
        assert_eq!(fixtures.host_count(), 0);
        assert!(fixtures.docker_items_toml.is_none());
        assert!(fixtures.metrics.is_empty());
    }

    #[test]
    fn hosts_and_credentials_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), HOSTS_FILE, HOSTS);

        let fixtures = Fixtures::load(dir.path()).expect("load");
        assert_eq!(fixtures.host_count(), 2);

        let host = fixtures.hosts.get_by_id(1).expect("host 1");
        assert_eq!(host.hostname, "10.0.0.10");
        assert_eq!(host.display_name.as_deref(), Some("fixture-a"));

        let creds = fixtures.hosts.get_credentials(1).expect("credentials");
        assert_eq!(creds.username, "alice");
    }

    #[test]
    fn metrics_load_and_are_keyed_by_host() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            METRICS_FILE,
            r#"[
              {"host_id": 1, "cpu_percent": 42.5, "mem_used_bytes": 1024, "online": true},
              {"host_id": 2, "online": false}
            ]"#,
        );

        let fixtures = Fixtures::load(dir.path()).expect("load");
        assert_eq!(fixtures.metrics.len(), 2);

        let first = fixtures.metrics_for(1).expect("metrics for host 1");
        assert_eq!(first.cpu_percent, Some(42.5));
        assert_eq!(first.mem_used_bytes, Some(1024));
        assert!(first.online);

        let second = fixtures.metrics_for(2).expect("metrics for host 2");
        assert!(!second.online);
        assert_eq!(second.cpu_percent, None);
        assert!(fixtures.metrics_for(99).is_none());
    }

    #[test]
    fn metrics_default_to_online() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), METRICS_FILE, r#"[{"host_id": 1}]"#);
        let fixtures = Fixtures::load(dir.path()).expect("load");
        assert!(fixtures.metrics_for(1).expect("metrics").online);
    }

    #[test]
    fn docker_entries_are_kept_as_text_for_the_storage_layer() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), DOCKER_FILE, "next_slot = 1\n");
        let fixtures = Fixtures::load(dir.path()).expect("load");
        assert_eq!(
            fixtures.docker_items_toml.as_deref(),
            Some("next_slot = 1\n")
        );
    }

    #[test]
    fn a_malformed_host_file_is_reported_with_its_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), HOSTS_FILE, "this is not [ toml");
        match Fixtures::load(dir.path()) {
            Err(FixtureError::Parse { path, .. }) => {
                assert!(path.ends_with(HOSTS_FILE), "{}", path.display());
            }
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_metrics_file_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), METRICS_FILE, "{not json");
        assert!(matches!(
            Fixtures::load(dir.path()),
            Err(FixtureError::Parse { .. })
        ));
    }

    #[test]
    fn an_oversized_fixture_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let big = "#".repeat((MAX_FIXTURE_BYTES + 1) as usize);
        write(dir.path(), HOSTS_FILE, &big);
        assert!(matches!(
            Fixtures::load(dir.path()),
            Err(FixtureError::TooLarge(_))
        ));
    }

    #[test]
    fn isolating_remote_access_leaves_no_reachable_hosts() {
        // Publish something first so the clearing is observable.
        let mut target = crate::remote::RemoteTarget::new("10.0.0.10", "alice");
        target.host_id = Some(1);
        crate::remote::with_shared(|executor| executor.set_target(1, target));

        Fixtures::isolate_remote_access();

        let known = crate::remote::with_shared(|executor| executor.known_hosts());
        assert_eq!(known, 0, "a fixture run must not be able to reach a host");
    }

    #[test]
    fn a_settings_block_in_the_host_file_is_tolerated() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            HOSTS_FILE,
            "next_id = 2\n\n[settings]\nstorage_mode = \"plaintext\"\n\n[[hosts]]\nid = 1\nhostname = \"h\"\nport = 22\nconnection_count = 0\n\n[credentials.1]\nusername = \"u\"\nsave = true\n",
        );
        let fixtures = Fixtures::load(dir.path()).expect("load");
        assert_eq!(fixtures.host_count(), 1);
    }
}
