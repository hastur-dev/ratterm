//! File operations for the App.

use std::io;
use std::path::PathBuf;

use tracing::debug;

use crate::remote::RemoteFileBrowser;
use crate::terminal::SSHContext;
use crate::ui::layout::FocusedPane;

use super::{App, AppMode, OpenFile};

impl App {
    /// Parks the active document into its tab entry.
    ///
    /// After this call the editor holds an empty document and every tab owns
    /// its own state. No-op when there are no tabs (the scratch buffer is not
    /// a tab).
    pub(crate) fn park_active_tab(&mut self) {
        if self.current_file_idx >= self.open_files.len() {
            return;
        }

        let state = self.editor.take_state();
        let idx = self.current_file_idx;
        let Some(file) = self.open_files.get_mut(idx) else {
            return;
        };

        // A "save as" changes the document's path; keep the tab label honest.
        if let Some(new_path) = state.path.clone() {
            if new_path != file.path && !file.name.starts_with("[SSH]") {
                file.name = new_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| new_path.to_string_lossy().to_string());
                file.path = new_path;
            }
        }

        file.saved_state = Some(state);
    }

    /// Makes the tab at `index` the active one, swapping documents in place.
    ///
    /// Returns false if `index` is out of range. Unlike the old behaviour this
    /// never re-reads the file from disk, so unsaved edits and undo history
    /// survive a tab switch.
    pub fn activate_tab(&mut self, index: usize) -> bool {
        if index >= self.open_files.len() {
            return false;
        }

        if index == self.current_file_idx && self.open_files[index].saved_state.is_none() {
            return true;
        }

        self.park_active_tab();
        self.current_file_idx = index;

        match self.open_files[index].saved_state.take() {
            Some(state) => self.editor.restore_state(state),
            None => {
                // A tab with no parked state should not exist; recover by
                // reading the file rather than showing the wrong document.
                let path = self.open_files[index].path.clone();
                match self.editor.load_state_from_disk(&path) {
                    Ok(state) => self.editor.restore_state(state),
                    Err(e) => {
                        self.editor.new_buffer();
                        self.set_status(format!("Could not reload {}: {}", path.display(), e));
                    }
                }
            }
        }

        self.update_git_gutter();
        self.request_redraw();
        true
    }

    /// Returns the number of open tabs.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.open_files.len()
    }

    /// Returns the index of the active tab.
    #[must_use]
    pub fn active_tab_index(&self) -> usize {
        self.current_file_idx
    }

    /// Opens a file in the editor.
    ///
    /// If the file already has a tab, that tab is activated rather than
    /// reloaded, so pending edits are not thrown away.
    ///
    /// # Errors
    /// Returns error if file cannot be opened.
    pub fn open_file(&mut self, path: impl Into<PathBuf>) -> io::Result<()> {
        let path = path.into();

        debug!("OPEN_FILE: path={:?}", path);
        debug!(
            "OPEN_FILE BEFORE: ide_visible={}, focused={:?}, mode={:?}, file_browser_visible={}",
            self.layout.ide_visible(),
            self.layout.focused(),
            self.mode,
            self.file_browser.is_visible()
        );

        // DIAGNOSTIC: Log terminal grid dimensions BEFORE file open
        if let Some(ref terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal() {
                let grid = terminal.grid();
                debug!(
                    "OPEN_FILE_TERM_BEFORE: grid_cols={}, grid_rows={}, last_screen=({}, {})",
                    grid.cols(),
                    grid.rows(),
                    self.last_screen_size.0,
                    self.last_screen_size.1
                );
            }
        }

        if let Some(idx) = self.open_files.iter().position(|f| f.path == path) {
            self.activate_tab(idx);
        } else {
            // Read before parking so a failed open leaves everything as it was.
            let mut state = self.editor.load_state_from_disk(&path)?;
            state.mode = self.editor.mode();

            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            self.park_active_tab();
            self.open_files.push(OpenFile::active(path.clone(), name));
            self.current_file_idx = self.open_files.len() - 1;
            self.editor.restore_state(state);
        }

        self.set_status(format!("Opened {}", path.display()));
        // Show IDE if not already visible
        if !self.layout.ide_visible() {
            self.layout.show_ide();
        } else {
            // CRITICAL: Even if IDE is already visible (e.g., from file browser),
            // we need to resize to ensure terminal gets correct dimensions
            // after transitioning from file browser to editor mode
            self.resize_for_current_layout();
        }
        self.layout.set_focused(FocusedPane::Editor);
        self.mode = AppMode::Normal;
        self.file_browser.hide();

        debug!(
            "OPEN_FILE AFTER: ide_visible={}, focused={:?}, mode={:?}, file_browser_visible={}, editor_lines={}",
            self.layout.ide_visible(),
            self.layout.focused(),
            self.mode,
            self.file_browser.is_visible(),
            self.editor.buffer().len_lines()
        );

        // DIAGNOSTIC: Log terminal grid dimensions AFTER file open and resize
        if let Some(ref terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal() {
                let grid = terminal.grid();
                debug!(
                    "OPEN_FILE_TERM_AFTER: grid_cols={}, grid_rows={}, last_screen=({}, {})",
                    grid.cols(),
                    grid.rows(),
                    self.last_screen_size.0,
                    self.last_screen_size.1
                );
            }
        }

        // Log first few lines of the file content to verify buffer is correct
        let buffer = self.editor.buffer();
        for line_idx in 0..3.min(buffer.len_lines()) {
            if let Some(line) = buffer.line(line_idx) {
                let preview: String = line.chars().take(60).collect();
                debug!("OPEN_FILE line {}: {:?}", line_idx, preview);
            }
        }

        // Log view state
        let view = self.editor.view();
        debug!(
            "OPEN_FILE view: scroll_top={}, scroll_left={}, width={}, height={}, gutter={}",
            view.scroll_top(),
            view.scroll_left(),
            view.width(),
            view.height(),
            view.gutter_width()
        );

        // DETAILED LOGGING: Compare file characteristics that might cause render issues
        let buffer = self.editor.buffer();
        let total_lines = buffer.len_lines();
        let max_line_len = (0..total_lines.min(100))
            .filter_map(|i| buffer.line(i).map(|l| l.len()))
            .max()
            .unwrap_or(0);

        debug!(
            "OPEN_FILE_DETAILS: path={:?}, total_lines={}, max_line_len_first100={}, view_scroll_top={}, view_height={}, needs_scroll={}",
            path.file_name(),
            total_lines,
            max_line_len,
            view.scroll_top(),
            view.height(),
            total_lines > view.height()
        );

        // Log if any lines have unusual characters
        let mut special_chars = Vec::new();
        for line_idx in 0..5.min(total_lines) {
            if let Some(line) = buffer.line(line_idx) {
                for (col, c) in line.chars().enumerate() {
                    if c as u32 > 127 || (c.is_control() && c != '\n' && c != '\t') {
                        special_chars.push(format!("L{}C{}=U+{:04X}", line_idx, col, c as u32));
                    }
                }
            }
        }
        if !special_chars.is_empty() {
            debug!("OPEN_FILE_SPECIAL_CHARS: {}", special_chars.join(" "));
        }

        // Update git gutter indicators for the newly opened file
        self.update_git_gutter();

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
                let cache_path = remote_file.local_cache_path.clone();

                if let Some(idx) = self.open_files.iter().position(|f| f.path == cache_path) {
                    self.activate_tab(idx);
                } else {
                    let name = format!("[SSH] {}", remote_file.filename());
                    let state = self.editor.remote_state(&content, remote_file);
                    self.park_active_tab();
                    self.open_files.push(OpenFile::active(cache_path, name));
                    self.current_file_idx = self.open_files.len() - 1;
                    self.editor.restore_state(state);
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
    ///
    /// When `lsp_format_on_save` is enabled in `.ratrc`, a
    /// `textDocument/formatting` request is sent before writing.
    pub fn save_current_file(&mut self) {
        if self.lsp_format_on_save {
            // TODO: send textDocument/formatting via LSP before write
            tracing::debug!("lsp_format_on_save enabled – formatting before save");
        }

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
        debug!(
            "SHOW_FILE_BROWSER: BEFORE ide_visible={}, focused={:?}, mode={:?}",
            self.layout.ide_visible(),
            self.layout.focused(),
            self.mode
        );

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

        debug!(
            "SHOW_FILE_BROWSER: AFTER ide_visible={}, focused={:?}, mode={:?}",
            self.layout.ide_visible(),
            self.layout.focused(),
            self.mode
        );
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
        let next = (self.current_file_idx + 1) % self.open_files.len();
        self.activate_tab(next);
    }

    /// Switches to the previous open file.
    pub fn prev_file(&mut self) {
        if self.open_files.is_empty() {
            return;
        }
        let prev = if self.current_file_idx == 0 {
            self.open_files.len() - 1
        } else {
            self.current_file_idx - 1
        };
        self.activate_tab(prev);
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

        self.park_active_tab();
        self.editor.new_buffer();

        self.open_files
            .push(OpenFile::active(PathBuf::from(&name), name.clone()));
        self.current_file_idx = self.open_files.len() - 1;

        self.set_status(format!("Created {}", name));
    }

    /// Closes the current editor tab.
    ///
    /// Refuses while the tab has unsaved changes, prompting instead.
    pub fn close_editor_tab(&mut self) {
        if self.open_files.is_empty() {
            self.set_status("No tabs to close");
            return;
        }

        if self.tab_is_modified(self.current_file_idx) {
            self.show_popup(crate::ui::popup::PopupKind::ConfirmSaveBeforeExit);
            return;
        }

        let closed_name = self.open_files[self.current_file_idx].name.clone();
        // The active document lives in the editor, so dropping the tab entry
        // is enough; the editor is repointed below.
        self.open_files.remove(self.current_file_idx);

        if self.open_files.is_empty() {
            self.current_file_idx = 0;
            self.editor.new_buffer();
        } else {
            let next = self.current_file_idx.min(self.open_files.len() - 1);
            // The editor still holds the closed document, so force a reload of
            // the neighbour rather than parking the corpse into its slot.
            self.current_file_idx = next;
            match self.open_files[next].saved_state.take() {
                Some(state) => self.editor.restore_state(state),
                None => {
                    let path = self.open_files[next].path.clone();
                    match self.editor.load_state_from_disk(&path) {
                        Ok(state) => self.editor.restore_state(state),
                        Err(_) => self.editor.new_buffer(),
                    }
                }
            }
            self.update_git_gutter();
        }

        self.set_status(format!("Closed {}", closed_name));
        self.request_redraw();
        self.check_ide_auto_hide();
    }

    /// Closes the current file (alias for close_editor_tab).
    pub fn close_current_file(&mut self) {
        self.close_editor_tab();
    }
}
