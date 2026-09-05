//! Kubernetes access: contexts, resources, actions, logs and stored settings.
//!
//! This module is the API layer for the Kubernetes screens. It owns every
//! dependency on `kube` and `k8s-openapi` so the rest of the application never
//! sees one: listings come back as the plain owned structs in [`resources`],
//! actions return typed results, and every failure is a [`K8sError`] whose
//! message says what to do about it.
//!
//! # Where things are
//!
//! | Module | What it does |
//! |---|---|
//! | [`kubeconfig`] | finds and parses `~/.kube/config` and `$KUBECONFIG` |
//! | [`contexts`] | the contexts those files define, flattened for a list |
//! | [`endpoint`] | which context, and whether to tunnel through a fleet host |
//! | [`client`] | builds a `kube` client, opening the SSH tunnel when asked |
//! | [`remote_config`] | reads a fleet host's kubeconfig over SFTP and caches it |
//! | [`resources`] | owned views of pods, deployments, services, nodes, events |
//! | [`list`] | listing and watching, in a stable order |
//! | [`actions`] | scale, rollout restart, delete, exec |
//! | [`forward`] | forwarding a local port to a pod port |
//! | [`logs`] | pod logs into the Docker log buffer and storage |
//! | [`storage`] | `~/.ratterm/k8s.toml` |
//!
//! # Async and the synchronous façade
//!
//! `kube` is async; the rest of ratterm is not. Every public entry point here
//! is synchronous and drives one process-wide multi-threaded Tokio runtime
//! (see [`block_on`]).
//!
//! A dedicated runtime is used rather than a borrowed
//! [`tokio::runtime::Handle`] because the UI thread has no ambient runtime to
//! borrow from, and a multi-threaded runtime rather than a current-thread one
//! because `kube` spawns background tasks — auth token refresh, watch
//! recovery, the port-forward pump — that have to keep running while a caller
//! is blocked inside `block_on`.
//!
//! # What the tests cannot reach
//!
//! Anything that needs a live API server: listing, watching, exec, pod port
//! forwarding and log streaming. Those functions are deliberately thin. The
//! logic underneath them — kubeconfig parsing, resource conversion, patch
//! bodies, sort comparators, log line conversion, stored settings — is pure
//! and is tested here.

pub mod actions;
pub mod atomic;
pub mod client;
pub mod contexts;
pub mod endpoint;
pub mod forward;
pub mod kubeconfig;
pub mod list;
pub mod logs;
pub mod remote_config;
pub mod resources;
pub mod storage;

use std::future::Future;
use std::sync::OnceLock;

use thiserror::Error;
use tokio::runtime::Runtime;

pub use actions::{
    ExecOutput, MAX_REPLICAS, RESTART_ANNOTATION, RolloutRestart, ScaleResult, restart_patch_body,
    scale_patch_body,
};
pub use client::K8sClient;
pub use contexts::{ContextSet, KubeContext, load_contexts, load_contexts_from};
pub use endpoint::{ClusterEndpoint, split_server_url};
pub use forward::PodPortForward;
pub use kubeconfig::{kubeconfig_paths, load_kubeconfig, load_kubeconfig_from};
pub use list::{WatchHandle, WatchUpdate};
pub use logs::{PodLogOptions, PodLogStream};
pub use remote_config::{REMOTE_KUBECONFIG_PATH, RemoteKubeconfig};
pub use resources::{
    DeploymentView, EventView, NodeReady, NodeView, PodPhase, PodView, ServicePortView,
    ServiceView, format_age,
};
pub use storage::{K8sSettings, K8sStorage};

