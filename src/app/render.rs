//! Rendering methods for the App.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Clear;

use crate::ui::{
    editor_tabs::EditorTabBar,
    editor_widget::EditorWidget,
    file_picker::{FilePickerWidget, RemoteFilePickerWidget},
    layout::FocusedPane,
    popup::{
        KeybindingNotificationWidget, ModeSwitcherWidget, PopupWidget, ShellInstallPromptWidget,
        ShellSelectorWidget, ThemeSelectorWidget,
    },
    ssh_manager::SSHManagerWidget,
    statusbar::StatusBar,
    terminal_tabs::TerminalTabBar,
    terminal_widget::TerminalWidget,
};

use super::App;

impl App {
    /// Renders the application.
    pub fn render(&self, frame: &mut ratatui::Frame) {
        let area = frame.area();

        // Clear the entire frame first to prevent rendering artifacts
        frame.render_widget(Clear, area);

        // Explicitly reset entire buffer to prevent ghost characters
        let bg_color = self.config.theme_manager.current().editor.background;
        let clear_style = Style::default().bg(bg_color).fg(Color::Reset);
        let buf = frame.buffer_mut();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                    cell.set_char(' ');
                    cell.set_style(clear_style);
                }
            }
        }

        let areas = self.layout.calculate(area);

        // Render terminal pane (with split support)
        if areas.has_terminal() {
            self.render_terminal_pane(frame, &areas);
        }

        // Render editor or file browser
        if areas.has_editor() {
            self.render_editor_pane(frame, &areas);
        }

        // Render status bar
        self.render_status_bar(frame, &areas);

        // Render popup if visible
        if self.popup.is_visible() {
            self.render_popup(frame, area);
        }
    }

    /// Renders the terminal pane.
    fn render_terminal_pane(
        &self,
        frame: &mut ratatui::Frame,
        areas: &crate::ui::layout::LayoutAreas,
    ) {
        if let Some(ref terminals) = self.terminals {
            let is_focused = self.layout.focused() == FocusedPane::Terminal;
            let tab_info = terminals.tab_info();

            // Split area for tab bar + terminal content
            let terminal_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(areas.terminal);

            // Render tab bar
            let tab_bar = TerminalTabBar::new(&tab_info).focused(is_focused);
            frame.render_widget(tab_bar, terminal_chunks[0]);

            // Render terminal content in remaining area
            let terminal_area = terminal_chunks[1];

            // Cache the terminal area for mouse coordinate conversion
            self.last_terminal_area.set(terminal_area);

            if let Some(tab) = terminals.active_tab() {
                let terminal_theme = &self.config.theme_manager.current().terminal;
                let (grid_cols, grid_rows) = tab.grid.dimensions();
                let focused_idx = tab.grid.focused_index();

                match (grid_cols, grid_rows) {
                    (1, 1) => {
                        // Single terminal (no grid split)
                        if let Some(terminal) = tab.grid.get(0) {
                            let widget = TerminalWidget::new(terminal)
                                .focused(is_focused)
                                .title(terminal.title())
                                .theme(terminal_theme);
                            frame.render_widget(widget, terminal_area);
                        }
                    }
                    (2, 1) => {
                        // Two terminals side-by-side (vertical split)
                        self.render_terminal_grid_2x1(
                            frame,
                            terminal_area,
                            tab,
                            is_focused,
                            focused_idx,
                            terminal_theme,
                        );
                    }
                    (2, 2) => {
                        // 2x2 grid
                        self.render_terminal_grid_2x2(
                            frame,
                            terminal_area,
                            tab,
                            is_focused,
                            focused_idx,
                            terminal_theme,
                        );
                    }
                    _ => {
                        // Fallback: render focused terminal
                        if let Some(terminal) = tab.grid.focused() {
                            let widget = TerminalWidget::new(terminal)
                                .focused(is_focused)
                                .title(terminal.title())
                                .theme(terminal_theme);
                            frame.render_widget(widget, terminal_area);
                        }
                    }
                }
            }
        }
    }

    /// Renders a 2x1 terminal grid (two terminals side-by-side).
    fn render_terminal_grid_2x1(
        &self,
        frame: &mut ratatui::Frame,
        terminal_area: ratatui::layout::Rect,
        tab: &crate::terminal::TerminalTab,
        is_focused: bool,
        focused_idx: usize,
        terminal_theme: &crate::theme::TerminalTheme,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(terminal_area);

        for (i, chunk) in chunks.iter().enumerate() {
            if let Some(terminal) = tab.grid.get(i) {
                let pane_focused = is_focused && focused_idx == i;
                let widget = TerminalWidget::new(terminal)
                    .focused(pane_focused)
                    .title(terminal.title())
                    .theme(terminal_theme);
                frame.render_widget(widget, *chunk);
            }
        }
    }

    /// Renders a 2x2 terminal grid.
    fn render_terminal_grid_2x2(
        &self,
        frame: &mut ratatui::Frame,
        terminal_area: ratatui::layout::Rect,
        tab: &crate::terminal::TerminalTab,
        is_focused: bool,
        focused_idx: usize,
        terminal_theme: &crate::theme::TerminalTheme,
    ) {
        let row_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(terminal_area);

        // Top row
        let top_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(row_chunks[0]);

        // Bottom row
        let bottom_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(row_chunks[1]);

        // Grid layout: 0=top-left, 1=top-right, 2=bottom-left, 3=bottom-right
        let all_chunks = [top_cols[0], top_cols[1], bottom_cols[0], bottom_cols[1]];

        for (i, chunk) in all_chunks.iter().enumerate() {
            if let Some(terminal) = tab.grid.get(i) {
                let pane_focused = is_focused && focused_idx == i;
                let widget = TerminalWidget::new(terminal)
                    .focused(pane_focused)
                    .title(terminal.title())
                    .theme(terminal_theme);
                frame.render_widget(widget, *chunk);
            }
        }
    }

    /// Renders the editor pane (or file browser if visible).
    fn render_editor_pane(
        &self,
        frame: &mut ratatui::Frame,
        areas: &crate::ui::layout::LayoutAreas,
    ) {
        let is_focused = self.layout.focused() == FocusedPane::Editor;

        // Check for remote file browser first
        if let Some(ref remote_browser) = self.remote_file_browser {
            let widget = RemoteFilePickerWidget::new(remote_browser).focused(is_focused);
            frame.render_widget(widget, areas.editor);
        } else if self.file_browser.is_visible() {
            let widget = FilePickerWidget::new(&self.file_browser).focused(is_focused);
            frame.render_widget(widget, areas.editor);
        } else {
            // Split area for tab bar + editor content
            let editor_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(areas.editor);

            // Render editor tab bar
            let editor_tabs = self.editor_tab_info();
            let tab_bar = EditorTabBar::new(&editor_tabs).focused(is_focused);
            frame.render_widget(tab_bar, editor_chunks[0]);

            // Render editor content with completion suggestion
            let widget = EditorWidget::new(&self.editor)
                .focused(is_focused)
                .theme(&self.config.theme_manager.current().editor)
                .suggestion(self.completion_suggestion());
            frame.render_widget(widget, editor_chunks[1]);
        }
    }

    /// Renders the status bar.
    pub(crate) fn render_status_bar(
        &self,
        frame: &mut ratatui::Frame,
        areas: &crate::ui::layout::LayoutAreas,
    ) {
        let path_string = self.editor.path().map(|p| p.display().to_string());
        let path_ref = path_string.as_deref();
        let terminal_title = self
            .terminals
            .as_ref()
            .and_then(|t| t.active_terminal())
            .map(|t| t.title().to_string());

        let mut status_bar = StatusBar::new()
            .focused_pane(self.layout.focused())
            .keybinding_mode(self.config.mode)
            .message(&self.status);

        if self.layout.focused() == FocusedPane::Editor {
            status_bar = status_bar
                .editor_mode(self.editor.mode())
                .cursor_position(self.editor.cursor_position());

            if let Some(path) = path_ref {
                status_bar = status_bar.file_path(path);
            }
        } else if let Some(ref title) = terminal_title {
            status_bar = status_bar.terminal_title(title);
        }

        // Add tab info to status bar if we have multiple tabs
        let final_message = if let Some(ref terminals) = self.terminals {
            let tab_count = terminals.tab_count();
            if tab_count > 1 {
                let active = terminals.active_tab_index() + 1;
                format!("[Tab {}/{}] {}", active, tab_count, self.status)
            } else {
                self.status.clone()
            }
        } else {
            self.status.clone()
        };

        if !final_message.is_empty() && final_message != self.status {
            status_bar = status_bar.message(&final_message);
        }

        // Add background process indicators
        status_bar = status_bar.background_processes(
            self.background_running_count(),
            self.background_error_count(),
        );

        frame.render_widget(status_bar, areas.status_bar);
    }

    /// Renders the popup overlay.
    fn render_popup(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        // Use special widget for mode switcher
        if let Some(ref switcher) = self.mode_switcher {
            let widget = ModeSwitcherWidget::new(switcher);
            frame.render_widget(widget, area);
        } else if let Some(ref selector) = self.shell_selector {
            // Use special widget for shell selector
            let widget = ShellSelectorWidget::new(selector);
            frame.render_widget(widget, area);
        } else if let Some(ref prompt) = self.shell_install_prompt {
            // Use special widget for shell install prompt
            let widget = ShellInstallPromptWidget::new(prompt);
            frame.render_widget(widget, area);
        } else if let Some(ref selector) = self.theme_selector {
            // Use special widget for theme selector
            let widget = ThemeSelectorWidget::new(selector);
            frame.render_widget(widget, area);
        } else if let Some(ref manager) = self.ssh_manager {
            // Use special widget for SSH manager
            let widget = SSHManagerWidget::new(manager);
            frame.render_widget(widget, area);
        } else if self.popup.kind().is_keybinding_notification() {
            // Use special widget for Windows 11 keybinding notification
            let widget = KeybindingNotificationWidget::new();
            frame.render_widget(widget, area);
        } else {
            let popup_widget = PopupWidget::new(&self.popup);
            frame.render_widget(popup_widget, area);
        }
    }
}
