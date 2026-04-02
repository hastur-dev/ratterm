//! Git dashboard operations (open/close/refresh/stage/unstage/commit).

use std::path::Path;

use crate::git::api;
use crate::git::gutter::compute_gutter_indicators;
use crate::ui::popup::PopupKind;

use super::App;

impl App {
    /// Opens the git dashboard, loading current repo status.
    pub(crate) fn show_git_dashboard(&mut self) {
        let repo_path = self.current_repo_path();
        let Some(repo_path) = repo_path else {
            self.set_status("No git repository found");
            return;
        };

        let mut dashboard = self
            .git_dashboard
            .take()
            .unwrap_or_else(|| crate::git::dashboard::GitDashboard::new(&repo_path));

        dashboard.repo_path = repo_path.clone();

        // Load status
        match api::git_status(Path::new(&repo_path)) {
            Ok(entries) => dashboard.load_status(entries),
            Err(e) => {
                dashboard.error = Some(format!("Status error: {}", e));
            }
        }

        // Load branch name
        if let Ok(branch) = api::git_current_branch(Path::new(&repo_path)) {
            dashboard.current_branch = branch;
        }

        // Load commit log
        if let Ok(log) = api::git_log(Path::new(&repo_path), 50) {
            dashboard.commit_log = log;
        }

        // Load branches
        if let Ok(branches) = api::git_branch_list(Path::new(&repo_path)) {
            dashboard.branch_list = branches;
        }

        // Load stashes
        if let Ok(stashes) = api::git_stash_list(Path::new(&repo_path)) {
            dashboard.stash_list = stashes;
        }

        self.git_dashboard = Some(dashboard);
        self.popup.set_kind(PopupKind::GitDashboard);
        self.popup.show();
        self.mode = super::AppMode::Popup;
        self.set_status("Git Dashboard | j/k=nav Tab=section s=stage u=unstage c=commit ?=help");
    }

    /// Closes the git dashboard.
    pub(crate) fn hide_git_dashboard(&mut self) {
        self.git_dashboard = None;
        self.hide_popup();
    }

    /// Refreshes the git dashboard data.
    pub(crate) fn refresh_git_dashboard(&mut self) {
        let Some(ref mut dashboard) = self.git_dashboard else {
            return;
        };

        let repo_path = dashboard.repo_path.clone();

        match api::git_status(Path::new(&repo_path)) {
            Ok(entries) => dashboard.load_status(entries),
            Err(e) => dashboard.error = Some(format!("Refresh error: {}", e)),
        }

        if let Ok(branch) = api::git_current_branch(Path::new(&repo_path)) {
            dashboard.current_branch = branch;
        }

        if let Ok(log) = api::git_log(Path::new(&repo_path), 50) {
            dashboard.commit_log = log;
        }

        if let Ok(branches) = api::git_branch_list(Path::new(&repo_path)) {
            dashboard.branch_list = branches;
        }

        if let Ok(stashes) = api::git_stash_list(Path::new(&repo_path)) {
            dashboard.stash_list = stashes;
        }

        self.set_status("Git Dashboard refreshed");
    }

    /// Stages the currently selected file in the dashboard.
    pub(crate) fn git_stage_selected(&mut self) {
        let (repo_path, file_path) = {
            let Some(ref dashboard) = self.git_dashboard else {
                return;
            };
            let Some(file) = dashboard.selected_file_path() else {
                return;
            };
            (dashboard.repo_path.clone(), file.to_string())
        };

        let repo = Path::new(&repo_path);
        let file = repo.join(&file_path);

        match api::git_stage_file(repo, &file) {
            Ok(()) => {
                self.set_status(format!("Staged: {}", file_path));
                self.refresh_git_dashboard();
            }
            Err(e) => {
                self.set_status(format!("Stage failed: {}", e));
            }
        }
    }

    /// Unstages the currently selected file in the dashboard.
    pub(crate) fn git_unstage_selected(&mut self) {
        let (repo_path, file_path) = {
            let Some(ref dashboard) = self.git_dashboard else {
                return;
            };
            let Some(file) = dashboard.selected_file_path() else {
                return;
            };
            (dashboard.repo_path.clone(), file.to_string())
        };

        let repo = Path::new(&repo_path);
        let file = repo.join(&file_path);

        match api::git_unstage_file(repo, &file) {
            Ok(()) => {
                self.set_status(format!("Unstaged: {}", file_path));
                self.refresh_git_dashboard();
            }
            Err(e) => {
                self.set_status(format!("Unstage failed: {}", e));
            }
        }
    }

    /// Starts entering a commit message.
    pub(crate) fn git_start_commit(&mut self) {
        if let Some(ref mut dashboard) = self.git_dashboard {
            if dashboard.staged_files.is_empty() {
                self.set_status("Nothing staged to commit");
                return;
            }
            dashboard.mode = crate::git::dashboard::GitDashboardMode::CommitMessage;
            dashboard.commit_message.clear();
            dashboard.amend = false;
            self.set_status("Enter commit message (Enter=commit, Esc=cancel)");
        }
    }

