//! SSH network scanning operations for the App.

use tracing::{debug, info, warn};

use crate::ssh::{NetworkScanner, SSHCredentials, ScanResult};
use crate::ui::ssh_manager::SSHManagerMode;

use super::App;

impl App {
    /// Starts a network scan for SSH hosts.
    pub fn start_ssh_scan(&mut self) {
        info!("Starting SSH network scan (auto-detect subnet)");

        let mut scanner = self.ssh_scanner.take().unwrap_or_default();

        match scanner.start_auto_scan() {
            Ok(()) => {
                let subnet = scanner
                    .current_subnet()
                    .map(String::from)
                    .unwrap_or_else(|| "unknown".to_string());

                info!("SSH scan started on subnet: {}", subnet);

                if let Some(ref mut manager) = self.ssh_manager {
                    manager.set_mode(SSHManagerMode::Scanning);
                    manager.set_scan_progress(0, 254);
                    manager.set_scanning_subnet(subnet.clone());
                    manager.clear_error();
                }
                self.set_status(format!("Scanning {} for SSH hosts...", subnet));
                self.ssh_scanner = Some(scanner);
            }
            Err(e) => {
                warn!("SSH scan failed to start: {}", e);
                if let Some(ref mut manager) = self.ssh_manager {
                    manager.set_error(format!("Scan failed: {}", e));
                }
                self.set_status(format!("Network scan failed: {}", e));
            }
        }
    }

    /// Starts a network scan with a specific subnet.
    pub fn start_ssh_scan_subnet(&mut self, subnet: &str) {
        let mut scanner = self.ssh_scanner.take().unwrap_or_default();

        match scanner.start_scan(subnet) {
            Ok(()) => {
                if let Some(ref mut manager) = self.ssh_manager {
                    manager.set_mode(SSHManagerMode::Scanning);
                    manager.set_scan_progress(0, 254);
                    manager.set_scanning_subnet(subnet.to_string());
                    manager.clear_error();
                }
                self.ssh_scanner = Some(scanner);
                self.set_status(format!("Scanning {} for SSH hosts...", subnet));
            }
            Err(e) => {
                if let Some(ref mut manager) = self.ssh_manager {
                    manager.set_error(format!("Scan failed: {}", e));
                }
                self.set_status(format!("Network scan failed: {}", e));
            }
        }
    }

    /// Polls the network scanner for results.
    pub fn poll_ssh_scanner(&mut self) {
        let results: Vec<ScanResult> = {
            let Some(ref mut scanner) = self.ssh_scanner else {
                return;
            };
            let mut collected = Vec::new();
            while let Some(result) = scanner.poll() {
                collected.push(result);
            }
            collected
        };

        let mut should_clear_scanner = false;
        let mut status_message: Option<String> = None;

        for result in results {
            match result {
                ScanResult::Progress(scanned, total) => {
                    if let Some(ref mut manager) = self.ssh_manager {
                        manager.set_scan_progress(scanned, total);
                    }
                }
                ScanResult::HostFound(ip, _port) => {
                    if !self.ssh_hosts.contains_hostname(&ip) {
                        if let Some(id) = self.ssh_hosts.add_host(ip.clone(), 22) {
                            debug!("Found SSH host: {} (id={})", ip, id);
                            if let Some(ref mut manager) = self.ssh_manager {
                                manager.update_from_list(&self.ssh_hosts);
                            }
                            self.set_status(format!("Found SSH host: {}", ip));
                        }
                    }
                }
                ScanResult::Complete(hosts) => {
                    if let Some(ref mut manager) = self.ssh_manager {
                        manager.update_from_list(&self.ssh_hosts);
                        manager.set_mode(SSHManagerMode::List);
                        manager.clear_scan_progress();
                    }
                    self.save_ssh_hosts();
                    status_message = Some(format!("Scan complete. Found {} hosts.", hosts.len()));
                    should_clear_scanner = true;
                }
                ScanResult::Error(e) => {
                    if let Some(ref mut manager) = self.ssh_manager {
                        manager.set_error(e.clone());
                        manager.set_mode(SSHManagerMode::List);
                        manager.clear_scan_progress();
                    }
                    status_message = Some(format!("Scan error: {}", e));
                    should_clear_scanner = true;
                }
                ScanResult::Cancelled => {
                    if let Some(ref mut manager) = self.ssh_manager {
                        manager.set_mode(SSHManagerMode::List);
                        manager.clear_scan_progress();
                    }
                    status_message = Some("Scan cancelled".to_string());
                    should_clear_scanner = true;
                }
                ScanResult::AuthProgress(scanned, total, success, fail) => {
                    if let Some(ref mut manager) = self.ssh_manager {
                        manager.set_scan_progress(scanned, total);
                        manager.update_auth_counts(success, fail);
                    }
                }
                ScanResult::AuthSuccess(ip, _port) => {
                    self.handle_auth_success_result(ip);
                }
                ScanResult::AuthComplete(hosts) => {
                    self.handle_auth_complete_result(hosts);
                    should_clear_scanner = true;
                }
            }
        }

        if should_clear_scanner {
            self.ssh_scanner = None;
        }
        if let Some(msg) = status_message {
            self.set_status(msg);
        }
    }

