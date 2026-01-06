//! SSH connection and credential operations for the App.

use tracing::{info, warn};

use crate::ssh::SSHCredentials;
use crate::ui::{popup::PopupKind, ssh_manager::SSHManagerMode};

use super::App;

impl App {
    /// Shows the credential entry dialog for the selected host.
    pub fn show_ssh_credential_prompt(&mut self) {
        let host_id = {
            let Some(ref manager) = self.ssh_manager else {
                return;
            };
            match manager.selected_host_id() {
                Some(id) => id,
                None => return,
            }
        };

        let all_host_ids: Vec<u32> = self.ssh_hosts.hosts().map(|h| h.id).collect();
        let ids_with_creds: Vec<u32> = self
            .ssh_hosts
            .hosts()
            .filter(|h| self.ssh_hosts.get_credentials(h.id).is_some())
            .map(|h| h.id)
            .collect();

        let maybe_creds = self.ssh_hosts.get_credentials(host_id).cloned();

        if let Some(creds) = maybe_creds {
            self.set_status(format!(
                "FOUND creds for id={} (user={}) | Connecting...",
                host_id, creds.username
            ));
            self.connect_ssh_with_credentials(host_id, creds);
        } else {
            self.set_status(format!(
                "NO creds for id={} | All IDs: {:?} | With creds: {:?}",
                host_id, all_host_ids, ids_with_creds
            ));
            if let Some(ref mut manager) = self.ssh_manager {
                manager.clear_credentials();
                manager.set_credential_target(host_id);
                manager.set_mode(SSHManagerMode::CredentialEntry);
            }
            self.popup.set_kind(PopupKind::SSHCredentialPrompt);
        }
    }

    /// Submits the SSH credentials and attempts connection.
    pub fn submit_ssh_credentials(&mut self) {
        let Some(ref manager) = self.ssh_manager else {
            self.set_status("SSH Manager not available".to_string());
            return;
        };

        let Some(host_id) = manager.credential_target() else {
            if let Some(ref mut m) = self.ssh_manager {
                m.set_error("No host selected for connection".to_string());
            }
            self.set_status("No host selected".to_string());
            return;
        };

        let username = manager.username().to_string();
        let password = manager.password().to_string();
        let save = manager.save_credentials();

        if username.is_empty() {
            if let Some(ref mut m) = self.ssh_manager {
                m.set_error("Username is required".to_string());
            }
            return;
        }

        let creds = SSHCredentials::new(
            username,
            if password.is_empty() {
                None
            } else {
                Some(password)
            },
        );

        if self.ssh_hosts.get_by_id(host_id).is_none() {
            if let Some(ref mut m) = self.ssh_manager {
                m.set_error("Host no longer exists".to_string());
                m.set_mode(SSHManagerMode::List);
                m.update_from_list(&self.ssh_hosts);
            }
            return;
        }

        if save {
            let mut creds_to_save = creds.clone();
            creds_to_save.save = true;
            if self.ssh_hosts.set_credentials(host_id, creds_to_save) {
                self.save_ssh_hosts();
            }
        }

        self.connect_ssh_with_credentials(host_id, creds);
    }

    /// Connects to an SSH host with the given credentials.
    pub(super) fn connect_ssh_with_credentials(&mut self, host_id: u32, creds: SSHCredentials) {
        let (host_display, hostname, port, jump_chain) = {
            let Some(host) = self.ssh_hosts.get_by_id(host_id) else {
                self.set_status("Host not found".to_string());
                return;
            };

            // Build jump chain if the host has a jump host configured
            let jump_chain = match self.ssh_hosts.build_jump_chain(host_id) {
                Ok(chain) => chain,
                Err(e) => {
                    self.set_status(format!("Jump host error: {}", e));
                    return;
                }
            };

            (
                host.display().to_string(),
                host.hostname.clone(),
                host.port,
                jump_chain,
            )
        };

        self.ssh_hosts.mark_connected(host_id);
        self.save_ssh_hosts();

        self.hide_ssh_manager();

        // Get jump host string and passwords if there's a chain
        let jump_host_str = jump_chain.as_ref().map(|j| j.proxy_jump_string());
        let jump_passwords = jump_chain
            .as_ref()
            .map(|j| j.collect_passwords())
            .unwrap_or_default();

        self.create_ssh_terminal_tab_with_hop_passwords(
            &hostname,
            port,
            &creds.username,
            creds.password.as_deref(),
            jump_host_str.as_deref(),
            jump_passwords,
        );

        if jump_chain.is_some() {
            self.set_status(format!("Connecting to {} via jump host...", host_display));
        } else {
            self.set_status(format!("Connecting to {}...", host_display));
        }
    }

