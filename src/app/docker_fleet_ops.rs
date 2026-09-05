//! The fleet view: every container on every host, on one screen.
//!
//! The Docker manager shows one host at a time, which answers "what is running
//! here". The question a fleet raises is "what is running anywhere", and
//! answering it by visiting each host in turn is how a stopped container goes
//! unnoticed for a week.
//!
//! The typed layer in `src/docker` holds the connections and the data; this
//! opens and closes the screen, moves work off the render path, and turns a
//! key into a call. Nothing here decides what a row says or how it sorts —
//! that is `src/docker/fleet_rows.rs`, tested without a daemon.

use tracing::{info, warn};

use crate::docker::ContainerAction;
use crate::ui::docker_manager::{FleetAction, handle_fleet_key};

use super::App;

/// How many hosts are refreshed on the calling thread before it is worth
/// warning that the interface will pause.
///
/// A refresh connects and lists; against a reachable daemon that is tens of
/// milliseconds, and against an unreachable one it is the connect timeout. A
/// handful is tolerable on a key press, a rack is not.
const SYNCHRONOUS_REFRESH_LIMIT: usize = 4;

impl App {
    /// Opens the fleet view and refreshes every tracked host.
    pub fn open_docker_fleet(&mut self) {
        self.docker_fleet.sync_hosts(&self.ssh_hosts);

        let hosts = self.docker_fleet.fleet.keys().len();
        info!("docker fleet: {hosts} host(s) tracked");
        if hosts > SYNCHRONOUS_REFRESH_LIMIT {
            self.set_status(format!(
                "Docker fleet: refreshing {hosts} hosts, this may pause"
            ));
        }

        self.docker_fleet_open = true;
        self.refresh_docker_fleet();
    }

    /// Closes the fleet view, leaving the connections open.
    ///
    /// The connections are what make reopening fast, and each one is a
    /// long-lived HTTP client rather than a process, so keeping them costs a
    /// socket rather than a shell.
    pub fn close_docker_fleet(&mut self) {
        self.docker_fleet_open = false;
        self.set_status("Docker fleet closed");
    }

    /// True while the fleet view is showing.
    #[must_use]
    pub const fn is_docker_fleet_open(&self) -> bool {
        self.docker_fleet_open
    }

    /// Reconnects and relists every host.
    pub fn refresh_docker_fleet(&mut self) {
        let now = crate::telemetry::unix_now();
        let failures = self.docker_fleet.refresh_all(now);

        if failures.is_empty() {
            let counts = self.docker_fleet.counts();
            self.set_status(counts.headline());
            return;
        }

        for (key, reason) in &failures {
            warn!("docker fleet: host {key:?} failed: {reason}");
        }

        // The first failure in the status bar, the rest in the log: a status
        // line listing six hosts is unreadable, and the per-host state is
        // already visible in the list itself.
        let (_, first) = &failures[0];
        let message = if failures.len() == 1 {
            first.clone()
        } else {
            format!("{first} (+{} more hosts failed)", failures.len() - 1)
        };
        self.set_status(message);
    }

    /// Handles a key while the fleet view is open.
    ///
    /// Returns true if the key was used, which is always: the view covers the
    /// pane, so a key falling through to the editor underneath would type into
    /// a buffer the user cannot see.
    pub(super) fn handle_docker_fleet_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let rows = self.docker_fleet.rows();
        let action = handle_fleet_key(&mut self.docker_fleet_view, key, rows.len());

        match action {
            FleetAction::None => {}
            FleetAction::Close => self.close_docker_fleet(),
            FleetAction::Refresh => self.refresh_docker_fleet(),
            FleetAction::CycleSort => {
                let sort = self.docker_fleet.cycle_sort();
                self.set_status(format!("Sorted by {}", sort.as_str()));
            }
            FleetAction::FilterChanged => {
                let filter = self.docker_fleet_view.filter().to_string();
                self.docker_fleet.set_filter(filter);
            }
            FleetAction::Activate => self.connect_to_selected_fleet_container(&rows),
            FleetAction::Start => self.act_on_selected_container(&rows, ContainerAction::Start),
            FleetAction::Stop => self.act_on_selected_container(&rows, ContainerAction::Stop),
            FleetAction::Restart => self.act_on_selected_container(&rows, ContainerAction::Restart),
        }

        true
    }

    /// Starts, stops or restarts the selected container.
    fn act_on_selected_container(
        &mut self,
        rows: &[crate::docker::FleetRow],
        action: ContainerAction,
    ) {
        let Some(row) = rows.get(self.docker_fleet_view.selected()) else {
            self.set_status("No container selected");
            return;
        };

        let host = row.host_key;
        let id = row.container.id.clone();
        let name = row.container.name.clone();

        match self.docker_fleet.container_action(host, &id, action) {
            Ok(()) => {
                self.set_status(format!("{} {name}", action.as_str()));
                // The container's state has changed, so the row is stale until
                // the next listing; refresh the one host rather than all.
                let now = crate::telemetry::unix_now();
                if let Err(e) = self.docker_fleet.refresh(host, now) {
                    warn!("docker fleet: refresh after {} failed: {e}", action.as_str());
                }
            }
            Err(e) => self.set_status(format!("Could not {} {name}: {e}", action.as_str())),
        }
    }

    /// Opens a shell in the selected container.
    fn connect_to_selected_fleet_container(&mut self, rows: &[crate::docker::FleetRow]) {
        let Some(row) = rows.get(self.docker_fleet_view.selected()) else {
            self.set_status("No container selected");
            return;
        };

        if !row.container.is_running() {
            self.set_status(format!("{} is not running", row.container.name));
            return;
        }

        let id = row.container.id.clone();
        let name = row.container.name.clone();
        let label = row.host_label.clone();

        // The exec helper reads the Docker manager's selected host, so point
        // that at the row's host first. Without this, activating a row on one
        // host would open a shell on whichever host the manager last showed.
        self.docker_items.selected_host = match row.host_key {
            None => crate::docker::DockerHost::Local,
            Some(host_id) => crate::docker::DockerHost::remote_labelled(host_id, label),
        };

        // The shell runs in a terminal tab, which is where an interactive
        // session belongs; the fleet view closes so the tab is visible.
        self.close_docker_fleet();
        self.exec_into_container(&id, &name);
    }
}
