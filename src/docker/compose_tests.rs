//! Tests for `compose.rs`.
//!
//! Included with `#[path]`, so they are a child module of it and can
//! still reach its private items while living in their own file.

use super::*;
use std::collections::HashMap;

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

/// A realistic label set as Compose v2 writes it.
fn compose_labels<'a>(
    project: &'a str,
    service: &'a str,
    number: &'a str,
) -> Vec<(&'a str, &'a str)> {
    vec![
        (PROJECT_LABEL, project),
        (SERVICE_LABEL, service),
        (CONTAINER_NUMBER_LABEL, number),
        (WORKING_DIR_LABEL, "/srv/shop"),
        (CONFIG_FILES_LABEL, "/srv/shop/compose.yaml"),
    ]
}

#[test]
fn an_empty_list_groups_to_nothing() {
    let grouping = group_by_project(&[]);
    assert!(grouping.is_empty());
    assert!(grouping.unmanaged.is_empty());
    assert!(grouping.project("shop").is_none());
}

#[test]
fn containers_without_compose_labels_are_unmanaged_rather_than_dropped() {
    let grouping = group_by_project(&[
        detail("standalone", "Up", &[]),
        detail("also-plain", "Exited (0) 1 hour ago", &[("env", "dev")]),
    ]);
    assert!(grouping.is_empty());
    assert_eq!(grouping.unmanaged.len(), 2);
    assert_eq!(grouping.unmanaged[0].name, "standalone");
}

#[test]
fn a_blank_project_label_counts_as_no_project() {
    let grouping = group_by_project(&[detail("x", "Up", &[(PROJECT_LABEL, "  ")])]);
    assert!(grouping.is_empty());
    assert_eq!(grouping.unmanaged.len(), 1);
}

#[test]
fn a_real_project_groups_into_services_in_name_order() {
    let grouping = group_by_project(&[
        detail(
            "shop-web-1",
            "Up 2 hours",
            &compose_labels("shop", "web", "1"),
        ),
        detail(
            "shop-db-1",
            "Up 2 hours",
            &compose_labels("shop", "db", "1"),
        ),
        detail(
            "shop-web-2",
            "Up 2 hours",
            &compose_labels("shop", "web", "2"),
        ),
    ]);

    assert_eq!(grouping.projects.len(), 1);
    let project = grouping.project("shop").expect("the project is there");
    assert_eq!(
        project
            .services
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["db", "web"]
    );
    assert_eq!(project.container_count(), 3);
    assert_eq!(project.running_count(), 3);
    assert_eq!(project.state(), ProjectState::Running);
    assert_eq!(project.working_dir.as_deref(), Some("/srv/shop"));
    assert_eq!(project.config_files, vec!["/srv/shop/compose.yaml"]);
}

#[test]
fn replicas_are_ordered_by_their_container_number() {
    let grouping = group_by_project(&[
        detail("shop-web-10", "Up", &compose_labels("shop", "web", "10")),
        detail("shop-web-2", "Up", &compose_labels("shop", "web", "2")),
        detail("shop-web-1", "Up", &compose_labels("shop", "web", "1")),
    ]);
    let project = grouping.project("shop").expect("the project is there");
    let names: Vec<&str> = project.services[0]
        .containers
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, vec!["shop-web-1", "shop-web-2", "shop-web-10"]);
}

#[test]
fn a_container_number_that_is_not_a_number_still_sorts() {
    let grouping = group_by_project(&[
        detail("b", "Up", &compose_labels("shop", "web", "not-a-number")),
        detail("a", "Up", &compose_labels("shop", "web", "not-a-number")),
    ]);
    let project = grouping.project("shop").expect("the project is there");
    let names: Vec<&str> = project.services[0]
        .containers
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, vec!["a", "b"], "ties fall back to the name");
}