/// Everything that can go wrong reaching or driving a cluster.
///
/// Each message names the thing that failed and the next step, because these
/// strings are shown directly in the status bar.
#[derive(Debug, Error)]
pub enum K8sError {
    /// No kubeconfig could be found on this machine.
    #[error(
        "no kubeconfig found; create ~/.kube/config or point KUBECONFIG at one, \
         or pick a fleet host and read its kubeconfig over SSH"
    )]
    NoKubeconfig,

    /// A kubeconfig exists but could not be read or parsed.
    #[error(
        "kubeconfig {path} could not be read: {reason}; fix the file or set KUBECONFIG to a \
         different one"
    )]
    MalformedKubeconfig {
        /// The file that failed.
        path: String,
        /// What the parser reported.
        reason: String,
    },

    /// The named context is not in the kubeconfig.
    #[error(
        "no context named '{0}' in the kubeconfig; open the context list and pick one that exists"
    )]
    UnknownContext(String),

    /// The context names a cluster the kubeconfig does not define.
    #[error(
        "context '{context}' points at cluster '{cluster}', which the kubeconfig does not define; \
         add a clusters entry for '{cluster}' or pick a different context"
    )]
    UnknownCluster {
        /// The context that is broken.
        context: String,
        /// The cluster it names.
        cluster: String,
    },

    /// The API server did not answer.
    #[error(
        "cluster '{context}' is unreachable: {reason}; check the server address in the kubeconfig, \
         or route the connection through a fleet host with an SSH tunnel"
    )]
    Unreachable {
        /// The context that was being used.
        context: String,
        /// What the transport reported.
        reason: String,
    },

    /// The API server refused the request on authorisation grounds.
    #[error(
        "not permitted to {action}: {reason}; ask a cluster admin for RBAC covering this verb, or \
         switch to a context with more rights"
    )]
    NotPermitted {
        /// What was being attempted, in the imperative.
        action: String,
        /// What the API server said.
        reason: String,
    },

    /// The object does not exist.
    #[error(
        "{kind} '{name}' was not found in namespace '{namespace}'; refresh the list, it may \
         already be gone"
    )]
    NotFound {
        /// The resource kind, for example `pod`.
        kind: String,
        /// The object name.
        name: String,
        /// The namespace searched.
        namespace: String,
    },

    /// The SSH tunnel to the API server could not be established or died.
    #[error(
        "the SSH tunnel to the API server failed: {reason}; check that host {host_id} is in the \
         SSH host list and reachable, then try again"
    )]
    SshTunnel {
        /// The fleet host the tunnel runs through.
        host_id: u32,
        /// What the SSH layer reported.
        reason: String,
    },

    /// The caller asked for something the API cannot express.
    #[error("{0}")]
    InvalidRequest(String),

    /// Reading or writing `~/.ratterm/k8s.toml` failed.
    #[error("could not use the Kubernetes settings file: {0}; check permissions on ~/.ratterm")]
    Storage(String),

    /// The API server answered, but with an error this module does not
    /// classify further.
    #[error(
        "the Kubernetes API rejected the request: {0}; check the cluster's own logs for detail"
    )]
    Api(String),

    /// The Tokio runtime backing the synchronous façade could not be used.
    #[error("could not start the Kubernetes runtime: {0}; restart ratterm and try again")]
    Runtime(String),
}

impl K8sError {
    /// Converts a `kube` error into a [`K8sError`], keeping the HTTP status
    /// code's meaning.
    ///
    /// `action` is the imperative description of what was being attempted
    /// (`"list pods"`, `"scale deployment api"`), so a permission failure can
    /// name the verb the user needs.
    pub(crate) fn from_kube(err: &kube::Error, context: &str, action: &str) -> Self {
        match err {
            kube::Error::Api(status) => Self::from_status(status.code, &status.message, action),
            kube::Error::Service(_) | kube::Error::HyperError(_) => Self::Unreachable {
                context: context.to_string(),
                reason: err.to_string(),
            },
            other => Self::Api(other.to_string()),
        }
    }

    /// Classifies an API status code.
    ///
    /// Split out from [`K8sError::from_kube`] so the mapping can be tested
    /// without constructing transport errors.
    pub(crate) fn from_status(code: u16, message: &str, action: &str) -> Self {
        let reason = if message.is_empty() {
            format!("HTTP {code}")
        } else {
            message.to_string()
        };
        match code {
            401 | 403 => Self::NotPermitted {
                action: action.to_string(),
                reason,
            },
            404 => Self::NotFound {
                kind: "object".to_string(),
                name: action.to_string(),
                namespace: "-".to_string(),
            },
            _ => Self::Api(reason),
        }
    }
}

/// A [`Result`] carrying a [`K8sError`].
pub type Result<T> = std::result::Result<T, K8sError>;

/// The process-wide runtime, or `None` if it could not be built.
static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();

/// Returns the runtime every Kubernetes call is driven on.
///
/// Built once, on first use, so a session that never opens the Kubernetes
/// screens pays nothing for it.
pub(crate) fn runtime() -> Result<&'static Runtime> {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("ratterm-k8s")
                .enable_all()
                .build()
                .map_err(|e| tracing::error!("the Kubernetes runtime could not start: {e}"))
                .ok()
        })
        .as_ref()
        .ok_or_else(|| K8sError::Runtime("the worker threads could not be started".to_string()))
}

