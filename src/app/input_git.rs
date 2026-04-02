//! Input handling for the Git Dashboard.
//!
//! Follows the unified dashboard navigation pattern.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::git::dashboard::{GitDashboardMode, GitDashboardView};

use super::App;
use super::dashboard_nav::{NavResult, apply_dashboard_navigation};

impl App {
    /// Handles key events when the Git Dashboard popup is open.
    pub(super) fn handle_git_dashboard_key(&mut self, key: KeyEvent) {
        // Handle hotkey overlay if visible
        if self.hotkey_overlay.as_ref().is_some_and(|o| o.is_visible()) {
            match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Esc)
                | (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('?')) => {
                    self.hotkey_overlay = None;
                }
                (KeyModifiers::NONE, KeyCode::Down)
                | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                    if let Some(ref mut overlay) = self.hotkey_overlay {
                        overlay.scroll_down();
                    }
                }
                (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                    if let Some(ref mut overlay) = self.hotkey_overlay {
                        overlay.scroll_up();
                    }
                }
                _ => {}
            }
            return;
        }

        let Some(ref dashboard) = self.git_dashboard else {
            return;
        };

        // Route by mode
        match dashboard.mode {
            GitDashboardMode::List => self.handle_git_list_key(key),
            GitDashboardMode::CommitMessage => self.handle_git_commit_message_key(key),
        }
    }

    /// Handles keys in the git dashboard list mode.
    fn handle_git_list_key(&mut self, key: KeyEvent) {
        // Apply unified navigation first
        if let Some(ref mut dashboard) = self.git_dashboard {
            match apply_dashboard_navigation(dashboard, &key) {
                NavResult::Handled => return,
                NavResult::ShowHelp => {
                    self.toggle_hotkey_overlay_git();
                    return;
                }
                NavResult::Close => {
                    self.hide_git_dashboard();
                    return;
                }
                NavResult::Activate => {
                    // Enter opens diff view for selected file
                    self.git_open_diff_for_selected();
                    return;
                }
                NavResult::Unhandled => {}
            }
        }

        // Git-specific keys
        match (key.modifiers, key.code) {
            // Stage file
            (KeyModifiers::NONE, KeyCode::Char('s')) => {
                self.git_stage_selected();
            }
            // Unstage file
            (KeyModifiers::NONE, KeyCode::Char('u')) => {
                self.git_unstage_selected();
            }
            // Commit
            (KeyModifiers::NONE, KeyCode::Char('c')) => {
                self.git_start_commit();
            }
            // Branch view
            (KeyModifiers::NONE, KeyCode::Char('b')) => {
                self.git_switch_view(GitDashboardView::Branches);
            }
            // Log view
            (KeyModifiers::NONE, KeyCode::Char('l')) => {
                self.git_switch_view(GitDashboardView::Log);
            }
            // Diff view
            (KeyModifiers::NONE, KeyCode::Char('d')) => {
                self.git_switch_view(GitDashboardView::Diff);
            }
            // Back to status view
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.git_switch_view(GitDashboardView::Status);
            }
            // Refresh
            (KeyModifiers::NONE, KeyCode::Char('r')) => {
                self.refresh_git_dashboard();
            }
            // Toggle blame (Ctrl+B)
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                self.toggle_git_blame();
            }
            // Stash pop
            (KeyModifiers::NONE, KeyCode::Char('p')) => {
                self.git_stash_pop();
            }
            // Stash push
            (KeyModifiers::SHIFT, KeyCode::Char('P')) => {
                self.git_stash_push();
            }
            // Tab to switch section
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if let Some(ref mut dashboard) = self.git_dashboard {
                    dashboard.next_section();
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Tab | KeyCode::BackTab)
            | (KeyModifiers::NONE, KeyCode::BackTab) => {
                if let Some(ref mut dashboard) = self.git_dashboard {
                    dashboard.prev_section();
                }
            }
            // Checkout branch (Enter in branch view)
            _ => {
                if let Some(ref dashboard) = self.git_dashboard {
                    if dashboard.view == GitDashboardView::Branches {
                        if key.code == KeyCode::Enter {
                            self.git_checkout_selected_branch();
                        }
                    }
                }
            }
        }
    }

    /// Handles keys in commit message entry mode.
    fn handle_git_commit_message_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.git_cancel_commit();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.git_execute_commit();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut dashboard) = self.git_dashboard {
                    dashboard.commit_message.pop();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                self.git_toggle_amend();
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                if let Some(ref mut dashboard) = self.git_dashboard {
                    dashboard.commit_message.push(c);
                }
            }
            _ => {}
        }
    }

    /// Opens diff view for the selected file.
    fn git_open_diff_for_selected(&mut self) {
        // Just switch to diff view — rendering will show the diff for selected file
        self.git_switch_view(GitDashboardView::Diff);
    }

    /// Toggles the git dashboard hotkey overlay.
    fn toggle_hotkey_overlay_git(&mut self) {
        use crate::app::dashboard_hotkeys::git_dashboard_hotkeys;
        use crate::ui::hotkey_overlay::HotkeyOverlay;

        if self.hotkey_overlay.as_ref().is_some_and(|o| o.is_visible()) {
            self.hotkey_overlay = None;
        } else {
            self.hotkey_overlay = Some(HotkeyOverlay::new(git_dashboard_hotkeys()));
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::app::dashboard_nav::{NavResult, apply_dashboard_navigation};
    use crate::git::dashboard::{GitDashboard, StatusSection};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_dashboard_navigation_j_k() {
        let mut dash = GitDashboard::new("/tmp/repo");
        let entries = vec![
            crate::git::api::StatusEntry {
                path: "a.rs".to_string(),
                staged: true,
                kind: crate::git::api::StatusKind::Modified,
            },
            crate::git::api::StatusEntry {
                path: "b.rs".to_string(),
                staged: true,
                kind: crate::git::api::StatusKind::New,
            },
        ];
        dash.load_status(entries);

        let result = apply_dashboard_navigation(&mut dash, &key(KeyCode::Char('j')));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(dash.selected_index, 1);

        let result = apply_dashboard_navigation(&mut dash, &key(KeyCode::Char('k')));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(dash.selected_index, 0);
    }

    #[test]
    fn test_dashboard_escape_closes() {
        let mut dash = GitDashboard::new("/tmp/repo");
        let result = apply_dashboard_navigation(&mut dash, &key(KeyCode::Esc));
        assert_eq!(result, NavResult::Close);
    }

    #[test]
    fn test_dashboard_enter_activates() {
        let mut dash = GitDashboard::new("/tmp/repo");
        let result = apply_dashboard_navigation(&mut dash, &key(KeyCode::Enter));
        assert_eq!(result, NavResult::Activate);
    }

    #[test]
    fn test_dashboard_question_mark_shows_help() {
        let mut dash = GitDashboard::new("/tmp/repo");
        let result = apply_dashboard_navigation(&mut dash, &key(KeyCode::Char('?')));
        assert_eq!(result, NavResult::ShowHelp);
    }

    #[test]
    fn test_section_cycling_via_keys() {
        let mut dash = GitDashboard::new("/tmp/repo");
        assert_eq!(dash.section, StatusSection::Staged);

        dash.next_section();
        assert_eq!(dash.section, StatusSection::Unstaged);

        dash.next_section();
        assert_eq!(dash.section, StatusSection::Untracked);
    }
}
