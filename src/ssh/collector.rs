//! Metrics collector for SSH health monitoring.
//!
//! Collects system metrics (CPU, memory, disk, GPU) from remote SSH hosts
//! using background threads for parallel collection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows flags to prevent spawned processes from affecting the parent console.
/// DETACHED_PROCESS: Process has no console at all
/// CREATE_NO_WINDOW: Process has no visible window
/// Combined, these should prevent plink.exe from corrupting keyboard input.
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x00000008;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

use tracing::{debug, error, info, warn};

use super::host::{SSHCredentials, SSHHost, SSHHostList};
use super::metrics::{
    DeviceMetrics, GpuMetrics, GpuType, MAX_CONCURRENT_CONNECTIONS, MetricStatus,
    SSH_COMMAND_TIMEOUT_SECS,
};

/// Total process timeout for SSH commands in seconds.
/// Must exceed the SSH `ConnectTimeout` (5s) to let the SSH client
/// report its own timeout errors before we force-kill the process.
const SSH_PROCESS_TIMEOUT_SECS: u64 = SSH_COMMAND_TIMEOUT_SECS + 5;

/// Combined command to collect all metrics in one SSH exec.
const METRICS_COMMAND: &str = r#"echo "===CPU===" && cat /proc/loadavg && nproc && echo "===MEM===" && cat /proc/meminfo | grep -E 'MemTotal|MemAvailable|MemFree|SwapTotal|SwapFree' && echo "===DISK===" && df -BG / | tail -1 && echo "===GPU===" && (nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu --format=csv,noheader,nounits 2>/dev/null || rocm-smi --showuse 2>/dev/null || echo "NO_GPU")"#;

/// Information needed to collect metrics from a host.
#[derive(Debug, Clone)]
pub struct HostCollectionInfo {
    /// Host ID.
    pub host_id: u32,
    /// SSH hostname or IP.
    pub hostname: String,
    /// SSH port.
    pub port: u16,
    /// SSH username.
    pub username: String,
    /// SSH password (optional).
    pub password: Option<String>,
    /// SSH key path (optional).
    pub key_path: Option<String>,
    /// ProxyJump string for multi-hop (optional).
    pub jump_host: Option<String>,
}

impl HostCollectionInfo {
    /// Creates collection info from an SSH host and credentials.
    pub fn from_host(host: &SSHHost, creds: Option<&SSHCredentials>) -> Option<Self> {
        let creds = creds?;
        Some(Self {
            host_id: host.id,
            hostname: host.hostname.clone(),
            port: host.port,
            username: creds.username.clone(),
            password: creds.password.clone(),
            key_path: creds.key_path.clone(),
            jump_host: None,
        })
    }

    /// Sets the jump host for this collection info.
    #[must_use]
    pub fn with_jump_host(mut self, jump_host: String) -> Self {
        self.jump_host = Some(jump_host);
        self
    }
}

/// Metrics collector for SSH hosts.
///
/// Uses an `mpsc` channel so background threads never contend with the
/// main thread.  Background threads `send()` completed metrics through
/// the channel; the main thread drains them with `try_recv()` inside
/// [`poll_results`].  This guarantees the main event loop is never
/// blocked by SSH timeout / lock contention.
pub struct MetricsCollector {
    /// Local (main-thread-only) cache of metrics by host ID.
    metrics: HashMap<u32, DeviceMetrics>,
    /// Receiver end of the results channel (main thread only).
    results_rx: mpsc::Receiver<(u32, DeviceMetrics)>,
    /// Sender end cloned into each background thread.
    results_tx: mpsc::Sender<(u32, DeviceMetrics)>,
    /// Flag to cancel running collection threads.
    running: Arc<AtomicBool>,
    /// Collection thread handles.
    handles: Vec<JoinHandle<()>>,
    /// Last collection start time.
    last_collection: Instant,
}

