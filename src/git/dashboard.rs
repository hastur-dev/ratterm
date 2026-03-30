//! Git dashboard state management.
//!
//! Holds the state for the Git Status Dashboard popup: file lists,
//! navigation, commit log, branches, and stash entries.

use crate::app::input_traits::ListSelectable;

use super::api::{BranchEntry, CommitEntry, StashEntry, StatusEntry, StatusKind};

/// Which view is currently active in the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitDashboardView {
    /// File status list (staged/unstaged/untracked).
    #[default]
    Status,
    /// Commit log view.
    Log,
    /// Branch list view.
    Branches,
    /// Diff viewer for selected file.
    Diff,
}

/// The current mode of the dashboard (which section is focused).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitDashboardMode {
    /// Navigating the file status list.
    #[default]
    List,
    /// Entering a commit message.
    CommitMessage,
}

/// Section within the status view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusSection {
    /// Staged files.
    #[default]
    Staged,
    /// Unstaged (modified) files.
    Unstaged,
    /// Untracked files.
    Untracked,
}

/// Git dashboard state.
#[derive(Debug, Clone)]
pub struct GitDashboard {
    /// Current view.
    pub view: GitDashboardView,
    /// Current mode.
    pub mode: GitDashboardMode,
    /// Which status section is focused.
    pub section: StatusSection,
    /// Staged files.
    pub staged_files: Vec<StatusEntry>,
    /// Unstaged (modified) files.
    pub unstaged_files: Vec<StatusEntry>,
    /// Untracked files.
    pub untracked_files: Vec<StatusEntry>,
    /// Commit log.
    pub commit_log: Vec<CommitEntry>,
    /// Branch list.
    pub branch_list: Vec<BranchEntry>,
    /// Stash list.
    pub stash_list: Vec<StashEntry>,
    /// Currently selected index within the active section.
    pub selected_index: usize,
    /// Scroll offset for the active list.
    pub scroll_offset: usize,
    /// Current branch name.
    pub current_branch: String,
    /// Commit message being typed.
    pub commit_message: String,
    /// Whether amend is toggled.
    pub amend: bool,
    /// Error message (if any).
    pub error: Option<String>,
    /// Repo path this dashboard is for.
    pub repo_path: String,
}

impl Default for GitDashboard {
    fn default() -> Self {
        Self {
            view: GitDashboardView::Status,
            mode: GitDashboardMode::List,
            section: StatusSection::Staged,
            staged_files: Vec::new(),
            unstaged_files: Vec::new(),
            untracked_files: Vec::new(),
            commit_log: Vec::new(),
            branch_list: Vec::new(),
            stash_list: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            current_branch: String::new(),
            commit_message: String::new(),
            amend: false,
            error: None,
            repo_path: String::new(),
        }
    }
}

impl GitDashboard {
    /// Creates a new git dashboard for the given repo path.
    pub fn new(repo_path: &str) -> Self {
        assert!(!repo_path.is_empty(), "repo path must not be empty");
        Self {
            repo_path: repo_path.to_string(),
            ..Self::default()
        }
    }

    /// Returns the number of items in the current active list.
    pub fn active_list_len(&self) -> usize {
        match self.view {
            GitDashboardView::Status => match self.section {
                StatusSection::Staged => self.staged_files.len(),
                StatusSection::Unstaged => self.unstaged_files.len(),
                StatusSection::Untracked => self.untracked_files.len(),
            },
            GitDashboardView::Log => self.commit_log.len(),
            GitDashboardView::Branches => self.branch_list.len(),
            GitDashboardView::Diff => 0,
        }
    }

    /// Returns the currently selected file path (if in status view).
    pub fn selected_file_path(&self) -> Option<&str> {
        let list = match self.section {
            StatusSection::Staged => &self.staged_files,
            StatusSection::Unstaged => &self.unstaged_files,
            StatusSection::Untracked => &self.untracked_files,
        };
        list.get(self.selected_index).map(|e| e.path.as_str())
    }

    /// Loads status entries, splitting them into staged/unstaged/untracked.
    pub fn load_status(&mut self, entries: Vec<StatusEntry>) {
        self.staged_files.clear();
        self.unstaged_files.clear();
        self.untracked_files.clear();

        for entry in entries {
            if entry.staged {
                self.staged_files.push(entry);
            } else if entry.kind == StatusKind::New {
                self.untracked_files.push(entry);
            } else {
                self.unstaged_files.push(entry);
            }
        }

        // Reset selection if out of bounds
        let len = self.active_list_len();
        if self.selected_index >= len && len > 0 {
            self.selected_index = len - 1;
        } else if len == 0 {
            self.selected_index = 0;
        }
    }

