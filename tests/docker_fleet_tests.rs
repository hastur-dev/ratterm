//! The Docker fleet, exercised through its public API.
//!
//! Nothing here needs a Docker daemon, a network or a reachable SSH host: the
//! fleet is driven with snapshots recorded by hand, which is exactly what a
//! successful refresh would have produced. That keeps the whole suite runnable
//! on a machine with no Docker installed, on any platform.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

use ratterm::docker::client::HostSnapshot;
use ratterm::docker::compose::{
    CONTAINER_NUMBER_LABEL, PROJECT_LABEL, ProjectState, SERVICE_LABEL, group_by_project,
};
use ratterm::docker::compose_ops::{ProjectAction, plan};
use ratterm::docker::container::DockerContainer;
use ratterm::docker::error::DockerError;
use ratterm::docker::events::{DockerEvents, FleetEvent};
use ratterm::docker::fleet::DockerFleet;
use ratterm::docker::fleet_rows::FleetSort;
use ratterm::docker::host::DockerHost;
use ratterm::docker::model::ContainerDetail;
use ratterm::docker::transport::{
    DEFAULT_NAMED_PIPE, DEFAULT_UNIX_SOCKET, Platform, TransportChoice, TransportProbe,
    choose_transport,
};

/// A container as the daemon would have described it.
fn detail(name: &str, status: &str, labels: &[(&str, &str)]) -> ContainerDetail {
    ContainerDetail {
        container: DockerContainer::new(
            format!("id-{name}"),
            name.to_string(),
            "img".to_string(),
            status.to_string(),
        ),
        labels: labels
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect::<HashMap<_, _>>(),
        state: status.to_lowercase(),
        command: String::new(),
    }
}

/// The label set Compose v2 writes onto a service container.
fn compose(project: &str, service: &str, replica: &str) -> Vec<(&'static str, String)> {
    vec![
        (PROJECT_LABEL, project.to_string()),
        (SERVICE_LABEL, service.to_string()),
        (CONTAINER_NUMBER_LABEL, replica.to_string()),
    ]
}

/// Borrows an owned label set as the `&str` pairs `detail` wants.
fn as_pairs<'a>(labels: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    labels.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

/// Puts a host into the connected state with data, without a daemon.
fn connect_with(fleet: &mut DockerFleet, key: Option<u32>, containers: Vec<ContainerDetail>) {
    let recorded = fleet.record_snapshot(
        key,
        "unix socket /var/run/docker.sock".to_string(),
        "the local daemon socket is present",
        HostSnapshot {
            containers,
            ..Default::default()
        },
        100,
    );
    assert!(recorded, "the host must be tracked before it is recorded");
}

#[test]
fn a_connected_host_contributes_rows_tagged_with_its_name() {
    let mut fleet = DockerFleet::new();
    fleet.track(&DockerHost::Local, "Local");
    fleet.track(&DockerHost::remote(1), "rock5c");
    connect_with(&mut fleet, None, vec![detail("web", "Up 1 hour", &[])]);
    connect_with(
        &mut fleet,
        Some(1),
        vec![
            detail("db", "Up 2 hours", &[]),
            detail("old", "Exited (0) 1 hour ago", &[]),
        ],
    );

    let rows = fleet.rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].host_label, "Local");
    assert_eq!(rows[1].host_label, "rock5c");

    let counts = fleet.counts();
    assert_eq!(counts.hosts, 2);
    assert_eq!(counts.connected, 2);
    assert_eq!(counts.containers, 3);
    assert_eq!(counts.running, 2);
    assert!(
        counts.headline().contains("2 hosts"),
        "{}",
        counts.headline()
    );
}

