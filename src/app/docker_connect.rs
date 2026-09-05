//! Docker container connection operations.

use tracing::{info, warn};

use crate::docker::{DockerDiscovery, DockerHost};

use super::App;

impl App {
    /// Builds a command, wrapping with SSH for remote hosts.
    ///
    /// This produces a command line for a *terminal tab*, which still needs a
    /// real `ssh` invocation the user can see and interrupt. Programmatic
    /// Docker calls go through the pooled session instead and never build a
    /// command line at all.
    fn build_command_for_host(&self, docker_cmd: &str) -> String {
        let host = &self.docker_items.selected_host;
        match host.host_id() {
            None => docker_cmd.to_string(),
            Some(host_id) => match self.ssh_hosts.target(host_id) {
                Some(target) => {
                    let port_flag = if target.port == 22 {
                        String::new()
                    } else {
                        format!("-p {} ", target.port)
                    };
                    format!(
                        "ssh {port_flag}{}@{} {docker_cmd}",
                        target.username, target.hostname
                    )
                }
                None => docker_cmd.to_string(),
            },
        }
    }

    /// Executes into a running container.
    ///
    /// For local containers, creates a docker exec terminal directly.
    /// For remote containers, creates an SSH session that runs docker exec.
    pub fn exec_into_container(&mut self, container_id: &str, container_name: &str) {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let shell = self.docker_default_shell().to_string();
        let host = self.docker_items.selected_host.clone();
        let host_name = self.docker_host_display_name();
        // Resolve before borrowing the terminal multiplexer mutably.
        let target = host.host_id().and_then(|id| self.ssh_hosts.target(id));

        self.set_status(format!(
            "Connecting to {} on {}...",
            container_name, host_name
        ));

        let Some(ref mut terminals) = self.terminals else {
            self.set_status("No terminal available".to_string());
            return;
        };

        let result = match &host {
            DockerHost::Local => {
                // Local container - use direct docker exec
                terminals.add_docker_exec_tab(container_id, container_name, &shell)
            }
            DockerHost::Remote { host_id, .. } => {
                // Remote container: resolve the connection from the registry
                // rather than from a copy stored with the container entry.
                match target {
                    Some(target) => terminals.add_docker_exec_ssh_tab(
                        container_id,
                        container_name,
                        &shell,
                        &target.hostname,
                        target.port,
                        &target.username,
                        *host_id,
                        target.password.as_ref().map(|p| p.as_str()),
                    ),
                    None => Err(crate::terminal::pty::PtyError::Other(format!(
                        "SSH host {host_id} is not configured; open the SSH manager and add credentials"
                    ))),
                }
            }
        };

        match result {
            Ok(_) => {
                self.set_status(format!("Connected to {} on {}", container_name, host_name));
            }
            Err(e) => {
                self.set_status(format!("Failed to connect: {}", e));
            }
        }

        // Hide Docker manager if visible
        self.hide_docker_manager();
    }

    /// Starts a stopped container and execs into it.
    pub fn start_and_exec_container(&mut self, container_id: &str, container_name: &str) {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        let host_name = self.docker_host_display_name();
        self.set_status(format!("Starting {} on {}...", container_name, host_name));

        // Start the container (handles remote via discovery)
        let host = self.docker_items.selected_host.clone();
        match DockerDiscovery::start_container_on_host(container_id, &host) {
            Ok(()) => {
                self.set_status(format!("Started {}, connecting...", container_name));
                self.exec_into_container(container_id, container_name);
            }
            Err(e) => {
                self.set_status(format!("Failed to start {}: {}", container_name, e));
            }
        }
    }

    /// Runs an image as a new container interactively.
    pub fn run_image_interactive(&mut self, image_name: &str, display_name: &str) {
        assert!(!image_name.is_empty(), "image_name must not be empty");

        let shell = self.docker_default_shell().to_string();
        let docker_cmd = DockerDiscovery::build_run_command(image_name, &shell);
        let cmd = self.build_command_for_host(&docker_cmd);

        let host_name = self.docker_host_display_name();
        self.set_status(format!("Running {} on {}...", display_name, host_name));

        // Create a new terminal tab with the docker run command
        self.create_docker_terminal_tab(&cmd, image_name, display_name);

        // Hide Docker manager if visible
        self.hide_docker_manager();
    }

