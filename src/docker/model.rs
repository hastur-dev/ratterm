//! Turning the Docker API's structs into the shapes the UI already renders.
//!
//! Every function here is pure: it takes a value the daemon sent and returns a
//! value the rest of the application understands. That is what makes the typed
//! client testable without a daemon — a test builds a `ContainerSummary` by
//! hand and checks what comes out.

use std::collections::HashMap;

use bollard::models::{ContainerSummary, ImageSummary, Network, Port, Volume};

use super::container::{DockerContainer, DockerImage, DockerStatus};

/// Units used when a byte count is rendered for a list.
const SIZE_UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

/// A container as the daemon describes it, with the labels the UI needs.
///
/// [`DockerContainer`] is the display shape and is persisted in
/// `docker_items.toml`; adding a label map to it would change that file's
/// format for no gain. This carries the extra fields alongside instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDetail {
    /// The display shape.
    pub container: DockerContainer,
    /// Every label the daemon reported.
    pub labels: HashMap<String, String>,
    /// The API's `State` word, for example `running`.
    pub state: String,
    /// The command the container was started with.
    pub command: String,
}

impl ContainerDetail {
    /// The value of one label, if present.
    #[must_use]
    pub fn label(&self, key: &str) -> Option<&str> {
        self.labels.get(key).map(String::as_str)
    }
}

/// A Docker volume, reduced to what a list shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DockerVolume {
    /// Volume name.
    pub name: String,
    /// Storage driver, for example `local`.
    pub driver: String,
    /// Where the driver put it on the host.
    pub mountpoint: String,
    /// Labels attached to the volume.
    pub labels: HashMap<String, String>,
}

/// A Docker network, reduced to what a list shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DockerNetwork {
    /// Network id.
    pub id: String,
    /// Network name, for example `bridge`.
    pub name: String,
    /// Network driver.
    pub driver: String,
    /// Scope: `local`, `global` or `swarm`.
    pub scope: String,
    /// Whether the network is internal.
    pub internal: bool,
}

/// Formats a byte count the way `docker images` does.
#[must_use]
pub fn format_size(bytes: i64) -> String {
    if bytes <= 0 {
        return "0B".to_string();
    }

    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < SIZE_UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{value:.1}{}", SIZE_UNITS[unit])
    }
}