    /// Cycles to the next status section.
    pub fn next_section(&mut self) {
        self.section = match self.section {
            StatusSection::Staged => StatusSection::Unstaged,
            StatusSection::Unstaged => StatusSection::Untracked,
            StatusSection::Untracked => StatusSection::Staged,
        };
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Cycles to the previous status section.
    pub fn prev_section(&mut self) {
        self.section = match self.section {
            StatusSection::Staged => StatusSection::Untracked,
            StatusSection::Unstaged => StatusSection::Staged,
            StatusSection::Untracked => StatusSection::Unstaged,
        };
        self.selected_index = 0;
        self.scroll_offset = 0;
    }
}

impl ListSelectable for GitDashboard {
    fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn select_next(&mut self) {
        let len = self.active_list_len();
        if len > 0 && self.selected_index < len - 1 {
            self.selected_index += 1;
        }
    }

    fn select_first(&mut self) {
        self.selected_index = 0;
    }

    fn select_last(&mut self) {
        let len = self.active_list_len();
        if len > 0 {
            self.selected_index = len - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(path: &str, staged: bool, kind: StatusKind) -> StatusEntry {
        StatusEntry {
            path: path.to_string(),
            staged,
            kind,
        }
    }

    #[test]
    fn test_new_dashboard_has_defaults() {
        let dash = GitDashboard::new("/tmp/repo");
        assert_eq!(dash.repo_path, "/tmp/repo");
        assert_eq!(dash.view, GitDashboardView::Status);
        assert_eq!(dash.mode, GitDashboardMode::List);
        assert_eq!(dash.selected_index, 0);
        assert!(dash.staged_files.is_empty());
    }

    #[test]
    fn test_load_status_splits_correctly() {
        let mut dash = GitDashboard::new("/tmp/repo");
        let entries = vec![
            make_entry("staged.rs", true, StatusKind::Modified),
            make_entry("modified.rs", false, StatusKind::Modified),
            make_entry("new_file.rs", false, StatusKind::New),
        ];
        dash.load_status(entries);

        assert_eq!(dash.staged_files.len(), 1);
        assert_eq!(dash.unstaged_files.len(), 1);
        assert_eq!(dash.untracked_files.len(), 1);
    }

    #[test]
    fn test_section_cycling() {
        let mut dash = GitDashboard::new("/tmp/repo");
        assert_eq!(dash.section, StatusSection::Staged);

        dash.next_section();
        assert_eq!(dash.section, StatusSection::Unstaged);

        dash.next_section();
        assert_eq!(dash.section, StatusSection::Untracked);

        dash.next_section();
        assert_eq!(dash.section, StatusSection::Staged);

        dash.prev_section();
        assert_eq!(dash.section, StatusSection::Untracked);
    }

    #[test]
    fn test_list_selectable_navigation() {
        let mut dash = GitDashboard::new("/tmp/repo");
        let entries = vec![
            make_entry("a.rs", true, StatusKind::Modified),
            make_entry("b.rs", true, StatusKind::New),
            make_entry("c.rs", true, StatusKind::Deleted),
        ];
        dash.load_status(entries);

        assert_eq!(dash.selected_index, 0);

        dash.select_next();
        assert_eq!(dash.selected_index, 1);

        dash.select_next();
        assert_eq!(dash.selected_index, 2);

        // Should not go past end
        dash.select_next();
        assert_eq!(dash.selected_index, 2);

        dash.select_first();
        assert_eq!(dash.selected_index, 0);

        dash.select_last();
        assert_eq!(dash.selected_index, 2);

        dash.select_prev();
        assert_eq!(dash.selected_index, 1);
    }

    #[test]
    fn test_selected_file_path() {
        let mut dash = GitDashboard::new("/tmp/repo");
        let entries = vec![
            make_entry("foo.rs", true, StatusKind::Modified),
            make_entry("bar.rs", true, StatusKind::New),
        ];
        dash.load_status(entries);

        assert_eq!(dash.selected_file_path(), Some("foo.rs"));
        dash.select_next();
        assert_eq!(dash.selected_file_path(), Some("bar.rs"));
    }

    #[test]
    fn test_empty_list_navigation_safe() {
        let mut dash = GitDashboard::new("/tmp/repo");
        assert_eq!(dash.active_list_len(), 0);

        // Should not panic
        dash.select_next();
        dash.select_prev();
        dash.select_first();
        dash.select_last();
        assert_eq!(dash.selected_index, 0);
        assert_eq!(dash.selected_file_path(), None);
    }
}