    /// Executes the commit with the current message.
    pub(crate) fn git_execute_commit(&mut self) {
        let (repo_path, message, amend) = {
            let Some(ref dashboard) = self.git_dashboard else {
                return;
            };
            if dashboard.commit_message.trim().is_empty() {
                self.set_status("Commit message cannot be empty");
                return;
            }
            (
                dashboard.repo_path.clone(),
                dashboard.commit_message.clone(),
                dashboard.amend,
            )
        };

        match api::git_commit(Path::new(&repo_path), &message, amend) {
            Ok(()) => {
                self.set_status(format!("Committed: {}", message));
                if let Some(ref mut dashboard) = self.git_dashboard {
                    dashboard.mode = crate::git::dashboard::GitDashboardMode::List;
                    dashboard.commit_message.clear();
                }
                self.refresh_git_dashboard();
            }
            Err(e) => {
                self.set_status(format!("Commit failed: {}", e));
            }
        }
    }

    /// Cancels the commit message entry.
    pub(crate) fn git_cancel_commit(&mut self) {
        if let Some(ref mut dashboard) = self.git_dashboard {
            dashboard.mode = crate::git::dashboard::GitDashboardMode::List;
            dashboard.commit_message.clear();
        }
        self.set_status("Commit cancelled");
    }

    /// Toggles the amend flag.
    pub(crate) fn git_toggle_amend(&mut self) {
        if let Some(ref mut dashboard) = self.git_dashboard {
            dashboard.amend = !dashboard.amend;
            let msg = if dashboard.amend {
                "Amend ON"
            } else {
                "Amend OFF"
            };
            self.set_status(msg);
        }
    }

    /// Toggles git blame for the current file.
    pub(crate) fn toggle_git_blame(&mut self) {
        self.git_blame_active = !self.git_blame_active;

        if self.git_blame_active {
            self.load_git_blame();
        } else {
            self.git_blame_data.clear();
            self.set_status("Blame view off");
        }
    }

    /// Loads blame data for the current editor file.
    fn load_git_blame(&mut self) {
        let Some(file_path) = self.editor.path().cloned() else {
            self.set_status("No file open for blame");
            self.git_blame_active = false;
            return;
        };

        let Some(repo_path) = self.current_repo_path() else {
            self.set_status("Not in a git repository");
            self.git_blame_active = false;
            return;
        };

        match api::git_blame(Path::new(&repo_path), &file_path) {
            Ok(blame) => {
                self.git_blame_data = blame;
                self.set_status("Blame view on");
            }
            Err(e) => {
                self.set_status(format!("Blame failed: {}", e));
                self.git_blame_active = false;
            }
        }
    }

    /// Updates git gutter indicators for the current file.
    pub(crate) fn update_git_gutter(&mut self) {
        self.git_gutter.clear();

        if !self.config.git_gutter {
            return;
        }

        let Some(file_path) = self.editor.path().cloned() else {
            return;
        };

        let Some(repo_path) = self.current_repo_path() else {
            return;
        };

        if let Ok(diff) = api::git_diff(Path::new(&repo_path), Some(&file_path)) {
            self.git_gutter = compute_gutter_indicators(&diff);
        }
    }

    /// Returns the repo path for the current working directory.
    fn current_repo_path(&self) -> Option<String> {
        let cwd = self.file_browser.path().to_path_buf();
        // Try to discover a git repo from the cwd
        match git2::Repository::discover(&cwd) {
            Ok(repo) => repo.workdir().map(|p| p.display().to_string()),
            Err(_) => None,
        }
    }

    /// Switches the git dashboard view.
    pub(crate) fn git_switch_view(&mut self, view: crate::git::dashboard::GitDashboardView) {
        if let Some(ref mut dashboard) = self.git_dashboard {
            dashboard.view = view;
            dashboard.selected_index = 0;
            dashboard.scroll_offset = 0;
        }
    }

    /// Pops the top stash entry.
    pub(crate) fn git_stash_pop(&mut self) {
        let repo_path = {
            let Some(ref dashboard) = self.git_dashboard else {
                return;
            };
            dashboard.repo_path.clone()
        };

        match api::git_stash_op(Path::new(&repo_path), api::StashOp::Pop) {
            Ok(()) => {
                self.set_status("Stash popped");
                self.refresh_git_dashboard();
            }
            Err(e) => self.set_status(format!("Stash pop failed: {}", e)),
        }
    }

    /// Pushes current changes to stash.
    pub(crate) fn git_stash_push(&mut self) {
        let repo_path = {
            let Some(ref dashboard) = self.git_dashboard else {
                return;
            };
            dashboard.repo_path.clone()
        };

        match api::git_stash_op(Path::new(&repo_path), api::StashOp::Push) {
            Ok(()) => {
                self.set_status("Changes stashed");
                self.refresh_git_dashboard();
            }
            Err(e) => self.set_status(format!("Stash push failed: {}", e)),
        }
    }

    /// Checks out the selected branch.
    pub(crate) fn git_checkout_selected_branch(&mut self) {
        let (repo_path, branch_name) = {
            let Some(ref dashboard) = self.git_dashboard else {
                return;
            };
            let Some(branch) = dashboard.branch_list.get(dashboard.selected_index) else {
                return;
            };
            if branch.is_current {
                self.set_status("Already on this branch");
                return;
            }
            (dashboard.repo_path.clone(), branch.name.clone())
        };

        match api::git_checkout_branch(Path::new(&repo_path), &branch_name) {
            Ok(()) => {
                self.set_status(format!("Switched to branch: {}", branch_name));
                self.refresh_git_dashboard();
            }
            Err(e) => self.set_status(format!("Checkout failed: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::git::api::detect_conflict_markers;

    #[test]
    fn test_conflict_markers_integration() {
        let content = "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\n";
        let markers = detect_conflict_markers(content);
        assert_eq!(markers.len(), 3);
    }
}
