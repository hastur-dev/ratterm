//! Metrics collector for SSH health monitoring.
//!
//! Collects system metrics (CPU, memory, disk, GPU) from remote SSH hosts
//! using background threads for parallel collection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tracing::{debug, error, info, warn};

use super::host::{SSHCredentials, SSHHost, SSHHostList};
use super::metrics::{
    DeviceMetrics, GpuMetrics, GpuType, MAX_CONCURRENT_CONNECTIONS, MetricStatus,
};

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
pub struct MetricsCollector {
    /// Cached metrics by host ID.
    metrics: Arc<Mutex<HashMap<u32, DeviceMetrics>>>,
    /// Flag to stop collection.
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
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
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

    /// Gets cached metrics for a host.
    #[must_use]
    pub fn get_metrics(&self, host_id: u32) -> Option<DeviceMetrics> {
        let guard = self.metrics.lock().ok()?;
        guard.get(&host_id).cloned()
    }

    /// Gets all cached metrics.
    #[must_use]
    pub fn get_all_metrics(&self) -> HashMap<u32, DeviceMetrics> {
        self.metrics
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
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

        // Mark all hosts as collecting
        if let Ok(mut guard) = self.metrics.lock() {
            for host in hosts {
                guard
                    .entry(host.host_id)
                    .or_insert_with(|| DeviceMetrics::collecting(host.host_id))
                    .status = MetricStatus::Collecting;
            }
        }

        self.last_collection = Instant::now();
        self.running.store(true, Ordering::Relaxed);

        // Spawn collection threads (limited concurrency)
        let chunk_size = (hosts.len() / MAX_CONCURRENT_CONNECTIONS).max(1);
        for chunk in hosts.chunks(chunk_size) {
            for host in chunk {
                let host_info = host.clone();
                let metrics = Arc::clone(&self.metrics);
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

                    if let Ok(mut guard) = metrics.lock() {
                        guard.insert(host_info.host_id, result);
                    }
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

    // Check if we need password authentication
    if let Some(ref password) = host.password {
        debug!("Using password authentication for {}", host.hostname);
        return collect_with_password(host, password);
    }

    // No password - use key-based authentication with BatchMode
    debug!("Using key-based authentication for {}", host.hostname);
    collect_with_key(host)
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
fn collect_with_plink(host: &HostCollectionInfo, password: &str) -> DeviceMetrics {
    // Check if plink is available
    if !is_plink_available() {
        return DeviceMetrics::with_error(
            host.host_id,
            "plink not found - install PuTTY for password auth".to_string(),
        );
    }

    let mut cmd = Command::new("plink");

    // Use -batch mode but also add -no-antispoof to suppress prompts
    // Note: -batch will fail if host key not cached, but user likely connected via SSH manager already
    cmd.arg("-batch");
    cmd.arg("-no-antispoof");

    // Password
    cmd.arg("-pw").arg(password);

    // Add port if not default
    if host.port != 22 {
        cmd.arg("-P").arg(host.port.to_string());
    }

    // Add user@host
    let target = format!("{}@{}", host.username, host.hostname);
    cmd.arg(&target);

    // Add the metrics command
    cmd.arg(METRICS_COMMAND);

    debug!(
        "plink command: plink -batch -no-antispoof -pw *** {} {}",
        if host.port != 22 {
            format!("-P {}", host.port)
        } else {
            String::new()
        },
        target
    );

    let result = execute_ssh_command(cmd, host.host_id);

    // If plink failed (likely host key issue), show helpful message
    if result.status == MetricStatus::Error {
        if let Some(ref err) = result.error {
            if err.contains("host key") || err.contains("SSH failed") {
                warn!(
                    "plink failed for {} - try connecting via SSH Manager first to cache host key",
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

/// Executes an SSH command and parses the output into metrics.
fn execute_ssh_command(mut cmd: Command, host_id: u32) -> DeviceMetrics {
    // Configure for non-interactive execution
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    debug!("Executing SSH command for host_id={}", host_id);

    // Execute with timeout
    let output = match cmd
        .output()
        .map_err(|e| format!("Failed to execute SSH: {}", e))
    {
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
        warn!(
            "SSH failed for host_id={}: stderr={}, stdout_len={}",
            host_id,
            stderr.chars().take(200).collect::<String>(),
            stdout.len()
        );

        let error = if stderr.contains("Permission denied") {
            "Permission denied".to_string()
        } else if stderr.contains("Connection refused") {
            "Connection refused".to_string()
        } else if stderr.contains("Connection timed out") || stderr.contains("timed out") {
            "Connection timed out".to_string()
        } else if stderr.is_empty() {
            format!("SSH failed (exit code {:?})", output.status.code())
        } else {
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
    hosts
        .hosts()
        .filter_map(|host| {
            let creds = hosts.get_credentials(host.id)?;
            let mut info = HostCollectionInfo::from_host(host, Some(creds))?;

            // Add jump host if configured
            if let Ok(Some(jump_info)) = hosts.build_jump_chain(host.id) {
                info.jump_host = Some(jump_info.proxy_jump_string());
            }

            Some(info)
        })
        .collect()
}

#[cfg(test)]
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
}