impl MetricsCollector {
    /// Creates a new metrics collector.
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            metrics: HashMap::new(),
            results_rx: rx,
            results_tx: tx,
            running: Arc::new(AtomicBool::new(false)),
            handles: Vec::new(),
            last_collection: Instant::now(),
        }
    }

    /// Returns true if collection is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Drains completed results from background threads (non-blocking).
    ///
    /// Call this from the main event loop every tick.  It uses
    /// `try_recv()` so it never blocks — if no results are ready it
    /// returns immediately.
    pub fn poll_results(&mut self) {
        // Drain up to 50 results per tick to stay bounded.
        for _ in 0..50 {
            match self.results_rx.try_recv() {
                Ok((host_id, metrics)) => {
                    self.metrics.insert(host_id, metrics);
                }
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    /// Gets cached metrics for a host (main thread only, no lock).
    #[must_use]
    pub fn get_metrics(&self, host_id: u32) -> Option<DeviceMetrics> {
        self.metrics.get(&host_id).cloned()
    }

    /// Gets all cached metrics (main thread only, no lock).
    #[must_use]
    pub fn get_all_metrics(&self) -> &HashMap<u32, DeviceMetrics> {
        &self.metrics
    }

    /// Starts a collection cycle for the given hosts.
    pub fn collect(&mut self, hosts: &[HostCollectionInfo]) {
        assert!(
            hosts.len() <= 50,
            "Cannot collect metrics for more than 50 hosts"
        );

        info!("Starting metrics collection for {} hosts", hosts.len());
        for host in hosts {
            debug!(
                "  Host: {} (id={}, user={}, has_password={}, has_jump={})",
                host.hostname,
                host.host_id,
                host.username,
                host.password.is_some(),
                host.jump_host.is_some()
            );
        }

        // Clean up finished threads
        let before = self.handles.len();
        self.handles.retain(|h| !h.is_finished());
        let after = self.handles.len();
        if before != after {
            debug!("Cleaned up {} finished threads", before - after);
        }

        // Set "Collecting" status for hosts that don't have data yet.
        // This is now a local HashMap — no lock contention.
        for host in hosts {
            self.metrics
                .entry(host.host_id)
                .or_insert_with(|| DeviceMetrics::collecting(host.host_id));
        }

        self.last_collection = Instant::now();
        self.running.store(true, Ordering::Relaxed);

        // Spawn collection threads (limited concurrency)
        let chunk_size = (hosts.len() / MAX_CONCURRENT_CONNECTIONS).max(1);
        for chunk in hosts.chunks(chunk_size) {
            for host in chunk {
                let host_info = host.clone();
                let tx = self.results_tx.clone();
                let running = Arc::clone(&self.running);

                let handle = thread::spawn(move || {
                    if !running.load(Ordering::Relaxed) {
                        debug!("Collection cancelled for host {}", host_info.hostname);
                        return;
                    }

                    info!(
                        "Collecting metrics for {} (id={})",
                        host_info.hostname, host_info.host_id
                    );
                    let result = collect_host_metrics(&host_info);
                    info!(
                        "Collection complete for {}: status={:?}",
                        host_info.hostname, result.status
                    );

                    // Send through channel — never blocks the main thread.
                    let _ = tx.send((host_info.host_id, result));
                });

                self.handles.push(handle);
            }
        }
        info!("Spawned {} collection threads", self.handles.len());
    }

    /// Stops all collection threads.
    pub fn stop(&mut self) {
        info!(
            "Stopping metrics collector ({} active threads)",
            self.handles.len()
        );
        self.running.store(false, Ordering::Relaxed);
        // Don't wait for threads - they'll finish on their own
    }

    /// Returns the time since last collection started.
    #[must_use]
    pub fn time_since_collection(&self) -> Duration {
        self.last_collection.elapsed()
    }

    /// Checks if all collection threads have finished.
    #[must_use]
    pub fn is_collection_complete(&self) -> bool {
        self.handles.iter().all(|h| h.is_finished())
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MetricsCollector {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Collects metrics from a single host via SSH.
fn collect_host_metrics(host: &HostCollectionInfo) -> DeviceMetrics {
    debug!(
        "collect_host_metrics: host={}, has_password={}, has_key={}, has_jump={}",
        host.hostname,
        host.password.is_some(),
        host.key_path.is_some(),
        host.jump_host.is_some()
    );

    // Strategy: Try key-based auth first (uses SSH agent/default keys), then fall back to password
    // This is because many servers are configured for key-only auth, and the user's SSH
    // terminal connections work via keys even when passwords are stored in the app.

    // First, try key-based authentication (works with SSH agent and ~/.ssh/ keys)
    info!(
        "Trying key-based authentication first for {}",
        host.hostname
    );
    let key_result = collect_with_key(host);

    // If key-based auth succeeded, return the result
    if key_result.status != MetricStatus::Error {
        info!(
            "Key-based auth succeeded for {}: {:?}",
            host.hostname, key_result.status
        );
        return key_result;
    }

    // Key-based auth failed - check if we have a password to try
    if let Some(ref password) = host.password {
        // Check if the error was auth-related (worth trying password) vs network (don't bother)
        let error_msg = key_result.error.as_deref().unwrap_or("");
        if error_msg.contains("timed out") || error_msg.contains("Connection refused") {
            info!(
                "Key-based auth failed with network error for {}, not trying password: {}",
                host.hostname, error_msg
            );
            return key_result;
        }

        info!(
            "Key-based auth failed for {}, trying password authentication. Error was: {}",
            host.hostname, error_msg
        );
        return collect_with_password(host, password);
    }

    // No password available, return the key-based auth error
    info!(
        "Key-based auth failed for {} and no password available",
        host.hostname
    );
    key_result
}

/// Collects metrics using SSH key authentication.
fn collect_with_key(host: &HostCollectionInfo) -> DeviceMetrics {
    let ssh_path = find_ssh_path();

    let mut cmd = Command::new(&ssh_path);

    // Build SSH arguments for key-based auth
    cmd.arg("-o").arg("BatchMode=yes");
    cmd.arg("-o").arg("ConnectTimeout=5");
    cmd.arg("-o").arg("StrictHostKeyChecking=no");

    // Add key path if specified
    if let Some(ref key_path) = host.key_path {
        cmd.arg("-i").arg(key_path);
    }

    // Add jump host if specified
    if let Some(ref jump) = host.jump_host {
        cmd.arg("-J").arg(jump);
    }

    // Add port if not default
    if host.port != 22 {
        cmd.arg("-p").arg(host.port.to_string());
    }

    // Add user@host
    let target = format!("{}@{}", host.username, host.hostname);
    cmd.arg(&target);

    // Add the metrics command
    cmd.arg(METRICS_COMMAND);

    execute_ssh_command(cmd, host.host_id)
}

/// Collects metrics using password authentication via sshpass (Linux/Mac) or plink/WSL (Windows).
fn collect_with_password(host: &HostCollectionInfo, password: &str) -> DeviceMetrics {
    if cfg!(target_os = "windows") {
        debug!("Windows: checking auth methods for {}", host.hostname);

        // On Windows, prefer WSL sshpass (handles jump hosts), fall back to plink
        if is_wsl_sshpass_available() {
            info!("Using WSL sshpass for {}", host.hostname);
            return collect_with_wsl_sshpass(host, password);
        }
        debug!("WSL sshpass not available");

        // plink doesn't support ProxyJump, so jump hosts won't work
        if host.jump_host.is_some() {
            error!(
                "Jump host configured for {} but WSL sshpass not available",
                host.hostname
            );
            return DeviceMetrics::with_error(
                host.host_id,
                "Jump hosts need WSL with sshpass (apt install sshpass)".to_string(),
            );
        }

        // Try plink for direct connections
        if is_plink_available() {
            info!("Using plink for {}", host.hostname);
            return collect_with_plink(host, password);
        }
        debug!("plink not available");

        error!("No auth method available for {}", host.hostname);
        DeviceMetrics::with_error(
            host.host_id,
            "Install WSL+sshpass or PuTTY for password auth".to_string(),
        )
    } else {
        info!("Using sshpass for {}", host.hostname);
        collect_with_sshpass(host, password)
    }
}

/// Collects metrics using plink (PuTTY) on Windows.
///
/// Uses cmd.exe to pipe 'y' to plink to auto-accept host key prompts.
fn collect_with_plink(host: &HostCollectionInfo, password: &str) -> DeviceMetrics {
    // Check if plink is available
    if !is_plink_available() {
        return DeviceMetrics::with_error(
            host.host_id,
            "plink not found - install PuTTY for password auth".to_string(),
        );
    }

    // Build the plink command string
    // We use cmd.exe to pipe 'y' to auto-accept host key if not cached
    let port_arg = if host.port != 22 {
        format!("-P {} ", host.port)
    } else {
        String::new()
    };

    let target = format!("{}@{}", host.username, host.hostname);

    // Build plink command - use -batch to avoid interactive prompts
    // Don't use cmd.exe wrapper - call plink directly to avoid shell escaping issues
    info!(
        "plink command for {}: plink -batch -no-antispoof -pw *** {}{}",
        host.hostname, port_arg, target
    );

    let mut cmd = Command::new("plink");
    cmd.arg("-batch"); // Non-interactive mode, auto-accept host key
    cmd.arg("-no-antispoof");
    cmd.arg("-pw").arg(password); // Pass password directly, no escaping needed

    if host.port != 22 {
        cmd.arg("-P").arg(host.port.to_string());
    }

    cmd.arg(&target);
    cmd.arg(METRICS_COMMAND);

    let result = execute_ssh_command(cmd, host.host_id);

    // Log the result
    info!(
        "plink result for {}: status={:?}, error={:?}",
        host.hostname, result.status, result.error
    );

    // If plink still failed, show helpful message
    if result.status == MetricStatus::Error {
        if let Some(ref err) = result.error {
            error!("plink failed for {}: {}", host.hostname, err);
            if err.contains("host key") || err.contains("refused") || err.contains("Access denied")
            {
                warn!(
                    "plink auth failed for {} - check credentials or use key-based auth",
                    host.hostname
                );
            }
        }
    }

    result
}

/// Collects metrics using sshpass on Linux/Mac.
fn collect_with_sshpass(host: &HostCollectionInfo, password: &str) -> DeviceMetrics {
    // Check if sshpass is available
    if !is_sshpass_available() {
        return DeviceMetrics::with_error(
            host.host_id,
            "sshpass not found - install it for password auth".to_string(),
        );
    }

    let mut cmd = Command::new("sshpass");

    // Pass password via -p
    cmd.arg("-p").arg(password);

    // SSH command
    cmd.arg("ssh");
    cmd.arg("-o").arg("ConnectTimeout=5");
    cmd.arg("-o").arg("StrictHostKeyChecking=no");

    // Add key path if specified (in addition to password)
    if let Some(ref key_path) = host.key_path {
        cmd.arg("-i").arg(key_path);
    }

    // Add jump host if specified
    if let Some(ref jump) = host.jump_host {
        cmd.arg("-J").arg(jump);
    }

    // Add port if not default
    if host.port != 22 {
        cmd.arg("-p").arg(host.port.to_string());
    }

    // Add user@host
    let target = format!("{}@{}", host.username, host.hostname);
    cmd.arg(&target);

    // Add the metrics command
    cmd.arg(METRICS_COMMAND);

    execute_ssh_command(cmd, host.host_id)
}

/// Spawns a command and waits for completion with a process-level timeout.
///
/// If the process doesn't exit within `timeout_secs`, it is killed and
/// an error is returned. This prevents SSH processes from hanging
/// indefinitely when a host is partially reachable (accepts TCP but
/// stalls during authentication or command execution).
fn spawn_with_timeout(
    cmd: &mut Command,
    timeout_secs: u64,
) -> Result<std::process::Output, String> {
    assert!(timeout_secs > 0, "timeout must be positive");

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to execute SSH: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|e| format!("Failed to read SSH output: {e}"));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait(); // Reap the process
                return Err("SSH command timed out".to_string());
            }
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Error waiting for SSH process: {e}"));
            }
        }
    }
}