    /// Creates a new terminal tab with an SSH connection.
    #[allow(dead_code)]
    fn create_ssh_terminal_tab(
        &mut self,
        hostname: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
    ) {
        self.create_ssh_terminal_tab_with_jump(hostname, port, username, password, None);
    }

    /// Creates a new terminal tab with an SSH connection, optionally via jump host.
    #[allow(dead_code)]
    fn create_ssh_terminal_tab_with_jump(
        &mut self,
        hostname: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        jump_host: Option<&str>,
    ) {
        self.create_ssh_terminal_tab_with_hop_passwords(
            hostname,
            port,
            username,
            password,
            jump_host,
            Vec::new(),
        );
    }

    /// Creates a new terminal tab with SSH connection, with passwords for jump hosts.
    fn create_ssh_terminal_tab_with_hop_passwords(
        &mut self,
        hostname: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        jump_host: Option<&str>,
        jump_passwords: Vec<String>,
    ) {
        let Some(ref mut terminals) = self.terminals else {
            self.set_status("Terminal not available".to_string());
            return;
        };

        match terminals.add_ssh_tab_with_hop_passwords(
            username,
            hostname,
            port,
            password,
            jump_host,
            jump_passwords.clone(),
        ) {
            Ok(idx) => {
                let context_status = if let Some(terminal) = terminals.active_terminal() {
                    if terminal.is_ssh() {
                        if let Some(ctx) = terminal.ssh_context() {
                            if ctx.jump_host.is_some() {
                                format!("CTX OK: {}@{} (hop)", ctx.username, ctx.hostname)
                            } else {
                                format!("CTX OK: {}@{}", ctx.username, ctx.hostname)
                            }
                        } else {
                            "is_ssh=true but context=None".to_string()
                        }
                    } else {
                        "is_ssh=false".to_string()
                    }
                } else {
                    "No active terminal after add".to_string()
                };

                let ssh_cmd = if let Some(jump) = jump_host {
                    if port == 22 {
                        format!("ssh -J {} {}@{}", jump, username, hostname)
                    } else {
                        format!("ssh -J {} -p {} {}@{}", jump, port, username, hostname)
                    }
                } else if port == 22 {
                    format!("ssh {}@{}", username, hostname)
                } else {
                    format!("ssh -p {} {}@{}", port, username, hostname)
                };

                let hop_info = if !jump_passwords.is_empty() {
                    format!(" ({} hop passwords queued)", jump_passwords.len())
                } else {
                    String::new()
                };

                self.set_status(format!(
                    "SSH started: {} (tab {}) [{}]{}",
                    ssh_cmd,
                    idx + 1,
                    context_status,
                    hop_info
                ));
            }
            Err(e) => {
                self.set_status(format!("Failed to start SSH session: {}", e));
            }
        }
    }

