//! Background process manager.
//!
//! Manages background terminal processes that run without UI rendering.
//! Provides status tracking and output collection for AI manipulation.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

/// Maximum output buffer size per process (characters).
const MAX_OUTPUT_BUFFER: usize = 100_000;

/// Maximum number of concurrent background processes.
const MAX_BACKGROUND_PROCESSES: usize = 10;

/// Background process status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    /// Process is currently running.
    Running,
    /// Process completed successfully (exit code 0).
    Completed,
    /// Process completed with an error (non-zero exit code).
    Error,
    /// Process was killed by user.
    Killed,
}

impl ProcessStatus {
    /// Returns true if this status represents an error state.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, ProcessStatus::Error)
    }

    /// Returns true if the process has finished (not running).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        !matches!(self, ProcessStatus::Running)
    }
}

/// Information about a background process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Unique process ID (internal).
    pub id: u64,
    /// Command that was executed.
    pub command: String,
    /// Current status.
    pub status: ProcessStatus,
    /// Exit code (if finished).
    pub exit_code: Option<i32>,
    /// Error message (if error occurred).
    pub error_message: Option<String>,
    /// When the process was started.
    pub started_at: Instant,
    /// When the process finished (if finished).
    pub finished_at: Option<Instant>,
}

/// Internal state for a background process.
struct BackgroundProcess {
    /// Process info (shared with readers).
    info: ProcessInfo,
    /// Output buffer (stdout + stderr combined).
    output: String,
    /// Thread handle for output reader.
    reader_handle: Option<JoinHandle<()>>,
    /// Flag to signal thread to stop.
    stop_flag: Arc<AtomicBool>,
    /// Child process handle (for killing).
    child: Option<Child>,
}

/// Background process manager.
///
/// Manages multiple background processes, tracks their status,
/// and collects their output for later retrieval.
pub struct BackgroundManager {
    /// Active processes.
    processes: HashMap<u64, Arc<Mutex<BackgroundProcess>>>,
    /// Next process ID.
    next_id: AtomicU64,
    /// Count of currently running processes.
    running_count: usize,
    /// Count of processes with errors.
    error_count: usize,
}

