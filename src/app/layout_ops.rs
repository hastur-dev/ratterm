//! Layout and IDE visibility operations for the App.

use tracing::debug;

use super::App;

impl App {
    /// Shows the IDE pane (editor).
    pub fn show_ide(&mut self) {
        debug!(
            "SHOW_IDE: BEFORE ide_visible={}, split={}%",
            self.layout.ide_visible(),
            self.layout.split_percent()
        );

        self.layout.show_ide();
        self.resize_for_current_layout();
        self.request_redraw();
        self.set_status("IDE opened");

        debug!(
            "SHOW_IDE: AFTER ide_visible={}, split={}%",
            self.layout.ide_visible(),
            self.layout.split_percent()
        );
    }

    /// Hides the IDE pane.
    pub fn hide_ide(&mut self) {
        self.layout.hide_ide();
        self.resize_for_current_layout();
        self.request_redraw();
        self.set_status("IDE closed");
    }

    /// Toggles the IDE pane visibility.
    pub fn toggle_ide(&mut self) {
        if self.layout.ide_visible() {
            self.hide_ide();
        } else {
            self.show_ide();
        }
    }

    /// Returns true if the IDE pane is visible.
    #[must_use]
    pub fn ide_visible(&self) -> bool {
        self.layout.ide_visible()
    }

    /// Moves split left.
    pub fn move_split_left(&mut self) {
        self.layout.move_split_left();
        self.resize_for_current_layout();
        self.request_redraw();
    }

    /// Moves split right.
    pub fn move_split_right(&mut self) {
        self.layout.move_split_right();
        self.resize_for_current_layout();
        self.request_redraw();
    }

    /// Checks if IDE should auto-hide.
    pub fn check_ide_auto_hide(&mut self) {
        if self.config.ide_always {
            return;
        }
        if self.open_files.is_empty() && self.layout.ide_visible() {
            self.hide_ide();
        }
    }
}