    /// Handles an AuthSuccess scan result.
    fn handle_auth_success_result(&mut self, ip: String) {
        info!("AuthSuccess received for ip: {}", ip);
        if !self.ssh_hosts.contains_hostname(&ip) {
            let (username, password) = if let Some(ref manager) = self.ssh_manager {
                (
                    manager.scan_username().to_string(),
                    manager.scan_password().to_string(),
                )
            } else {
                info!("WARNING: ssh_manager is None in AuthSuccess, skipping host");
                return;
            };

            if let Some(id) = self.ssh_hosts.add_host(ip.clone(), 22) {
                info!("Added SSH host: {} with id={}", ip, id);
                let creds = SSHCredentials::new(username, Some(password));
                self.ssh_hosts.set_credentials(id, creds);
                self.set_status(format!("Authenticated: {}", ip));
            } else {
                info!("WARNING: add_host returned None for ip: {}", ip);
            }
        } else {
            info!("Host {} already exists in ssh_hosts, skipping", ip);
        }
    }

    /// Handles an AuthComplete scan result.
    fn handle_auth_complete_result(&mut self, hosts: Vec<String>) {
        info!(
            "AuthComplete received: {} hosts in scan result, {} hosts in ssh_hosts",
            hosts.len(),
            self.ssh_hosts.len()
        );

        for host in self.ssh_hosts.hosts() {
            info!(
                "  - Host in ssh_hosts: id={}, hostname={}",
                host.id, host.hostname
            );
        }

        let manager_count = if let Some(ref mut manager) = self.ssh_manager {
            manager.update_from_list(&self.ssh_hosts);
            manager.set_mode(SSHManagerMode::List);
            manager.clear_scan_progress();
            let count = manager.host_count();
            info!("Manager updated: {} hosts in manager", count);
            count
        } else {
            info!("WARNING: ssh_manager is None at AuthComplete!");
            0
        };

        self.save_ssh_hosts();

        self.set_status(format!(
            "Scan complete: {} authenticated, {} in list, {} in storage",
            hosts.len(),
            manager_count,
            self.ssh_hosts.len()
        ));
    }

    /// Cancels the ongoing SSH scan.
    pub fn cancel_ssh_scan(&mut self) {
        if let Some(ref mut scanner) = self.ssh_scanner {
            scanner.cancel();
        }
        self.ssh_scanner = None;

        if let Some(ref mut manager) = self.ssh_manager {
            manager.set_mode(SSHManagerMode::List);
            manager.clear_scan_progress();
        }
        self.set_status("Scan cancelled".to_string());
    }

    /// Starts an authenticated SSH scan with the entered credentials.
    pub fn start_authenticated_ssh_scan(&mut self) {
        let Some(ref manager) = self.ssh_manager else {
            return;
        };

        let username = manager.scan_username().to_string();
        let password = manager.scan_password().to_string();
        let subnet = manager.scan_subnet().to_string();

        if username.is_empty() {
            if let Some(ref mut m) = self.ssh_manager {
                m.set_error("Username is required".to_string());
            }
            return;
        }

        let subnet = if subnet.is_empty() {
            match NetworkScanner::detect_primary_subnet_static() {
                Ok(s) => s,
                Err(e) => {
                    if let Some(ref mut m) = self.ssh_manager {
                        m.set_error(format!("Failed to detect network: {}", e));
                    }
                    return;
                }
            }
        } else {
            subnet
        };

        let mut scanner = NetworkScanner::new();
        match scanner.start_authenticated_scan(&subnet, username, password) {
            Ok(()) => {
                if let Some(ref mut m) = self.ssh_manager {
                    m.start_authenticated_scanning(subnet.clone());
                    m.set_scanning_subnet(subnet);
                }
                self.ssh_scanner = Some(scanner);
                self.set_status("Starting authenticated scan...".to_string());
            }
            Err(e) => {
                if let Some(ref mut m) = self.ssh_manager {
                    m.set_error(format!("Failed to start scan: {}", e));
                    m.set_mode(SSHManagerMode::List);
                }
                self.set_status(format!("Scan failed: {}", e));
            }
        }
    }
}
