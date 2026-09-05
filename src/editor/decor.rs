//! What the renderer needs to know about a line, decided here rather than in
//! the widget.
//!
//! The widget walks characters and paints cells; every question about *what*
//! a character is — a keyword, part of the selection, the matching bracket, a
//! search hit, a second cursor — is answered by [`Editor::line_decor`], which
//! is testable without a terminal.

use super::highlight::{HighlightKind, HighlightSpan, kind_at};
use super::{Editor, Position};

/// The role a character plays, over and above its syntax colour.
///
/// Ordered by painting priority: a later variant wins over an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CharRole {
    /// Nothing special.
    Plain,
    /// Inside the selection.
    Selected,
    /// One of the two brackets around the cursor.
    MatchedBracket,
    /// A search hit that is not the current one.
    SearchMatch,
    /// The search hit the user is on.
    CurrentMatch,
    /// A secondary cursor sits here.
    ExtraCursor,
}

/// Everything the renderer needs for one line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineDecor {
    /// Buffer line this describes.
    pub line: usize,
    /// Syntax runs, in character columns.
    pub syntax: Vec<HighlightSpan>,
    /// The role of each character column that is not plain.
    ///
    /// Sparse on purpose: most lines have none.
    pub roles: Vec<(usize, CharRole)>,
    /// Number of lines hidden by a fold that starts here; zero when none does.
    pub folded_lines: usize,
}

impl LineDecor {
    /// Returns the syntax kind at a column.
    #[must_use]
    pub fn syntax_at(&self, col: usize) -> Option<HighlightKind> {
        kind_at(&self.syntax, col)
    }

    /// Returns the role at a column.
    #[must_use]
    pub fn role_at(&self, col: usize) -> CharRole {
        self.roles
            .iter()
            .find(|(c, _)| *c == col)
            .map_or(CharRole::Plain, |(_, role)| *role)
    }

    /// Returns true when this line stands in for a collapsed region.
    #[must_use]
    pub const fn is_folded(&self) -> bool {
        self.folded_lines > 0
    }

    /// Returns the marker drawn after a folded line's text.
    #[must_use]
    pub fn fold_marker(&self) -> String {
        if self.folded_lines == 0 {
            return String::new();
        }
        let unit = if self.folded_lines == 1 {
            "line"
        } else {
            "lines"
        };
        format!(" ⋯ {} {unit}", self.folded_lines)
    }
}

/// Records `role` at `col`, keeping the higher-priority role when two collide.
fn paint(roles: &mut Vec<(usize, CharRole)>, col: usize, role: CharRole) {
    match roles.iter_mut().find(|(c, _)| *c == col) {
        Some(slot) => {
            if role > slot.1 {
                slot.1 = role;
            }
        }
        None => roles.push((col, role)),
    }
}

impl Editor {
    /// Returns everything the renderer needs for one buffer line.
    #[must_use]
    pub fn line_decor(&self, line: usize) -> LineDecor {
        self.decor_for(line, self.matching_bracket_pair())
    }

    /// Builds one line's decorations with the bracket pair already resolved.
    ///
    /// The pair is the same for every line in a frame, and finding it costs a
    /// bracket scan, so a frame resolves it once.
    fn decor_for(&self, line: usize, pair: Option<(Position, Position)>) -> LineDecor {
        let width = self.buffer.line_len_chars(line);
        let mut roles: Vec<(usize, CharRole)> = Vec::new();

        if let Some((start, end)) = self.cursor.selection_range() {
            for col in selection_columns(line, width, start, end) {
                paint(&mut roles, col, CharRole::Selected);
            }
        }

        if let Some((open, close)) = pair {
            for pos in [open, close] {
                if pos.line == line && pos.col < width {
                    paint(&mut roles, pos.col, CharRole::MatchedBracket);
                }
            }
        }

        if self.search.is_active() {
            for (start, end, current) in self.search.matches_on_line(line) {
                let role = if current {
                    CharRole::CurrentMatch
                } else {
                    CharRole::SearchMatch
                };
                for col in start..end.min(width) {
                    paint(&mut roles, col, role);
                }
            }
        }

        for col in self.cursors.columns_on_line(line) {
            paint(&mut roles, col.min(width), CharRole::ExtraCursor);
        }

        roles.sort_by_key(|(col, _)| *col);

        LineDecor {
            line,
            syntax: self.highlight_line(line),
            roles,
            folded_lines: self.folds.hidden_below(line),
        }
    }

    /// Returns the decorations for every line a frame draws, in screen order.
    #[must_use]
    pub fn screen_decor(&self) -> Vec<LineDecor> {
        let pair = self.matching_bracket_pair();
        self.screen_lines()
            .into_iter()
            .map(|line| self.decor_for(line, pair))
            .collect()
    }

    /// Returns the title the editor pane shows.
    #[must_use]
    pub fn pane_title(&self) -> String {
        let path = self
            .path
            .as_ref()
            .map_or_else(|| "[No File]".to_string(), |p| p.display().to_string());
        let modified = if self.is_modified() { " [+]" } else { "" };
        format!("{path}{modified} {}", self.mode().label())
    }
}