    /// Runs an image with custom options.
    pub fn run_image_with_options(
        &mut self,
        image_name: &str,
        display_name: &str,
        options: &crate::docker::DockerRunOptions,
    ) {
        assert!(!image_name.is_empty(), "image_name must not be empty");

        // Build the command with options
        let docker_cmd = if cfg!(target_os = "windows") {
            "docker.exe"
        } else {
            "docker"
        };

        let args = options.build_args(image_name);
        let docker_run_cmd = format!("{} run {}", docker_cmd, args.join(" "));
        let cmd = self.build_command_for_host(&docker_run_cmd);

        let host_name = self.docker_host_display_name();
        self.set_status(format!(
            "Running {} with options on {}...",
            display_name, host_name
        ));

        // Create a new terminal tab with the docker run command
        self.create_docker_terminal_tab(&cmd, image_name, display_name);

        // Hide Docker manager if visible
        self.hide_docker_manager();
    }

    /// Creates a terminal tab for Docker commands.
    fn create_docker_terminal_tab(
        &mut self,
        command: &str,
        container_id: &str,
        container_name: &str,
    ) {
        let Some(ref mut terminals) = self.terminals else {
            self.set_status("No terminal available".to_string());
            return;
        };

        // Add a new tab
        match terminals.add_tab() {
            Ok(tab_idx) => {
                // Set the tab name to the container name
                terminals.set_tab_name(tab_idx, container_name.to_string());

                // Send the command to the new terminal
                if let Some(terminal) = terminals.active_terminal_mut() {
                    // Write the command
                    let _ = terminal.write(command.as_bytes());
                    let _ = terminal.write(b"\n");

                    // Store Docker context for stats/logs hotkeys
                    terminal.set_docker_context(Some(crate::terminal::DockerContext::new(
                        container_id.to_string(),
                        container_name.to_string(),
                    )));
                }

                self.set_status(format!("Connected to {}", container_name));
            }
            Err(e) => {
                self.set_status(format!("Failed to create terminal tab: {}", e));
            }
        }
    }

    /// Builds a command for a specific container host.
    fn build_command_for_container_host(
        &self,
        docker_cmd: &str,
        host: &crate::terminal::ContainerHost,
    ) -> String {
        match host {
            crate::terminal::ContainerHost::Local => docker_cmd.to_string(),
            crate::terminal::ContainerHost::Remote {
                hostname,
                port,
                username,
                ..
            } => {
                // Build SSH command to run Docker on remote host
                // The container's terminal already carries the SSH details,
                // so build the remote invocation from those directly.
                let port_flag = if *port == 22 {
                    String::new()
                } else {
                    format!("-p {port} ")
                };
                format!("ssh {port_flag}{username}@{hostname} {docker_cmd}")
            }
        }
    }

    /// Shows Docker stats in a split pane.
    pub fn show_docker_stats(&mut self) {
        let Some(ref terminals) = self.terminals else {
            self.set_status("No terminal available".to_string());
            return;
        };

        // Get Docker context from active terminal
        let context = terminals
            .active_terminal()
            .and_then(|t| t.docker_context().cloned());

        let Some(ctx) = context else {
            self.set_status("Not in a Docker session".to_string());
            return;
        };

        let docker_cmd = DockerDiscovery::build_stats_command(&ctx.container_id);
        // Use the host from the container context, not the currently selected host
        let cmd = self.build_command_for_container_host(&docker_cmd, &ctx.host);

        // Split the terminal and run stats command
        self.split_terminal_with_command(&cmd, &format!("Stats: {}", ctx.container_name));
    }

