//! Docker log viewer input handling at the App level.

use crossterm::event::KeyEvent;

use crate::docker_logs::ui::input::{LogAction, handle_log_input};
use crate::docker_logs::ui::state::LogViewMode;

use super::App;

impl App {
    /// Handles key events when the Docker Manager is in LogView mode.
    pub fn handle_docker_logs_key(&mut self, key: KeyEvent) {
        let mode = {
            let Some(ref manager) = self.docker_manager else {
                return;
            };
            let Some(ref state) = manager.docker_logs_state else {
                return;
            };
            state.mode()
        };

        let action = handle_log_input(mode, &key);

        match action {
            LogAction::None => {}
            LogAction::NavigateUp => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.select_prev();
                    }
                }
            }
            LogAction::NavigateDown => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.select_next();
                    }
                }
            }
            LogAction::NavigateFirst => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.select_first();
                    }
                }
            }
            LogAction::NavigateLast => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.select_last();
                    }
                }
            }
            LogAction::PageUp => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.log_buffer_mut().scroll_up(20);
                        if state.mode() == LogViewMode::Streaming {
                            state.pause();
                        }
                    }
                }
            }
            LogAction::PageDown => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.log_buffer_mut().scroll_down(20);
                        if state.log_buffer().is_at_bottom()
                            && state.mode() == LogViewMode::Paused
                        {
                            state.resume();
                        }
                    }
                }
            }
            LogAction::Activate => {
                self.docker_logs_activate();
            }
            LogAction::Close => {
                self.docker_logs_close();
            }
            LogAction::TogglePause => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.toggle_pause();
                    }
                }
            }
            LogAction::StartSearch => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.enter_search();
                    }
                }
            }
            LogAction::ApplySearch => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.exit_search();
                    }
                }
            }
            LogAction::CancelSearch => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.cancel_search();
                    }
                }
            }
            LogAction::InsertChar(c) => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.search_insert_char(c);
                    }
                }
            }
            LogAction::SearchBackspace => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.search_backspace();
                    }
                }
            }
            LogAction::ClearLogs => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.log_buffer_mut().clear();
                        self.set_status("Logs cleared");
                    }
                }
            }
            LogAction::ToggleTimestamps => {
                // Toggle is handled in config — update would go here
                self.set_status("Timestamps toggled");
            }
            LogAction::ShowSavedSearches => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        state.enter_saved_searches();
                    }
                }
            }
            LogAction::SaveSearch => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        let input = state.search_input().to_string();
                        if !input.is_empty() {
                            state.save_current_search(input.clone());
                            let _ = state.search_manager_mut().save();
                            self.set_status(format!("Search saved: {}", input));
                        }
                    }
                }
            }
            LogAction::DeleteSavedSearch => {
                if let Some(ref mut manager) = self.docker_manager {
                    if let Some(ref mut state) = manager.docker_logs_state {
                        let idx = state.saved_search_idx();
                        if state.search_manager_mut().remove(idx) {
                            let _ = state.search_manager_mut().save();
                            self.set_status("Saved search deleted");
                        }
                    }
                }
            }
            LogAction::ApplySavedSearch => {
                self.docker_logs_apply_saved_search();
            }
            LogAction::ShowHelp => {
                self.toggle_hotkey_overlay_docker_logs();
            }
        }
    }

    /// Activates the selected item in log view.
    fn docker_logs_activate(&mut self) {
        let container_info = {
            let Some(ref manager) = self.docker_manager else {
                return;
            };
            let Some(ref state) = manager.docker_logs_state else {
                return;
            };

            match state.mode() {
                LogViewMode::ContainerList => {
                    state.selected_container().map(|c| {
                        (c.id.clone(), c.name.clone())
                    })
                }
                _ => None,
            }
        };

        if let Some((id, name)) = container_info {
            self.docker_start_log_stream(&id, &name);
        }
    }

    /// Closes the current log view or goes back.
    fn docker_logs_close(&mut self) {
        let should_go_to_list = {
            let Some(ref manager) = self.docker_manager else {
                return;
            };
            let Some(ref state) = manager.docker_logs_state else {
                return;
            };
            matches!(
                state.mode(),
                LogViewMode::Streaming | LogViewMode::Paused | LogViewMode::Searching
            )
        };

        if should_go_to_list {
            self.docker_stop_log_stream();
            if let Some(ref mut manager) = self.docker_manager {
                if let Some(ref mut state) = manager.docker_logs_state {
                    state.back_to_list();
                }
            }
        } else {
            // From container list or saved searches, go back to Docker Manager list
            self.docker_close_log_view();
        }
    }

    /// Applies a saved search to the current filter.
    fn docker_logs_apply_saved_search(&mut self) {
        let pattern = {
            let Some(ref manager) = self.docker_manager else {
                return;
            };
            let Some(ref state) = manager.docker_logs_state else {
                return;
            };
            let idx = state.saved_search_idx();
            state.search_manager().get(idx).map(|s| s.pattern.clone())
        };

        if let Some(pattern) = pattern {
            if let Some(ref mut manager) = self.docker_manager {
                if let Some(ref mut state) = manager.docker_logs_state {
                    state.log_buffer_mut().set_filter(pattern);
                    state.exit_saved_searches();
                }
            }
        }
    }

    /// Toggles the hotkey overlay for Docker logs.
    fn toggle_hotkey_overlay_docker_logs(&mut self) {
        use crate::app::dashboard_hotkeys::docker_logs_hotkeys;
        use crate::ui::hotkey_overlay::HotkeyOverlay;

        if self.hotkey_overlay.as_ref().is_some_and(|o| o.is_visible()) {
            self.hotkey_overlay = None;
        } else {
            self.hotkey_overlay = Some(HotkeyOverlay::new(docker_logs_hotkeys()));
        }
    }
}

// ListSelectable import for the select_* methods
use crate::app::input_traits::ListSelectable;
