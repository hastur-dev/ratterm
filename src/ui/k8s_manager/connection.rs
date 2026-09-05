//! The cluster the screens are connected to.
//!
//! Separate from the manager because it owns a resource: the client holds the
//! SSH forward the connection was made through, so dropping this is what
//! closes the tunnel. Keeping that in one small file makes the lifetime
//! obvious.

use crate::k8s::K8sClient;

/// The cluster currently connected, and what was loaded from it.
pub struct ConnectedCluster {
    /// The client, which owns any SSH forward the connection needed.
    pub client: K8sClient,
    /// The context this client was built from.
    pub context: String,
    /// Namespaces the cluster reports.
    pub namespaces: Vec<String>,
}

impl std::fmt::Debug for ConnectedCluster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectedCluster")
            .field("context", &self.context)
            .field("namespaces", &self.namespaces.len())
            .finish()
    }
}
