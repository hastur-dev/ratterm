//! The contexts a kubeconfig defines, flattened for a list.
//!
//! A context is presented as one owned struct with the cluster, user,
//! namespace and server already resolved, so the UI never walks the
//! kubeconfig's three cross-referencing sections itself.

use std::path::PathBuf;

use kube::config::Kubeconfig;

pub use super::kubeconfig::{kubeconfig_paths, load_kubeconfig, load_kubeconfig_from, paths_from};
use super::{K8sError, Result};

/// The namespace a context uses when it does not name one.
const DEFAULT_NAMESPACE: &str = "default";

/// One context from a kubeconfig, flattened for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubeContext {
    /// The context name, as written in the kubeconfig.
    pub name: String,
    /// The cluster this context points at.
    pub cluster: String,
    /// The user entry this context authenticates as, if it names one.
    pub user: Option<String>,
    /// The namespace requests default to. `default` when the context omits it.
    pub namespace: String,
    /// The API server URL from the cluster entry, if the cluster is defined.
    pub server: Option<String>,
    /// True for the kubeconfig's `current-context`.
    pub is_current: bool,
    /// False when the context names a cluster the kubeconfig does not define.
    ///
    /// Such a context is listed rather than hidden, because seeing it and its
    /// broken cluster reference is what tells the user what to fix.
    pub cluster_defined: bool,
}

impl KubeContext {
    /// Returns a one-line summary for a list row.
    #[must_use]
    pub fn summary(&self) -> String {
        let marker = if self.is_current { "* " } else { "  " };
        let server = self.server.as_deref().unwrap_or("<cluster not defined>");
        format!(
            "{marker}{} [{}] ns={} {}",
            self.name, self.cluster, self.namespace, server
        )
    }
}

/// The contexts found in a kubeconfig, plus which one is current.
#[derive(Debug, Clone, Default)]
pub struct ContextSet {
    contexts: Vec<KubeContext>,
    current: Option<String>,
    sources: Vec<PathBuf>,
}

impl ContextSet {
    /// Returns the contexts, sorted by name.
    #[must_use]
    pub fn contexts(&self) -> &[KubeContext] {
        &self.contexts
    }

    /// Returns how many contexts were found.
    #[must_use]
    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    /// Returns true when the kubeconfig defines no contexts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }

    /// Returns the name of the current context, if the kubeconfig sets one.
    #[must_use]
    pub fn current_name(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Returns the current context, if it is set and defined.
    #[must_use]
    pub fn current(&self) -> Option<&KubeContext> {
        self.contexts.iter().find(|c| c.is_current)
    }

    /// Returns the context with this name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&KubeContext> {
        self.contexts.iter().find(|c| c.name == name)
    }

    /// Returns the context with this name, or an error naming what is wrong.
    ///
    /// # Errors
    /// [`K8sError::UnknownContext`] if there is no such context, or
    /// [`K8sError::UnknownCluster`] if it points at a cluster the kubeconfig
    /// does not define.
    pub fn require(&self, name: &str) -> Result<&KubeContext> {
        let ctx = self
            .get(name)
            .ok_or_else(|| K8sError::UnknownContext(name.to_string()))?;
        if !ctx.cluster_defined {
            return Err(K8sError::UnknownCluster {
                context: ctx.name.clone(),
                cluster: ctx.cluster.clone(),
            });
        }
        Ok(ctx)
    }

    /// Returns the context names, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.contexts.iter().map(|c| c.name.clone()).collect()
    }

    /// Returns the files this set was read from.
    #[must_use]
    pub fn sources(&self) -> &[PathBuf] {
        &self.sources
    }
}

/// Lists the contexts in a parsed kubeconfig.
///
/// Contexts are sorted by name so the list does not reorder between reads;
/// kubeconfig order is insertion order and changes whenever a tool rewrites
/// the file.
#[must_use]
pub fn contexts_from(config: &Kubeconfig, sources: Vec<PathBuf>) -> ContextSet {
    let current = config.current_context.clone();

    let mut contexts: Vec<KubeContext> = config
        .contexts
        .iter()
        .filter_map(|named| {
            let inner = named.context.as_ref()?;
            let cluster = config
                .clusters
                .iter()
                .find(|c| c.name == inner.cluster)
                .and_then(|c| c.cluster.as_ref());
            Some(KubeContext {
                name: named.name.clone(),
                cluster: inner.cluster.clone(),
                user: inner.user.clone().filter(|u| !u.is_empty()),
                namespace: inner
                    .namespace
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| DEFAULT_NAMESPACE.to_string()),
                server: cluster.and_then(|c| c.server.clone()),
                is_current: current.as_deref() == Some(named.name.as_str()),
                cluster_defined: cluster.is_some(),
            })
        })
        .collect();

    contexts.sort_by(|a, b| a.name.cmp(&b.name));

    ContextSet {
        contexts,
        current,
        sources,
    }
}

/// Reads the configured kubeconfig files and lists their contexts.
///
/// # Errors
/// See [`load_kubeconfig`].
pub fn load_contexts() -> Result<ContextSet> {
    let paths = kubeconfig_paths();
    let config = load_kubeconfig_from(&paths)?;
    Ok(contexts_from(&config, paths))
}

