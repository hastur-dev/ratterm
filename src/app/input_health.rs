//! Health Dashboard input handling for the App.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::{debug, info};

use crate::ui::health_dashboard::DashboardMode;

use super::App;

impl App {
    /// Handles key events when the health dashboard is open.
    ///
    /// Called from handle_popup_key when popup kind is HealthDashboard.
    pub(super) fn handle_health_dashboard_key(&mut self, key: KeyEvent) {
        info!("DASHBOARD: handle_health_dashboard_key called, code={:?}", key.code);

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
        info!("DASHBOARD OVERVIEW: code={:?}, mods={:?}", key.code, key.modifiers);

        match (key.modifiers, key.code) {
            // Navigation
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                info!("DASHBOARD: select_previous");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_previous();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                info!("DASHBOARD: select_next");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_next();
                }
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                info!("DASHBOARD: select_first");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_first();
                }
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                info!("DASHBOARD: select_last");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_last();
                }
            }

            // Enter detail mode
            (KeyModifiers::NONE, KeyCode::Enter) => {
                info!("DASHBOARD: enter_detail");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.enter_detail();
                }
            }

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

            // Close dashboard
            (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Char('q')) => {
                info!("DASHBOARD: close");
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
