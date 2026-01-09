//! Health Dashboard input handling for the App.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::{debug, info, warn};

use crate::ui::health_dashboard::DashboardMode;

use super::App;

impl App {
    /// Handles key events when the health dashboard is open.
    ///
    /// Returns true if the key was handled by the dashboard.
    pub fn handle_health_dashboard_key(&mut self, key: KeyEvent) -> bool {
        info!(">>> handle_health_dashboard_key ENTRY");

        let dashboard = match self.health_dashboard.as_mut() {
            Some(d) => {
                info!("Dashboard exists, getting mode...");
                d
            }
            None => {
                warn!("!!! handle_health_dashboard_key called but dashboard is None!");
                return false;
            }
        };

        let dashboard_mode = dashboard.mode();
        info!(
            "Dashboard mode={:?}, key.code={:?}, key.modifiers={:?}",
            dashboard_mode, key.code, key.modifiers
        );

        let handled = match dashboard_mode {
            DashboardMode::Overview => {
                info!("Calling handle_dashboard_overview_key");
                self.handle_dashboard_overview_key(key)
            }
            DashboardMode::Detail => {
                info!("Calling handle_dashboard_detail_key");
                self.handle_dashboard_detail_key(key)
            }
        };

        info!("<<< handle_health_dashboard_key EXIT, returning {}", handled);
        handled
    }

    /// Handles keys in overview mode.
    fn handle_dashboard_overview_key(&mut self, key: KeyEvent) -> bool {
        info!(">>> handle_dashboard_overview_key: modifiers={:?}, code={:?}", key.modifiers, key.code);

        match (key.modifiers, key.code) {
            // Navigation
            (KeyModifiers::NONE, KeyCode::Up) => {
                info!("MATCHED: Up arrow");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_previous();
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) => {
                info!("MATCHED: 'k' key");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_previous();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                info!("MATCHED: Down arrow");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_next();
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) => {
                info!("MATCHED: 'j' key");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_next();
                }
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                info!("MATCHED: Home");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_first();
                }
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                info!("MATCHED: End");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.select_last();
                }
            }

            // Enter detail mode
            (KeyModifiers::NONE, KeyCode::Enter) => {
                info!("MATCHED: Enter");
                if let Some(ref mut dashboard) = self.health_dashboard {
                    dashboard.enter_detail();
                }
            }

            // Refresh
            (KeyModifiers::NONE, KeyCode::Char('r')) => {
                info!("MATCHED: 'r' refresh");
                self.refresh_health_dashboard();
            }

            // Toggle auto-refresh
            (KeyModifiers::NONE, KeyCode::Char(' ')) => {
                info!("MATCHED: Space toggle auto-refresh");
                self.toggle_dashboard_auto_refresh();
            }

            // Close dashboard
            (KeyModifiers::NONE, KeyCode::Esc) => {
                info!("MATCHED: Esc - closing dashboard");
                self.close_health_dashboard();
            }
            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                info!("MATCHED: 'q' - closing dashboard");
                self.close_health_dashboard();
            }

            _ => {
                info!("NO MATCH - unhandled key (still consuming): modifiers={:?}, code={:?}", key.modifiers, key.code);
            }
        }
        // IMPORTANT: Always return true to consume ALL keys when dashboard is open
        // This prevents keys from falling through to the terminal
        info!("<<< handle_dashboard_overview_key returning TRUE");
        true
    }

    /// Handles keys in detail mode.
    fn handle_dashboard_detail_key(&mut self, key: KeyEvent) -> bool {
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

            // Close dashboard
            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                info!(
                    "Dashboard close requested (detail mode) - key: {:?}",
                    key.code
                );
                self.close_health_dashboard();
                info!(
                    "Dashboard closed, health_dashboard is_some: {}",
                    self.health_dashboard.is_some()
                );
            }

            _ => {
                debug!("Unhandled key in dashboard detail (consumed): {:?}", key);
            }
        }
        // IMPORTANT: Always return true to consume ALL keys when dashboard is open
        // This prevents keys from falling through to the terminal
        true
    }
}
