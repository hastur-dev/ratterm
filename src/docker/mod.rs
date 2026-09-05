//! Docker container and image management.
//!
//! Two layers live here.
//!
//! The **typed layer** ([`client`], [`transport`], [`model`], [`fleet`],
//! [`events`], [`compose`]) talks the Docker API through bollard. A host is
//! reached over whichever transport [`transport::choose_transport`] picks — a
//! Unix socket, a Windows named pipe, or an SSH port forward to a remote
//! daemon — so a remote host gives typed responses, event streams and stats
//! rather than text to parse. Several hosts are held at once by
//! [`fleet::DockerFleet`], each with its own connection state, so one
//! unreachable machine never blocks the rest.
//!
//! The **CLI layer** ([`discovery`], [`scan`], [`ops`], [`images`], [`cli`],
//! [`parse`]) shells out to `docker` and parses its output. It stays for the
//! things the API cannot do from here — Docker Hub search, `docker pull`
//! progress, starting Docker Desktop — and as the fallback for a host whose
//! daemon socket cannot be forwarded.
//!
//! Data types ([`container`], [`host`], [`items`], [`create`]) are shared by
//! both.

pub mod api;
pub mod cli;
pub mod client;
pub mod client_blocking;
pub mod compose;
pub mod compose_ops;
pub mod connect;
pub mod container;
pub mod create;
pub mod discovery;
pub mod error;
pub mod event_model;
pub mod event_stream;
pub mod events;
pub mod fleet;
pub mod fleet_host;
pub mod fleet_rows;
pub mod host;
pub mod images;
pub mod items;
pub mod model;
pub mod ops;
pub mod parse;
pub mod run_options;
pub mod runtime;
pub mod scan;
pub mod session;
pub mod session_actions;
pub mod storage;
pub mod transport;

pub use api::{DockerApi, DockerHostManager};
pub use client::{DockerClient, HostSnapshot};
pub use compose::{
    ComposeGrouping, ComposeProject, ComposeService, PROJECT_LABEL, ProjectState, SERVICE_LABEL,
    group_by_project,
};
pub use compose_ops::{ProjectAction, ProjectOutcome};
pub use container::{DockerContainer, DockerImage, DockerItemType, DockerStatus};
pub use create::{
    ContainerCreationState, DockerSearchResult, MAX_SEARCH_RESULTS, VolumeMountConfig,
};
pub use discovery::{DockerAvailability, DockerDiscovery, DockerDiscoveryResult};
pub use error::DockerError;
pub use event_stream::{EventFeed, EventSubscription, subscribe};
pub use events::{DockerEvents, FleetEvent};
pub use fleet::{DockerFleet, FleetHost, HostConnection};
pub use fleet_rows::{FleetCounts, FleetRow, FleetSort};
pub use host::DockerHost;
pub use items::{DockerItemList, DockerQuickConnectItem, MAX_QUICK_CONNECT, QuickConnectSlots};
pub use model::{ContainerDetail, DockerNetwork, DockerVolume};
pub use run_options::DockerRunOptions;
pub use session::{ContainerAction, DockerFleetState};
pub use storage::{DockerStorage, DockerStorageError};
pub use transport::{Transport, TransportChoice, TransportProbe};