#[test]
fn a_project_container_without_a_service_label_is_filed_under_its_own_name() {
    let grouping = group_by_project(&[detail("orphan", "Up", &[(PROJECT_LABEL, "shop")])]);
    let project = grouping.project("shop").expect("the project is there");
    assert_eq!(project.services.len(), 1);
    assert_eq!(project.services[0].name, "orphan");
    assert!(project.working_dir.is_none());
    assert!(project.config_files.is_empty());
}

#[test]
fn a_partly_running_project_says_so() {
    let grouping = group_by_project(&[
        detail(
            "shop-web-1",
            "Up 2 hours",
            &compose_labels("shop", "web", "1"),
        ),
        detail(
            "shop-db-1",
            "Exited (0) 1 hour ago",
            &compose_labels("shop", "db", "1"),
        ),
    ]);
    let project = grouping.project("shop").expect("the project is there");
    assert_eq!(project.state(), ProjectState::Partial);
    assert_eq!(project.running_count(), 1);
    assert!(
        project.summary().contains("partial"),
        "{}",
        project.summary()
    );
    assert!(project.summary().contains("1/2"), "{}", project.summary());
    assert!(
        project.summary().contains("2 services"),
        "{}",
        project.summary()
    );
}

#[test]
fn a_fully_stopped_project_says_stopped() {
    let grouping = group_by_project(&[detail(
        "shop-web-1",
        "Exited (0) 1 hour ago",
        &compose_labels("shop", "web", "1"),
    )]);
    let project = grouping.project("shop").expect("the project is there");
    assert_eq!(project.state(), ProjectState::Stopped);
    assert!(
        project.summary().contains("1 service"),
        "{}",
        project.summary()
    );
    assert!(
        !project.summary().contains("services"),
        "{}",
        project.summary()
    );
}

#[test]
fn a_project_with_no_containers_reads_as_stopped() {
    let project = ComposeProject {
        name: "empty".to_string(),
        working_dir: None,
        config_files: Vec::new(),
        services: Vec::new(),
    };
    assert_eq!(project.state(), ProjectState::Stopped);
    assert!(project.container_ids().is_empty());
}

#[test]
fn several_projects_and_loose_containers_coexist() {
    let grouping = group_by_project(&[
        detail("shop-web-1", "Up", &compose_labels("shop", "web", "1")),
        detail("blog-web-1", "Up", &compose_labels("blog", "web", "1")),
        detail("scratch", "Up", &[]),
    ]);
    assert_eq!(
        grouping
            .projects
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        vec!["blog", "shop"],
        "projects come back in name order"
    );
    assert_eq!(grouping.unmanaged.len(), 1);
}

#[test]
fn container_ids_come_back_in_service_then_replica_order() {
    let grouping = group_by_project(&[
        detail("shop-web-2", "Up", &compose_labels("shop", "web", "2")),
        detail("shop-db-1", "Up", &compose_labels("shop", "db", "1")),
        detail("shop-web-1", "Up", &compose_labels("shop", "web", "1")),
    ]);
    let project = grouping.project("shop").expect("the project is there");
    assert_eq!(
        project.container_ids(),
        vec!["id-shop-db-1", "id-shop-web-1", "id-shop-web-2"]
    );
}

#[test]
fn several_config_files_are_split_and_trimmed() {
    let labels = vec![
        (PROJECT_LABEL, "shop"),
        (SERVICE_LABEL, "web"),
        (
            CONFIG_FILES_LABEL,
            "/srv/compose.yaml, /srv/compose.override.yaml , ",
        ),
    ];
    let grouping = group_by_project(&[detail("shop-web-1", "Up", &labels)]);
    let project = grouping.project("shop").expect("the project is there");
    assert_eq!(
        project.config_files,
        vec!["/srv/compose.yaml", "/srv/compose.override.yaml"]
    );
}

#[test]
fn project_states_have_names() {
    assert_eq!(ProjectState::Running.as_str(), "running");
    assert_eq!(ProjectState::Partial.as_str(), "partial");
    assert_eq!(ProjectState::Stopped.as_str(), "stopped");
}