/// Executes an SSH command and parses the output into metrics.
fn execute_ssh_command(mut cmd: Command, host_id: u32) -> DeviceMetrics {
    // Configure for non-interactive execution
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // CRITICAL: On Windows, use DETACHED_PROCESS to completely detach from
    // the parent console. This prevents plink.exe from corrupting the console's
    // input mode, which would cause crossterm to only receive Release events
    // (no Press events) for special keys like Escape, Ctrl, and arrow keys.
    #[cfg(windows)]
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);

    debug!("Executing SSH command for host_id={}", host_id);

    // Execute with process-level timeout to prevent indefinite hangs
    let output = match spawn_with_timeout(&mut cmd, SSH_PROCESS_TIMEOUT_SECS) {
        Ok(output) => output,
        Err(e) => {
            error!("SSH execution failed for host_id={}: {}", host_id, e);
            return DeviceMetrics::with_error(host_id, e);
        }
    };

    debug!(
        "SSH command completed for host_id={}: exit_code={:?}",
        host_id,
        output.status.code()
    );

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Log full stderr for debugging
        error!(
            "SSH failed for host_id={}: exit_code={:?}",
            host_id,
            output.status.code()
        );
        error!("  STDERR: {}", stderr);
        error!(
            "  STDOUT (len={}): {}",
            stdout.len(),
            stdout.chars().take(200).collect::<String>()
        );

        let error = if stderr.contains("Permission denied") || stderr.contains("permission denied")
        {
            "Permission denied".to_string()
        } else if stderr.contains("Access denied") || stderr.contains("access denied") {
            "Access denied".to_string()
        } else if stderr.contains("Connection refused") {
            "Connection refused".to_string()
        } else if stderr.contains("Connection timed out") || stderr.contains("timed out") {
            "Connection timed out".to_string()
        } else if stderr.contains("No such file") || stderr.contains("not found") {
            "SSH client not found".to_string()
        } else if stderr.is_empty() {
            format!("SSH failed (exit code {:?})", output.status.code())
        } else {
            // Return first 100 chars of stderr for unknown errors
            stderr.chars().take(100).collect()
        };
        return DeviceMetrics::with_error(host_id, error);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!(
        "SSH stdout for host_id={} (len={}): {}",
        host_id,
        stdout.len(),
        stdout.chars().take(200).collect::<String>()
    );

    parse_metrics_output(&stdout, host_id)
}

