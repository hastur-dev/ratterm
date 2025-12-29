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
    /// Host successfully authenticated.
    AuthSuccess(String, u16),
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
                    if let Ok(is_open) = handle.join() {
                        if is_open {
                            let ip_str = ip.to_string();
                            found_hosts.push(ip_str.clone());
                            let _ = tx.send(ScanResult::HostFound(ip_str, 22));
                        }
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

        // Try platform-specific detection
        #[cfg(windows)]
        Self::detect_windows_interfaces(&mut interfaces);

        #[cfg(unix)]
        Self::detect_unix_interfaces(&mut interfaces);

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
    #[cfg(windows)]
    fn detect_windows_interfaces(interfaces: &mut Vec<NetworkInterface>) {
        use std::process::Command;

        let output = match Command::new("ipconfig").output() {
            Ok(o) => o,
            Err(_) => return,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut current_adapter = String::new();

        for line in stdout.lines() {
            // Detect adapter names
            if line.ends_with(':') && !line.starts_with(' ') {
                current_adapter = line.trim_end_matches(':').to_string();
            }

            // Look for IPv4 addresses
            if line.contains("IPv4") || line.contains("IP Address") {
                if let Some(ip_str) = line.split(':').nth(1) {
                    let ip_str = ip_str.trim();
                    if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                        // Skip loopback and link-local
                        if !ip.is_loopback() && !ip.is_link_local() {
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
        }
    }

    /// Unix-specific interface detection.
    #[cfg(unix)]
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

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Simple regex-free parsing for inet lines
        for line in stdout.lines() {
            let line = line.trim();

            // Look for "inet X.X.X.X" patterns
            if line.starts_with("inet ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    // Handle both "inet 192.168.1.5/24" and "inet 192.168.1.5 netmask"
                    let ip_part = parts[1];
                    let ip_str = ip_part.split('/').next().unwrap_or(ip_part);

                    if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                        if !ip.is_loopback() && !ip.is_link_local() {
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
                        if auth_result {
                            let ip_str = ip.to_string();
                            authenticated_hosts.push(ip_str.clone());
                            auth_success += 1;
                            let _ = tx.send(ScanResult::AuthSuccess(ip_str, 22));
                        } else {
                            auth_fail += 1;
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
    /// Returns true if authentication succeeds, false otherwise.
    fn try_authenticate(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        timeout: Duration,
    ) -> bool {
        // Connect with timeout
        let addr: SocketAddr = match format!("{}:{}", host, port).parse() {
            Ok(a) => a,
            Err(_) => return false,
        };

        let stream = match TcpStream::connect_timeout(&addr, timeout) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Set read/write timeout
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));

        // Create SSH session
        let mut session = match Session::new() {
            Ok(s) => s,
            Err(_) => return false,
        };

        session.set_tcp_stream(stream);

        // Perform SSH handshake
        if session.handshake().is_err() {
            return false;
        }

        // Attempt password authentication
        session.userauth_password(username, password).is_ok()
    }
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
}
