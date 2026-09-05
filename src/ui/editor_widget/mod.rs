//! Editor pane widget.
//!
//! The widget draws; it decides nothing. What each character is — a keyword,
//! part of the selection, a search hit, the matching bracket — comes from
//! [`Editor::screen_decor`](crate::editor::Editor::screen_decor), and which
//! buffer lines a frame shows comes from
//! [`Editor::screen_lines`](crate::editor::Editor::screen_lines), so folding
//! and highlighting are both testable without a terminal.

pub mod content;
pub mod gutter;
pub mod search_bar;
pub mod style;

use std::collections::HashMap;

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use tracing::debug;

use self::style::Palette;
use super::ghost_text::GhostTextWidget;
use crate::editor::{Editor, EditorMode};
use crate::git::gutter::GutterMark;
use crate::theme::EditorTheme;

/// Editor widget for rendering.
pub struct EditorWidget<'a> {
    /// Editor to render.
    editor: &'a Editor,
    /// Whether the editor is focused.
    focused: bool,
    /// Theme for rendering colors.
    theme: Option<&'a EditorTheme>,
    /// Completion suggestion to display as ghost text.
    suggestion: Option<&'a str>,
    /// Git gutter marks (line index -> mark).
    git_gutter: Option<&'a HashMap<usize, GutterMark>>,
    /// Breakpoint lines (0-based line indices).
    breakpoint_lines: Option<&'a [usize]>,
}

impl<'a> EditorWidget<'a> {
    /// Creates a new editor widget.
    #[must_use]
    pub fn new(editor: &'a Editor) -> Self {
        Self {
            editor,
            focused: false,
            theme: None,
            suggestion: None,
            git_gutter: None,
            breakpoint_lines: None,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Sets the theme.
    #[must_use]
    pub fn theme(mut self, theme: &'a EditorTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Sets the git gutter marks.
    #[must_use]
    pub fn git_gutter(mut self, marks: &'a HashMap<usize, GutterMark>) -> Self {
        if !marks.is_empty() {
            self.git_gutter = Some(marks);
        }
        self
    }

    /// Sets breakpoint lines to display in the gutter.
    #[must_use]
    pub fn breakpoints(mut self, lines: &'a [usize]) -> Self {
        if !lines.is_empty() {
            self.breakpoint_lines = Some(lines);
        }
        self
    }

    /// Sets the completion suggestion to display as ghost text.
    #[must_use]
    pub fn suggestion(mut self, suggestion: Option<&'a str>) -> Self {
        self.suggestion = suggestion;
        self
    }

    /// Paints the whole inner area with the background colour.
    ///
    /// Without this, scrolling leaves ghost characters behind on Windows
    /// terminals.
    fn clear(palette: &Palette, area: Rect, buf: &mut RatatuiBuffer) {
        let style = Style::default().bg(palette.background).fg(Color::Reset);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(style);
                }
            }
        }
    }

    /// Draws the block cursor over the character it sits on.
    fn render_cursor(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let pos = self.editor.cursor_position();
        let Some(row) = self.editor.screen_row_of(pos.line) else {
            return;
        };
        let view = self.editor.view();
        if pos.col < view.scroll_left() {
            return;
        }
        let x = area.x
            + u16::try_from(pos.col - view.scroll_left() + view.gutter_width() + 1)
                .unwrap_or(u16::MAX);
        let y = area.y + row;

        if x < area.x + area.width
            && y < area.y + area.height
            && let Some(cell) = buf.cell_mut((x, y))
        {
            let current = cell.style();
            let styled = match self.editor.mode() {
                EditorMode::Visual => current.bg(Color::Magenta),
                EditorMode::Command => current.add_modifier(Modifier::UNDERLINED),
                EditorMode::Insert | EditorMode::Normal => current.add_modifier(Modifier::REVERSED),
            };
            cell.set_style(styled);
        }
    }
}

