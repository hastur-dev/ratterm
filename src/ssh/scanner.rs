//! Network scanner for discovering SSH hosts.
//!
//! Scans local network for hosts with port 22 (SSH) open.
//! Runs in a background thread to avoid blocking the UI.
//! Supports authenticated scanning to only add hosts that accept given credentials.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ssh2::Session;

/// Result of an SSH authentication attempt.
///
/// Distinguishes between hosts that are not reachable (TCP connect failed),
/// hosts that are reachable but auth failed, and successful authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    /// TCP connect failed — host is not reachable on port 22.
    NotReachable,
    /// Port is open but SSH authentication failed.
    ReachableNotAuthenticated,
    /// Authentication succeeded, with an optional remote hostname.
    Authenticated(Option<String>),
}

/// Maximum hosts to scan in a single subnet.
const MAX_HOSTS: usize = 254;

/// Connection timeout for port scanning.
/// Increased to 1500ms for slower networks.
const CONNECT_TIMEOUT_MS: u64 = 1500;

/// Maximum parallel connections during scan.
const MAX_PARALLEL: usize = 32;

/// Represents a detected network interface.
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    /// Display name of the interface (e.g., "WiFi (192.168.1.5)").
    pub name: String,
    /// Subnet in CIDR notation (e.g., "192.168.1.0/24").
    pub subnet: String,
    /// Whether this is the primary/default interface.
    pub is_primary: bool,
}

/// Result from the network scanner.
#[derive(Debug, Clone)]
pub enum ScanResult {
    /// Scan progress update (scanned, total).
    Progress(usize, usize),
    /// Found a host with SSH port open.
    HostFound(String, u16),
    /// Scan completed with list of all found hosts.
    Complete(Vec<String>),
    /// Scan error.
    Error(String),
    /// Scan was cancelled.
    Cancelled,
    /// Authenticated scan progress update (scanned, total, auth_success, auth_fail).
    AuthProgress(usize, usize, usize, usize),
    /// Host successfully authenticated, with optional remote hostname.
    AuthSuccess(String, u16, Option<String>),
    /// Authenticated scan completed with list of authenticated hosts.
    AuthComplete(Vec<String>),
}

/// Asynchronous network scanner for SSH hosts.
#[derive(Debug)]
pub struct NetworkScanner {
    /// Background scan thread handle.
    scan_handle: Option<JoinHandle<Vec<String>>>,
    /// Channel for receiving scan results.
    result_rx: Option<Receiver<ScanResult>>,
    /// Current scan progress.
    progress: Arc<AtomicUsize>,
    /// Total hosts to scan.
    total: Arc<AtomicUsize>,
    /// Flag to cancel the scan.
    cancelled: Arc<AtomicBool>,
    /// Whether a scan is in progress.
    scanning: bool,
    /// The subnet currently being scanned.
    current_subnet: Option<String>,
}