    /// Shows Docker logs in a split pane.
    pub fn show_docker_logs(&mut self) {
        let Some(ref terminals) = self.terminals else {
            self.set_status("No terminal available".to_string());
            return;
        };

        // Get Docker context from active terminal
        let context = terminals
            .active_terminal()
            .and_then(|t| t.docker_context().cloned());

        let Some(ctx) = context else {
            self.set_status("Not in a Docker session".to_string());
            return;
        };

        let docker_cmd = DockerDiscovery::build_logs_command(&ctx.container_id);
        // Use the host from the container context, not the currently selected host
        let cmd = self.build_command_for_container_host(&docker_cmd, &ctx.host);

        // Split the terminal and run logs command
        self.split_terminal_with_command(&cmd, &format!("Logs: {}", ctx.container_name));
    }

    /// Splits the terminal and runs a command in the new pane.
    fn split_terminal_with_command(&mut self, command: &str, name: &str) {
        let Some(ref mut terminals) = self.terminals else {
            return;
        };

        // Split the current tab
        match terminals.split() {
            Ok(()) => {
                // Get the new terminal and send the command
                if let Some(terminal) = terminals.active_terminal_mut() {
                    let _ = terminal.write(command.as_bytes());
                    let _ = terminal.write(b"\n");
                }
                self.set_status(format!("Opened {}", name));
            }
            Err(e) => {
                self.set_status(format!("Failed to split terminal: {}", e));
            }
        }
    }

    /// Checks if the active terminal is a Docker session.
    #[must_use]
    pub fn is_docker_session(&self) -> bool {
        self.terminals
            .as_ref()
            .and_then(|t| t.active_terminal())
            .map(|t| t.docker_context().is_some())
            .unwrap_or(false)
    }

    /// Gets the Docker context from the active terminal.
    #[must_use]
    pub fn active_docker_context(&self) -> Option<&crate::terminal::DockerContext> {
        self.terminals
            .as_ref()
            .and_then(|t| t.active_terminal())
            .and_then(|t| t.docker_context())
    }

    // =========================================================================
    // Background Image Pull Operations
    // =========================================================================

    /// Spawns a background task to pull a Docker image.
    ///
    /// The result will be available via `check_docker_background_tasks()`.
    pub fn spawn_background_image_pull(&mut self, host: DockerHost, image_name: String) {
        use std::sync::mpsc::channel;
        use std::thread;

        info!(
            "Spawning background pull for image '{}' on {:?}",
            image_name, host
        );

        let (tx, rx) = channel();
        self.docker_background_rx = Some(rx);

        let image_clone = image_name.clone();
        thread::spawn(move || {
            let result = DockerDiscovery::pull_image_on_host(&host, &image_clone);
            let msg = super::DockerBackgroundResult::ImagePulled {
                image: image_clone,
                success: result.is_ok(),
                error: result.err(),
            };
            let _ = tx.send(msg);
        });

        self.set_status(format!("Downloading image '{}'...", image_name));
    }

    /// Checks for completed background Docker operations.
    ///
    /// Call this periodically (e.g., in the event loop) to handle results.
    /// Returns `true` if a result was processed.
    pub fn check_docker_background_tasks(&mut self) -> bool {
        let result = if let Some(ref rx) = self.docker_background_rx {
            match rx.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Empty) => return false,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.docker_background_rx = None;
                    return false;
                }
            }
        } else {
            return false;
        };

        if let Some(result) = result {
            self.docker_background_rx = None;
            self.handle_docker_background_result(result);
            return true;
        }

        false
    }

    /// Handles a completed background Docker operation.
    fn handle_docker_background_result(&mut self, result: super::DockerBackgroundResult) {
        match result {
            super::DockerBackgroundResult::ImagePulled {
                image,
                success,
                error,
            } => {
                if success {
                    info!("Background pull completed for '{}'", image);
                    self.set_status(format!("Downloaded '{}' successfully", image));

                    // Update creation state
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.on_image_pull_complete(true, None);
                    }
                } else {
                    let err_msg = error.unwrap_or_else(|| "Unknown error".to_string());
                    warn!("Background pull failed for '{}': {}", image, err_msg);
                    self.set_status(format!("Failed to download '{}': {}", image, err_msg));

                    // Update creation state with error
                    if let Some(ref mut manager) = self.docker_manager {
                        manager.on_image_pull_complete(false, Some(err_msg));
                    }
                }
                self.request_redraw();
            }
        }
    }
}
