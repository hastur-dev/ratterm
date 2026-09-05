//! Tests for `fleet_rows.rs`.
//!
//! Included with `#[path]`, so they are a child module of it and can
//! still reach its private items while living in their own file.

use super::*;
use std::collections::HashMap;

fn detail(name: &str, image: &str, status: &str, labels: &[(&str, &str)]) -> ContainerDetail {
    ContainerDetail {
        container: DockerContainer::new(
            format!("id-{name}"),
            name.to_string(),
            image.to_string(),
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

fn row(host: Option<u32>, label: &str, name: &str, image: &str, status: &str) -> FleetRow {
    FleetRow::for_host(host, label, &[detail(name, image, status, &[])])
        .pop()
        .expect("one row per container")
}

#[test]
fn a_host_with_no_containers_contributes_no_rows() {
    assert!(FleetRow::for_host(Some(1), "rock5c", &[]).is_empty());
}

#[test]
fn a_row_carries_its_host_and_compose_labels() {
    let rows = FleetRow::for_host(
        Some(2),
        "cthulhu",
        &[detail(
            "shop-web-1",
            "nginx",
            "Up",
            &[(PROJECT_LABEL, "shop"), (SERVICE_LABEL, "web")],
        )],
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].host_label, "cthulhu");
    assert_eq!(rows[0].host_key, Some(2));
    assert_eq!(rows[0].compose_path().as_deref(), Some("shop/web"));
    assert!(rows[0].line().contains("[shop/web]"), "{}", rows[0].line());
    assert!(rows[0].line().starts_with('*'), "running rows are marked");
}

#[test]
fn a_container_with_a_project_but_no_service_shows_only_the_project() {
    let rows = FleetRow::for_host(
        None,
        "Local",
        &[detail("orphan", "img", "Up", &[(PROJECT_LABEL, "shop")])],
    );
    assert_eq!(rows[0].compose_path().as_deref(), Some("shop"));
}

#[test]
fn a_container_with_no_compose_labels_has_no_path() {
    let rows = FleetRow::for_host(None, "Local", &[detail("plain", "img", "Up", &[])]);
    assert!(rows[0].compose_path().is_none());
    assert!(!rows[0].line().contains('['), "{}", rows[0].line());
}

#[test]
fn a_stopped_row_is_not_marked_as_running() {
    let rows = FleetRow::for_host(
        None,
        "Local",
        &[detail("plain", "img", "Exited (0) 1 hour ago", &[])],
    );
    assert!(rows[0].line().starts_with(' '), "{}", rows[0].line());
    assert!(rows[0].line().contains("Exited"), "{}", rows[0].line());
}

#[test]
fn sorting_by_host_keeps_a_host_together_with_local_first() {
    let mut rows = vec![
        row(Some(2), "b-host", "alpha", "img", "Up"),
        row(None, "Local", "zeta", "img", "Up"),
        row(Some(1), "a-host", "mid", "img", "Up"),
        row(None, "Local", "alpha", "img", "Up"),
    ];
    sort_rows(&mut rows, FleetSort::Host);
    let order: Vec<&str> = rows.iter().map(|r| r.container.name.as_str()).collect();
    assert_eq!(order, vec!["alpha", "zeta", "mid", "alpha"]);
    assert_eq!(rows[0].host_key, None, "the local daemon sorts first");
}

#[test]
fn sorting_by_name_crosses_hosts() {
    let mut rows = vec![
        row(Some(2), "b", "zeta", "img", "Up"),
        row(Some(1), "a", "alpha", "img", "Up"),
        row(None, "Local", "mid", "img", "Up"),
    ];
    sort_rows(&mut rows, FleetSort::Name);
    let order: Vec<&str> = rows.iter().map(|r| r.container.name.as_str()).collect();
    assert_eq!(order, vec!["alpha", "mid", "zeta"]);
}

#[test]
fn sorting_by_image_groups_the_same_image_together() {
    let mut rows = vec![
        row(None, "Local", "b", "redis", "Up"),
        row(None, "Local", "a", "nginx", "Up"),
        row(Some(1), "a", "c", "nginx", "Up"),
    ];
    sort_rows(&mut rows, FleetSort::Image);
    let order: Vec<&str> = rows.iter().map(|r| r.container.image.as_str()).collect();
    assert_eq!(order, vec!["nginx", "nginx", "redis"]);
    assert_eq!(rows[0].container.name, "a", "then host, then name");
}

#[test]
fn sorting_by_status_puts_running_first() {
    let mut rows = vec![
        row(None, "Local", "stopped", "img", "Exited (0) 1 hour ago"),
        row(None, "Local", "running", "img", "Up 2 hours"),
        row(None, "Local", "restarting", "img", "Restarting"),
    ];
    sort_rows(&mut rows, FleetSort::Status);
    let order: Vec<&str> = rows.iter().map(|r| r.container.name.as_str()).collect();
    assert_eq!(order, vec!["running", "restarting", "stopped"]);
}

#[test]
fn sorting_is_stable_across_runs_whatever_the_input_order() {
    let build = || {
        vec![
            row(Some(1), "a", "b", "img", "Up"),
            row(Some(1), "a", "a", "img", "Up"),
            row(None, "Local", "a", "img", "Up"),
        ]
    };
    for sort in [
        FleetSort::Host,
        FleetSort::Name,
        FleetSort::Image,
        FleetSort::Status,
    ] {
        let mut first = build();
        let mut second = build();
        second.reverse();
        sort_rows(&mut first, sort);
        sort_rows(&mut second, sort);
        assert_eq!(first, second, "{sort:?} must be a total order");
    }
}

#[test]
fn sorting_an_empty_list_is_harmless() {
    let mut rows: Vec<FleetRow> = Vec::new();
    sort_rows(&mut rows, FleetSort::Name);
    assert!(rows.is_empty());
}

#[test]
fn an_empty_query_keeps_every_row() {
    let rows = vec![row(None, "Local", "a", "img", "Up")];
    assert_eq!(filter_rows(&rows, "").len(), 1);
    assert_eq!(filter_rows(&rows, "   ").len(), 1);
}

#[test]
fn a_query_matches_the_host_the_name_the_image_and_the_status() {
    let rows = vec![
        row(Some(1), "rock5c", "web", "nginx:latest", "Up"),
        row(None, "Local", "db", "postgres:16", "Exited (0) 1 hour ago"),
    ];
    assert_eq!(filter_rows(&rows, "rock5c").len(), 1);
    assert_eq!(filter_rows(&rows, "postgres").len(), 1);
    assert_eq!(filter_rows(&rows, "exited").len(), 1);
    assert_eq!(filter_rows(&rows, "id-web").len(), 1);
}

#[test]
fn a_query_is_case_insensitive() {
    let rows = vec![row(Some(1), "Rock5C", "Web", "NGINX", "Up")];
    assert_eq!(filter_rows(&rows, "rock5c web nginx").len(), 1);
}

#[test]
fn several_terms_must_all_match() {
    let rows = vec![
        row(Some(1), "rock5c", "web", "nginx", "Up"),
        row(Some(1), "rock5c", "db", "postgres", "Up"),
    ];
    assert_eq!(filter_rows(&rows, "rock5c web").len(), 1);
    assert_eq!(filter_rows(&rows, "rock5c redis").len(), 0);
}

#[test]
fn a_compose_path_is_searchable() {
    let rows = FleetRow::for_host(
        Some(1),
        "rock5c",
        &[detail(
            "shop-web-1",
            "nginx",
            "Up",
            &[(PROJECT_LABEL, "shop"), (SERVICE_LABEL, "web")],
        )],
    );
    assert_eq!(filter_rows(&rows, "shop/web").len(), 1);
    assert_eq!(filter_rows(&rows, "shop").len(), 1);
}

#[test]
fn the_sort_order_cycles_and_names_itself() {
    let mut sort = FleetSort::default();
    assert_eq!(sort, FleetSort::Host);
    let mut seen = Vec::new();
    for _ in 0..4 {
        seen.push(sort.as_str());
        sort = sort.next();
    }
    assert_eq!(seen, vec!["host", "name", "image", "status"]);
    assert_eq!(sort, FleetSort::Host, "four steps return to the start");
}

#[test]
fn the_headline_counts_hosts_and_containers() {
    let counts = FleetCounts {
        hosts: 6,
        connected: 4,
        failed: 2,
        containers: 23,
        running: 17,
    };
    let line = counts.headline();
    assert!(line.contains("6 hosts"), "{line}");
    assert!(line.contains("4 connected"), "{line}");
    assert!(line.contains("2 unreachable"), "{line}");
    assert!(line.contains("23 containers"), "{line}");
    assert!(line.contains("17 running"), "{line}");
}

#[test]
fn the_headline_leaves_out_unreachable_hosts_when_there_are_none() {
    let counts = FleetCounts {
        hosts: 1,
        connected: 1,
        failed: 0,
        containers: 1,
        running: 0,
    };
    let line = counts.headline();
    assert!(!line.contains("unreachable"), "{line}");
    assert!(line.contains("1 host,"), "{line}");
    assert!(line.contains("1 container "), "{line}");
}

#[test]
fn an_empty_fleet_still_produces_a_headline() {
    let line = FleetCounts::default().headline();
    assert!(line.contains("0 hosts"), "{line}");
    assert!(line.contains("0 containers"), "{line}");
}