/// Checks if sshpass is available on the system.
fn is_sshpass_available() -> bool {
    Command::new("sshpass")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Checks if plink (PuTTY) is available on the system.
fn is_plink_available() -> bool {
    Command::new("plink")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Checks if sshpass is available via WSL on Windows.
fn is_wsl_sshpass_available() -> bool {
    Command::new("wsl")
        .args(["which", "sshpass"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Collects metrics using sshpass via WSL on Windows.
fn collect_with_wsl_sshpass(host: &HostCollectionInfo, password: &str) -> DeviceMetrics {
    let mut cmd = Command::new("wsl");

    // Build the sshpass command to run in WSL
    let mut ssh_args = vec![
        "sshpass".to_string(),
        "-p".to_string(),
        password.to_string(),
        "ssh".to_string(),
        "-o".to_string(),
        "ConnectTimeout=5".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=no".to_string(),
    ];

    // Add jump host if specified
    if let Some(ref jump) = host.jump_host {
        ssh_args.push("-J".to_string());
        ssh_args.push(jump.clone());
    }

    // Add port if not default
    if host.port != 22 {
        ssh_args.push("-p".to_string());
        ssh_args.push(host.port.to_string());
    }

    // Add user@host
    ssh_args.push(format!("{}@{}", host.username, host.hostname));

    // Add the metrics command
    ssh_args.push(METRICS_COMMAND.to_string());

    cmd.args(&ssh_args);

    execute_ssh_command(cmd, host.host_id)
}

/// Parses the combined metrics output from SSH.
fn parse_metrics_output(output: &str, host_id: u32) -> DeviceMetrics {
    let mut metrics = DeviceMetrics::new(host_id);

    // Split by section markers and pair them: ["CPU", data], ["MEM", data], etc.
    // The output format is: ===CPU===\ndata\n===MEM===\ndata\n...
    let sections: Vec<&str> = output.split("===").collect();

    // Sections array: ["", "CPU", "\ndata\n", "MEM", "\ndata\n", ...]
    // We need to pair markers (odd indices) with their data (next even index)
    let mut i = 1;
    while i + 1 < sections.len() {
        let marker = sections[i].trim();
        let data = sections[i + 1];

        match marker {
            "CPU" => parse_cpu_section(data, &mut metrics),
            "MEM" => parse_mem_section(data, &mut metrics),
            "DISK" => parse_disk_section(data, &mut metrics),
            "GPU" => parse_gpu_section(data, &mut metrics),
            _ => {}
        }

        i += 2;
    }

    metrics.mark_online();
    metrics
}

/// Parses the CPU section of metrics output.
fn parse_cpu_section(data: &str, metrics: &mut DeviceMetrics) {
    // Data contains: "0.45 0.62 0.38 1/234 5678\n8\n"
    let lines: Vec<&str> = data.lines().filter(|l| !l.trim().is_empty()).collect();

    // First line: load average (e.g., "0.45 0.62 0.38 1/234 5678")
    if let Some(load_line) = lines.first() {
        let parts: Vec<&str> = load_line.split_whitespace().collect();
        if parts.len() >= 3 {
            let load1 = parts[0].parse().unwrap_or(0.0);
            let load5 = parts[1].parse().unwrap_or(0.0);
            let load15 = parts[2].parse().unwrap_or(0.0);
            metrics.load_avg = (load1, load5, load15);

            // Estimate CPU usage from load average and cores
            if metrics.cpu_cores > 0 {
                let usage = (load1 / metrics.cpu_cores as f32) * 100.0;
                metrics.cpu_usage_percent = usage.clamp(0.0, 100.0);
            }
        }
    }

    // Second line: nproc output (number of cores)
    if let Some(nproc_line) = lines.get(1) {
        if let Ok(cores) = nproc_line.trim().parse::<u16>() {
            metrics.cpu_cores = cores;

            // Recalculate CPU usage with correct core count
            let usage = (metrics.load_avg.0 / cores as f32) * 100.0;
            metrics.cpu_usage_percent = usage.clamp(0.0, 100.0);
        }
    }

    assert!(
        metrics.cpu_usage_percent >= 0.0,
        "CPU usage cannot be negative"
    );
    assert!(
        metrics.cpu_usage_percent <= 100.0,
        "CPU usage cannot exceed 100%"
    );
}

/// Parses the memory section of metrics output.
fn parse_mem_section(data: &str, metrics: &mut DeviceMetrics) {
    for line in data.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let key = parts[0].trim_end_matches(':');
            let value_kb: u64 = parts[1].parse().unwrap_or(0);
            let value_mb = value_kb / 1024;

            match key {
                "MemTotal" => metrics.mem_total_mb = value_mb,
                "MemAvailable" => metrics.mem_available_mb = value_mb,
                "MemFree" => {
                    // Use MemFree if MemAvailable not present
                    if metrics.mem_available_mb == 0 {
                        metrics.mem_available_mb = value_mb;
                    }
                }
                "SwapTotal" => metrics.swap_total_mb = value_mb,
                "SwapFree" => {
                    metrics.swap_used_mb = metrics.swap_total_mb.saturating_sub(value_mb);
                }
                _ => {}
            }
        }
    }

    // Calculate used memory
    metrics.mem_used_mb = metrics
        .mem_total_mb
        .saturating_sub(metrics.mem_available_mb);

    assert!(
        metrics.mem_used_mb <= metrics.mem_total_mb,
        "Used memory cannot exceed total memory"
    );
}

/// Parses the disk section of metrics output.
fn parse_disk_section(data: &str, metrics: &mut DeviceMetrics) {
    // df output: Filesystem Size Used Avail Use% Mounted
    for line in data.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            // Size and Used are in format like "512G"
            metrics.disk_total_gb = parse_size_gb(parts[1]);
            metrics.disk_used_gb = parse_size_gb(parts[2]);
            break; // Only need root filesystem
        }
    }

    assert!(
        metrics.disk_used_gb <= metrics.disk_total_gb,
        "Used disk cannot exceed total disk"
    );
}

/// Parses a size string like "512G" to GB.
fn parse_size_gb(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }

    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().unwrap_or(0)
}