impl NetworkScanner {
    /// Creates a new network scanner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scan_handle: None,
            result_rx: None,
            progress: Arc::new(AtomicUsize::new(0)),
            total: Arc::new(AtomicUsize::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
            scanning: false,
            current_subnet: None,
        }
    }

    /// Returns the subnet currently being scanned.
    #[must_use]
    pub fn current_subnet(&self) -> Option<&str> {
        self.current_subnet.as_deref()
    }

    /// Returns true if a scan is currently in progress.
    #[must_use]
    pub fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// Returns the current scan progress as (scanned, total).
    #[must_use]
    pub fn progress(&self) -> (usize, usize) {
        (
            self.progress.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    /// Starts a network scan for the given subnet.
    ///
    /// # Arguments
    /// * `subnet` - Subnet in CIDR notation (e.g., "192.168.1.0/24")
    ///
    /// Returns Ok(()) if scan started, Err if already scanning.
    pub fn start_scan(&mut self, subnet: &str) -> Result<(), String> {
        if self.scanning {
            return Err("Scan already in progress".to_string());
        }

        // Parse subnet
        let (base_ip, prefix_len) = self.parse_subnet(subnet)?;

        // Calculate host range
        let hosts = self.calculate_hosts(base_ip, prefix_len);
        let host_count = hosts.len();

        if host_count == 0 {
            return Err("No hosts in subnet".to_string());
        }

        // Store the subnet being scanned
        self.current_subnet = Some(subnet.to_string());

        // Reset state
        self.progress.store(0, Ordering::Relaxed);
        self.total.store(host_count, Ordering::Relaxed);
        self.cancelled.store(false, Ordering::Relaxed);

        // Create channel
        let (tx, rx) = mpsc::channel();
        self.result_rx = Some(rx);

        // Clone atomics for thread
        let progress = Arc::clone(&self.progress);
        let cancelled = Arc::clone(&self.cancelled);

        // Spawn background thread
        let handle = thread::spawn(move || {
            let mut found_hosts = Vec::new();
            let timeout = Duration::from_millis(CONNECT_TIMEOUT_MS);

            // Process hosts in chunks for parallelism
            for chunk in hosts.chunks(MAX_PARALLEL) {
                if cancelled.load(Ordering::Relaxed) {
                    let _ = tx.send(ScanResult::Cancelled);
                    return found_hosts;
                }

                // Spawn threads for this chunk
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|ip| {
                        let ip = *ip;
                        thread::spawn(move || {
                            let addr = SocketAddr::new(IpAddr::V4(ip), 22);
                            TcpStream::connect_timeout(&addr, timeout).is_ok()
                        })
                    })
                    .collect();

                // Collect results
                for (ip, handle) in chunk.iter().zip(handles) {
                    if let Ok(is_open) = handle.join()
                        && is_open
                    {
                        let ip_str = ip.to_string();
                        found_hosts.push(ip_str.clone());
                        let _ = tx.send(ScanResult::HostFound(ip_str, 22));
                    }
                    progress.fetch_add(1, Ordering::Relaxed);
                }

                // Send progress update
                let current = progress.load(Ordering::Relaxed);
                let _ = tx.send(ScanResult::Progress(current, host_count));
            }

            // Send completion
            let _ = tx.send(ScanResult::Complete(found_hosts.clone()));
            found_hosts
        });

        self.scan_handle = Some(handle);
        self.scanning = true;

        Ok(())
    }

    /// Starts a scan using auto-detected local subnet.
    pub fn start_auto_scan(&mut self) -> Result<(), String> {
        let subnet = self.detect_local_subnet()?;
        self.start_scan(&subnet)
    }

    /// Polls for scan results (non-blocking).
    ///
    /// Returns Some(result) if there's a result, None otherwise.
    pub fn poll(&mut self) -> Option<ScanResult> {
        let Some(ref rx) = self.result_rx else {
            return None;
        };

        // Return one result at a time to ensure none are lost
        match rx.try_recv() {
            Ok(result) => {
                // Check if scan is complete
                if matches!(
                    result,
                    ScanResult::Complete(_)
                        | ScanResult::AuthComplete(_)
                        | ScanResult::Error(_)
                        | ScanResult::Cancelled
                ) {
                    self.scanning = false;
                    // Clean up thread handle
                    if let Some(handle) = self.scan_handle.take() {
                        let _ = handle.join();
                    }
                }
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.scanning = false;
                None
            }
        }
    }

    /// Cancels the ongoing scan.
    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        // Wait for thread to finish
        if let Some(handle) = self.scan_handle.take() {
            let _ = handle.join();
        }
        self.scanning = false;
        self.current_subnet = None;
    }

    /// Detects the local network subnet using primary interface.
    fn detect_local_subnet(&self) -> Result<String, String> {
        // Try to get local IP by connecting to a public address
        // This doesn't actually send data, just determines the local interface
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to create socket: {}", e))?;

        socket
            .connect("8.8.8.8:80")
            .map_err(|e| format!("Failed to connect: {}", e))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?;

        match local_addr.ip() {
            IpAddr::V4(ip) => {
                // Assume /24 subnet for simplicity
                let octets = ip.octets();
                Ok(format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]))
            }
            IpAddr::V6(_) => Err("IPv6 not supported for scanning".to_string()),
        }
    }

    /// Detects all available local network interfaces and their subnets.
    /// Returns a list of (interface_name, subnet) tuples.
    #[must_use]
    pub fn detect_all_interfaces() -> Vec<NetworkInterface> {
        let mut interfaces = Vec::new();

        // Try to detect primary interface first
        if let Ok(subnet) = Self::detect_primary_subnet() {
            interfaces.push(NetworkInterface {
                name: "Primary".to_string(),
                subnet,
                is_primary: true,
            });
        }

        // Add common private network subnets that might be in use
        // These are the most common home/office network ranges
        let common_subnets = [
            ("192.168.0.0/24", "192.168.0.x"),
            ("192.168.1.0/24", "192.168.1.x"),
            ("192.168.2.0/24", "192.168.2.x"),
            ("10.0.0.0/24", "10.0.0.x"),
            ("10.0.1.0/24", "10.0.1.x"),
            ("172.16.0.0/24", "172.16.0.x"),
        ];

        for (subnet, name) in common_subnets {
            // Don't add duplicates
            if !interfaces.iter().any(|i| i.subnet == subnet) {
                interfaces.push(NetworkInterface {
                    name: name.to_string(),
                    subnet: subnet.to_string(),
                    is_primary: false,
                });
            }
        }

        // Try platform-specific detection. The branch is chosen with `cfg!`
        // rather than `#[cfg]` so both detectors are type-checked, linted and
        // tested on every host; only the call is compiled away.
        if cfg!(windows) {
            Self::detect_windows_interfaces(&mut interfaces);
        } else if cfg!(unix) {
            Self::detect_unix_interfaces(&mut interfaces);
        }

        interfaces
    }

    /// Detects the primary subnet (static version, can be called without instance).
    pub fn detect_primary_subnet_static() -> Result<String, String> {
        Self::detect_primary_subnet()
    }

    /// Detects the primary subnet (internal implementation).
    fn detect_primary_subnet() -> Result<String, String> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to create socket: {}", e))?;

        socket
            .connect("8.8.8.8:80")
            .map_err(|e| format!("Failed to connect: {}", e))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?;

        match local_addr.ip() {
            IpAddr::V4(ip) => {
                let octets = ip.octets();
                Ok(format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]))
            }
            IpAddr::V6(_) => Err("IPv6 not supported".to_string()),
        }
    }

    /// Windows-specific interface detection using ipconfig.
    fn detect_windows_interfaces(interfaces: &mut Vec<NetworkInterface>) {
        use std::process::Command;

        let output = match Command::new("ipconfig").output() {
            Ok(o) => o,
            Err(_) => return,
        };

        Self::parse_ipconfig(&String::from_utf8_lossy(&output.stdout), interfaces);
    }

    /// Extracts interfaces from the text `ipconfig` prints.
    ///
    /// Split out from the command so the Windows parser is compiled, linted and
    /// tested on every platform. A parser that exists only on one target is a
    /// parser no other target's build can catch a mistake in, which is how a
    /// Windows-only edit reached this branch and broke every other job.
    fn parse_ipconfig(stdout: &str, interfaces: &mut Vec<NetworkInterface>) {
        let mut current_adapter = String::new();

        for line in stdout.lines() {
            // Detect adapter names
            if line.ends_with(':') && !line.starts_with(' ') {
                current_adapter = line.trim_end_matches(':').to_string();
            }

            // Look for IPv4 addresses
            if (line.contains("IPv4") || line.contains("IP Address"))
                && let Some(ip_str) = line.split(':').nth(1)
            {
                let ip_str = ip_str.trim();
                // Skip loopback and link-local
                if let Ok(ip) = ip_str.parse::<Ipv4Addr>()
                    && !ip.is_loopback()
                    && !ip.is_link_local()
                {
                    let octets = ip.octets();
                    let subnet = format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]);

                    // Don't add duplicates
                    if !interfaces.iter().any(|i| i.subnet == subnet) {
                        let name = if current_adapter.contains("Wi-Fi")
                            || current_adapter.contains("Wireless")
                        {
                            format!("WiFi ({})", ip_str)
                        } else if current_adapter.contains("Ethernet") {
                            format!("Ethernet ({})", ip_str)
                        } else {
                            format!("{} ({})", current_adapter, ip_str)
                        };

                        interfaces.push(NetworkInterface {
                            name,
                            subnet,
                            is_primary: false,
                        });
                    }
                }
            }
        }
    }

    /// Unix-specific interface detection.
    fn detect_unix_interfaces(interfaces: &mut Vec<NetworkInterface>) {
        use std::process::Command;

        // Try 'ip addr' first (Linux), then 'ifconfig' (macOS/BSD)
        let output = Command::new("ip")
            .args(["addr", "show"])
            .output()
            .or_else(|_| Command::new("ifconfig").output());

        let output = match output {
            Ok(o) => o,
            Err(_) => return,
        };

        Self::parse_inet_lines(&String::from_utf8_lossy(&output.stdout), interfaces);
    }

    /// Extracts interfaces from `ip addr show` or `ifconfig` output.
    ///
    /// Both spellings appear: `inet 192.168.1.5/24` from iproute2 on Linux, and
    /// `inet 192.168.1.5 netmask 0xffffff00` from `ifconfig` on macOS and BSD.
    fn parse_inet_lines(stdout: &str, interfaces: &mut Vec<NetworkInterface>) {
        for line in stdout.lines() {
            let line = line.trim();

            // Look for "inet X.X.X.X" patterns
            if !line.starts_with("inet ") {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            // Handle both "inet 192.168.1.5/24" and "inet 192.168.1.5 netmask"
            let ip_part = parts[1];
            let ip_str = ip_part.split('/').next().unwrap_or(ip_part);

            if let Ok(ip) = ip_str.parse::<Ipv4Addr>()
                && !ip.is_loopback()
                && !ip.is_link_local()
            {
                let octets = ip.octets();
                let subnet = format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]);

                if !interfaces.iter().any(|i| i.subnet == subnet) {
                    interfaces.push(NetworkInterface {
                        name: format!("Interface ({})", ip_str),
                        subnet,
                        is_primary: false,
                    });
                }
            }
        }
    }

    /// Parses a subnet string into base IP and prefix length.
    fn parse_subnet(&self, subnet: &str) -> Result<(Ipv4Addr, u8), String> {
        let parts: Vec<&str> = subnet.split('/').collect();

        if parts.len() != 2 {
            return Err("Invalid subnet format (expected IP/prefix)".to_string());
        }

        let ip: Ipv4Addr = parts[0]
            .parse()
            .map_err(|_| "Invalid IP address".to_string())?;

        let prefix: u8 = parts[1]
            .parse()
            .map_err(|_| "Invalid prefix length".to_string())?;

        if prefix > 32 {
            return Err("Prefix length must be <= 32".to_string());
        }

        Ok((ip, prefix))
    }

    /// Calculates all host IPs in a subnet.
    fn calculate_hosts(&self, base_ip: Ipv4Addr, prefix_len: u8) -> Vec<Ipv4Addr> {
        if prefix_len >= 31 {
            // /31 or /32 - no usable hosts or just the IP itself
            return if prefix_len == 32 {
                vec![base_ip]
            } else {
                vec![]
            };
        }

        let base = u32::from(base_ip);
        let mask = if prefix_len == 0 {
            0
        } else {
            !((1u32 << (32 - prefix_len)) - 1)
        };

        let network = base & mask;
        let broadcast = network | !mask;

        // Skip network and broadcast addresses
        let first_host = network + 1;
        let last_host = broadcast - 1;

        let host_count = (last_host - first_host + 1) as usize;

        // Limit to MAX_HOSTS
        let limit = host_count.min(MAX_HOSTS);

        let mut hosts = Vec::with_capacity(limit);
        for i in 0..limit {
            let ip = first_host + i as u32;
            hosts.push(Ipv4Addr::from(ip));
        }

        hosts
    }

    /// Checks if a single host has SSH port open.
    #[must_use]
    pub fn check_host(ip: &str, port: u16) -> bool {
        let addr: SocketAddr = match format!("{}:{}", ip, port).parse() {
            Ok(a) => a,
            Err(_) => return false,
        };

        let timeout = Duration::from_millis(CONNECT_TIMEOUT_MS);
        TcpStream::connect_timeout(&addr, timeout).is_ok()
    }

    /// Returns common SSH ports to scan.
    #[must_use]
    pub fn common_ports() -> &'static [u16] {
        &[22, 2222, 22222]
    }

    /// Starts an authenticated network scan.
    ///
    /// This scans for open SSH ports and attempts authentication with the
    /// provided credentials. Only hosts that successfully authenticate are reported.
    ///
    /// # Arguments
    /// * `subnet` - Subnet in CIDR notation (e.g., "192.168.1.0/24")
    /// * `username` - SSH username to authenticate with
    /// * `password` - SSH password to authenticate with
    pub fn start_authenticated_scan(
        &mut self,
        subnet: &str,
        username: String,
        password: String,
    ) -> Result<(), String> {
        if self.scanning {
            return Err("Scan already in progress".to_string());
        }

        // Parse subnet
        let (base_ip, prefix_len) = self.parse_subnet(subnet)?;
        let hosts = self.calculate_hosts(base_ip, prefix_len);
        let host_count = hosts.len();

        if host_count == 0 {
            return Err("No hosts in subnet".to_string());
        }

        // Store the subnet being scanned
        self.current_subnet = Some(subnet.to_string());

        // Reset state
        self.progress.store(0, Ordering::Relaxed);
        self.total.store(host_count, Ordering::Relaxed);
        self.cancelled.store(false, Ordering::Relaxed);

        // Create channel
        let (tx, rx) = mpsc::channel();
        self.result_rx = Some(rx);

        // Clone atomics for thread
        let progress = Arc::clone(&self.progress);
        let cancelled = Arc::clone(&self.cancelled);

        // Spawn background thread for authenticated scan
        let handle = thread::spawn(move || {
            let mut authenticated_hosts = Vec::new();
            let mut auth_success = 0usize;
            let mut auth_fail = 0usize;
            let timeout = Duration::from_millis(CONNECT_TIMEOUT_MS);

            // Process hosts - use smaller parallelism for auth (more resource intensive)
            const AUTH_PARALLEL: usize = 8;

            for chunk in hosts.chunks(AUTH_PARALLEL) {
                if cancelled.load(Ordering::Relaxed) {
                    let _ = tx.send(ScanResult::Cancelled);
                    return authenticated_hosts;
                }

                // Spawn threads for this chunk
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|ip| {
                        let ip = *ip;
                        let user = username.clone();
                        let pass = password.clone();
                        thread::spawn(move || {
                            Self::try_authenticate(&ip.to_string(), 22, &user, &pass, timeout)
                        })
                    })
                    .collect();

                // Collect results
                for (ip, handle) in chunk.iter().zip(handles) {
                    if let Ok(auth_result) = handle.join() {
                        match auth_result {
                            AuthResult::Authenticated(remote_hostname) => {
                                let ip_str = ip.to_string();
                                authenticated_hosts.push(ip_str.clone());
                                auth_success += 1;
                                let _ =
                                    tx.send(ScanResult::AuthSuccess(ip_str, 22, remote_hostname));
                            }
                            AuthResult::ReachableNotAuthenticated => {
                                let ip_str = ip.to_string();
                                auth_fail += 1;
                                let _ = tx.send(ScanResult::HostFound(ip_str, 22));
                            }
                            AuthResult::NotReachable => {
                                auth_fail += 1;
                            }
                        }
                    }
                    progress.fetch_add(1, Ordering::Relaxed);
                }

                // Send progress update with auth stats
                let current = progress.load(Ordering::Relaxed);
                let _ = tx.send(ScanResult::AuthProgress(
                    current,
                    host_count,
                    auth_success,
                    auth_fail,
                ));
            }

            // Send completion
            let _ = tx.send(ScanResult::AuthComplete(authenticated_hosts.clone()));
            authenticated_hosts
        });

        self.scan_handle = Some(handle);
        self.scanning = true;

        Ok(())
    }

    /// Attempts SSH authentication to a host.
    ///
    /// Returns [`AuthResult::NotReachable`] if the TCP connection fails,
    /// [`AuthResult::ReachableNotAuthenticated`] if the port is open but
    /// authentication fails, and [`AuthResult::Authenticated`] with an
    /// optional remote hostname on success.
    fn try_authenticate(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        timeout: Duration,
    ) -> AuthResult {
        // Parse socket address
        let addr: SocketAddr = match format!("{}:{}", host, port).parse() {
            Ok(a) => a,
            Err(_) => return AuthResult::NotReachable,
        };

        // Connect with timeout — failure means host is not reachable
        let stream = match TcpStream::connect_timeout(&addr, timeout) {
            Ok(s) => s,
            Err(_) => return AuthResult::NotReachable,
        };

        // Set read/write timeout
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));

        // Create SSH session
        let mut session = match Session::new() {
            Ok(s) => s,
            Err(_) => return AuthResult::ReachableNotAuthenticated,
        };

        session.set_tcp_stream(stream);

        // Perform SSH handshake — port is open so this is "reachable"
        if session.handshake().is_err() {
            return AuthResult::ReachableNotAuthenticated;
        }

        // Attempt password authentication
        if session.userauth_password(username, password).is_err() {
            return AuthResult::ReachableNotAuthenticated;
        }

        // Authentication succeeded — query the remote hostname
        let remote_hostname = Self::query_remote_hostname(&session);
        AuthResult::Authenticated(remote_hostname)
    }

    /// Queries the remote machine's hostname over an authenticated SSH session.
    ///
    /// Runs `hostname` and returns the trimmed output, or `None` on failure.
    fn query_remote_hostname(session: &Session) -> Option<String> {
        use std::io::Read;

        let mut channel = session.channel_session().ok()?;
        channel.exec("hostname").ok()?;

        let mut output = String::with_capacity(64);
        channel.read_to_string(&mut output).ok()?;
        channel.wait_close().ok()?;

        let name = output.trim().to_string();
        if name.is_empty() { None } else { Some(name) }
    }

    /// Saves the host key for `host:port` to `~/.ssh/known_hosts`.
    ///
    /// Runs `ssh-keyscan` to retrieve the host's public keys, then appends
    /// any keys not already present in `known_hosts`. This is best-effort:
    /// failures are returned as `Err` for the caller to log.
    pub fn save_host_key(host: &str, port: u16) -> Result<(), String> {
        assert!(!host.is_empty(), "host must not be empty");
        assert!(port > 0, "port must be positive");

        let ssh_dir = dirs::home_dir()
            .ok_or_else(|| "Could not determine home directory".to_string())?
            .join(".ssh");

        if !ssh_dir.exists() {
            std::fs::create_dir_all(&ssh_dir)
                .map_err(|e| format!("Failed to create ~/.ssh: {e}"))?;
        }

        let output = Self::run_keyscan(host, port)?;
        let known_hosts_path = ssh_dir.join("known_hosts");
        let existing = std::fs::read_to_string(&known_hosts_path).unwrap_or_default();
        let new_keys = filter_new_host_keys(&output, &existing);

        if new_keys.is_empty() {
            return Ok(());
        }

        Self::append_keys(&known_hosts_path, &new_keys)
    }

    /// Runs `ssh-keyscan` and returns its stdout.
    fn run_keyscan(host: &str, port: u16) -> Result<String, String> {
        use std::process::Command;

        let mut cmd = Command::new("ssh-keyscan");
        if port != 22 {
            cmd.args(["-p", &port.to_string()]);
        }
        cmd.arg(host);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run ssh-keyscan: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.trim().is_empty() {
            return Err(format!("ssh-keyscan returned no keys for {host}"));
        }

        Ok(stdout)
    }

    /// Appends key lines to the known_hosts file.
    fn append_keys(path: &std::path::Path, keys: &[String]) -> Result<(), String> {
        use std::io::Write;

        assert!(!keys.is_empty(), "keys must not be empty");

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("Failed to open known_hosts: {e}"))?;

        for key in keys {
            writeln!(file, "{key}").map_err(|e| format!("Failed to write to known_hosts: {e}"))?;
        }

        Ok(())
    }
}

