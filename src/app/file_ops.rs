//! File operations for the App.

use std::io;
use std::path::PathBuf;

use crate::remote::RemoteFileBrowser;
use crate::terminal::SSHContext;
use crate::ui::layout::FocusedPane;

use super::{App, AppMode, OpenFile};

impl App {
    /// Opens a file in the editor.
    ///
    /// # Errors
    /// Returns error if file cannot be opened.
    pub fn open_file(&mut self, path: impl Into<PathBuf>) -> io::Result<()> {
        let path = path.into();
        self.editor.open(&path)?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        if !self.open_files.iter().any(|f| f.path == path) {
            self.open_files.push(OpenFile {
                path: path.clone(),
                name,
            });
            self.current_file_idx = self.open_files.len() - 1;
        }

        self.set_status(format!("Opened {}", path.display()));
        self.layout.set_focused(FocusedPane::Editor);
        self.mode = AppMode::Normal;
        self.file_browser.hide();

        self.request_redraw();
        Ok(())
    }

    /// Opens a remote file via SFTP from an SSH session.
    ///
    /// If the path is absolute, no CWD lookup is needed.
    pub fn open_remote_file(&mut self, ctx: &SSHContext, remote_path: &str) {
        self.open_remote_file_with_cwd(ctx, remote_path, None);
    }

    /// Opens a remote file via SFTP, with an optional known CWD.
    ///
    /// If `cwd` is provided, skips the blocking CWD lookup.
    pub fn open_remote_file_with_cwd(
        &mut self,
        ctx: &SSHContext,
        remote_path: &str,
        cwd: Option<&str>,
    ) {
        // Show loading status
        self.set_status(format!("Loading {}...", remote_path));
        self.request_redraw();

        // Use provided CWD or get it (blocking if not provided)
        let resolved_cwd: String;
        let cwd = if let Some(c) = cwd {
            c
        } else if remote_path.starts_with('/') {
            // Absolute path - use a dummy CWD since it won't be used
            "/"
        } else {
            // Need to get CWD for relative path resolution
            match self.remote_manager.get_remote_cwd(ctx) {
                Ok(c) => {
                    resolved_cwd = c;
                    &resolved_cwd
                }
                Err(e) => {
                    self.set_status(format!("Failed to get remote CWD: {}", e));
                    return;
                }
            }
        };

        match self.remote_manager.fetch_file(ctx, remote_path, cwd) {
            Ok((content, remote_file)) => {
                let display = remote_file.display_string();

                self.editor.open_remote(&content, remote_file.clone());

                let name = format!("[SSH] {}", remote_file.filename());
                if !self
                    .open_files
                    .iter()
                    .any(|f| f.path == remote_file.local_cache_path)
                {
                    self.open_files.push(OpenFile {
                        path: remote_file.local_cache_path,
                        name,
                    });
                    self.current_file_idx = self.open_files.len() - 1;
                }

                self.set_status(format!("Opened {}", display));
                self.layout.set_focused(FocusedPane::Editor);
                self.mode = AppMode::Normal;
                self.file_browser.hide();
                // Also hide remote file browser
                self.remote_file_browser = None;
                self.request_redraw();
            }
            Err(e) => {
                self.set_status(format!("Failed to open remote file: {}", e));
            }
        }
    }

    /// Saves the current file (handles both local and remote files).
    pub fn save_current_file(&mut self) {
        if let Some(remote_file) = self.editor.remote_file().cloned() {
            let content = self.editor.buffer().text().to_string();
            match self.remote_manager.save_file(&remote_file, &content) {
                Ok(()) => {
                    self.editor.buffer_mut().mark_saved();
                    self.set_status(format!("Saved {}", remote_file.display_string()));
                }
                Err(e) => {
                    self.set_status(format!("Failed to save remote file: {}", e));
                }
            }
        } else if let Err(e) = self.editor.save() {
            self.set_status(format!("Save failed: {}", e));
        }
    }

