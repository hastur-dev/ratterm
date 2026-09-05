//! The service list view.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::{Service, ServicePort};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

use super::{age_at, age_display, labels_of, name_of, namespace_of, to_utc};

/// One published port of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicePortView {
    /// Port name, set when a service publishes more than one port.
    pub name: Option<String>,
    /// The port the service listens on.
    pub port: i32,
    /// The pod port traffic is sent to, as written in the spec: either a
    /// number or a named port.
    pub target_port: Option<String>,
    /// The node port, for `NodePort` and `LoadBalancer` services.
    pub node_port: Option<i32>,
    /// Transport protocol. Defaults to `TCP`, as the API does.
    pub protocol: String,
}

impl ServicePortView {
    /// Builds the view from an API object.
    #[must_use]
    pub fn from_api(port: &ServicePort) -> Self {
        Self {
            name: port.name.clone().filter(|n| !n.is_empty()),
            port: port.port,
            target_port: port.target_port.as_ref().map(|t| match t {
                IntOrString::Int(n) => n.to_string(),
                IntOrString::String(s) => s.clone(),
            }),
            node_port: port.node_port,
            protocol: port
                .protocol
                .clone()
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "TCP".to_string()),
        }
    }

    /// Renders the port the way `kubectl get svc` does: `80:30080/TCP`, or
    /// `80/TCP` when there is no node port.
    #[must_use]
    pub fn display(&self) -> String {
        match self.node_port {
            Some(node_port) => format!("{}:{}/{}", self.port, node_port, self.protocol),
            None => format!("{}/{}", self.port, self.protocol),
        }
    }
}

/// A service as a list row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceView {
    /// Service name.
    pub name: String,
    /// Namespace the service is in.
    pub namespace: String,
    /// Service type. Defaults to `ClusterIP`, as the API does.
    pub service_type: String,
    /// Cluster IP, absent for headless services (`None` in the spec) and for
    /// `ExternalName` services.
    pub cluster_ip: Option<String>,
    /// Addresses reachable from outside the cluster: the spec's external IPs
    /// plus any load balancer ingress address the cluster has assigned.
    pub external_addresses: Vec<String>,
    /// Published ports.
    pub ports: Vec<ServicePortView>,
    /// The pod selector, empty for services with manual endpoints.
    pub selector: BTreeMap<String, String>,
    /// Service labels.
    pub labels: BTreeMap<String, String>,
    /// Creation time, absent only on malformed objects.
    pub created: Option<DateTime<Utc>>,
}

impl ServiceView {
    /// Builds the view from an API object.
    #[must_use]
    pub fn from_api(service: &Service) -> Self {
        let meta = &service.metadata;
        let spec = service.spec.as_ref();

        let mut external_addresses: Vec<String> = spec
            .and_then(|s| s.external_ips.clone())
            .unwrap_or_default();

        if let Some(ingress) = service
            .status
            .as_ref()
            .and_then(|s| s.load_balancer.as_ref())
            .and_then(|lb| lb.ingress.as_ref())
        {
            for entry in ingress {
                if let Some(ip) = entry.ip.as_ref().filter(|v| !v.is_empty()) {
                    external_addresses.push(ip.clone());
                } else if let Some(host) = entry.hostname.as_ref().filter(|v| !v.is_empty()) {
                    external_addresses.push(host.clone());
                }
            }
        }

        // An ExternalName service has no addresses of its own; its target is
        // the useful thing to show in that column.
        if let Some(external_name) = spec
            .and_then(|s| s.external_name.clone())
            .filter(|n| !n.is_empty())
        {
            external_addresses.push(external_name);
        }

        Self {
            name: name_of(meta),
            namespace: namespace_of(meta),
            service_type: spec
                .and_then(|s| s.type_.clone())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "ClusterIP".to_string()),
            cluster_ip: spec
                .and_then(|s| s.cluster_ip.clone())
                .filter(|ip| !ip.is_empty() && ip != "None"),
            external_addresses,
            ports: spec
                .and_then(|s| s.ports.as_ref())
                .map(|ports| ports.iter().map(ServicePortView::from_api).collect())
                .unwrap_or_default(),
            selector: spec.and_then(|s| s.selector.clone()).unwrap_or_default(),
            labels: labels_of(meta),
            created: to_utc(meta.creation_timestamp.as_ref()),
        }
    }

    /// Returns the service's age at `now`.
    #[must_use]
    pub fn age(&self, now: DateTime<Utc>) -> Option<Duration> {
        age_at(self.created, now)
    }

    /// Returns the short age string for the list, or `-`.
    #[must_use]
    pub fn age_display(&self, now: DateTime<Utc>) -> String {
        age_display(self.created, now)
    }

    /// Renders every port for the ports column, or `-` when there are none.
    #[must_use]
    pub fn ports_display(&self) -> String {
        if self.ports.is_empty() {
            return "-".to_string();
        }
        self.ports
            .iter()
            .map(ServicePortView::display)
            .collect::<Vec<_>>()
            .join(",")
    }

    /// True for a headless service, which has no cluster IP of its own.
    #[must_use]
    pub const fn is_headless(&self) -> bool {
        self.cluster_ip.is_none()
    }
}