/// Parses the GPU section of metrics output.
fn parse_gpu_section(data: &str, metrics: &mut DeviceMetrics) {
    let content = data.trim();

    if content.is_empty() || content.contains("NO_GPU") {
        metrics.gpu = None;
        return;
    }

    // Try NVIDIA format: name, utilization.gpu, memory.used, memory.total, temperature
    // Example: "NVIDIA GeForce RTX 3060, 85, 8192, 12288, 68"
    let parts: Vec<&str> = content.split(',').map(|s| s.trim()).collect();

    if parts.len() >= 5 {
        let mut gpu = GpuMetrics::new(GpuType::Nvidia, parts[0].to_string());
        gpu.usage_percent = parts[1].parse().unwrap_or(0.0);
        gpu.memory_used_mb = parts[2].parse().unwrap_or(0);
        gpu.memory_total_mb = parts[3].parse().unwrap_or(0);
        gpu.temperature_celsius = parts[4].parse().ok();
        metrics.gpu = Some(gpu);
        return;
    }

    // Try AMD rocm-smi format (simplified)
    if content.contains("GPU") || content.contains("rocm") {
        let gpu = GpuMetrics::new(GpuType::Amd, "AMD GPU".to_string());
        // Parse AMD output if needed
        metrics.gpu = Some(gpu);
    }
}