    /// Connects to an SSH host by index (for quick connect hotkeys).
    pub fn ssh_connect_by_index(&mut self, index: usize) {
        if self.ssh_hosts.is_empty() {
            self.load_ssh_hosts();
        }

        let Some(host) = self.ssh_hosts.get_by_index(index) else {
            self.set_status(format!("No SSH host at position {}", index + 1));
            return;
        };

        let host_id = host.id;
        let host_display = host.display().to_string();

        if let Some(creds) = self.ssh_hosts.get_credentials(host_id) {
            self.connect_ssh_with_credentials(host_id, creds.clone());
        } else {
            self.show_ssh_manager();
            if let Some(ref mut manager) = self.ssh_manager {
                for _ in 0..index {
                    manager.select_next();
                }
                manager.set_credential_target(host_id);
                manager.set_mode(SSHManagerMode::CredentialEntry);
            }
            self.popup.set_kind(PopupKind::SSHCredentialPrompt);
            self.set_status(format!("Enter credentials for {}", host_display));
        }
    }

    /// Adds a new SSH host manually.
    pub fn add_ssh_host(&mut self, hostname: String, port: u16, display_name: Option<String>) {
        self.add_ssh_host_with_credentials(hostname, port, display_name, None);
    }

    /// Adds a new SSH host with optional credentials.
    pub fn add_ssh_host_with_credentials(
        &mut self,
        hostname: String,
        port: u16,
        display_name: Option<String>,
        credentials: Option<SSHCredentials>,
    ) {
        if hostname.is_empty() {
            if let Some(ref mut manager) = self.ssh_manager {
                manager.set_error("Hostname is required".to_string());
            }
            return;
        }

        if self.ssh_hosts.contains_hostname(&hostname) {
            if let Some(ref mut manager) = self.ssh_manager {
                manager.set_error("Host already exists".to_string());
            }
            return;
        }

        let id = if let Some(name) = display_name {
            self.ssh_hosts
                .add_host_with_name(hostname.clone(), port, name)
        } else {
            self.ssh_hosts.add_host(hostname.clone(), port)
        };

        if let Some(id) = id {
            if let Some(creds) = credentials {
                self.ssh_hosts.set_credentials(id, creds);
            }

            self.save_ssh_hosts();
            if let Some(ref mut manager) = self.ssh_manager {
                manager.clear_add_host();
                manager.update_from_list(&self.ssh_hosts);
                manager.set_mode(SSHManagerMode::List);
                manager.clear_error();
            }
            self.set_status(format!("Added host: {} (id={})", hostname, id));
            info!("Successfully added SSH host: {} (id={})", hostname, id);
        } else if let Some(ref mut manager) = self.ssh_manager {
            manager.set_error("Maximum hosts reached".to_string());
            warn!("Failed to add host: maximum hosts reached");
        }
    }

    /// Deletes the selected SSH host.
    pub fn delete_selected_ssh_host(&mut self) {
        let Some(ref manager) = self.ssh_manager else {
            return;
        };

        let Some(host_id) = manager.selected_host_id() else {
            return;
        };

        let host_name = self
            .ssh_hosts
            .get_by_id(host_id)
            .map(|h| h.display().to_string())
            .unwrap_or_default();

        if self.ssh_hosts.remove_host(host_id) {
            self.save_ssh_hosts();
            if let Some(ref mut m) = self.ssh_manager {
                m.update_from_list(&self.ssh_hosts);
            }
            self.set_status(format!("Deleted host: {}", host_name));
        }
    }

    /// Saves the edited host name.
    pub fn save_host_name(&mut self) {
        let Some(ref manager) = self.ssh_manager else {
            return;
        };

        let Some(host_id) = manager.edit_name_target() else {
            if let Some(ref mut m) = self.ssh_manager {
                m.cancel_edit_name();
            }
            return;
        };

        let new_name = manager.edit_name_input().to_string();

        self.ssh_hosts.set_display_name(host_id, new_name.clone());
        self.save_ssh_hosts();

        if let Some(ref mut m) = self.ssh_manager {
            m.update_from_list(&self.ssh_hosts);
            m.clear_edit_name();
        }

        self.set_status(format!("Host renamed to: {}", new_name));
    }