    /// Shows the file browser.
    pub fn show_file_browser(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                let cwd = terminal.current_working_dir();
                if cwd.is_dir() && cwd != self.file_browser.path() {
                    let _ = self.file_browser.change_dir(&cwd);
                }
            }
        }

        let _ = self.file_browser.refresh();
        self.file_browser.show();
        self.mode = AppMode::FileBrowser;
        self.layout.set_focused(FocusedPane::Editor);
        self.request_redraw();
    }

    /// Shows the remote file browser for an SSH session.
    pub fn show_remote_file_browser(&mut self, ssh_context: &SSHContext) {
        // Get the remote current working directory
        let remote_cwd = match self.remote_manager.get_remote_cwd(ssh_context) {
            Ok(cwd) => cwd,
            Err(e) => {
                self.set_status(format!("Failed to get remote CWD: {}", e));
                // Fall back to local file browser
                self.show_file_browser();
                return;
            }
        };

        // Create a new remote file browser
        let mut browser = RemoteFileBrowser::new(ssh_context.clone(), remote_cwd.clone());

        // Refresh to load the directory contents
        if let Err(e) = browser.refresh(&mut self.remote_manager) {
            self.set_status(format!("Failed to list remote directory: {}", e));
            // Fall back to local file browser
            self.show_file_browser();
            return;
        }

        // Set the remote file browser as active
        self.remote_file_browser = Some(browser);

        // Hide local file browser
        self.file_browser.hide();

        self.set_status(format!(
            "[SSH] {}@{}: {}",
            ssh_context.username, ssh_context.hostname, remote_cwd
        ));

        self.mode = AppMode::FileBrowser;
        self.layout.set_focused(FocusedPane::Editor);
        self.request_redraw();
    }

    /// Returns true if the remote file browser is active.
    #[must_use]
    pub fn is_remote_browsing(&self) -> bool {
        self.remote_file_browser.is_some()
    }

    /// Hides the remote file browser.
    pub fn hide_remote_file_browser(&mut self) {
        self.remote_file_browser = None;
        self.mode = AppMode::Normal;
        self.request_redraw();
    }

    /// Hides the file browser (both local and remote).
    pub fn hide_file_browser(&mut self) {
        self.file_browser.hide();
        self.remote_file_browser = None;
        self.mode = AppMode::Normal;
        self.request_redraw();
    }

    /// Switches to the next open file.
    pub fn next_file(&mut self) {
        if self.open_files.is_empty() {
            return;
        }
        self.current_file_idx = (self.current_file_idx + 1) % self.open_files.len();
        if let Some(file) = self.open_files.get(self.current_file_idx) {
            let _ = self.editor.open(&file.path);
        }
    }

    /// Switches to the previous open file.
    pub fn prev_file(&mut self) {
        if self.open_files.is_empty() {
            return;
        }
        self.current_file_idx = if self.current_file_idx == 0 {
            self.open_files.len() - 1
        } else {
            self.current_file_idx - 1
        };
        if let Some(file) = self.open_files.get(self.current_file_idx) {
            let _ = self.editor.open(&file.path);
        }
    }

    /// Creates a new untitled editor tab.
    pub fn new_editor_tab(&mut self) {
        let untitled_count = self
            .open_files
            .iter()
            .filter(|f| f.name.starts_with("Untitled"))
            .count();

        let name = if untitled_count == 0 {
            "Untitled".to_string()
        } else {
            format!("Untitled-{}", untitled_count + 1)
        };

        self.editor.new_buffer();

        self.open_files.push(OpenFile {
            path: PathBuf::from(&name),
            name: name.clone(),
        });
        self.current_file_idx = self.open_files.len() - 1;

        self.set_status(format!("Created {}", name));
    }

    /// Closes the current editor tab.
    pub fn close_editor_tab(&mut self) {
        if self.open_files.is_empty() {
            self.set_status("No tabs to close");
            return;
        }

        if self.editor.is_modified() {
            self.show_popup(crate::ui::popup::PopupKind::ConfirmSaveBeforeExit);
            return;
        }

        let closed_name = self.open_files[self.current_file_idx].name.clone();
        self.open_files.remove(self.current_file_idx);

        if self.current_file_idx >= self.open_files.len() && !self.open_files.is_empty() {
            self.current_file_idx = self.open_files.len() - 1;
        }

        if let Some(file) = self.open_files.get(self.current_file_idx) {
            let _ = self.editor.open(&file.path);
        } else {
            self.editor.new_buffer();
            self.current_file_idx = 0;
        }

        self.set_status(format!("Closed {}", closed_name));
        self.check_ide_auto_hide();
    }

    /// Closes the current file (alias for close_editor_tab).
    pub fn close_current_file(&mut self) {
        self.close_editor_tab();
    }
}