/// Finds the SSH executable path.
fn find_ssh_path() -> PathBuf {
    #[cfg(windows)]
    {
        // Try Windows OpenSSH first
        let windows_ssh = PathBuf::from(r"C:\Windows\System32\OpenSSH\ssh.exe");
        if windows_ssh.exists() {
            return windows_ssh;
        }

        // Try Git Bash SSH
        let git_ssh = PathBuf::from(r"C:\Program Files\Git\usr\bin\ssh.exe");
        if git_ssh.exists() {
            return git_ssh;
        }

        // Fall back to PATH
        PathBuf::from("ssh")
    }

    #[cfg(not(windows))]
    {
        PathBuf::from("ssh")
    }
}

/// Builds collection info for all hosts with credentials.
pub fn build_collection_info(hosts: &SSHHostList) -> Vec<HostCollectionInfo> {
    info!(
        "build_collection_info: Building collection info for {} total hosts",
        hosts.len()
    );

    let mut included_count = 0;
    let mut excluded_count = 0;

    let result: Vec<HostCollectionInfo> = hosts
        .hosts()
        .filter_map(|host| {
            let creds = hosts.get_credentials(host.id);
            match creds {
                Some(c) => {
                    let mut info = HostCollectionInfo::from_host(host, Some(c))?;

                    // Add jump host if configured
                    if let Ok(Some(jump_info)) = hosts.build_jump_chain(host.id) {
                        info.jump_host = Some(jump_info.proxy_jump_string());
                        debug!(
                            "  [INCLUDE] Host {} '{}': username='{}', jump_host='{}'",
                            host.id,
                            host.hostname,
                            c.username,
                            info.jump_host.as_deref().unwrap_or("-")
                        );
                    } else {
                        debug!(
                            "  [INCLUDE] Host {} '{}': username='{}', no jump host",
                            host.id, host.hostname, c.username
                        );
                    }

                    included_count += 1;
                    Some(info)
                }
                None => {
                    debug!(
                        "  [EXCLUDE] Host {} '{}': no credentials found",
                        host.id, host.hostname
                    );
                    excluded_count += 1;
                    None
                }
            }
        })
        .collect();

    info!(
        "build_collection_info: Result: {} hosts included, {} excluded (no credentials)",
        included_count, excluded_count
    );

    if result.is_empty() && !hosts.is_empty() {
        warn!(
            "build_collection_info: EMPTY RESULT - {} hosts exist but none have credentials! \
             Metrics collection will not run.",
            hosts.len()
        );
    }

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_gb() {
        assert_eq!(parse_size_gb("512G"), 512);
        assert_eq!(parse_size_gb("100G"), 100);
        assert_eq!(parse_size_gb("0G"), 0);
        assert_eq!(parse_size_gb(""), 0);
    }

    #[test]
    fn test_parse_cpu_section() {
        // Data portion only, without the "CPU===" marker
        let data = "0.45 0.62 0.38 1/234 5678\n8";
        let mut metrics = DeviceMetrics::new(1);
        parse_cpu_section(data, &mut metrics);

        assert!((metrics.load_avg.0 - 0.45).abs() < 0.01);
        assert_eq!(metrics.cpu_cores, 8);
    }

    #[test]
    fn test_parse_mem_section() {
        // Data portion only, without the "MEM===" marker
        let data = "MemTotal:       16384000 kB\nMemAvailable:    8192000 kB\nSwapTotal:       4096000 kB\nSwapFree:        4096000 kB";
        let mut metrics = DeviceMetrics::new(1);
        parse_mem_section(data, &mut metrics);

        assert_eq!(metrics.mem_total_mb, 16000); // 16384000 / 1024
        assert_eq!(metrics.mem_available_mb, 8000);
        assert_eq!(metrics.swap_used_mb, 0);
    }

    #[test]
    fn test_parse_gpu_section_nvidia() {
        // Data portion only, without the "GPU===" marker
        let data = "NVIDIA GeForce RTX 3060, 85, 8192, 12288, 68";
        let mut metrics = DeviceMetrics::new(1);
        parse_gpu_section(data, &mut metrics);

        assert!(metrics.gpu.is_some());
        let gpu = metrics.gpu.unwrap();
        assert_eq!(gpu.gpu_type, GpuType::Nvidia);
        assert!((gpu.usage_percent - 85.0).abs() < 0.01);
        assert_eq!(gpu.memory_used_mb, 8192);
    }

    #[test]
    fn test_parse_gpu_section_none() {
        // Data portion only, without the "GPU===" marker
        let data = "NO_GPU";
        let mut metrics = DeviceMetrics::new(1);
        parse_gpu_section(data, &mut metrics);

        assert!(metrics.gpu.is_none());
    }

    #[test]
    fn test_collector_creation() {
        let collector = MetricsCollector::new();
        assert!(!collector.is_running());
        assert!(collector.get_all_metrics().is_empty());
    }

    #[test]
    fn test_collector_poll_results_is_nonblocking() {
        let mut collector = MetricsCollector::new();
        assert!(collector.get_all_metrics().is_empty());

        // poll_results on an empty channel should return instantly.
        let start = std::time::Instant::now();
        collector.poll_results();
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 50,
            "poll_results() should be non-blocking (took {}ms)",
            elapsed.as_millis()
        );
        assert!(collector.get_all_metrics().is_empty());
    }

    #[test]
    fn test_collector_receives_results_via_channel() {
        let mut collector = MetricsCollector::new();

        // Manually send a result through the internal channel.
        let tx = collector.results_tx.clone();
        let metrics = DeviceMetrics::new(42);
        tx.send((42, metrics)).unwrap();

        // Before polling, cache should be empty.
        assert!(collector.get_all_metrics().is_empty());

        // After polling, the result should appear.
        collector.poll_results();
        assert_eq!(collector.get_all_metrics().len(), 1);
        assert!(collector.get_metrics(42).is_some());
    }

    // ========================================================================
    // spawn_with_timeout tests
    // ========================================================================

    #[test]
    fn test_spawn_with_timeout_completes_fast_command() {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/c", "echo", "hello"]);
            c
        } else {
            let mut c = Command::new("echo");
            c.arg("hello");
            c
        };
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let result = spawn_with_timeout(&mut cmd, 5);
        assert!(result.is_ok(), "Fast command should succeed: {:?}", result);

        let output = result.unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty(), "Should capture stdout");
    }

    #[test]
    fn test_spawn_with_timeout_kills_slow_process() {
        // Spawn a process that runs much longer than the timeout
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("ping");
            c.args(["-n", "30", "127.0.0.1"]);
            c
        } else {
            let mut c = Command::new("sleep");
            c.arg("30");
            c
        };
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let start = Instant::now();
        let result = spawn_with_timeout(&mut cmd, 1);
        let elapsed = start.elapsed();

        assert!(result.is_err(), "Should timeout");
        assert!(
            result.unwrap_err().contains("timed out"),
            "Error should mention timeout"
        );
        // Should finish in about 1-2 seconds, not 30
        assert!(
            elapsed.as_secs() < 5,
            "Should be killed quickly, took {}s",
            elapsed.as_secs()
        );
    }

    #[test]
    fn test_spawn_with_timeout_returns_error_on_bad_command() {
        let mut cmd = Command::new("this_command_does_not_exist_99999");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let result = spawn_with_timeout(&mut cmd, 5);
        assert!(result.is_err(), "Bad command should return error");
        assert!(
            result.unwrap_err().contains("Failed to execute"),
            "Should report spawn failure"
        );
    }

    #[test]
    fn test_spawn_with_timeout_captures_nonzero_exit() {
        // Command that exits with non-zero status
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/c", "exit", "1"]);
            c
        } else {
            Command::new("false")
        };
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let result = spawn_with_timeout(&mut cmd, 5);
        assert!(result.is_ok(), "Should return output even on non-zero exit");
        assert!(
            !result.unwrap().status.success(),
            "Exit status should be non-zero"
        );
    }
}
