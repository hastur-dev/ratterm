//! Health Dashboard input handling for the App.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::{debug, info};

use crate::ui::health_dashboard::DashboardMode;

use super::App;

impl App {
    /// Handles key events when the health dashboard is open.
    ///
    /// Returns true if the key was handled by the dashboard.
    pub fn handle_health_dashboard_key(&mut self, key: KeyEvent) -> bool {
        let dashboard = match self.health_dashboard.as_mut() {
            Some(d) => d,
            None => return false,
        };

        match dashboard.mode() {
            DashboardMode::Overview => self.handle_dashboard_overview_key(key),
            DashboardMode::Detail => self.handle_dashboard_detail_key(key),
        }
    }

    /// Handles keys in overview mode.
    fn handle_dashboard_overview_key(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            // Navigation
            (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => {
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_previous();
                }
                true
            }
            (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => {
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_next();
                }
                true
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_first();
                }
                true
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_last();
                }
                true
            }

            // Enter detail mode
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.enter_detail();
                }
                true
            }

            // Refresh
            (KeyModifiers::NONE, KeyCode::Char('r')) => {
                self.refresh_health_dashboard();
                true
            }

            // Toggle auto-refresh
            (KeyModifiers::NONE, KeyCode::Char(' ')) => {
                self.toggle_dashboard_auto_refresh();
                true
            }

            // Close dashboard
            (KeyModifiers::NONE, KeyCode::Esc | KeyCode::Char('q')) => {
                info!(
                    "Dashboard close requested (overview mode) - key: {:?}",
                    key.code
                );
                self.close_health_dashboard();
                info!(
                    "Dashboard closed, health_dashboard is_some: {}",
                    self.health_dashboard.is_some()
                );
                true
            }

            _ => {
                debug!("Unhandled key in dashboard overview: {:?}", key);
                false
            }
        }
    }

    /// Handles keys in detail mode.
    fn handle_dashboard_detail_key(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            // Back to overview
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.exit_detail();
                }
                true
            }

            // Refresh
            (KeyModifiers::NONE, KeyCode::Char('r')) => {
                self.refresh_health_dashboard();
                true
            }

            // Close dashboard
            (KeyModifiers::NONE, KeyCode::Esc | KeyCode::Char('q')) => {
                info!(
                    "Dashboard close requested (detail mode) - key: {:?}",
                    key.code
                );
                self.close_health_dashboard();
                info!(
                    "Dashboard closed, health_dashboard is_some: {}",
                    self.health_dashboard.is_some()
                );
                true
            }

            _ => {
                debug!("Unhandled key in dashboard detail: {:?}", key);
                false
            }
        }
    }
}