#[test]
fn an_unreachable_host_does_not_hide_the_others() {
    let mut fleet = DockerFleet::new();
    fleet.track(&DockerHost::Local, "Local");
    fleet.track(&DockerHost::remote(994_601), "ghost");
    connect_with(&mut fleet, None, vec![detail("web", "Up", &[])]);

    // No SSH host is registered under that id, so the refresh cannot connect.
    let error = fleet
        .refresh(Some(994_601), 200)
        .expect_err("no such SSH host");
    assert!(matches!(error, DockerError::UnknownHost(_)), "{error:?}");

    assert!(
        fleet.host(Some(994_601)).unwrap().connection.is_failed(),
        "the failure is recorded against the host that had it"
    );
    assert_eq!(fleet.rows().len(), 1, "the good host still lists its rows");
    assert_eq!(fleet.counts().failed, 1);
    assert_eq!(fleet.counts().connected, 1);

    // Refreshing everything attempts every host and files each failure against
    // the host that had it. Whether the local daemon answers depends on the
    // machine the suite runs on, so only the ghost is asserted.
    let failures = fleet.refresh_all(300);
    assert!(
        failures.iter().any(|(key, _)| *key == Some(994_601)),
        "the ghost host must be reported: {failures:?}"
    );
}

#[test]
fn the_fleet_list_can_be_sorted_and_filtered() {
    let mut fleet = DockerFleet::new();
    fleet.track(&DockerHost::Local, "Local");
    fleet.track(&DockerHost::remote(1), "rock5c");
    connect_with(&mut fleet, None, vec![detail("zeta", "Up", &[])]);
    connect_with(
        &mut fleet,
        Some(1),
        vec![
            detail("alpha", "Up", &[]),
            detail("mid", "Exited (0) 1 hour ago", &[]),
        ],
    );

    // Host order puts the local daemon first.
    assert_eq!(fleet.rows()[0].container.name, "zeta");

    fleet.set_sort(FleetSort::Name);
    let names: Vec<String> = fleet
        .rows()
        .iter()
        .map(|r| r.container.name.clone())
        .collect();
    assert_eq!(names, vec!["alpha", "mid", "zeta"]);

    fleet.set_sort(FleetSort::Status);
    assert!(fleet.rows()[0].container.is_running());
    assert!(!fleet.rows()[2].container.is_running());

    fleet.set_filter("rock5c alpha");
    assert_eq!(fleet.rows().len(), 1);
    assert_eq!(
        fleet.all_rows().len(),
        3,
        "the filter hides, it does not delete"
    );
}

#[test]
fn compose_projects_group_across_a_real_label_set() {
    let web = compose("shop", "web", "1");
    let web2 = compose("shop", "web", "2");
    let db = compose("shop", "db", "1");
    let other = compose("blog", "web", "1");

    let grouping = group_by_project(&[
        detail("shop-web-1", "Up 2 hours", &as_pairs(&web)),
        detail("shop-web-2", "Up 2 hours", &as_pairs(&web2)),
        detail("shop-db-1", "Exited (0) 1 hour ago", &as_pairs(&db)),
        detail("blog-web-1", "Up", &as_pairs(&other)),
        detail("scratch", "Up", &[]),
        detail("also-loose", "Up", &[("env", "dev")]),
    ]);

    assert_eq!(
        grouping
            .projects
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        vec!["blog", "shop"]
    );
    assert_eq!(
        grouping.unmanaged.len(),
        2,
        "containers with no project label are kept, not dropped"
    );

    let shop = grouping.project("shop").unwrap();
    assert_eq!(shop.container_count(), 3);
    assert_eq!(shop.running_count(), 2);
    assert_eq!(shop.state(), ProjectState::Partial);
    assert_eq!(
        shop.services
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["db", "web"]
    );
    assert_eq!(
        shop.container_ids(),
        vec!["id-shop-db-1", "id-shop-web-1", "id-shop-web-2"],
        "service order, then replica order"
    );

    let (start, skip) = plan(shop, ProjectAction::Start);
    assert_eq!(
        start,
        vec!["id-shop-db-1"],
        "only the stopped one is started"
    );
    assert_eq!(skip.len(), 2);
}

