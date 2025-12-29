//! Mouse input handling for the application.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::ui::layout::FocusedPane;

use super::{App, AppMode};

impl App {
    /// Handles mouse events.
    pub(super) fn handle_mouse(&mut self, event: MouseEvent) {
        if self.mode != AppMode::Normal {
            return;
        }

        let terminal_area = self.cached_terminal_area();

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    self.layout.set_focused(FocusedPane::Terminal);
                    let (col, row) =
                        self.to_terminal_coords(event.column, event.row, terminal_area);
                    self.start_terminal_selection(col, row);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    let (col, row) =
                        self.to_terminal_coords(event.column, event.row, terminal_area);
                    self.update_terminal_selection(col, row);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.finalize_terminal_selection();
            }
            MouseEventKind::ScrollUp => {
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    if let Some(ref mut terminals) = self.terminals {
                        if let Some(terminal) = terminals.active_terminal_mut() {
                            terminal.scroll_view_up(3);
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    if let Some(ref mut terminals) = self.terminals {
                        if let Some(terminal) = terminals.active_terminal_mut() {
                            terminal.scroll_view_down(3);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Returns the cached terminal area.
    fn cached_terminal_area(&self) -> ratatui::layout::Rect {
        self.last_terminal_area.get()
    }

    /// Checks if a point is within an area.
    fn is_point_in_area(&self, x: u16, y: u16, area: ratatui::layout::Rect) -> bool {
        x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
    }

    /// Converts screen coordinates to terminal coordinates.
    fn to_terminal_coords(&self, x: u16, y: u16, area: ratatui::layout::Rect) -> (u16, u16) {
        let local_x = x.saturating_sub(area.x + 1);
        let local_y = y.saturating_sub(area.y + 2);
        (local_x, local_y)
    }

    /// Starts a terminal text selection at the given position.
    fn start_terminal_selection(&mut self, col: u16, row: u16) {
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                terminal.start_selection(col, row);
            }
        }
    }

    /// Updates a terminal text selection to the given position.
    fn update_terminal_selection(&mut self, col: u16, row: u16) {
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                terminal.update_selection(col, row);
            }
        }
    }

    /// Finalizes a terminal text selection.
    fn finalize_terminal_selection(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                terminal.finalize_selection();
            }
        }
    }
}