/// Runs `future` to completion on the Kubernetes runtime and returns its value.
///
/// # Errors
/// Returns [`K8sError::Runtime`] if the runtime cannot be built, or if the
/// caller is already inside a Tokio runtime — blocking there would deadlock
/// the calling worker, so it is refused rather than attempted.
pub fn block_on<F: Future>(future: F) -> Result<F::Output> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(K8sError::Runtime(
            "this call was made from inside an async task; call the async API directly".to_string(),
        ));
    }
    Ok(runtime()?.block_on(future))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn every_error_variant_names_a_next_step() {
        let cases = vec![
            K8sError::NoKubeconfig,
            K8sError::MalformedKubeconfig {
                path: "/tmp/config".to_string(),
                reason: "unexpected character".to_string(),
            },
            K8sError::UnknownContext("prod".to_string()),
            K8sError::UnknownCluster {
                context: "prod".to_string(),
                cluster: "prod-cluster".to_string(),
            },
            K8sError::Unreachable {
                context: "prod".to_string(),
                reason: "connection refused".to_string(),
            },
            K8sError::NotPermitted {
                action: "delete pod api-0".to_string(),
                reason: "forbidden".to_string(),
            },
            K8sError::NotFound {
                kind: "pod".to_string(),
                name: "api-0".to_string(),
                namespace: "default".to_string(),
            },
            K8sError::SshTunnel {
                host_id: 3,
                reason: "no route to host".to_string(),
            },
            K8sError::InvalidRequest("replicas must not be negative".to_string()),
            K8sError::Storage("permission denied".to_string()),
            K8sError::Api("etcdserver: request timed out".to_string()),
            K8sError::Runtime("out of threads".to_string()),
        ];

        // Every message must be a full sentence that mentions an action the
        // user can take. `InvalidRequest` carries the caller's own wording, so
        // it is checked only for non-emptiness.
        for err in cases {
            let text = err.to_string();
            assert!(!text.is_empty(), "{err:?} rendered nothing");
            if matches!(err, K8sError::InvalidRequest(_)) {
                continue;
            }
            let actionable = [
                "check", "pick", "set", "create", "ask", "add", "fix", "open", "refresh", "restart",
            ]
            .iter()
            .any(|verb| text.contains(verb));
            assert!(actionable, "no next step in: {text}");
        }
    }

    #[test]
    fn messages_name_the_thing_that_failed() {
        let err = K8sError::UnknownContext("staging".to_string());
        assert!(err.to_string().contains("staging"), "{err}");

        let err = K8sError::NotFound {
            kind: "deployment".to_string(),
            name: "api".to_string(),
            namespace: "web".to_string(),
        };
        let text = err.to_string();
        assert!(text.contains("deployment"), "{text}");
        assert!(text.contains("api"), "{text}");
        assert!(text.contains("web"), "{text}");

        let err = K8sError::SshTunnel {
            host_id: 7,
            reason: "timed out".to_string(),
        };
        assert!(err.to_string().contains("host 7"), "{err}");
    }

    #[test]
    fn a_403_becomes_a_permission_error() {
        let err = K8sError::from_status(403, "pods is forbidden", "list pods");
        match err {
            K8sError::NotPermitted { action, reason } => {
                assert_eq!(action, "list pods");
                assert_eq!(reason, "pods is forbidden");
            }
            other => panic!("expected NotPermitted, got {other:?}"),
        }
    }

    #[test]
    fn a_401_is_also_a_permission_error() {
        assert!(matches!(
            K8sError::from_status(401, "", "list pods"),
            K8sError::NotPermitted { .. }
        ));
    }

    #[test]
    fn a_404_becomes_a_not_found_error() {
        assert!(matches!(
            K8sError::from_status(404, "pods \"api\" not found", "get pod api"),
            K8sError::NotFound { .. }
        ));
    }

    #[test]
    fn any_other_code_falls_back_to_the_api_variant() {
        let err = K8sError::from_status(500, "internal", "list pods");
        assert!(matches!(err, K8sError::Api(_)));
        assert!(err.to_string().contains("internal"));
    }

    #[test]
    fn an_empty_api_message_still_names_the_status_code() {
        let err = K8sError::from_status(500, "", "list pods");
        assert!(err.to_string().contains("500"), "{err}");
    }

    #[test]
    fn an_api_error_from_kube_keeps_its_code() {
        let status = kube::core::Status {
            status: None,
            code: 403,
            message: "denied".to_string(),
            metadata: None,
            reason: "Forbidden".to_string(),
            details: None,
        };
        let err = K8sError::from_kube(&kube::Error::Api(Box::new(status)), "prod", "delete pod x");
        assert!(matches!(err, K8sError::NotPermitted { .. }), "{err:?}");
    }

    #[test]
    fn a_non_api_kube_error_falls_back_to_the_api_variant() {
        let err = K8sError::from_kube(
            &kube::Error::LinesCodecMaxLineLengthExceeded,
            "prod",
            "list pods",
        );
        assert!(matches!(err, K8sError::Api(_)), "{err:?}");
    }

    #[test]
    fn the_runtime_is_built_once_and_runs_futures() {
        let first = runtime().expect("runtime");
        let second = runtime().expect("runtime");
        assert!(std::ptr::eq(first, second), "the runtime was rebuilt");

        let value = block_on(async { 2 + 2 }).expect("block_on");
        assert_eq!(value, 4);
    }

    #[test]
    fn blocking_from_inside_a_runtime_is_refused_rather_than_deadlocking() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");
        let outcome = rt.block_on(async { block_on(async { 1 }) });
        assert!(matches!(outcome, Err(K8sError::Runtime(_))), "{outcome:?}");
    }
}