/// Filters `ssh-keyscan` output, returning only key lines whose
/// host+keytype combination is not already present in `existing`.
///
/// Skips comment lines (starting with `#`) and empty lines.
/// Each key line is expected to have the format: `host keytype keydata`.
fn filter_new_host_keys(keyscan_output: &str, existing: &str) -> Vec<String> {
    let mut new_keys = Vec::new();

    for line in keyscan_output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Key line format: "host keytype keydata"
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 3 {
            continue;
        }

        let host_field = parts[0];
        let key_type = parts[1];

        let already_exists = existing.lines().any(|existing_line| {
            let el = existing_line.trim();
            if el.is_empty() || el.starts_with('#') {
                return false;
            }
            let ep: Vec<&str> = el.splitn(3, ' ').collect();
            ep.len() >= 2 && ep[0] == host_field && ep[1] == key_type
        });

        if !already_exists {
            new_keys.push(line.to_string());
        }
    }

    new_keys
}

impl Default for NetworkScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NetworkScanner {
    fn drop(&mut self) {
        // Cancel any ongoing scan
        self.cancel();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// A trimmed `ip addr show` listing, as iproute2 prints it on Linux.
    const IP_ADDR_OUTPUT: &str = "\
1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN group default
    inet 127.0.0.1/8 scope host lo
2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default
    inet 192.168.1.5/24 brd 192.168.1.255 scope global dynamic eth0
3: eth1: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default
    inet 169.254.7.7/16 brd 169.254.255.255 scope link eth1
4: eth2: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default
    inet 10.20.30.40/24 brd 10.20.30.255 scope global eth2
";

    /// A trimmed `ifconfig` listing, as macOS and BSD print it. The netmask is
    /// a hex word rather than a prefix, which is why the parser cannot simply
    /// split on `/`.
    const IFCONFIG_OUTPUT: &str = "\
lo0: flags=8049<UP,LOOPBACK,RUNNING,MULTICAST> mtu 16384
\tinet 127.0.0.1 netmask 0xff000000
en0: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
\tinet 192.168.1.5 netmask 0xffffff00 broadcast 192.168.1.255
en1: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
\tinet 169.254.7.7 netmask 0xffff0000 broadcast 169.254.255.255
";

    /// A trimmed `ipconfig` listing, as Windows prints it.
    const IPCONFIG_OUTPUT: &str = "\
Windows IP Configuration


Wireless LAN adapter Wi-Fi:

   Connection-specific DNS Suffix  . : lan
   IPv4 Address. . . . . . . . . . . : 192.168.1.5
   Subnet Mask . . . . . . . . . . . : 255.255.255.0

Ethernet adapter Ethernet 2:

   IPv4 Address. . . . . . . . . . . : 10.20.30.40
   Subnet Mask . . . . . . . . . . . : 255.255.255.0

Unknown adapter Tailscale:

   IPv4 Address. . . . . . . . . . . : 100.64.0.7
   Subnet Mask . . . . . . . . . . . : 255.255.255.255

Tunnel adapter Loopback Pseudo-Interface 1:

   IPv4 Address. . . . . . . . . . . : 127.0.0.1
";

    #[test]
    fn parse_inet_lines_reads_iproute2_output() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_inet_lines(IP_ADDR_OUTPUT, &mut interfaces);

        let subnets: Vec<&str> = interfaces.iter().map(|i| i.subnet.as_str()).collect();
        assert_eq!(subnets, vec!["192.168.1.0/24", "10.20.30.0/24"]);
        assert_eq!(interfaces[0].name, "Interface (192.168.1.5)");
        assert!(interfaces.iter().all(|i| !i.is_primary));
    }

