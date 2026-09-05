//! Which screen is showing, and which kind of resource it lists.
//!
//! Split out of the manager because these are decisions about presentation
//! that have nothing to do with the state around them: what a tab is called,
//! what its columns are, whether the namespace filter applies to it. Each is a
//! function of nothing but the enum, so each is checkable on its own.

/// Which screen the manager is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum K8sView {
    /// Pick a context to connect to.
    #[default]
    Contexts,
    /// Browse the resources of the connected cluster.
    Resources,
}

/// Which kind of resource the resource view is listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceKind {
    /// Pods.
    #[default]
    Pods,
    /// Deployments.
    Deployments,
    /// Services.
    Services,
    /// Nodes.
    Nodes,
    /// Events.
    Events,
}

impl ResourceKind {
    /// Every kind, in the order the tabs present them.
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::Pods,
            Self::Deployments,
            Self::Services,
            Self::Nodes,
            Self::Events,
        ]
    }

    /// The tab label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pods => "Pods",
            Self::Deployments => "Deployments",
            Self::Services => "Services",
            Self::Nodes => "Nodes",
            Self::Events => "Events",
        }
    }

    /// The column headings for this kind.
    #[must_use]
    pub const fn headings(self) -> &'static [&'static str] {
        match self {
            Self::Pods => &["NAME", "READY", "STATUS", "RESTARTS", "NODE", "AGE"],
            Self::Deployments => &["NAME", "READY", "UP-TO-DATE", "AVAILABLE", "AGE"],
            Self::Services => &["NAME", "TYPE", "CLUSTER-IP", "PORTS", "AGE"],
            Self::Nodes => &["NAME", "STATUS", "ROLES", "VERSION", "AGE"],
            Self::Events => &["TYPE", "REASON", "OBJECT", "MESSAGE", "AGE"],
        }
    }

    /// True when this kind is not namespaced.
    ///
    /// A node list ignores the namespace filter; showing one would suggest it
    /// does something.
    #[must_use]
    pub const fn is_cluster_scoped(self) -> bool {
        matches!(self, Self::Nodes)
    }

    /// The next kind, wrapping.
    ///
    /// Tabs wrap because there are five of them in a row and the user is
    /// cycling, not scanning a result list.
    #[must_use]
    pub fn next(self) -> Self {
        let all = Self::all();
        let index = all.iter().position(|k| *k == self).unwrap_or(0);
        all[(index + 1) % all.len()]
    }

    /// The previous kind, wrapping.
    #[must_use]
    pub fn previous(self) -> Self {
        let all = Self::all();
        let index = all.iter().position(|k| *k == self).unwrap_or(0);
        all[(index + all.len() - 1) % all.len()]
    }
}


#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn resource_kinds_cycle_in_both_directions() {
        assert_eq!(ResourceKind::Pods.next(), ResourceKind::Deployments);
        assert_eq!(ResourceKind::Events.next(), ResourceKind::Pods, "wraps");
        assert_eq!(ResourceKind::Pods.previous(), ResourceKind::Events, "wraps");
        assert_eq!(
            ResourceKind::Deployments.previous(),
            ResourceKind::Pods
        );
    }
    #[test]
    fn every_kind_has_a_label_and_headings() {
        for kind in ResourceKind::all() {
            assert!(!kind.label().is_empty());
            assert!(!kind.headings().is_empty(), "{kind:?}");
        }
    }
}