impl Widget for EditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        let palette = Palette::resolve(self.theme);

        let border_color = if self.focused {
            palette.border_focused
        } else {
            palette.border
        };
        let block = Block::default()
            .title(self.editor.pane_title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color).bg(palette.background));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        debug!(
            "EDITOR_WIDGET: inner=({}, {}, {}x{}) focused={} lines={} folded={}",
            inner.x,
            inner.y,
            inner.width,
            inner.height,
            self.focused,
            self.editor.buffer().len_lines(),
            self.editor.folds().ranges().len()
        );

        Self::clear(&palette, inner, buf);

        // The find bar, when open, takes the bottom row of the pane.
        let bar_open = self.editor.search().is_active();
        let text_area = if bar_open && inner.height > 1 {
            Rect {
                height: inner.height - 1,
                ..inner
            }
        } else {
            inner
        };

        gutter::render(
            self.editor,
            &palette,
            self.git_gutter,
            self.breakpoint_lines,
            text_area,
            buf,
        );
        content::render(self.editor, &palette, text_area, buf);

        if let Some(suggestion) = self.suggestion
            && !suggestion.is_empty()
            && self.focused
        {
            let pos = self.editor.cursor_position();
            let line = self.editor.buffer().line(pos.line).unwrap_or_default();
            let ghost = GhostTextWidget::new(
                Some(suggestion),
                pos.line,
                pos.col,
                self.editor.view(),
                &line,
            );
            ghost.render(text_area, buf);
        }

        if self.focused {
            self.render_cursor(text_area, buf);
        }

        if bar_open {
            search_bar::render(self.editor.search(), &palette, inner, buf);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::Position;
    use crate::editor::highlight::Language;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn draw(editor: &Editor, width: u16, height: u16) -> RatatuiBuffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("a terminal");
        terminal
            .draw(|frame| {
                let widget = EditorWidget::new(editor).focused(true);
                frame.render_widget(widget, frame.area());
            })
            .expect("a frame");
        terminal.backend().buffer().clone()
    }

    fn row_text(buf: &RatatuiBuffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| {
                buf.cell((x, y))
                    .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
            })
            .collect()
    }

    #[test]
    fn test_editor_widget_builder() {
        let editor = Editor::new(80, 24);
        let widget = EditorWidget::new(&editor).focused(true);
        assert!(widget.focused);
    }

    #[test]
    fn the_title_shows_the_mode_and_the_dirty_marker() {
        let mut editor = Editor::new(60, 6);
        editor.insert_str("x");
        let buf = draw(&editor, 60, 6);
        let title = row_text(&buf, 0, 60);
        assert!(title.contains("[+]"), "title was {title:?}");
        assert!(title.contains("NORMAL"), "title was {title:?}");
    }

    #[test]
    fn the_find_bar_appears_only_when_the_search_is_open() {
        let mut editor = Editor::new(60, 8);
        editor.insert_str("cat cat\n");
        let before = row_text(&draw(&editor, 60, 8), 6, 60);
        assert!(!before.contains("Find:"));

        editor.open_search(false);
        for c in "cat".chars() {
            editor.search_mut().push_char(c);
        }
        editor.refresh_search();
        let after = row_text(&draw(&editor, 60, 8), 6, 60);
        assert!(after.contains("Find: cat"), "row was {after:?}");
        assert!(after.contains("1/2"), "row was {after:?}");
    }

    #[test]
    fn a_folded_region_takes_one_row() {
        let mut editor = Editor::new(60, 10);
        editor.insert_str("fn f() {\n    g();\n    h();\n}\nlet z = 1;\n");
        editor.set_language(Language::Rust);
        editor.set_cursor_position(Position::new(0, 0));
        assert!(editor.toggle_fold());

        let buf = draw(&editor, 60, 10);
        let first = row_text(&buf, 1, 60);
        let second = row_text(&buf, 2, 60);
        assert!(first.contains("fn f()"), "row was {first:?}");
        assert!(first.contains('⋯'), "row was {first:?}");
        assert!(second.contains("let z"), "row was {second:?}");
    }

    #[test]
    fn a_keyword_is_drawn_in_the_keyword_colour() {
        let mut editor = Editor::new(60, 6);
        editor.insert_str("fn main() {}\n");
        editor.set_language(Language::Rust);
        let buf = draw(&editor, 60, 6);
        // +1 for the border, then the gutter and the separator.
        let x = 1 + u16::try_from(editor.view().gutter_width()).unwrap_or(0) + 1;
        let cell = buf.cell((x, 1)).expect("cell");
        assert_eq!(cell.symbol(), "f");
        assert_eq!(
            cell.style().fg,
            Some(style::syntax_color(
                crate::editor::highlight::HighlightKind::Keyword
            ))
        );
    }

    #[test]
    fn an_unfocused_pane_draws_no_cursor() {
        let mut editor = Editor::new(60, 6);
        editor.insert_str("abc\n");
        let backend = TestBackend::new(60, 6);
        let mut terminal = Terminal::new(backend).expect("a terminal");
        terminal
            .draw(|frame| {
                frame.render_widget(EditorWidget::new(&editor).focused(false), frame.area());
            })
            .expect("a frame");
        let buf = terminal.backend().buffer().clone();
        let x = 1 + u16::try_from(editor.view().gutter_width()).unwrap_or(0) + 1;
        let cell = buf.cell((x, 1)).expect("cell");
        assert!(!cell.style().add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn a_pane_with_no_room_inside_renders_nothing_and_does_not_panic() {
        let editor = Editor::new(10, 10);
        let mut buf = RatatuiBuffer::empty(Rect::new(0, 0, 2, 2));
        EditorWidget::new(&editor).render(Rect::new(0, 0, 2, 2), &mut buf);
    }
}