    #[test]
    fn parse_inet_lines_reads_ifconfig_output() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_inet_lines(IFCONFIG_OUTPUT, &mut interfaces);

        // The hex netmask must not be mistaken for part of the address, and
        // 127.0.0.1 and 169.254.7.7 are both skipped.
        let subnets: Vec<&str> = interfaces.iter().map(|i| i.subnet.as_str()).collect();
        assert_eq!(subnets, vec!["192.168.1.0/24"]);
    }

    #[test]
    fn parse_inet_lines_skips_loopback_and_link_local() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_inet_lines(
            "    inet 127.0.0.1/8 scope host lo
    inet 169.254.1.2/16 scope link eth0
",
            &mut interfaces,
        );
        assert!(interfaces.is_empty());
    }

    #[test]
    fn parse_inet_lines_ignores_malformed_lines() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_inet_lines(
            "inet
inet not-an-address
inet6 fe80::1/64 scope link
",
            &mut interfaces,
        );
        assert!(interfaces.is_empty());
    }

    #[test]
    fn parse_inet_lines_does_not_repeat_a_subnet() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_inet_lines(
            "    inet 192.168.1.5/24 scope global eth0
    inet 192.168.1.9/24 scope global eth1
",
            &mut interfaces,
        );
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].subnet, "192.168.1.0/24");
    }

    #[test]
    fn parse_inet_lines_keeps_entries_already_collected() {
        let mut interfaces = vec![NetworkInterface {
            name: "Common Home Network".to_string(),
            subnet: "192.168.1.0/24".to_string(),
            is_primary: false,
        }];
        NetworkScanner::parse_inet_lines(IP_ADDR_OUTPUT, &mut interfaces);

        // The pre-seeded subnet is not duplicated, and the new one is appended.
        assert_eq!(interfaces.len(), 2);
        assert_eq!(interfaces[0].name, "Common Home Network");
        assert_eq!(interfaces[1].subnet, "10.20.30.0/24");
    }

    #[test]
    fn parse_ipconfig_names_the_adapter() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_ipconfig(IPCONFIG_OUTPUT, &mut interfaces);

        let named: Vec<(&str, &str)> = interfaces
            .iter()
            .map(|i| (i.name.as_str(), i.subnet.as_str()))
            .collect();
        // A wireless adapter and an ethernet adapter get the short labels; an
        // adapter matching neither keeps the name ipconfig printed. Loopback
        // is dropped.
        assert_eq!(
            named,
            vec![
                ("WiFi (192.168.1.5)", "192.168.1.0/24"),
                ("Ethernet (10.20.30.40)", "10.20.30.0/24"),
                ("Unknown adapter Tailscale (100.64.0.7)", "100.64.0.0/24"),
            ]
        );
    }

    #[test]
    fn parse_ipconfig_skips_loopback() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_ipconfig(
            "Tunnel adapter Loopback Pseudo-Interface 1:
                IPv4 Address. . . . . . . . . . . : 127.0.0.1
",
            &mut interfaces,
        );
        assert!(interfaces.is_empty());
    }

    #[test]
    fn parse_ipconfig_accepts_the_older_ip_address_label() {
        let mut interfaces = Vec::new();
        NetworkScanner::parse_ipconfig(
            "Ethernet adapter Local Area Connection:
                IP Address. . . . . . . . . . . . : 10.1.2.3
",
            &mut interfaces,
        );
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].subnet, "10.1.2.0/24");
    }

    #[test]
    fn detect_all_interfaces_runs_this_platforms_detector() {
        // Whichever detector this target uses, the common subnets are always
        // seeded and nothing may be listed twice. This is the assertion that
        // fails if the platform dispatch stops calling anything at all.
        let interfaces = NetworkScanner::detect_all_interfaces();
        assert!(!interfaces.is_empty());

        let mut seen: Vec<&str> = interfaces.iter().map(|i| i.subnet.as_str()).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "a subnet was listed twice");
    }

    #[test]
    fn test_parse_subnet() {
        let scanner = NetworkScanner::new();

        let (ip, prefix) = scanner.parse_subnet("192.168.1.0/24").unwrap();
        assert_eq!(ip, Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(prefix, 24);

        let (ip, prefix) = scanner.parse_subnet("10.0.0.0/8").unwrap();
        assert_eq!(ip, Ipv4Addr::new(10, 0, 0, 0));
        assert_eq!(prefix, 8);
    }

    #[test]
    fn test_parse_subnet_invalid() {
        let scanner = NetworkScanner::new();

        assert!(scanner.parse_subnet("invalid").is_err());
        assert!(scanner.parse_subnet("192.168.1.0").is_err());
        assert!(scanner.parse_subnet("192.168.1.0/33").is_err());
    }

    #[test]
    fn test_calculate_hosts_24() {
        let scanner = NetworkScanner::new();
        let hosts = scanner.calculate_hosts(Ipv4Addr::new(192, 168, 1, 0), 24);

        // /24 has 254 usable hosts (256 - network - broadcast)
        assert_eq!(hosts.len(), 254);
        assert_eq!(hosts[0], Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(hosts[253], Ipv4Addr::new(192, 168, 1, 254));
    }

    #[test]
    fn test_calculate_hosts_30() {
        let scanner = NetworkScanner::new();
        let hosts = scanner.calculate_hosts(Ipv4Addr::new(192, 168, 1, 0), 30);

        // /30 has 2 usable hosts
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0], Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(hosts[1], Ipv4Addr::new(192, 168, 1, 2));
    }

    #[test]
    fn test_calculate_hosts_32() {
        let scanner = NetworkScanner::new();
        let hosts = scanner.calculate_hosts(Ipv4Addr::new(192, 168, 1, 1), 32);

        // /32 is a single host
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0], Ipv4Addr::new(192, 168, 1, 1));
    }

    #[test]
    fn test_scanner_lifecycle() {
        let mut scanner = NetworkScanner::new();

        assert!(!scanner.is_scanning());
        assert_eq!(scanner.progress(), (0, 0));

        // Cancel should be safe even when not scanning
        scanner.cancel();
        assert!(!scanner.is_scanning());
    }

    #[test]
    fn test_common_ports() {
        let ports = NetworkScanner::common_ports();
        assert!(ports.contains(&22));
        assert!(ports.contains(&2222));
    }

    // ==================== AuthSuccess hostname tests ====================

    #[test]
    fn test_auth_success_carries_hostname() {
        // AuthSuccess should carry an optional hostname
        let result = ScanResult::AuthSuccess(
            "192.168.1.10".to_string(),
            22,
            Some("my-server".to_string()),
        );
        match result {
            ScanResult::AuthSuccess(ip, port, hostname) => {
                assert_eq!(ip, "192.168.1.10");
                assert_eq!(port, 22);
                assert_eq!(hostname, Some("my-server".to_string()));
            }
            _ => panic!("Expected AuthSuccess"),
        }
    }

    #[test]
    fn test_auth_success_without_hostname() {
        let result = ScanResult::AuthSuccess("10.0.0.5".to_string(), 22, None);
        match result {
            ScanResult::AuthSuccess(ip, port, hostname) => {
                assert_eq!(ip, "10.0.0.5");
                assert_eq!(port, 22);
                assert!(hostname.is_none());
            }
            _ => panic!("Expected AuthSuccess"),
        }
    }

    // ==================== AuthResult enum tests ====================

    #[test]
    fn test_auth_result_not_reachable() {
        let result = AuthResult::NotReachable;
        assert_eq!(result, AuthResult::NotReachable);
        assert_ne!(result, AuthResult::ReachableNotAuthenticated);
    }

    #[test]
    fn test_auth_result_reachable_not_authenticated() {
        let result = AuthResult::ReachableNotAuthenticated;
        assert_eq!(result, AuthResult::ReachableNotAuthenticated);
        assert_ne!(result, AuthResult::NotReachable);
    }

    #[test]
    fn test_auth_result_authenticated_with_hostname() {
        let result = AuthResult::Authenticated(Some("my-host".to_string()));
        assert_eq!(
            result,
            AuthResult::Authenticated(Some("my-host".to_string()))
        );
        assert_ne!(result, AuthResult::NotReachable);
        assert_ne!(result, AuthResult::ReachableNotAuthenticated);
    }

    #[test]
    fn test_auth_result_authenticated_without_hostname() {
        let result = AuthResult::Authenticated(None);
        assert_eq!(result, AuthResult::Authenticated(None));
    }

    #[test]
    fn test_auth_result_variants_are_distinct() {
        let not_reachable = AuthResult::NotReachable;
        let reachable_no_auth = AuthResult::ReachableNotAuthenticated;
        let auth_none = AuthResult::Authenticated(None);
        let auth_some = AuthResult::Authenticated(Some("host".to_string()));

        assert_ne!(not_reachable, reachable_no_auth);
        assert_ne!(not_reachable, auth_none);
        assert_ne!(reachable_no_auth, auth_none);
        assert_ne!(auth_none, auth_some);
    }

    /// Verifies try_authenticate returns NotReachable for an unreachable host.
    #[test]
    fn test_try_authenticate_unreachable_host() {
        // Use a non-routable IP to guarantee connect failure
        let result = NetworkScanner::try_authenticate(
            "192.0.2.1", // TEST-NET, non-routable
            22,
            "user",
            "pass",
            Duration::from_millis(100),
        );
        assert_eq!(result, AuthResult::NotReachable);
    }

    /// Verifies try_authenticate returns NotReachable for an invalid address.
    #[test]
    fn test_try_authenticate_invalid_address() {
        let result = NetworkScanner::try_authenticate(
            "not-a-valid-ip",
            22,
            "user",
            "pass",
            Duration::from_millis(100),
        );
        assert_eq!(result, AuthResult::NotReachable);
    }

    // ==================== Host key saving tests ====================

    /// When no existing keys, all valid keyscan lines are returned.
    #[test]
    fn test_filter_new_host_keys_no_existing() {
        let keyscan = "\
# 10.0.0.32:22 SSH-2.0-OpenSSH_8.9
10.0.0.32 ssh-rsa AAAA...key1...
10.0.0.32 ssh-ed25519 AAAA...key2...
";
        let existing = "";
        let result = filter_new_host_keys(keyscan, existing);

        assert_eq!(result.len(), 2);
        assert!(result[0].contains("ssh-rsa"));
        assert!(result[1].contains("ssh-ed25519"));
    }

    /// Duplicate host+keytype pairs are filtered out.
    #[test]
    fn test_filter_new_host_keys_dedup() {
        let keyscan = "\
10.0.0.32 ssh-rsa AAAA...newkey...
10.0.0.32 ssh-ed25519 AAAA...newkey2...
";
        let existing = "10.0.0.32 ssh-rsa AAAA...oldkey...\n";
        let result = filter_new_host_keys(keyscan, existing);

        // ssh-rsa already exists, only ssh-ed25519 is new
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("ssh-ed25519"));
    }

    /// When all keys already exist, returns empty vec.
    #[test]
    fn test_filter_new_host_keys_all_existing() {
        let keyscan = "10.0.0.32 ssh-rsa AAAA...key1...\n";
        let existing = "10.0.0.32 ssh-rsa AAAA...key1...\n";
        let result = filter_new_host_keys(keyscan, existing);

        assert!(result.is_empty());
    }

    /// Comments and empty lines in keyscan output are skipped.
    #[test]
    fn test_filter_new_host_keys_skips_comments() {
        let keyscan = "\
# this is a comment

# another comment
10.0.0.32 ssh-ed25519 AAAA...key...
";
        let existing = "";
        let result = filter_new_host_keys(keyscan, existing);

        assert_eq!(result.len(), 1);
        assert!(result[0].contains("ssh-ed25519"));
    }

    /// Malformed lines (fewer than 3 fields) are skipped.
    #[test]
    fn test_filter_new_host_keys_skips_malformed() {
        let keyscan = "\
10.0.0.32 ssh-rsa
10.0.0.32 ssh-ed25519 AAAA...valid...
just-a-hostname
";
        let existing = "";
        let result = filter_new_host_keys(keyscan, existing);

        assert_eq!(result.len(), 1);
        assert!(result[0].contains("ssh-ed25519"));
    }

    /// Non-standard port entries use [host]:port format and dedup correctly.
    #[test]
    fn test_filter_new_host_keys_nonstandard_port() {
        let keyscan = "[10.0.0.32]:2222 ssh-rsa AAAA...key...\n";
        let existing = "[10.0.0.32]:2222 ssh-rsa AAAA...oldkey...\n";
        let result = filter_new_host_keys(keyscan, existing);

        // Same host+keytype, different key data — still a duplicate by host+type
        assert!(result.is_empty());
    }

    /// Comments in existing known_hosts are ignored during dedup.
    #[test]
    fn test_filter_new_host_keys_existing_has_comments() {
        let keyscan = "10.0.0.32 ssh-rsa AAAA...key...\n";
        let existing = "\
# Added by ssh scan
# 10.0.0.32 ssh-rsa AAAA...commented-out...
";
        let result = filter_new_host_keys(keyscan, existing);

        // The commented-out line should NOT count as existing
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("ssh-rsa"));
    }

    /// save_host_key returns an error for an unreachable host.
    #[test]
    fn test_save_host_key_unreachable() {
        // TEST-NET address — ssh-keyscan should timeout/fail
        let result = NetworkScanner::save_host_key("192.0.2.1", 22);
        assert!(result.is_err());
    }

    /// Integration test: runs save_host_key against all hosts currently in the
    /// SSH manager dashboard. Requires LAN access to the 10.0.0.x subnet.
    ///
    /// Run with: `cargo test --lib ssh::scanner::tests::test_save_host_key_all_dashboard_hosts -- --ignored`
    #[test]
    #[ignore]
    fn test_save_host_key_all_dashboard_hosts() {
        let hosts = [
            ("10.0.0.32", 22),
            ("10.0.0.70", 22),
            ("10.0.0.18", 22),
            ("10.0.0.248", 22),
            ("10.0.0.44", 22),  // desk-rock
            ("10.0.0.116", 22), // kafka-3
        ];

        let known_hosts_path = dirs::home_dir()
            .expect("home dir")
            .join(".ssh")
            .join("known_hosts");

        let before = std::fs::read_to_string(&known_hosts_path).unwrap_or_default();
        let before_lines: usize = before.lines().count();

        let mut successes = Vec::new();
        let mut failures = Vec::new();

        for (host, port) in &hosts {
            match NetworkScanner::save_host_key(host, *port) {
                Ok(()) => successes.push(*host),
                Err(e) => failures.push((*host, e)),
            }
        }

        let after = std::fs::read_to_string(&known_hosts_path).unwrap_or_default();
        let after_lines: usize = after.lines().count();

        // Print results for manual inspection
        eprintln!("=== save_host_key results ===");
        eprintln!("Successes ({}):", successes.len());
        for host in &successes {
            eprintln!("  OK: {}", host);
        }
        eprintln!("Failures ({}):", failures.len());
        for (host, err) in &failures {
            eprintln!("  FAIL: {} — {}", host, err);
        }
        eprintln!(
            "known_hosts lines: {} -> {} (added {})",
            before_lines,
            after_lines,
            after_lines.saturating_sub(before_lines)
        );

        // At least one host should succeed (the subnet has known-reachable hosts)
        assert!(
            !successes.is_empty(),
            "Expected at least one host key saved, but all {} failed",
            failures.len()
        );
    }
}
