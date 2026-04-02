//! Debug panel UI widget.
//!
//! Renders the debug panel below/beside the editor showing call stack,
//! variables, debug console, and breakpoint markers in the gutter.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs, Widget},
};

use crate::debugger::DebugState;
use crate::debugger::console::ConsoleEntryKind;
use crate::debugger::session::{DebugPanelTab, DebugSession};

/// Debug panel widget that renders call stack, variables, and console.
pub struct DebugPanelWidget<'a> {
    session: &'a DebugSession,
}

impl<'a> DebugPanelWidget<'a> {
    /// Creates a new debug panel widget.
    #[must_use]
    pub fn new(session: &'a DebugSession) -> Self {
        Self { session }
    }

    /// Renders the tab bar at the top of the debug panel.
    fn render_tabs(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let titles = vec!["Call Stack", "Variables", "Console"];
        let selected = match self.session.active_tab() {
            DebugPanelTab::CallStack => 0,
            DebugPanelTab::Variables => 1,
            DebugPanelTab::Console => 2,
        };

        let tabs = Tabs::new(titles)
            .select(selected)
            .style(Style::default().fg(Color::DarkGray))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .divider("|");

        tabs.render(area, buf);
    }

    /// Renders the call stack panel.
    fn render_call_stack(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let frames = self.session.stack_frames();
        let selected = self.session.selected_frame();

        if frames.is_empty() {
            let msg = match self.session.state() {
                DebugState::Idle => "No debug session active",
                DebugState::Running => "Program running...",
                DebugState::Paused { .. } => "No stack frames available",
                DebugState::Stopped => "Program stopped",
            };
            let para = Paragraph::new(msg)
                .style(Style::default().fg(Color::DarkGray));
            para.render(area, buf);
            return;
        }

        for (i, frame) in frames.iter().enumerate() {
            if i >= area.height as usize {
                break;
            }

            let y = area.y + i as u16;
            let display = frame.display_line();
            let style = if i == selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Selection indicator
            let prefix = if i == selected { "> " } else { "  " };
            let line = format!("{}{}", prefix, display);

            for (j, c) in line.chars().enumerate() {
                if j >= area.width as usize {
                    break;
                }
                if let Some(cell) = buf.cell_mut((area.x + j as u16, y)) {
                    cell.set_char(c);
                    cell.set_style(style);
                }
            }
        }
    }

    /// Renders the variables panel.
    fn render_variables(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let variables = self.session.variables();
        let selected = self.session.selected_variable();

        if variables.is_empty() {
            let msg = if self.session.is_paused() {
                "No variables to display"
            } else {
                "Pause execution to inspect variables"
            };
            let para = Paragraph::new(msg)
                .style(Style::default().fg(Color::DarkGray));
            para.render(area, buf);
            return;
        }

        for (i, var) in variables.iter().enumerate() {
            if i >= area.height as usize {
                break;
            }

            let y = area.y + i as u16;
            let display = var.display_line();
            let style = if i == selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            for (j, c) in display.chars().enumerate() {
                if j >= area.width as usize {
                    break;
                }
                if let Some(cell) = buf.cell_mut((area.x + j as u16, y)) {
                    cell.set_char(c);
                    cell.set_style(style);
                }
            }
        }
    }

    /// Renders the debug console.
    fn render_console(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let console = self.session.console();

        if area.height < 2 {
            return;
        }

        // Split: entries above, input line at bottom
        let entry_area = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
        let input_area = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(1),
            area.width,
            1,
        );

        // Render entries
        let entries = console.entries();
        let visible_count = entry_area.height as usize;
        let start = entries.len().saturating_sub(visible_count);

        for (i, entry) in entries.iter().skip(start).enumerate() {
            if i >= visible_count {
                break;
            }

            let y = entry_area.y + i as u16;
            let (prefix, style) = match entry.kind {
                ConsoleEntryKind::Input => (
                    "> ",
                    Style::default().fg(Color::Cyan),
                ),
                ConsoleEntryKind::Output => (
                    "  ",
                    Style::default().fg(Color::White),
                ),
                ConsoleEntryKind::Error => (
                    "! ",
                    Style::default().fg(Color::Red),
                ),
                ConsoleEntryKind::Info => (
                    "i ",
                    Style::default().fg(Color::DarkGray),
                ),
            };

            let line = format!("{}{}", prefix, entry.text);
            for (j, c) in line.chars().enumerate() {
                if j >= area.width as usize {
                    break;
                }
                if let Some(cell) = buf.cell_mut((area.x + j as u16, y)) {
                    cell.set_char(c);
                    cell.set_style(style);
                }
            }
        }

        // Render input line
        let input_style = Style::default().fg(Color::Yellow);
        let input_text = format!("> {}", console.input());
        for (j, c) in input_text.chars().enumerate() {
            if j >= input_area.width as usize {
                break;
            }
            if let Some(cell) = buf.cell_mut((input_area.x + j as u16, input_area.y)) {
                cell.set_char(c);
                cell.set_style(input_style);
            }
        }
    }
}

impl Widget for DebugPanelWidget<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        if area.width < 10 || area.height < 4 {
            return;
        }

        // Clear area
        let bg_style = Style::default().bg(Color::Rgb(25, 25, 35)).fg(Color::White);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // Block with border
        let state_str = self.session.state().to_string();
        let title = format!(" Debug: {} ", state_str);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(200, 100, 50)));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // Tab bar (1 row) + content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

        self.render_tabs(chunks[0], buf);

        match self.session.active_tab() {
            DebugPanelTab::CallStack => self.render_call_stack(chunks[1], buf),
            DebugPanelTab::Variables => self.render_variables(chunks[1], buf),
            DebugPanelTab::Console => self.render_console(chunks[1], buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::callstack::StackFrame;
    use crate::debugger::launch::LaunchConfig;
    use crate::debugger::variables::Variable;
    use ratatui::buffer::Buffer as TestBuffer;
    use std::path::PathBuf;

    fn test_session() -> DebugSession {
        DebugSession::new(LaunchConfig::default(), PathBuf::from("/test"))
    }

    #[test]
    fn test_debug_panel_renders_without_crash() {
        let session = test_session();
        let widget = DebugPanelWidget::new(&session);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = TestBuffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_debug_panel_with_stack_frames() {
        let mut session = test_session();
        session.set_stack_frames(vec![
            StackFrame { id: 0, name: "main".into(), source_path: Some("src/main.rs".into()), line: 42, column: 0 },
            StackFrame { id: 1, name: "foo".into(), source_path: None, line: 10, column: 0 },
        ]);

        let widget = DebugPanelWidget::new(&session);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = TestBuffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_debug_panel_with_variables() {
        let mut session = test_session();
        session.next_tab(); // Switch to Variables
        session.set_variables(vec![
            Variable::new("x", "42").with_type("i32"),
            Variable::new("name", "\"hello\"").with_type("&str"),
        ]);

        let widget = DebugPanelWidget::new(&session);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = TestBuffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_debug_panel_console() {
        let mut session = test_session();
        session.next_tab(); // Variables
        session.next_tab(); // Console
        session.console_mut().push_output("Hello");
        session.console_mut().push_error("Oops");

        let widget = DebugPanelWidget::new(&session);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = TestBuffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_debug_panel_small_area() {
        let session = test_session();
        let widget = DebugPanelWidget::new(&session);
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = TestBuffer::empty(area);
        // Should not crash on tiny area
        widget.render(area, &mut buf);
    }
}
