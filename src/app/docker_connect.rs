//! Docker container connection operations.

use crate::docker::{DockerDiscovery, DockerHost};

use super::App;

impl App {
    /// Builds a command, wrapping with SSH for remote hosts.
    fn build_command_for_host(&self, docker_cmd: &str) -> String {
        let host = &self.docker_items.selected_host;
        match host {
            DockerHost::Local => docker_cmd.to_string(),
            DockerHost::Remote { .. } => {
                DockerDiscovery::build_remote_docker_command(host, docker_cmd)
            }
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
            DockerHost::Remote {
                host_id,
                hostname,
                port,
                username,
                password,
                ..
            } => {
                // Remote container - use SSH + docker exec
                terminals.add_docker_exec_ssh_tab(
                    container_id,
                    container_name,
                    &shell,
                    hostname,
                    *port,
                    username,
                    *host_id,
                    password.as_deref(),
                )
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
                let docker_host = DockerHost::Remote {
                    host_id: 0,
                    hostname: hostname.clone(),
                    port: *port,
                    username: username.clone(),
                    password: None,
                    display_name: None,
                };
                DockerDiscovery::build_remote_docker_command(&docker_host, docker_cmd)
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
}
