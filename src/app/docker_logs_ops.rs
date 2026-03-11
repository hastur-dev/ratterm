//! Docker log streaming operations at the App level.
//!
//! Handles starting/stopping log streams, polling for new entries,
//! and managing the log viewer lifecycle.

use tokio::sync::mpsc;

use crate::docker_logs::log_stream::LogStream;
use crate::docker_logs::ui::state::DockerLogsState;
use crate::ui::docker_manager::DockerManagerMode;

use super::App;

impl App {
    /// Opens the Docker log viewer from the Docker Manager list.
    ///
    /// Creates the log state and switches the Docker Manager to LogView mode.
    pub fn docker_open_logs(&mut self) {
        let config = self.config.docker_log_config.clone();
        let mut state = DockerLogsState::new(config);

        // Load saved searches
        let _ = state.search_manager_mut().load();

        // Populate container list from the current Docker Manager data
        let containers = {
            let Some(ref manager) = self.docker_manager else {
                return;
            };
            manager.all_container_log_infos()
        };

        state.set_containers(containers);

        if let Some(ref mut manager) = self.docker_manager {
            manager.docker_logs_state = Some(state);
            manager.set_mode(DockerManagerMode::LogView);
        }

        self.set_status("Docker Logs - select a container");
    }

    /// Starts streaming logs for a specific container.
    pub fn docker_start_log_stream(&mut self, container_id: &str, container_name: &str) {
        // Enter streaming mode in state
        if let Some(ref mut manager) = self.docker_manager {
            if let Some(ref mut state) = manager.docker_logs_state {
                state.enter_streaming(
                    container_id.to_string(),
                    container_name.to_string(),
                );
                state.set_status(format!("Connecting to {}...", container_name));
            }
        }

        // Try to connect to Docker and start the stream
        let docker = match bollard::Docker::connect_with_local_defaults() {
            Ok(d) => d,
            Err(e) => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.set_error(format!("Failed to connect to Docker: {}", e));
                    }
                }
                return;
            }
        };

        let tail_lines = self.config.docker_log_config.tail_lines;

        match LogStream::start(
            docker,
            container_id.to_string(),
            container_name.to_string(),
            tail_lines,
        ) {
            Ok((stream, rx)) => {
                self.docker_log_stream = Some(stream);
                self.docker_log_rx = Some(rx);
                self.set_status(format!("Streaming logs from {}", container_name));
            }
            Err(e) => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.set_error(format!("Failed to start stream: {}", e));
                    }
                }
            }
        }
    }

    /// Stops the active log stream.
    pub fn docker_stop_log_stream(&mut self) {
        if let Some(ref stream) = self.docker_log_stream {
            stream.stop();
        }
        self.docker_log_stream = None;
        self.docker_log_rx = None;
    }

    /// Closes the log view entirely and returns to Docker Manager list.
    pub fn docker_close_log_view(&mut self) {
        self.docker_stop_log_stream();
        if let Some(ref mut manager) = self.docker_manager {
            manager.docker_logs_state = None;
            manager.set_mode(DockerManagerMode::List);
        }
    }

    /// Polls the log stream receiver and pushes entries into the buffer.
    ///
    /// Called from `App::update()` on each frame.
    pub fn poll_docker_log_stream(&mut self) {
        let Some(ref mut rx) = self.docker_log_rx else {
            return;
        };

        // Drain up to 100 entries per frame to avoid blocking
        let mut count = 0;
        let max_per_frame = 100;

        loop {
            if count >= max_per_frame {
                break;
            }

            match rx.try_recv() {
                Ok(entry) => {
                    if let Some(ref mut manager) = self.docker_manager {
                        if let Some(ref mut state) = manager.docker_logs_state {
                            state.log_buffer_mut().push(entry);
                        }
                    }
                    count += 1;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Stream ended
                    self.docker_log_rx = None;
                    if let Some(ref mut manager) = self.docker_manager {
                        if let Some(ref mut state) = manager.docker_logs_state {
                            state.set_status("Stream ended".to_string());
                        }
                    }
                    break;
                }
            }
        }
    }
}
