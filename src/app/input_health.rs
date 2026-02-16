//! Health Dashboard input handling for the App.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::{debug, info};

use crate::app::dashboard_nav::{NavResult, apply_dashboard_navigation};
use crate::ui::health_dashboard::DashboardMode;

use super::App;

impl App {
    /// Handles key events when the health dashboard is open.
    ///
    /// Called from handle_popup_key when popup kind is HealthDashboard.
    pub(super) fn handle_health_dashboard_key(&mut self, key: KeyEvent) {
        info!(
            "DASHBOARD: handle_health_dashboard_key called, code={:?}",
            key.code
        );

        let Some(ref dashboard) = self.health_dashboard else {
            info!("DASHBOARD: dashboard is None, hiding popup");
            self.hide_popup();
            return;
        };

        let mode = dashboard.mode();
        info!("DASHBOARD: mode={:?}", mode);

        match mode {
            DashboardMode::Overview => self.handle_dashboard_overview_key(key),
            DashboardMode::Detail => self.handle_dashboard_detail_key(key),
        }
    }

    /// Handles keys in overview mode.
    fn handle_dashboard_overview_key(&mut self, key: KeyEvent) {
        info!(
            "DASHBOARD OVERVIEW: code={:?}, mods={:?}",
            key.code, key.modifiers
        );

        // Unified dashboard navigation layer
        if let Some(ref mut dashboard) = self.health_dashboard {
            match apply_dashboard_navigation(dashboard, &key) {
                NavResult::Handled => {
                    info!("DASHBOARD: navigation handled by unified layer");
                    return;
                }
                NavResult::ShowHelp => {
                    info!("DASHBOARD: show help requested");
                    self.toggle_hotkey_overlay_health_overview();
                    return;
                }
                NavResult::Close => {
                    info!("DASHBOARD: close via nav layer");
                    self.close_health_dashboard();
                    return;
                }
                NavResult::Activate => {
                    info!("DASHBOARD: enter_detail via nav layer");
                    dashboard.enter_detail();
                    return;
                }
                NavResult::Unhandled => {}
            }
        }

        // Screen-specific keys layered on top
        match (key.modifiers, key.code) {
            // Refresh
            (KeyModifiers::NONE, KeyCode::Char('r')) => {
                info!("DASHBOARD: refresh");
                self.refresh_health_dashboard();
            }

            // Toggle auto-refresh
            (KeyModifiers::NONE, KeyCode::Char(' ')) => {
                info!("DASHBOARD: toggle_auto_refresh");
                self.toggle_dashboard_auto_refresh();
            }

            // Close dashboard (q as alias)
            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                info!("DASHBOARD: close via q");
                self.close_health_dashboard();
            }

            _ => {
                info!("DASHBOARD: unhandled key {:?}", key.code);
            }
        }
    }

    /// Handles keys in detail mode.
    fn handle_dashboard_detail_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Back to overview
            (KeyModifiers::NONE, KeyCode::Backspace | KeyCode::Esc) => {
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.exit_detail();
                }
            }

            // Refresh
            (KeyModifiers::NONE, KeyCode::Char('r')) => {
                self.refresh_health_dashboard();
            }

            // Close dashboard completely
            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                self.close_health_dashboard();
            }

            _ => {
                debug!("Unhandled key in dashboard detail: {:?}", key.code);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::SSHHostList;
    use crate::ui::health_dashboard::{DashboardMode, HealthDashboard};
    use crossterm::event::{KeyEventKind, KeyEventState};

    /// Helper to create a KeyEvent for testing.
    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Helper to create an App with a health dashboard for testing.
    ///
    /// Note: App::new may fail if PTY is unavailable, but that's OK
    /// because terminals will be None and tests can still run.
    fn create_test_app_with_dashboard() -> Option<App> {
        let mut app = App::new(80, 24).ok()?;

        // Manually set up the health dashboard to bypass SSH host loading
        let ssh_hosts = SSHHostList::new();
        let dashboard = HealthDashboard::new(&ssh_hosts);
        app.health_dashboard = Some(dashboard);
        app.mode = super::super::AppMode::HealthDashboard;

        Some(app)
    }

    // ========================================================================
    // Overview Mode Key Handling Tests
    // ========================================================================

    #[test]
    fn test_escape_closes_dashboard_overview_mode() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            // Skip test if App cannot be created (no PTY)
            return;
        };

        assert!(app.is_health_dashboard_open(), "Dashboard should be open");
        assert_eq!(
            app.mode,
            super::super::AppMode::HealthDashboard,
            "Mode should be HealthDashboard"
        );

        // Press Escape in overview mode
        let esc_key = key_event(KeyCode::Esc);
        app.handle_health_dashboard_key(esc_key);

        assert!(
            !app.is_health_dashboard_open(),
            "Dashboard should be closed after Escape"
        );
        assert_eq!(
            app.mode,
            super::super::AppMode::Normal,
            "Mode should return to Normal"
        );
    }

    #[test]
    fn test_q_closes_dashboard_overview_mode() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        assert!(app.is_health_dashboard_open());

        let q_key = key_event(KeyCode::Char('q'));
        app.handle_health_dashboard_key(q_key);

        assert!(
            !app.is_health_dashboard_open(),
            "Dashboard should be closed after 'q'"
        );
        assert_eq!(app.mode, super::super::AppMode::Normal);
    }

    // ========================================================================
    // Detail Mode Key Handling Tests
    // ========================================================================

    #[test]
    fn test_escape_exits_detail_mode_to_overview() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        // Put dashboard into detail mode
        if let Some(ref mut dashboard) = app.health_dashboard {
            dashboard.toggle_mode(); // Switch to Detail
            assert_eq!(dashboard.mode(), DashboardMode::Detail);
        }

        let esc_key = key_event(KeyCode::Esc);
        app.handle_health_dashboard_key(esc_key);

        // Should return to overview, not close the dashboard
        assert!(
            app.is_health_dashboard_open(),
            "Dashboard should still be open (Escape exits detail to overview)"
        );
        if let Some(ref dashboard) = app.health_dashboard {
            assert_eq!(
                dashboard.mode(),
                DashboardMode::Overview,
                "Should be back in Overview mode"
            );
        }
    }

    #[test]
    fn test_backspace_exits_detail_mode_to_overview() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        if let Some(ref mut dashboard) = app.health_dashboard {
            dashboard.toggle_mode();
            assert_eq!(dashboard.mode(), DashboardMode::Detail);
        }

        let backspace_key = key_event(KeyCode::Backspace);
        app.handle_health_dashboard_key(backspace_key);

        assert!(app.is_health_dashboard_open());
        if let Some(ref dashboard) = app.health_dashboard {
            assert_eq!(dashboard.mode(), DashboardMode::Overview);
        }
    }

    #[test]
    fn test_q_closes_dashboard_from_detail_mode() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        if let Some(ref mut dashboard) = app.health_dashboard {
            dashboard.toggle_mode();
            assert_eq!(dashboard.mode(), DashboardMode::Detail);
        }

        let q_key = key_event(KeyCode::Char('q'));
        app.handle_health_dashboard_key(q_key);

        assert!(
            !app.is_health_dashboard_open(),
            "'q' should close dashboard even from detail mode"
        );
        assert_eq!(app.mode, super::super::AppMode::Normal);
    }

    // ========================================================================
    // Navigation Key Tests
    // ========================================================================

    #[test]
    fn test_navigation_keys_in_overview() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        // Test j/k navigation (should not close dashboard)
        let j_key = key_event(KeyCode::Char('j'));
        app.handle_health_dashboard_key(j_key);
        assert!(app.is_health_dashboard_open());

        let k_key = key_event(KeyCode::Char('k'));
        app.handle_health_dashboard_key(k_key);
        assert!(app.is_health_dashboard_open());

        // Test arrow key navigation
        let down_key = key_event(KeyCode::Down);
        app.handle_health_dashboard_key(down_key);
        assert!(app.is_health_dashboard_open());

        let up_key = key_event(KeyCode::Up);
        app.handle_health_dashboard_key(up_key);
        assert!(app.is_health_dashboard_open());
    }

    #[test]
    fn test_home_end_keys_in_overview() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        let home_key = key_event(KeyCode::Home);
        app.handle_health_dashboard_key(home_key);
        assert!(app.is_health_dashboard_open());

        let end_key = key_event(KeyCode::End);
        app.handle_health_dashboard_key(end_key);
        assert!(app.is_health_dashboard_open());
    }

    // ========================================================================
    // Refresh Key Tests
    // ========================================================================

    #[test]
    fn test_r_key_refreshes_dashboard() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        let r_key = key_event(KeyCode::Char('r'));
        app.handle_health_dashboard_key(r_key);

        // Dashboard should still be open after refresh
        assert!(app.is_health_dashboard_open());
    }

    #[test]
    fn test_space_toggles_auto_refresh() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        let initial_auto_refresh = app
            .health_dashboard
            .as_ref()
            .map(|d| d.auto_refresh())
            .unwrap_or(true);

        let space_key = key_event(KeyCode::Char(' '));
        app.handle_health_dashboard_key(space_key);

        let new_auto_refresh = app
            .health_dashboard
            .as_ref()
            .map(|d| d.auto_refresh())
            .unwrap_or(true);

        assert_ne!(
            initial_auto_refresh, new_auto_refresh,
            "Auto-refresh should be toggled"
        );
        assert!(app.is_health_dashboard_open());
    }

    // ========================================================================
    // Enter Key Tests
    // ========================================================================

    #[test]
    fn test_enter_key_enters_detail_mode() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        if let Some(ref dashboard) = app.health_dashboard {
            assert_eq!(dashboard.mode(), DashboardMode::Overview);
        }

        let enter_key = key_event(KeyCode::Enter);
        app.handle_health_dashboard_key(enter_key);

        // With no hosts, enter_detail does nothing (stays in Overview)
        assert!(app.is_health_dashboard_open());
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_handle_key_with_no_dashboard() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        // Remove the dashboard
        app.health_dashboard = None;

        let esc_key = key_event(KeyCode::Esc);

        // Should not panic, should hide popup instead
        app.handle_health_dashboard_key(esc_key);
    }

    #[test]
    fn test_multiple_escapes_only_closes_once() {
        let Some(mut app) = create_test_app_with_dashboard() else {
            return;
        };

        let esc_key = key_event(KeyCode::Esc);

        app.handle_health_dashboard_key(esc_key);
        assert!(!app.is_health_dashboard_open());

        // Second escape should not cause issues
        // (Dashboard is already closed, this tests robustness)
        app.mode = super::super::AppMode::HealthDashboard;
        app.handle_health_dashboard_key(esc_key);
    }

    // ========================================================================
    // Phase 0 Diagnostic Tests — Verify selected_index changes
    // ========================================================================

    /// Helper: creates an App with a dashboard containing 3 mock hosts.
    fn create_test_app_with_hosts() -> Option<App> {
        use crate::ssh::SSHCredentials;

        let mut app = App::new(80, 24).ok()?;

        let mut ssh_hosts = SSHHostList::new();
        let id1 = ssh_hosts.add_host("host1.example.com".into(), 22)?;
        let id2 = ssh_hosts.add_host("host2.example.com".into(), 22)?;
        let id3 = ssh_hosts.add_host("host3.example.com".into(), 22)?;

        // Credentials are required for hosts to appear in the dashboard
        ssh_hosts.set_credentials(id1, SSHCredentials::new("user".into(), Some("pass".into())));
        ssh_hosts.set_credentials(id2, SSHCredentials::new("user".into(), Some("pass".into())));
        ssh_hosts.set_credentials(id3, SSHCredentials::new("user".into(), Some("pass".into())));

        let dashboard = HealthDashboard::new(&ssh_hosts);
        assert_eq!(dashboard.host_count(), 3, "Dashboard should have 3 hosts");
        assert_eq!(dashboard.selected_index(), 0, "Initial selection should be 0");

        app.health_dashboard = Some(dashboard);
        app.mode = super::super::AppMode::HealthDashboard;

        Some(app)
    }

    #[test]
    fn test_diag_down_arrow_changes_selected_index() {
        let Some(mut app) = create_test_app_with_hosts() else {
            return;
        };

        let down = key_event(KeyCode::Down);
        app.handle_health_dashboard_key(down);

        let idx = app.health_dashboard.as_ref().map(|d| d.selected_index());
        assert_eq!(idx, Some(1), "Down arrow should move selection from 0 to 1");
    }

    #[test]
    fn test_diag_up_arrow_changes_selected_index() {
        let Some(mut app) = create_test_app_with_hosts() else {
            return;
        };

        // Move down first, then up
        let down = key_event(KeyCode::Down);
        app.handle_health_dashboard_key(down);
        assert_eq!(
            app.health_dashboard.as_ref().map(|d| d.selected_index()),
            Some(1)
        );

        let up = key_event(KeyCode::Up);
        app.handle_health_dashboard_key(up);
        assert_eq!(
            app.health_dashboard.as_ref().map(|d| d.selected_index()),
            Some(0),
            "Up arrow should move selection back to 0"
        );
    }

    #[test]
    fn test_diag_j_key_changes_selected_index() {
        let Some(mut app) = create_test_app_with_hosts() else {
            return;
        };

        let j = key_event(KeyCode::Char('j'));
        app.handle_health_dashboard_key(j);

        let idx = app.health_dashboard.as_ref().map(|d| d.selected_index());
        assert_eq!(idx, Some(1), "'j' should move selection from 0 to 1");
    }

    #[test]
    fn test_diag_k_key_changes_selected_index() {
        let Some(mut app) = create_test_app_with_hosts() else {
            return;
        };

        // Move down first
        let j = key_event(KeyCode::Char('j'));
        app.handle_health_dashboard_key(j);

        let k = key_event(KeyCode::Char('k'));
        app.handle_health_dashboard_key(k);

        let idx = app.health_dashboard.as_ref().map(|d| d.selected_index());
        assert_eq!(idx, Some(0), "'k' should move selection back to 0");
    }

    #[test]
    fn test_diag_home_end_change_selected_index() {
        let Some(mut app) = create_test_app_with_hosts() else {
            return;
        };

        // End should go to last
        let end = key_event(KeyCode::End);
        app.handle_health_dashboard_key(end);
        assert_eq!(
            app.health_dashboard.as_ref().map(|d| d.selected_index()),
            Some(2),
            "End should move to last host (index 2)"
        );

        // Home should go to first
        let home = key_event(KeyCode::Home);
        app.handle_health_dashboard_key(home);
        assert_eq!(
            app.health_dashboard.as_ref().map(|d| d.selected_index()),
            Some(0),
            "Home should move to first host (index 0)"
        );
    }

    #[test]
    fn test_diag_sequential_jjk_navigation() {
        let Some(mut app) = create_test_app_with_hosts() else {
            return;
        };

        // j, j, k => should end at index 1
        app.handle_health_dashboard_key(key_event(KeyCode::Char('j')));
        assert_eq!(app.health_dashboard.as_ref().map(|d| d.selected_index()), Some(1));

        app.handle_health_dashboard_key(key_event(KeyCode::Char('j')));
        assert_eq!(app.health_dashboard.as_ref().map(|d| d.selected_index()), Some(2));

        app.handle_health_dashboard_key(key_event(KeyCode::Char('k')));
        assert_eq!(
            app.health_dashboard.as_ref().map(|d| d.selected_index()),
            Some(1),
            "j,j,k should end at index 1"
        );
    }
}
