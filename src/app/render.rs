//! Rendering methods for the App.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use tracing::debug;

use crate::ui::{
    addon_manager::AddonManagerWidget,
    docker_manager::DockerManagerWidget,
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

        // Track frame count for debugging initial render issues
        static FRAME_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let frame_num = FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        debug!(
            "RENDER START: frame={}, frame_area=({}, {}, {}x{})",
            frame_num, area.x, area.y, area.width, area.height
        );

        // NOTE: Each pane (terminal, editor, status bar) handles its own clearing
        // with appropriate background colors. No frame-level clear needed - removing
        // it prevents flicker from inconsistent colors between panes.

        let areas = self.layout.calculate(area);

        debug!(
            "LAYOUT: terminal=({}, {}, {}x{}), editor=({}, {}, {}x{}), ide_visible={}, focused={:?}",
            areas.terminal.x,
            areas.terminal.y,
            areas.terminal.width,
            areas.terminal.height,
            areas.editor.x,
            areas.editor.y,
            areas.editor.width,
            areas.editor.height,
            self.layout.ide_visible(),
            self.layout.focused()
        );

        // On first 5 frames, log initial buffer state BEFORE any rendering
        if frame_num < 5 && areas.has_terminal() && areas.has_editor() {
            let buf = frame.buffer_mut();
            for y in 0..5.min(area.height) {
                let mut nonspace_cells = Vec::new();
                for x in 0..area.width {
                    if let Some(cell) = buf.cell((x, y)) {
                        let c = cell.symbol().chars().next().unwrap_or(' ');
                        if c != ' ' && c != '\0' {
                            nonspace_cells.push(format!("({},{}='{}')", x, y, c));
                        }
                    }
                }
                if !nonspace_cells.is_empty() {
                    debug!(
                        "FRAME_{}_INITIAL y={}: {}",
                        frame_num,
                        y,
                        nonspace_cells.join(" ")
                    );
                }
            }
        }

        // Render terminal pane (with split support)
        if areas.has_terminal() {
            debug!("RENDER: terminal pane");
            self.render_terminal_pane(frame, &areas);
        }

        // Log boundary cells AFTER terminal render, BEFORE editor render
        if areas.has_terminal() && areas.has_editor() {
            let buf = frame.buffer_mut();
            let boundary_x = areas.terminal.x + areas.terminal.width; // First column of editor
            // Sample a few rows at the boundary with detailed hex info
            for sample_y in [2, 10, 20].iter() {
                let y = *sample_y;
                if y < area.height {
                    // Log cells around the boundary with hex codes for debugging
                    let mut boundary_info = Vec::new();
                    for x_offset in -3i16..6 {
                        let x = (boundary_x as i16 + x_offset) as u16;
                        if x < area.width {
                            if let Some(cell) = buf.cell((x, y)) {
                                let symbol = cell.symbol();
                                let first_char = symbol.chars().next().unwrap_or(' ');
                                let char_code = first_char as u32;
                                let display = if first_char.is_control() || char_code > 0x7F {
                                    format!("x{:02}U+{:04X}", x, char_code)
                                } else {
                                    format!("x{}='{}'", x, first_char)
                                };
                                boundary_info.push(display);
                            }
                        }
                    }
                    debug!(
                        "BOUNDARY_DETAIL y={}: term_end={}, ed_start={} | {}",
                        y,
                        boundary_x.saturating_sub(1),
                        boundary_x,
                        boundary_info.join(" ")
                    );
                }
            }
        }

        // Render editor or file browser
        if areas.has_editor() {
            debug!(
                "RENDER: editor pane, file_browser_visible={}, remote_browser={}, open_files={}, current_idx={}",
                self.file_browser.is_visible(),
                self.remote_file_browser.is_some(),
                self.open_files.len(),
                self.current_file_idx
            );
            debug!(
                "RENDER: editor content mode - will render: {}",
                if self.remote_file_browser.is_some() {
                    "RemoteFilePicker"
                } else if self.file_browser.is_visible() {
                    "FilePicker"
                } else {
                    "EditorWidget + TabBar"
                }
            );
            self.render_editor_pane(frame, &areas);

            // Log boundary cells AFTER editor render - detailed hex info
            let buf = frame.buffer_mut();
            let boundary_x = areas.editor.x; // First column of editor
            for sample_y in [2, 10, 20].iter() {
                let y = *sample_y;
                if y < area.height {
                    let mut boundary_info = Vec::new();
                    for x_offset in -3i16..6 {
                        let x = (boundary_x as i16 + x_offset) as u16;
                        if x < area.width {
                            if let Some(cell) = buf.cell((x, y)) {
                                let symbol = cell.symbol();
                                let first_char = symbol.chars().next().unwrap_or(' ');
                                let char_code = first_char as u32;
                                let display = if first_char.is_control() || char_code > 0x7F {
                                    format!("x{:02}U+{:04X}", x, char_code)
                                } else {
                                    format!("x{}='{}'", x, first_char)
                                };
                                boundary_info.push(display);
                            }
                        }
                    }
                    debug!("AFTER_EDITOR y={}: | {}", y, boundary_info.join(" "));
                }
            }
        }

        // Render status bar
        self.render_status_bar(frame, &areas);

        // Render popup if visible
        if self.popup.is_visible() {
            self.render_popup(frame, area);
        }

        // On first 5 frames, log FINAL buffer state AFTER all rendering
        if frame_num < 5 && areas.has_terminal() && areas.has_editor() {
            let buf = frame.buffer_mut();
            // Check specific rows where the issue might be happening
            // Log both terminal and editor content at same rows for comparison
            for y in 0..5.min(area.height) {
                let mut term_chars = String::new();
                let mut ed_chars = String::new();
                // Sample terminal content (columns 0-20)
                for x in 0..20.min(areas.terminal.width) {
                    if let Some(cell) = buf.cell((x, y)) {
                        let c = cell.symbol().chars().next().unwrap_or(' ');
                        term_chars.push(if c.is_control() { '?' } else { c });
                    }
                }
                // Sample editor content (first 20 columns of editor area)
                for x in areas.editor.x..areas.editor.x + 20.min(areas.editor.width) {
                    if let Some(cell) = buf.cell((x, y)) {
                        let c = cell.symbol().chars().next().unwrap_or(' ');
                        ed_chars.push(if c.is_control() { '?' } else { c });
                    }
                }
                debug!(
                    "FRAME_{}_FINAL y={}: TERM[0..20]=[{}] EDIT[{}..{}]=[{}]",
                    frame_num,
                    y,
                    term_chars.trim_end(),
                    areas.editor.x,
                    areas.editor.x + 20.min(areas.editor.width),
                    ed_chars.trim_end()
                );
            }
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

            // DIAGNOSTIC: Log terminal grid vs render area dimensions
            if let Some(terminal) = terminals.active_terminal() {
                let grid = terminal.grid();
                let inner_width = areas.terminal.width.saturating_sub(2); // account for borders
                let inner_height = areas.terminal.height.saturating_sub(3); // account for tab bar + borders
                let cols_match = grid.cols() == inner_width;
                let rows_match = grid.rows() == inner_height;
                debug!(
                    "RENDER_TERM_PANE: terminal_area=({}, {}, {}x{}), inner={}x{}, grid={}x{}, MATCH=cols:{} rows:{}",
                    areas.terminal.x,
                    areas.terminal.y,
                    areas.terminal.width,
                    areas.terminal.height,
                    inner_width,
                    inner_height,
                    grid.cols(),
                    grid.rows(),
                    cols_match,
                    rows_match
                );
            }

            // CRITICAL: Clear the entire terminal area BEFORE any widget renders
            // This prevents any ghost characters from previous frames
            let terminal_theme = &self.config.theme_manager.current().terminal;
            let clear_style = Style::default()
                .bg(terminal_theme.background)
                .fg(Color::Reset);
            let buf = frame.buffer_mut();
            for y in areas.terminal.y..areas.terminal.y + areas.terminal.height {
                for x in areas.terminal.x..areas.terminal.x + areas.terminal.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.reset();
                        cell.set_char(' ');
                        cell.set_style(clear_style);
                    }
                }
            }

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

        debug!(
            "RENDER_EDITOR_PANE: editor_area=({}, {}, {}x{}), view_dims=({}, {}), buf_lines={}",
            areas.editor.x,
            areas.editor.y,
            areas.editor.width,
            areas.editor.height,
            self.editor.view().width(),
            self.editor.view().height(),
            self.editor.buffer().len_lines()
        );

        // CRITICAL: Clear the entire editor area BEFORE any widget renders
        // This prevents any ghost characters from previous frames
        let bg_color = self.config.theme_manager.current().editor.background;
        let clear_style = Style::default().bg(bg_color).fg(Color::Reset);
        {
            let buf = frame.buffer_mut();
            for y in areas.editor.y..areas.editor.y + areas.editor.height {
                for x in areas.editor.x..areas.editor.x + areas.editor.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.reset();
                        cell.set_char(' ');
                        cell.set_style(clear_style);
                    }
                }
            }
        }

        // Log boundary AFTER clear, BEFORE widget render - detailed hex info
        {
            let buf = frame.buffer_mut();
            let boundary_x = areas.editor.x;
            for sample_y in [2, 10, 20].iter() {
                let y = *sample_y;
                if y < areas.editor.y + areas.editor.height {
                    let mut boundary_info = Vec::new();
                    for x_offset in -3i16..6 {
                        let x = (boundary_x as i16 + x_offset) as u16;
                        if x < areas.editor.x + areas.editor.width + 3 {
                            if let Some(cell) = buf.cell((x, y)) {
                                let symbol = cell.symbol();
                                let first_char = symbol.chars().next().unwrap_or(' ');
                                let char_code = first_char as u32;
                                let display = if first_char.is_control() || char_code > 0x7F {
                                    format!("x{:02}U+{:04X}", x, char_code)
                                } else {
                                    format!("x{}='{}'", x, first_char)
                                };
                                boundary_info.push(display);
                            }
                        }
                    }
                    debug!("AFTER_CLEAR y={}: | {}", y, boundary_info.join(" "));
                }
            }
        }

        // Log state before widget selection - CRITICAL for debugging
        debug!(
            "RENDER_EDITOR_PANE: WIDGET_DECISION remote_browser={}, file_browser_visible={}, mode={:?}, open_files={}",
            self.remote_file_browser.is_some(),
            self.file_browser.is_visible(),
            self.mode,
            self.open_files.len()
        );

        // Check for remote file browser first
        if let Some(ref remote_browser) = self.remote_file_browser {
            debug!("RENDER_EDITOR_PANE: >>> RENDERING RemoteFilePicker <<<");
            let widget = RemoteFilePickerWidget::new(remote_browser).focused(is_focused);
            frame.render_widget(widget, areas.editor);
        } else if self.file_browser.is_visible() {
            debug!("RENDER_EDITOR_PANE: >>> RENDERING FilePicker <<<");
            let widget = FilePickerWidget::new(&self.file_browser).focused(is_focused);
            frame.render_widget(widget, areas.editor);
        } else {
            debug!("RENDER_EDITOR_PANE: >>> RENDERING EditorWidget + TabBar <<<");

            // Split area for tab bar + editor content
            let editor_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(areas.editor);

            debug!(
                "RENDER_EDITOR_PANE: split into tab_bar=({}, {}, {}x{}) + editor=({}, {}, {}x{})",
                editor_chunks[0].x,
                editor_chunks[0].y,
                editor_chunks[0].width,
                editor_chunks[0].height,
                editor_chunks[1].x,
                editor_chunks[1].y,
                editor_chunks[1].width,
                editor_chunks[1].height
            );

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
        } else if let Some(ref manager) = self.docker_manager {
            // Use special widget for Docker manager
            let widget = DockerManagerWidget::new(manager);
            frame.render_widget(widget, area);
        } else if let Some(ref manager) = self.addon_manager {
            // Use special widget for Add-ons manager
            let widget = AddonManagerWidget::new(manager);
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