/// Reads an explicit list of kubeconfig files and lists their contexts.
///
/// # Errors
/// See [`load_kubeconfig`].
pub fn load_contexts_from(paths: &[PathBuf]) -> Result<ContextSet> {
    let config = load_kubeconfig_from(paths)?;
    Ok(contexts_from(&config, paths.to_vec()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn write_config(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, body).expect("write");
        path
    }

    const THREE_CONTEXTS: &str = r#"
apiVersion: v1
kind: Config
current-context: prod
clusters:
  - name: prod-cluster
    cluster:
      server: https://prod.example.invalid:6443
  - name: dev-cluster
    cluster:
      server: https://dev.example.invalid:6443
users:
  - name: admin
    user: {}
contexts:
  - name: prod
    context:
      cluster: prod-cluster
      user: admin
      namespace: web
  - name: dev
    context:
      cluster: dev-cluster
      user: admin
  - name: broken
    context:
      cluster: missing-cluster
      user: admin
"#;

    fn three_contexts(dir: &tempfile::TempDir) -> ContextSet {
        let path = write_config(dir, "config", THREE_CONTEXTS);
        load_contexts_from(std::slice::from_ref(&path)).expect("load")
    }

    #[test]
    fn a_kubeconfig_with_several_contexts_is_listed_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(&dir, "config", THREE_CONTEXTS);

        let set = load_contexts_from(std::slice::from_ref(&path)).expect("load");
        assert_eq!(set.names(), vec!["broken", "dev", "prod"]);
        assert_eq!(set.len(), 3);
        assert!(!set.is_empty());
        assert_eq!(set.sources(), &[path]);
        assert_eq!(set.contexts().len(), 3);
    }

    #[test]
    fn the_current_context_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let set = three_contexts(&dir);

        assert_eq!(set.current_name(), Some("prod"));
        let current = set.current().expect("a current context");
        assert_eq!(current.name, "prod");
        assert!(current.is_current);
        assert!(!set.get("dev").unwrap().is_current);
    }

    #[test]
    fn a_context_carries_its_cluster_namespace_and_server() {
        let dir = tempfile::tempdir().unwrap();
        let set = three_contexts(&dir);

        let prod = set.get("prod").expect("prod");
        assert_eq!(prod.cluster, "prod-cluster");
        assert_eq!(prod.user.as_deref(), Some("admin"));
        assert_eq!(prod.namespace, "web");
        assert_eq!(
            prod.server.as_deref(),
            Some("https://prod.example.invalid:6443")
        );
        assert!(prod.cluster_defined);
    }

    #[test]
    fn a_context_without_a_namespace_defaults_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let set = three_contexts(&dir);
        assert_eq!(set.get("dev").expect("dev").namespace, "default");
    }

    #[test]
    fn a_context_naming_an_undefined_cluster_is_listed_but_rejected_on_use() {
        let dir = tempfile::tempdir().unwrap();
        let set = three_contexts(&dir);

        let broken = set.get("broken").expect("broken is still listed");
        assert!(!broken.cluster_defined);
        assert!(broken.server.is_none());
        assert!(broken.summary().contains("cluster not defined"));

        match set.require("broken") {
            Err(K8sError::UnknownCluster { context, cluster }) => {
                assert_eq!(context, "broken");
                assert_eq!(cluster, "missing-cluster");
            }
            other => panic!("expected UnknownCluster, got {other:?}"),
        }
    }

    #[test]
    fn requiring_a_context_that_does_not_exist_names_it() {
        let dir = tempfile::tempdir().unwrap();
        let set = three_contexts(&dir);

        match set.require("staging") {
            Err(K8sError::UnknownContext(name)) => assert_eq!(name, "staging"),
            other => panic!("expected UnknownContext, got {other:?}"),
        }
    }

    #[test]
    fn requiring_a_good_context_returns_it() {
        let dir = tempfile::tempdir().unwrap();
        let set = three_contexts(&dir);
        assert_eq!(set.require("prod").expect("prod").cluster, "prod-cluster");
    }

    #[test]
    fn a_kubeconfig_with_no_contexts_loads_as_an_empty_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            &dir,
            "config",
            "apiVersion: v1\nkind: Config\nclusters: []\nusers: []\ncontexts: []\n",
        );

        let set = load_contexts_from(&[path]).expect("load");
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert!(set.current().is_none());
        assert!(set.current_name().is_none());
        assert!(set.names().is_empty());
    }

    #[test]
    fn a_missing_kubeconfig_reports_that_there_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            load_contexts_from(&[dir.path().join("absent")]),
            Err(K8sError::NoKubeconfig)
        ));
    }

    #[test]
    fn an_empty_context_set_is_the_default() {
        let set = ContextSet::default();
        assert!(set.is_empty());
        assert!(set.sources().is_empty());
        assert!(set.current().is_none());
    }

    #[test]
    fn a_summary_marks_the_current_context() {
        let dir = tempfile::tempdir().unwrap();
        let set = three_contexts(&dir);

        let prod = set.get("prod").unwrap().summary();
        assert!(prod.starts_with("* "), "{prod}");
        assert!(prod.contains("ns=web"), "{prod}");

        let dev = set.get("dev").unwrap().summary();
        assert!(dev.starts_with("  "), "{dev}");
    }

    #[test]
    fn a_context_entry_with_no_body_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            &dir,
            "config",
            "apiVersion: v1\nkind: Config\nclusters: []\nusers: []\ncontexts:\n  - name: hollow\n",
        );
        let set = load_contexts_from(&[path]).expect("load");
        assert!(set.is_empty(), "a context with no body should be skipped");
    }

    #[test]
    fn loading_the_machine_contexts_either_succeeds_or_reports_why() {
        match load_contexts() {
            Ok(_) | Err(K8sError::NoKubeconfig | K8sError::MalformedKubeconfig { .. }) => {}
            Err(other) => panic!("unexpected error {other:?}"),
        }
    }
}