#[test]
fn container_events_are_kept_live_and_written_through_to_the_store() {
    let mut events = DockerEvents::ephemeral().expect("an in-memory database");
    assert!(events.is_durable());

    let event = |key, label: &str, action: &str, ts| FleetEvent {
        host_key: key,
        host_label: label.to_string(),
        container_id: "abc".to_string(),
        container_name: Some("web".to_string()),
        ts,
        action: action.to_string(),
        detail: None,
    };

    assert!(events.ingest(event(None, "Local", "create", 10)));
    assert!(events.ingest(event(None, "Local", "start", 20)));
    assert!(events.ingest(event(Some(1), "rock5c", "die", 30)));

    assert_eq!(events.live_count(), 3);
    assert_eq!(events.recent(2).len(), 2);
    assert_eq!(events.for_container("abc").len(), 3);

    let local = events.history(None, 0, i64::MAX);
    assert_eq!(local.len(), 2, "each host has its own history");
    assert_eq!(local[0].event, "create");
    assert_eq!(local[1].event, "start");

    let remote = events.history(Some(1), 0, i64::MAX);
    assert_eq!(remote.len(), 1);
    assert_eq!(remote[0].event, "die");

    // The live ring can be dropped without losing the stored history.
    events.clear_live();
    assert_eq!(events.live_count(), 0);
    assert_eq!(events.history(None, 0, i64::MAX).len(), 2);
}

#[test]
fn a_live_only_recorder_still_works_with_no_database() {
    let mut events = DockerEvents::in_memory_only();
    assert!(!events.is_durable());
    assert!(
        !events.ingest(FleetEvent {
            host_key: None,
            host_label: "Local".to_string(),
            container_id: "abc".to_string(),
            container_name: None,
            ts: 1,
            action: "start".to_string(),
            detail: None,
        }),
        "nothing was written, but the event is still live"
    );
    assert_eq!(events.live_count(), 1);
    assert!(events.history(None, 0, i64::MAX).is_empty());
}

#[test]
fn the_transport_decision_is_the_same_rule_on_every_platform() {
    // A remote host always goes through the SSH forward, whatever this machine
    // has locally: the local socket belongs to a different daemon.
    for platform in [Platform::Unix, Platform::Windows] {
        let mut probe = TransportProbe::remote(4).on(platform);
        probe.local_endpoint_present = true;
        assert!(matches!(
            choose_transport(&probe),
            TransportChoice::SshForward { host_id: 4, .. }
        ));
    }

    assert_eq!(
        choose_transport(&TransportProbe::local(true).on(Platform::Unix)),
        TransportChoice::UnixSocket(DEFAULT_UNIX_SOCKET.to_string())
    );
    assert_eq!(
        choose_transport(&TransportProbe::local(true).on(Platform::Windows)),
        TransportChoice::NamedPipe(DEFAULT_NAMED_PIPE.to_string())
    );
    assert_eq!(
        choose_transport(
            &TransportProbe::local(false)
                .on(Platform::Windows)
                .with_env(Some("tcp://10.0.0.5:2375".to_string()))
        ),
        TransportChoice::Environment("tcp://10.0.0.5:2375".to_string())
    );
    assert!(matches!(
        choose_transport(&TransportProbe::local(false).on(Platform::Unix)),
        TransportChoice::Unavailable(_)
    ));
}

#[test]
fn every_docker_error_tells_the_reader_what_to_do_next() {
    let cases = vec![
        DockerError::NoLocalEndpoint {
            probed: DEFAULT_UNIX_SOCKET.to_string(),
        },
        DockerError::UnknownHost(3),
        DockerError::Forward {
            host_id: 3,
            remote: "127.0.0.1:2375".to_string(),
            reason: "connection refused".to_string(),
        },
        DockerError::Runtime("no worker threads".to_string()),
        DockerError::UnknownComposeProject("shop".to_string()),
    ];

    for case in cases {
        let rendered = case.to_string();
        assert!(
            [
                "start", "check", "open", "add", "refresh", "restart", "pick"
            ]
            .iter()
            .any(|verb| rendered.contains(verb)),
            "no next step in: {rendered}"
        );
    }
}
