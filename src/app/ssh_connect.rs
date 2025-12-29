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
        let (host_display, hostname, port) = {
            let Some(host) = self.ssh_hosts.get_by_id(host_id) else {
                self.set_status("Host not found".to_string());
                return;
            };
            (host.display().to_string(), host.hostname.clone(), host.port)
        };

        self.ssh_hosts.mark_connected(host_id);
        self.save_ssh_hosts();

        self.hide_ssh_manager();

        self.create_ssh_terminal_tab(&hostname, port, &creds.username, creds.password.as_deref());
        self.set_status(format!("Connecting to {}...", host_display));
    }

    /// Creates a new terminal tab with an SSH connection.
    fn create_ssh_terminal_tab(
        &mut self,
        hostname: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
    ) {
        let Some(ref mut terminals) = self.terminals else {
            self.set_status("Terminal not available".to_string());
            return;
        };

        match terminals.add_ssh_tab_with_password(username, hostname, port, password) {
            Ok(idx) => {
                let context_status = if let Some(terminal) = terminals.active_terminal() {
                    if terminal.is_ssh() {
                        if let Some(ctx) = terminal.ssh_context() {
                            format!("CTX OK: {}@{}", ctx.username, ctx.hostname)
                        } else {
                            "is_ssh=true but context=None".to_string()
                        }
                    } else {
                        "is_ssh=false".to_string()
                    }
                } else {
                    "No active terminal after add".to_string()
                };

                let ssh_cmd = if port == 22 {
                    format!("ssh {}@{}", username, hostname)
                } else {
                    format!("ssh -p {} {}@{}", port, username, hostname)
                };

                self.set_status(format!(
                    "SSH started: {} (tab {}) [{}]",
                    ssh_cmd,
                    idx + 1,
                    context_status
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
        let (hostname, port_str, display_name, username, password) = {
            let Some(ref manager) = self.ssh_manager else {
                return;
            };
            (
                manager.hostname_input().to_string(),
                manager.port_input().to_string(),
                manager.add_host_display_name().to_string(),
                manager.add_host_username().to_string(),
                manager.add_host_password().to_string(),
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

        self.add_ssh_host_with_credentials(hostname, port, display_name_opt, credentials);
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