/// Formats a Unix timestamp as `YYYY-MM-DD HH:MM:SS` in UTC.
///
/// Returns an empty string for a timestamp of zero or one the calendar cannot
/// represent, so a missing value renders as a blank column rather than 1970.
#[must_use]
pub fn format_created(unix_seconds: i64) -> String {
    if unix_seconds <= 0 {
        return String::new();
    }
    chrono::DateTime::from_timestamp(unix_seconds, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Formats one port mapping the way `docker ps` prints it.
#[must_use]
pub fn format_port(port: &Port) -> String {
    let protocol = port
        .typ
        .map(|t| t.to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "tcp".to_string());

    match (port.public_port, port.ip.as_deref()) {
        (Some(public), Some(ip)) if !ip.is_empty() => {
            format!("{ip}:{public}->{}/{protocol}", port.private_port)
        }
        (Some(public), _) => format!("{public}->{}/{protocol}", port.private_port),
        (None, _) => format!("{}/{protocol}", port.private_port),
    }
}

/// Picks the name to show for a container.
///
/// The API returns every name with a leading slash and a container may have
/// several; the first is the one `docker ps` prints. A container with no name
/// falls back to its short id, which is what the CLI does too.
#[must_use]
pub fn display_name_for(names: &[String], id: &str) -> String {
    names
        .iter()
        .find_map(|n| {
            let trimmed = n.trim_start_matches('/').trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .unwrap_or_else(|| id.chars().take(12).collect())
}

/// Converts one `ContainerSummary` from the API.
///
/// Returns `None` for a summary with no id: there is nothing that could be
/// acted on, and [`DockerContainer`] requires one.
#[must_use]
pub fn container_from_summary(summary: &ContainerSummary) -> Option<ContainerDetail> {
    let id = summary.id.clone().filter(|id| !id.is_empty())?;

    let names = summary.names.clone().unwrap_or_default();
    let name = display_name_for(&names, &id);
    let image = summary
        .image
        .clone()
        .filter(|i| !i.is_empty())
        .unwrap_or_else(|| "<none>".to_string());
    let state = summary.state.clone().unwrap_or_default();
    let status_text = summary.status.clone().unwrap_or_default();

    // `State` is the machine-readable word and `Status` the human sentence.
    // Prefer the former; fall back so a daemon that only sends one still works.
    let status = if state.is_empty() {
        DockerStatus::parse(&status_text)
    } else {
        DockerStatus::parse(&state)
    };

    let mut container = DockerContainer::new(id, name, image, status_text);
    container.status = status;
    container.created = format_created(summary.created.unwrap_or(0));
    container.ports = summary
        .ports
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(format_port)
        .collect();
    container.ports.sort_unstable();
    container.ports.dedup();

    Some(ContainerDetail {
        container,
        labels: summary.labels.clone().unwrap_or_default(),
        state,
        command: summary.command.clone().unwrap_or_default(),
    })
}

/// Converts an `ImageSummary` into one [`DockerImage`] per repository tag.
///
/// An untagged image still appears, as `<none>:<none>`, because it can be
/// removed and its size is worth seeing.
#[must_use]
pub fn images_from_summary(summary: &ImageSummary) -> Vec<DockerImage> {
    if summary.id.is_empty() {
        return Vec::new();
    }

    let created = format_created(summary.created);
    let size = format_size(summary.size);

    if summary.repo_tags.is_empty() {
        let mut image = DockerImage::new(
            summary.id.clone(),
            "<none>".to_string(),
            "<none>".to_string(),
        );
        image.size = size;
        image.created = created;
        return vec![image];
    }

    summary
        .repo_tags
        .iter()
        .map(|repo_tag| {
            let (repository, tag) = split_repo_tag(repo_tag);
            let mut image = DockerImage::new(summary.id.clone(), repository, tag);
            image.size = size.clone();
            image.created = created.clone();
            image
        })
        .collect()
}

/// Splits `repository:tag` into its two halves.
///
/// A registry with a port — `localhost:5000/app:v1` — has a colon in the
/// repository, so the split is on the last colon that follows the last slash.
#[must_use]
pub fn split_repo_tag(repo_tag: &str) -> (String, String) {
    let last_slash = repo_tag.rfind('/').map_or(0, |i| i + 1);
    match repo_tag[last_slash..].rfind(':') {
        Some(offset) => {
            let at = last_slash + offset;
            (repo_tag[..at].to_string(), repo_tag[at + 1..].to_string())
        }
        None => (repo_tag.to_string(), "latest".to_string()),
    }
}

/// Converts a volume from the API.
#[must_use]
pub fn volume_from_api(volume: &Volume) -> DockerVolume {
    DockerVolume {
        name: volume.name.clone(),
        driver: volume.driver.clone(),
        mountpoint: volume.mountpoint.clone(),
        labels: volume.labels.clone(),
    }
}

/// Converts a network from the API.
#[must_use]
pub fn network_from_api(network: &Network) -> DockerNetwork {
    DockerNetwork {
        id: network.id.clone().unwrap_or_default(),
        name: network.name.clone().unwrap_or_default(),
        driver: network.driver.clone().unwrap_or_default(),
        scope: network.scope.clone().unwrap_or_default(),
        internal: network.internal.unwrap_or(false),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bollard::models::PortTypeEnum;

    fn summary(id: &str) -> ContainerSummary {
        ContainerSummary {
            id: Some(id.to_string()),
            names: Some(vec!["/web".to_string()]),
            image: Some("nginx:latest".to_string()),
            state: Some("running".to_string()),
            status: Some("Up 3 hours".to_string()),
            created: Some(1_700_000_000),
            ..Default::default()
        }
    }

    #[test]
    fn a_summary_becomes_a_container_with_its_state_and_ports() {
        let mut input = summary("abc123def456");
        input.ports = Some(vec![Port {
            ip: Some("0.0.0.0".to_string()),
            private_port: 80,
            public_port: Some(8080),
            typ: Some(PortTypeEnum::TCP),
        }]);
        input.labels = Some(HashMap::from([("env".to_string(), "prod".to_string())]));

        let detail = container_from_summary(&input).expect("an id is present");
        assert_eq!(detail.container.name, "web");
        assert_eq!(detail.container.image, "nginx:latest");
        assert!(detail.container.is_running());
        assert_eq!(detail.container.ports, vec!["0.0.0.0:8080->80/tcp"]);
        assert_eq!(detail.label("env"), Some("prod"));
        assert_eq!(detail.label("missing"), None);
        assert_eq!(detail.container.created, "2023-11-14 22:13:20");
    }

    #[test]
    fn a_summary_without_an_id_is_dropped_rather_than_panicking() {
        let mut input = summary("x");
        input.id = None;
        assert!(container_from_summary(&input).is_none());
        input.id = Some(String::new());
        assert!(container_from_summary(&input).is_none());
    }

    #[test]
    fn a_container_with_no_name_falls_back_to_its_short_id() {
        let mut input = summary("0123456789abcdef");
        input.names = Some(Vec::new());
        let detail = container_from_summary(&input).expect("an id is present");
        assert_eq!(detail.container.name, "0123456789ab");

        input.names = Some(vec!["/".to_string()]);
        let detail = container_from_summary(&input).expect("an id is present");
        assert_eq!(detail.container.name, "0123456789ab");
    }

    #[test]
    fn the_status_sentence_is_used_when_the_state_word_is_missing() {
        let mut input = summary("abc");
        input.state = None;
        input.status = Some("Exited (137) 2 hours ago".to_string());
        let detail = container_from_summary(&input).expect("an id is present");
        assert_eq!(detail.container.status, DockerStatus::Exited);
    }

    #[test]
    fn an_image_with_no_image_name_reads_as_none() {
        let mut input = summary("abc");
        input.image = Some(String::new());
        let detail = container_from_summary(&input).expect("an id is present");
        assert_eq!(detail.container.image, "<none>");
    }

    #[test]
    fn duplicate_port_rows_collapse() {
        let mut input = summary("abc");
        let port = Port {
            ip: None,
            private_port: 80,
            public_port: Some(8080),
            typ: Some(PortTypeEnum::TCP),
        };
        input.ports = Some(vec![port.clone(), port]);
        let detail = container_from_summary(&input).expect("an id is present");
        assert_eq!(detail.container.ports, vec!["8080->80/tcp"]);
    }

    #[test]
    fn a_port_with_no_public_side_shows_only_the_container_port() {
        let port = Port {
            ip: None,
            private_port: 5432,
            public_port: None,
            typ: Some(PortTypeEnum::TCP),
        };
        assert_eq!(format_port(&port), "5432/tcp");
    }

    #[test]
    fn a_port_with_no_protocol_defaults_to_tcp() {
        let port = Port {
            ip: Some(String::new()),
            private_port: 53,
            public_port: Some(53),
            typ: None,
        };
        assert_eq!(format_port(&port), "53->53/tcp");
    }

    #[test]
    fn a_udp_port_says_so() {
        let port = Port {
            ip: Some("127.0.0.1".to_string()),
            private_port: 53,
            public_port: Some(5353),
            typ: Some(PortTypeEnum::UDP),
        };
        assert_eq!(format_port(&port), "127.0.0.1:5353->53/udp");
    }

    #[test]
    fn sizes_are_rendered_with_a_unit() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(-5), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1_500), "1.5KB");
        assert_eq!(format_size(150_000_000), "150.0MB");
        assert_eq!(format_size(2_500_000_000), "2.5GB");
    }

    #[test]
    fn a_missing_timestamp_renders_as_nothing() {
        assert_eq!(format_created(0), "");
        assert_eq!(format_created(-1), "");
    }

    #[test]
    fn a_repo_tag_splits_on_the_tag_and_not_on_a_registry_port() {
        assert_eq!(
            split_repo_tag("nginx:latest"),
            ("nginx".to_string(), "latest".to_string())
        );
        assert_eq!(
            split_repo_tag("localhost:5000/app:v1"),
            ("localhost:5000/app".to_string(), "v1".to_string())
        );
        assert_eq!(
            split_repo_tag("localhost:5000/app"),
            ("localhost:5000/app".to_string(), "latest".to_string())
        );
        assert_eq!(
            split_repo_tag("nginx"),
            ("nginx".to_string(), "latest".to_string())
        );
    }

    fn image_summary(id: &str, tags: &[&str]) -> ImageSummary {
        ImageSummary {
            id: id.to_string(),
            repo_tags: tags.iter().map(|t| (*t).to_string()).collect(),
            created: 1_700_000_000,
            size: 150_000_000,
            ..Default::default()
        }
    }

    #[test]
    fn one_image_with_two_tags_becomes_two_rows() {
        let images =
            images_from_summary(&image_summary("sha256:a", &["nginx:1.25", "nginx:latest"]));
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].full_name(), "nginx:1.25");
        assert_eq!(images[1].full_name(), "nginx:latest");
        assert_eq!(images[0].size, "150.0MB");
    }

    #[test]
    fn an_untagged_image_still_appears() {
        let images = images_from_summary(&image_summary("sha256:b", &[]));
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].repository, "<none>");
        assert_eq!(images[0].id, "sha256:b");
    }

    #[test]
    fn an_image_with_no_id_produces_nothing() {
        assert!(images_from_summary(&image_summary("", &["nginx:latest"])).is_empty());
    }

    #[test]
    fn a_volume_carries_its_driver_and_mountpoint() {
        let volume = Volume {
            name: "data".to_string(),
            driver: "local".to_string(),
            mountpoint: "/var/lib/docker/volumes/data/_data".to_string(),
            labels: HashMap::from([("keep".to_string(), "yes".to_string())]),
            ..Default::default()
        };
        let converted = volume_from_api(&volume);
        assert_eq!(converted.name, "data");
        assert_eq!(converted.driver, "local");
        assert_eq!(
            converted.labels.get("keep").map(String::as_str),
            Some("yes")
        );
    }

    #[test]
    fn a_network_with_missing_fields_converts_to_empty_strings() {
        let converted = network_from_api(&Network::default());
        assert!(converted.id.is_empty());
        assert!(converted.name.is_empty());
        assert!(!converted.internal);

        let named = Network {
            id: Some("net1".to_string()),
            name: Some("bridge".to_string()),
            driver: Some("bridge".to_string()),
            scope: Some("local".to_string()),
            internal: Some(true),
            ..Default::default()
        };
        let converted = network_from_api(&named);
        assert_eq!(converted.name, "bridge");
        assert!(converted.internal);
    }
}