/// Returns the columns of `line` covered by the selection `start..end`.
fn selection_columns(
    line: usize,
    width: usize,
    start: Position,
    end: Position,
) -> std::ops::Range<usize> {
    if line < start.line || line > end.line {
        return 0..0;
    }
    let first = if line == start.line { start.col } else { 0 };
    let last = if line == end.line { end.col } else { width };
    first.min(width)..last.min(width)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::highlight::Language;

    fn rust_editor(text: &str) -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(text);
        editor.set_language(Language::Rust);
        editor.set_cursor_position(Position::new(0, 0));
        editor
    }

    #[test]
    fn a_keyword_gets_a_syntax_kind() {
        let editor = rust_editor("fn main() {}\n");
        let decor = editor.line_decor(0);
        assert_eq!(decor.syntax_at(0), Some(HighlightKind::Keyword));
        assert_eq!(decor.line, 0);
    }

    #[test]
    fn a_plain_line_has_no_roles() {
        let editor = rust_editor("let a = 1;\n");
        let decor = editor.line_decor(0);
        assert!(decor.roles.is_empty());
        assert_eq!(decor.role_at(0), CharRole::Plain);
        assert!(!decor.is_folded());
        assert_eq!(decor.fold_marker(), "");
    }

    #[test]
    fn a_selection_marks_its_columns() {
        let mut editor = rust_editor("hello world\n");
        editor.set_cursor_position(Position::new(0, 2));
        editor.cursor_mut().start_selection();
        editor.cursor_mut().extend_to(Position::new(0, 5));
        let decor = editor.line_decor(0);
        assert_eq!(decor.role_at(1), CharRole::Plain);
        assert_eq!(decor.role_at(2), CharRole::Selected);
        assert_eq!(decor.role_at(4), CharRole::Selected);
        assert_eq!(decor.role_at(5), CharRole::Plain);
    }

    #[test]
    fn a_selection_spanning_lines_covers_the_whole_middle_line() {
        let mut editor = rust_editor("aaa\nbbb\nccc\n");
        editor.set_cursor_position(Position::new(0, 1));
        editor.cursor_mut().start_selection();
        editor.cursor_mut().extend_to(Position::new(2, 1));
        assert_eq!(editor.line_decor(1).roles.len(), 3);
        assert_eq!(editor.line_decor(0).role_at(0), CharRole::Plain);
        assert_eq!(editor.line_decor(0).role_at(1), CharRole::Selected);
        assert_eq!(editor.line_decor(2).role_at(1), CharRole::Plain);
    }

    #[test]
    fn the_matching_bracket_is_marked_on_both_lines() {
        let mut editor = rust_editor("fn f() {\n    g();\n}\n");
        editor.set_cursor_position(Position::new(0, 7));
        assert_eq!(editor.line_decor(0).role_at(7), CharRole::MatchedBracket);
        assert_eq!(editor.line_decor(2).role_at(0), CharRole::MatchedBracket);
        assert_eq!(editor.line_decor(1).role_at(0), CharRole::Plain);
    }

    #[test]
    fn search_hits_are_marked_and_the_current_one_differs() {
        let mut editor = rust_editor("let cat = cat;\n");
        editor.open_search(false);
        for c in "cat".chars() {
            editor.search_mut().push_char(c);
        }
        editor.refresh_search();
        editor.search_mut().select_from(Position::new(0, 0));
        let decor = editor.line_decor(0);
        assert_eq!(decor.role_at(4), CharRole::CurrentMatch);
        assert_eq!(decor.role_at(10), CharRole::SearchMatch);
        assert_eq!(decor.role_at(0), CharRole::Plain);
    }

    #[test]
    fn a_secondary_cursor_outranks_the_selection() {
        let mut editor = rust_editor("aaa\nbbb\n");
        editor.set_cursor_position(Position::new(0, 0));
        editor.add_cursor_below();
        let decor = editor.line_decor(1);
        assert_eq!(decor.role_at(0), CharRole::ExtraCursor);
    }

    #[test]
    fn a_folded_line_reports_how_many_it_hides() {
        let mut editor = rust_editor("fn f() {\n    g();\n    h();\n}\n");
        editor.set_cursor_position(Position::new(0, 0));
        assert!(editor.toggle_fold());
        let decor = editor.line_decor(0);
        assert!(decor.is_folded());
        assert_eq!(decor.folded_lines, 3);
        assert_eq!(decor.fold_marker(), " ⋯ 3 lines");
    }

    #[test]
    fn a_one_line_fold_marker_is_singular() {
        let decor = LineDecor {
            folded_lines: 1,
            ..LineDecor::default()
        };
        assert_eq!(decor.fold_marker(), " ⋯ 1 line");
    }

    #[test]
    fn screen_decor_covers_exactly_the_visible_lines() {
        let mut editor = rust_editor("fn f() {\n    g();\n}\nlet z = 1;\n");
        editor.set_cursor_position(Position::new(0, 0));
        editor.toggle_fold();
        let lines: Vec<usize> = editor.screen_decor().iter().map(|d| d.line).collect();
        assert_eq!(lines, editor.screen_lines());
        assert!(!lines.contains(&1));
    }

    #[test]
    fn the_pane_title_shows_the_state_of_the_document() {
        let mut editor = Editor::new(80, 24);
        assert_eq!(editor.pane_title(), "[No File] NORMAL");
        editor.insert_str("x");
        assert!(editor.pane_title().contains("[+]"));
        editor.set_mode(crate::editor::EditorMode::Insert);
        assert!(editor.pane_title().ends_with("INSERT"));
    }

    #[test]
    fn painting_keeps_the_higher_priority_role() {
        let mut roles = Vec::new();
        paint(&mut roles, 3, CharRole::Selected);
        paint(&mut roles, 3, CharRole::CurrentMatch);
        paint(&mut roles, 3, CharRole::Selected);
        assert_eq!(roles, vec![(3, CharRole::CurrentMatch)]);
    }

    #[test]
    fn selection_columns_clamp_to_the_line_width() {
        assert_eq!(
            selection_columns(1, 5, Position::new(0, 0), Position::new(2, 0)),
            0..5
        );
        assert_eq!(
            selection_columns(9, 5, Position::new(0, 0), Position::new(2, 0)),
            0..0
        );
        assert_eq!(
            selection_columns(0, 3, Position::new(0, 1), Position::new(0, 99)),
            1..3
        );
    }
}