/// Orders services for a list: namespace, then name.
#[must_use]
pub fn compare_services(a: &ServiceView, b: &ServiceView) -> Ordering {
    a.namespace
        .cmp(&b.namespace)
        .then_with(|| a.name.cmp(&b.name))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use k8s_openapi::api::core::v1::{
        LoadBalancerIngress, LoadBalancerStatus, ServiceSpec, ServiceStatus,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::super::test_support::{time_at, utc_at};
    use super::*;

    fn full_service() -> Service {
        Service {
            metadata: ObjectMeta {
                name: Some("api".to_string()),
                namespace: Some("web".to_string()),
                labels: Some(BTreeMap::from([("tier".to_string(), "front".to_string())])),
                creation_timestamp: Some(time_at(1_000)),
                ..ObjectMeta::default()
            },
            spec: Some(ServiceSpec {
                type_: Some("LoadBalancer".to_string()),
                cluster_ip: Some("10.96.0.42".to_string()),
                external_ips: Some(vec!["203.0.113.9".to_string()]),
                selector: Some(BTreeMap::from([("app".to_string(), "api".to_string())])),
                ports: Some(vec![
                    ServicePort {
                        name: Some("http".to_string()),
                        port: 80,
                        node_port: Some(30_080),
                        protocol: Some("TCP".to_string()),
                        target_port: Some(IntOrString::Int(8080)),
                        ..ServicePort::default()
                    },
                    ServicePort {
                        name: Some("metrics".to_string()),
                        port: 9090,
                        target_port: Some(IntOrString::String("metrics".to_string())),
                        ..ServicePort::default()
                    },
                ]),
                ..ServiceSpec::default()
            }),
            status: Some(ServiceStatus {
                load_balancer: Some(LoadBalancerStatus {
                    ingress: Some(vec![LoadBalancerIngress {
                        ip: Some("198.51.100.4".to_string()),
                        ..LoadBalancerIngress::default()
                    }]),
                }),
                ..ServiceStatus::default()
            }),
        }
    }

    #[test]
    fn a_fully_populated_service_converts_every_field() {
        let view = ServiceView::from_api(&full_service());

        assert_eq!(view.name, "api");
        assert_eq!(view.namespace, "web");
        assert_eq!(view.service_type, "LoadBalancer");
        assert_eq!(view.cluster_ip.as_deref(), Some("10.96.0.42"));
        assert_eq!(view.external_addresses, vec!["203.0.113.9", "198.51.100.4"]);
        assert_eq!(view.ports.len(), 2);
        assert_eq!(view.selector.get("app").map(String::as_str), Some("api"));
        assert_eq!(view.labels.get("tier").map(String::as_str), Some("front"));
        assert_eq!(view.created, Some(utc_at(1_000)));
        assert_eq!(view.ports_display(), "80:30080/TCP,9090/TCP");
        assert!(!view.is_headless());
    }

    #[test]
    fn a_minimal_service_converts_to_documented_defaults() {
        let view = ServiceView::from_api(&Service::default());

        assert_eq!(view.name, "");
        assert_eq!(view.namespace, "");
        assert_eq!(view.service_type, "ClusterIP");
        assert!(view.cluster_ip.is_none());
        assert!(view.external_addresses.is_empty());
        assert!(view.ports.is_empty());
        assert!(view.selector.is_empty());
        assert!(view.labels.is_empty());
        assert!(view.created.is_none());
        assert_eq!(view.ports_display(), "-");
        assert!(view.is_headless());
        assert_eq!(view.age_display(utc_at(10)), "-");
    }

    #[test]
    fn a_headless_service_has_no_cluster_ip() {
        let mut service = full_service();
        if let Some(spec) = service.spec.as_mut() {
            spec.cluster_ip = Some("None".to_string());
        }
        let view = ServiceView::from_api(&service);
        assert!(view.cluster_ip.is_none());
        assert!(view.is_headless());
    }

    #[test]
    fn a_load_balancer_hostname_is_used_when_there_is_no_ip() {
        let mut service = full_service();
        if let Some(spec) = service.spec.as_mut() {
            spec.external_ips = None;
        }
        if let Some(ingress) = service
            .status
            .as_mut()
            .and_then(|s| s.load_balancer.as_mut())
            .and_then(|lb| lb.ingress.as_mut())
        {
            ingress[0].ip = None;
            ingress[0].hostname = Some("api.elb.example.invalid".to_string());
        }
        let view = ServiceView::from_api(&service);
        assert_eq!(view.external_addresses, vec!["api.elb.example.invalid"]);
    }

    #[test]
    fn an_external_name_service_shows_its_target() {
        let service = Service {
            spec: Some(ServiceSpec {
                type_: Some("ExternalName".to_string()),
                external_name: Some("db.example.invalid".to_string()),
                ..ServiceSpec::default()
            }),
            ..Service::default()
        };
        let view = ServiceView::from_api(&service);
        assert_eq!(view.service_type, "ExternalName");
        assert_eq!(view.external_addresses, vec!["db.example.invalid"]);
    }

    #[test]
    fn an_unexpected_service_type_is_kept_verbatim() {
        let mut service = full_service();
        if let Some(spec) = service.spec.as_mut() {
            spec.type_ = Some("MeshGateway".to_string());
        }
        assert_eq!(ServiceView::from_api(&service).service_type, "MeshGateway");
    }

    #[test]
    fn a_port_without_a_protocol_defaults_to_tcp() {
        let view = ServicePortView::from_api(&ServicePort {
            port: 443,
            ..ServicePort::default()
        });
        assert_eq!(view.protocol, "TCP");
        assert_eq!(view.display(), "443/TCP");
        assert!(view.name.is_none());
        assert!(view.target_port.is_none());
    }

    #[test]
    fn a_named_target_port_is_kept_as_a_name() {
        let view = ServicePortView::from_api(&ServicePort {
            port: 9090,
            target_port: Some(IntOrString::String("metrics".to_string())),
            ..ServicePort::default()
        });
        assert_eq!(view.target_port.as_deref(), Some("metrics"));
    }

    #[test]
    fn a_numeric_target_port_is_rendered_as_a_number() {
        let view = ServicePortView::from_api(&ServicePort {
            port: 80,
            target_port: Some(IntOrString::Int(8080)),
            ..ServicePort::default()
        });
        assert_eq!(view.target_port.as_deref(), Some("8080"));
    }

    #[test]
    fn a_udp_node_port_renders_both_ports_and_the_protocol() {
        let view = ServicePortView::from_api(&ServicePort {
            port: 53,
            node_port: Some(30_053),
            protocol: Some("UDP".to_string()),
            ..ServicePort::default()
        });
        assert_eq!(view.display(), "53:30053/UDP");
    }

    #[test]
    fn age_uses_the_creation_timestamp() {
        let view = ServiceView::from_api(&full_service());
        assert_eq!(view.age(utc_at(1_120)), Some(Duration::from_secs(120)));
        assert_eq!(view.age_display(utc_at(1_120)), "2m");
    }

    #[test]
    fn services_sort_by_namespace_then_name() {
        let mut views = [
            view_named("web", "zebra"),
            view_named("api", "beta"),
            view_named("web", "alpha"),
            view_named("api", "alpha"),
        ];
        views.sort_by(compare_services);

        let keys: Vec<String> = views
            .iter()
            .map(|v| format!("{}/{}", v.namespace, v.name))
            .collect();
        assert_eq!(
            keys,
            vec!["api/alpha", "api/beta", "web/alpha", "web/zebra"]
        );
    }

    #[test]
    fn the_service_comparator_is_a_total_order() {
        let a = view_named("web", "alpha");
        let b = view_named("web", "beta");
        assert_eq!(compare_services(&a, &a), Ordering::Equal);
        assert_eq!(compare_services(&a, &b), Ordering::Less);
        assert_eq!(compare_services(&b, &a), Ordering::Greater);

        let mut sorted = vec![a, b];
        let before = sorted.clone();
        sorted.sort_by(compare_services);
        assert_eq!(sorted, before);
    }

    fn view_named(namespace: &str, name: &str) -> ServiceView {
        let mut service = Service::default();
        service.metadata.name = Some(name.to_string());
        service.metadata.namespace = Some(namespace.to_string());
        ServiceView::from_api(&service)
    }
}
