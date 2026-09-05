# Docker

Ratterm manages Docker on several machines at once. This document describes
how it reaches a daemon, what it keeps about each host, and where the code is.

## Two layers

`src/docker/` holds two ways of talking to Docker.

**The typed layer** uses [bollard] to speak the Docker Engine API. It returns
typed structs, an event stream and per-container detail, and it works the same
against a remote daemon as a local one. Everything new goes through it.

**The CLI layer** shells out to the `docker` binary and parses
`--format '{{json .}}'` output. It remains for the things the API cannot do
from here — Docker Hub search, watching a `docker pull` make progress, starting
Docker Desktop — and as a fallback for a host whose daemon socket cannot be
forwarded.

| File | What it holds |
|---|---|
| `transport.rs` | The three transports and the pure rule that picks one |
| `connect.rs` | Probing this machine for a socket or a pipe, and opening one |
| `client.rs` | `DockerClient`: connect, list, start/stop/restart/remove |
| `client_blocking.rs` | The same calls from synchronous code |
| `runtime.rs` | The Tokio runtime those calls are driven on |
| `model.rs` | Turning API structs into the shapes the UI renders |
| `fleet.rs`, `fleet_host.rs` | Several hosts at once, each with its own state |
| `fleet_rows.rs` | The one flat list, its sort orders and its filter |
| `events.rs`, `event_model.rs` | Container lifecycle events, live and durable |
| `event_stream.rs` | Subscribing to one host's `/events` stream |
| `compose.rs`, `compose_ops.rs` | Grouping containers into projects, and acting on one |
| `session.rs`, `session_actions.rs` | The fleet, its events and its subscriptions in one value |
| `discovery.rs`, `scan.rs`, `ops.rs`, `images.rs`, `cli.rs`, `parse.rs` | The CLI layer |

## Choosing a transport

Three transports exist:

- a **Unix domain socket**, `/var/run/docker.sock`, on Linux and macOS;
- a **Windows named pipe**, `//./pipe/docker_engine`, which is what Docker
  Desktop exposes;
- an **SSH port forward** to a remote daemon: a loopback port on this machine
  is carried to `127.0.0.1:2375` on the remote host over the existing
  `RemoteSession`, and bollard is pointed at the local end.

The decision is made by `transport::choose_transport`, a pure function over a
`TransportProbe`. Probing touches the filesystem and the environment; deciding
does not. That split is deliberate: it lets the rule be tested for **both**
platforms on **either** platform, so a Windows machine checks the Unix branch
and vice versa, and it is why the platform is a field on the probe rather than
a `cfg!` inside the decision.

The order is:

1. A remote host always uses the forward. A local socket that happens to exist
   belongs to a different daemon, and using it would silently show the wrong
   containers.
2. For this machine, the platform endpoint wins when it is present.
3. `DOCKER_HOST` is the documented override when it is not.
4. Otherwise the host is unavailable, and the error names the path that was
   looked for.