    /// Submits the add host form from the SSH manager.
    pub fn submit_add_ssh_host(&mut self) {
        let (hostname, port_str, display_name, username, password, jump_host_id) = {
            let Some(ref manager) = self.ssh_manager else {
                return;
            };
            (
                manager.hostname_input().to_string(),
                manager.port_input().to_string(),
                manager.add_host_display_name().to_string(),
                manager.add_host_username().to_string(),
                manager.add_host_password().to_string(),
                manager.add_host_jump_host_id(),
            )
        };

        if hostname.is_empty() {
            if let Some(ref mut manager) = self.ssh_manager {
                manager.set_error("Hostname is required".to_string());
            }
            return;
        }

        let port: u16 = port_str.parse().unwrap_or(22);

        let display_name_opt = if display_name.is_empty() {
            None
        } else {
            Some(display_name)
        };

        let credentials = if !username.is_empty() {
            let pwd = if password.is_empty() {
                None
            } else {
                Some(password)
            };
            Some(SSHCredentials::new(username, pwd))
        } else {
            None
        };

        self.add_ssh_host_with_jump(hostname, port, display_name_opt, credentials, jump_host_id);
    }

    /// Adds a new SSH host with optional credentials and jump host.
    fn add_ssh_host_with_jump(
        &mut self,
        hostname: String,
        port: u16,
        display_name: Option<String>,
        credentials: Option<SSHCredentials>,
        jump_host_id: Option<u32>,
    ) {
        if hostname.is_empty() {
            if let Some(ref mut manager) = self.ssh_manager {
                manager.set_error("Hostname is required".to_string());
            }
            return;
        }

        if self.ssh_hosts.contains_hostname(&hostname) {
            if let Some(ref mut manager) = self.ssh_manager {
                manager.set_error("Host already exists".to_string());
            }
            return;
        }

        let id = if let Some(name) = display_name {
            self.ssh_hosts
                .add_host_with_name(hostname.clone(), port, name)
        } else {
            self.ssh_hosts.add_host(hostname.clone(), port)
        };

        if let Some(id) = id {
            // Set jump host if specified
            if let Some(jump_id) = jump_host_id {
                if !self.ssh_hosts.set_jump_host(id, Some(jump_id)) {
                    warn!("Failed to set jump host {} for host {}", jump_id, id);
                }
            }

            if let Some(creds) = credentials {
                self.ssh_hosts.set_credentials(id, creds);
            }

            self.save_ssh_hosts();
            if let Some(ref mut manager) = self.ssh_manager {
                manager.clear_add_host();
                manager.update_from_list(&self.ssh_hosts);
                manager.set_mode(SSHManagerMode::List);
                manager.clear_error();
            }

            let jump_info = if jump_host_id.is_some() {
                " (with jump host)"
            } else {
                ""
            };
            self.set_status(format!("Added host: {} (id={}){}", hostname, id, jump_info));
            info!(
                "Successfully added SSH host: {} (id={}){}",
                hostname, id, jump_info
            );
        } else if let Some(ref mut manager) = self.ssh_manager {
            manager.set_error("Maximum hosts reached".to_string());
            warn!("Failed to add host: maximum hosts reached");
        }
    }

    /// Unlocks the SSH storage with a master password.
    pub fn unlock_ssh_storage(&mut self, password: &str) {
        if password.is_empty() {
            self.set_status("Master password is required".to_string());
            return;
        }

        match self.ssh_storage.set_master_password(password) {
            Ok(()) => {
                if let Ok(list) = self.ssh_storage.load() {
                    self.ssh_hosts = list;
                    if let Some(ref mut manager) = self.ssh_manager {
                        manager.update_from_list(&self.ssh_hosts);
                    }
                    self.set_status("SSH storage unlocked".to_string());
                } else {
                    self.set_status("Failed to load hosts after unlock".to_string());
                }
            }
            Err(e) => {
                self.set_status(format!("Failed to unlock: {}", e));
            }
        }
    }
}