impl BackgroundManager {
    /// Creates a new background manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            next_id: AtomicU64::new(1),
            running_count: 0,
            error_count: 0,
        }
    }

    /// Returns the number of currently running background processes.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.running_count
    }

    /// Returns the number of processes that have errored.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Returns true if there are any running background processes.
    #[must_use]
    pub fn has_running(&self) -> bool {
        self.running_count > 0
    }

    /// Returns true if any background process has errored.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    /// Starts a new background process.
    ///
    /// # Arguments
    /// * `command` - The command to execute (passed to shell)
    ///
    /// # Returns
    /// The process ID on success, or an error message.
    pub fn start(&mut self, command: &str) -> Result<u64, String> {
        // Check limits
        if self.running_count >= MAX_BACKGROUND_PROCESSES {
            return Err(format!(
                "Maximum background processes ({}) reached",
                MAX_BACKGROUND_PROCESSES
            ));
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // Determine shell based on platform
        #[cfg(windows)]
        let (shell, shell_arg) = ("cmd", "/C");
        #[cfg(not(windows))]
        let (shell, shell_arg) = ("sh", "-c");

        // Spawn the process
        let child_result = Command::new(shell)
            .arg(shell_arg)
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        let mut child = match child_result {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to spawn process: {}", e)),
        };

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Take stdout and stderr
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let process = BackgroundProcess {
            info: ProcessInfo {
                id,
                command: command.to_string(),
                status: ProcessStatus::Running,
                exit_code: None,
                error_message: None,
                started_at: Instant::now(),
                finished_at: None,
            },
            output: String::new(),
            reader_handle: None,
            stop_flag,
            child: Some(child),
        };

        let process_arc = Arc::new(Mutex::new(process));
        let process_arc_clone = process_arc.clone();

        // Spawn thread to read output
        let handle = thread::Builder::new()
            .name(format!("bg-proc-{}", id))
            .spawn(move || {
                Self::output_reader_thread(process_arc_clone, stdout, stderr, stop_flag_clone);
            })
            .ok();

        if let Ok(mut proc) = process_arc.lock() {
            proc.reader_handle = handle;
        }

        self.processes.insert(id, process_arc);
        self.running_count += 1;

        Ok(id)
    }

    /// Output reader thread function.
    fn output_reader_thread(
        process: Arc<Mutex<BackgroundProcess>>,
        stdout: Option<std::process::ChildStdout>,
        stderr: Option<std::process::ChildStderr>,
        stop_flag: Arc<AtomicBool>,
    ) {
        // Read stdout in a separate thread
        let stdout_output = Arc::new(Mutex::new(String::new()));
        let stdout_output_clone = stdout_output.clone();
        let stop_flag_stdout = stop_flag.clone();

        let stdout_handle = stdout.map(|out| {
            thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines() {
                    if stop_flag_stdout.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(line) = line {
                        if let Ok(mut output) = stdout_output_clone.lock() {
                            if output.len() < MAX_OUTPUT_BUFFER {
                                output.push_str(&line);
                                output.push('\n');
                            }
                        }
                    }
                }
            })
        });

        // Read stderr
        let stderr_output = Arc::new(Mutex::new(String::new()));
        let stderr_output_clone = stderr_output.clone();
        let stop_flag_stderr = stop_flag.clone();

        let stderr_handle = stderr.map(|err| {
            thread::spawn(move || {
                let reader = BufReader::new(err);
                for line in reader.lines() {
                    if stop_flag_stderr.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(line) = line {
                        if let Ok(mut output) = stderr_output_clone.lock() {
                            if output.len() < MAX_OUTPUT_BUFFER {
                                output.push_str(&line);
                                output.push('\n');
                            }
                        }
                    }
                }
            })
        });

        // Wait for readers to finish
        if let Some(h) = stdout_handle {
            let _ = h.join();
        }
        if let Some(h) = stderr_handle {
            let _ = h.join();
        }

        // Combine output
        let combined_output = {
            let stdout_str = stdout_output.lock().map(|s| s.clone()).unwrap_or_default();
            let stderr_str = stderr_output.lock().map(|s| s.clone()).unwrap_or_default();
            format!("{}{}", stdout_str, stderr_str)
        };

        // Wait for process to finish and get exit status
        if let Ok(mut proc) = process.lock() {
            proc.output = combined_output;

            if let Some(ref mut child) = proc.child {
                match child.wait() {
                    Ok(status) => {
                        let exit_code = status.code();
                        proc.info.exit_code = exit_code;
                        proc.info.finished_at = Some(Instant::now());

                        if status.success() {
                            proc.info.status = ProcessStatus::Completed;
                        } else {
                            proc.info.status = ProcessStatus::Error;
                            proc.info.error_message = Some(format!(
                                "Process exited with code {}",
                                exit_code.unwrap_or(-1)
                            ));
                        }
                    }
                    Err(e) => {
                        proc.info.status = ProcessStatus::Error;
                        proc.info.error_message = Some(format!("Failed to wait for process: {}", e));
                        proc.info.finished_at = Some(Instant::now());
                    }
                }
            }
        }
    }

    /// Gets information about a specific process.
    #[must_use]
    pub fn get_info(&self, id: u64) -> Option<ProcessInfo> {
        self.processes
            .get(&id)
            .and_then(|p| p.lock().ok())
            .map(|p| p.info.clone())
    }

    /// Gets the output of a specific process.
    #[must_use]
    pub fn get_output(&self, id: u64) -> Option<String> {
        self.processes
            .get(&id)
            .and_then(|p| p.lock().ok())
            .map(|p| p.output.clone())
    }

    /// Lists all processes (including finished ones).
    #[must_use]
    pub fn list(&self) -> Vec<ProcessInfo> {
        self.processes
            .values()
            .filter_map(|p| p.lock().ok().map(|p| p.info.clone()))
            .collect()
    }

    /// Lists only running processes.
    #[must_use]
    pub fn list_running(&self) -> Vec<ProcessInfo> {
        self.list()
            .into_iter()
            .filter(|p| p.status == ProcessStatus::Running)
            .collect()
    }

    /// Kills a background process.
    pub fn kill(&mut self, id: u64) -> Result<(), String> {
        let process = self
            .processes
            .get(&id)
            .ok_or_else(|| format!("Process {} not found", id))?;

        if let Ok(mut proc) = process.lock() {
            if proc.info.status != ProcessStatus::Running {
                return Err(format!("Process {} is not running", id));
            }

            // Signal thread to stop
            proc.stop_flag.store(true, Ordering::SeqCst);

            // Kill the child process
            if let Some(ref mut child) = proc.child {
                if let Err(e) = child.kill() {
                    return Err(format!("Failed to kill process: {}", e));
                }
            }

            proc.info.status = ProcessStatus::Killed;
            proc.info.finished_at = Some(Instant::now());
        }

        Ok(())
    }

    /// Clears finished processes from the list.
    pub fn clear_finished(&mut self) {
        let finished_ids: Vec<u64> = self
            .processes
            .iter()
            .filter_map(|(id, p)| {
                p.lock()
                    .ok()
                    .filter(|p| p.info.status.is_finished())
                    .map(|_| *id)
            })
            .collect();

        for id in finished_ids {
            self.processes.remove(&id);
        }
    }

    /// Clears error count (acknowledges errors).
    pub fn clear_errors(&mut self) {
        self.error_count = 0;
    }

    /// Updates the running and error counts by checking process states.
    pub fn update_counts(&mut self) {
        let mut running = 0;
        let mut errors = 0;

        for process in self.processes.values() {
            if let Ok(proc) = process.lock() {
                match proc.info.status {
                    ProcessStatus::Running => running += 1,
                    ProcessStatus::Error => errors += 1,
                    _ => {}
                }
            }
        }

        self.running_count = running;
        self.error_count = errors;
    }
}

impl Default for BackgroundManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = BackgroundManager::new();
        assert_eq!(manager.running_count(), 0);
        assert_eq!(manager.error_count(), 0);
        assert!(!manager.has_running());
        assert!(!manager.has_errors());
    }

    #[test]
    fn test_process_status() {
        assert!(ProcessStatus::Error.is_error());
        assert!(!ProcessStatus::Running.is_error());
        assert!(!ProcessStatus::Completed.is_error());

        assert!(!ProcessStatus::Running.is_finished());
        assert!(ProcessStatus::Completed.is_finished());
        assert!(ProcessStatus::Error.is_finished());
        assert!(ProcessStatus::Killed.is_finished());
    }
}