`Transport`'s `Debug` names the transport in words — `unix socket
/var/run/docker.sock`, `ssh forward host 7: 127.0.0.1:54321 -> 127.0.0.1:2375`
— and `TransportChoice::rationale` says why it was picked. Both appear in the
fleet view, so a surprising choice can be understood without reading the
source.

### Exposing a remote daemon over TCP

The forward connects to `127.0.0.1:2375` on the remote host, so the daemon must
be listening there. It stays on loopback: the forward reaches it through SSH,
and nothing is published to the remote host's network.

On a systemd host:

```sh
sudo systemctl edit docker.service
# [Service]
# ExecStart=
# ExecStart=/usr/bin/dockerd -H fd:// -H tcp://127.0.0.1:2375
sudo systemctl daemon-reload && sudo systemctl restart docker
```

A host without that listener still works: the fleet records the connection
failure with the reason, and the CLI layer continues to serve the single-host
Docker manager over SSH.

## The fleet

`DockerFleet` holds every host side by side, keyed by fleet key — `None` for
the local daemon, `Some(ssh_host_id)` for a remote one — so the local daemon
always sorts first. Each host carries its own `HostConnection`: `Idle`,
`Connecting`, `Connected` with the transport and the reason it was chosen, or
`Failed` with a message that names the next step.

A host that is unreachable is recorded against itself and skipped.
`refresh_all` attempts every host and returns the ones that failed, so one dead
machine never blocks the rest.

`fleet_rows.rs` flattens the fleet into one list, each row tagged with its host
name, and provides:

- four sort orders — host, name, image, status — each ending in host-then-name
  so the result is a total order;
- a filter that ANDs whitespace-separated terms case-insensitively against the
  host, container name, image, status, id and Compose path.

Both are pure functions and are tested directly.

## Container events

Each connected host gets a subscription to its `/events` stream, filtered to
container events. `event_model.rs` converts a message into a `FleetEvent`,
keeping only the lifecycle actions that say something about whether a service
is up:

`create`, `start`, `restart`, `stop`, `kill`, `die`, `destroy`, `health_status`

`exec_start`, `attach`, `resize` and their kind fire constantly and would bury
the rest. A `die` keeps its exit code as the event detail; a `health_status:
unhealthy` splits into the action `health_status` and the detail `unhealthy`,
so a filter on the action finds all of them.

Every event goes through one ingest path, `DockerEvents::ingest`, the way
`Telemetry::ingest` does for metrics. It keeps a bounded ring in memory and
writes the same event to the `container_events` table in
`~/.ratterm/ratterm.db`.

**The store is optional.** If the database cannot be opened — a read-only home
directory, a corrupt file — events still appear live, the reason is logged
once, and the view says the history is not being saved.

## Compose

Compose keeps no server-side state: a project is a set of containers carrying
the same `com.docker.compose.project` label. Grouping is therefore a pure
function over the container list, `compose::group_by_project`, which reads:

| Label | Used for |
|---|---|
| `com.docker.compose.project` | The project name |
| `com.docker.compose.service` | The service name |
| `com.docker.compose.container-number` | Replica order within a service |
| `com.docker.compose.project.working_dir` | Where the project was brought up |
| `com.docker.compose.project.config_files` | The compose files it was built from |

A container with no project label is kept in `unmanaged` rather than dropped
or given an invented project. A container with a project but no service name
is filed under a service named for the container, so it still appears.

Starting, stopping and restarting a project applies the action to each of its
containers through the typed client, which means it works on a remote host with
no `docker compose` binary installed. `compose_ops::plan` decides what needs
touching — starting a project skips what is already running — and the outcome
reports what changed, what was already there, and what failed, per container.
Four services stopping and one refusing is more useful than all-or-nothing.

## Errors

`DockerError` is a `thiserror` enum, and every variant names the next thing a
user can do:

- no local endpoint → start Docker, or pick a remote host;
- unknown SSH host → open the SSH manager and check the host list;
- forward failed → check the host is reachable and that its daemon listens on
  that address;
- a daemon rejection → advice chosen from the message: a permission error
  points at the `docker` group, a 404 at refreshing the list, a refused
  connection at starting the daemon.

`DockerError::is_transient` tells a caller whether retrying could plausibly
help.

## Testing

Every test runs without a Docker daemon, a network or a reachable host, on
Windows, Linux and macOS. That is possible because the rules live in pure
functions:

| Rule | Function |
|---|---|
| Which transport to use | `transport::choose_transport` |
| Which lifecycle events to keep | `events::is_tracked`, `events::split_action` |
| How to read an API struct | `model::container_from_summary` and friends |
| How containers group into projects | `compose::group_by_project` |
| What a project action must touch | `compose_ops::plan` |
| How the fleet list is ordered and filtered | `fleet_rows::sort_rows`, `fleet_rows::filter_rows` |
| Where the list scrolls to | `ui::docker_manager` `visible_window` |

Calls that do need a daemon are tested against `http://127.0.0.1:1`, where
nothing listens: the point of those tests is that the call fails with a useful
message rather than hanging.

```sh
cargo test --lib docker::
cargo test --test docker_fleet_tests
```

[bollard]: https://docs.rs/bollard
