//! Reading and driving the rendered interface.
//!
//! The control API could already drive the PTY and the editor, but it could
//! not see the interface: popups, dashboards, the status bar and the key hint
//! bar were invisible to it, and there was no way to send a key to the
//! application rather than to the shell. An agent could type into a terminal
//! and read the buffer back, and nothing else.
//!
//! Three additions close that:
//!
//! - [`App::snapshot`] renders a frame into a `TestBackend` and returns the
//!   grid as text plus styled cells, so a caller can assert on what is on
//!   screen.
//! - [`App::inject_key`] and [`App::inject_mouse`] feed events into the same
//!   handlers a real keystroke reaches.
//! - Both work without a terminal, which is what makes headless runs and the
//!   scenario runner possible.

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use serde::{Deserialize, Serialize};

use super::App;

/// A single rendered cell, for assertions about colour and emphasis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCell {
    /// Column.
    pub x: u16,
    /// Row.
    pub y: u16,
    /// The character drawn there.
    pub ch: String,
    /// Foreground colour, as a lowercase name or `#rrggbb`.
    pub fg: String,
    /// Background colour.
    pub bg: String,
    /// Modifiers such as `bold` or `reversed`.
    pub mods: Vec<String>,
}

/// The rendered interface at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Frame width in cells.
    pub width: u16,
    /// Frame height in cells.
    pub height: u16,
    /// One string per row, trailing spaces trimmed.
    pub lines: Vec<String>,
    /// Cursor position, when the frame placed one.
    pub cursor: Option<(u16, u16)>,
    /// Styled cells; empty unless asked for, because it is large.
    pub cells: Vec<SnapshotCell>,
}

impl Snapshot {
    /// Returns the whole frame as one string.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Returns true if any row contains `needle`.
    #[must_use]
    pub fn contains(&self, needle: &str) -> bool {
        self.lines.iter().any(|line| line.contains(needle))
    }

    /// Returns the index of the first row containing `needle`.
    #[must_use]
    pub fn find_line(&self, needle: &str) -> Option<usize> {
        self.lines.iter().position(|line| line.contains(needle))
    }

    /// Returns the cell at a position, when styled cells were captured.
    #[must_use]
    pub fn cell_at(&self, x: u16, y: u16) -> Option<&SnapshotCell> {
        self.cells.iter().find(|c| c.x == x && c.y == y)
    }
}

/// What to include in a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotOptions {
    /// Frame width.
    pub width: u16,
    /// Frame height.
    pub height: u16,
    /// Include the styled cell list.
    ///
    /// Off by default: a 120x40 frame is 4,800 cells, which is a lot of JSON
    /// for an assertion that usually only needs the text.
    pub include_cells: bool,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            width: 120,
            height: 40,
            include_cells: false,
        }
    }
}

impl SnapshotOptions {
    /// Options for a frame of the given size.
    #[must_use]
    pub const fn sized(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            include_cells: false,
        }
    }

    /// Also capture styled cells.
    #[must_use]
    pub const fn with_cells(mut self) -> Self {
        self.include_cells = true;
        self
    }

    /// Returns the options with any zero dimension raised to one.
    #[must_use]
    fn sanitised(self) -> Self {
        Self {
            width: self.width.max(1),
            height: self.height.max(1),
            include_cells: self.include_cells,
        }
    }
}

/// Renders a colour the way the snapshot reports it.
#[must_use]
pub fn color_name(color: Color) -> String {
    match color {
        Color::Reset => "reset".to_string(),
        Color::Black => "black".to_string(),
        Color::Red => "red".to_string(),
        Color::Green => "green".to_string(),
        Color::Yellow => "yellow".to_string(),
        Color::Blue => "blue".to_string(),
        Color::Magenta => "magenta".to_string(),
        Color::Cyan => "cyan".to_string(),
        Color::Gray => "gray".to_string(),
        Color::DarkGray => "darkgray".to_string(),
        Color::LightRed => "lightred".to_string(),
        Color::LightGreen => "lightgreen".to_string(),
        Color::LightYellow => "lightyellow".to_string(),
        Color::LightBlue => "lightblue".to_string(),
        Color::LightMagenta => "lightmagenta".to_string(),
        Color::LightCyan => "lightcyan".to_string(),
        Color::White => "white".to_string(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(i) => format!("indexed({i})"),
    }
}

/// Lists the set modifiers by name.
#[must_use]
pub fn modifier_names(modifiers: Modifier) -> Vec<String> {
    const NAMED: &[(Modifier, &str)] = &[
        (Modifier::BOLD, "bold"),
        (Modifier::DIM, "dim"),
        (Modifier::ITALIC, "italic"),
        (Modifier::UNDERLINED, "underlined"),
        (Modifier::SLOW_BLINK, "slowblink"),
        (Modifier::RAPID_BLINK, "rapidblink"),
        (Modifier::REVERSED, "reversed"),
        (Modifier::HIDDEN, "hidden"),
        (Modifier::CROSSED_OUT, "crossedout"),
    ];

    NAMED
        .iter()
        .filter(|(flag, _)| modifiers.contains(*flag))
        .map(|(_, name)| (*name).to_string())
        .collect()
}

impl App {
    /// Renders the current interface into an off-screen buffer.
    ///
    /// The application is resized to the requested frame so the snapshot shows
    /// what a terminal of that size would show, then left at that size: a
    /// caller taking repeated snapshots wants a stable geometry, and the real
    /// terminal resizes the app again on its next event.
    ///
    /// # Errors
    /// Returns an error if the off-screen terminal cannot be created or drawn.
    pub fn snapshot(&mut self, options: SnapshotOptions) -> std::io::Result<Snapshot> {
        let options = options.sanitised();
        self.resize(options.width, options.height);

        let backend = TestBackend::new(options.width, options.height);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| self.render(frame))?;

        let buffer = terminal.backend().buffer().clone();
        let area = buffer.area;

        let mut lines = Vec::with_capacity(area.height as usize);
        let mut cells = Vec::new();

        for y in 0..area.height {
            let mut line = String::with_capacity(area.width as usize);
            for x in 0..area.width {
                let cell = &buffer[(x, y)];
                line.push_str(cell.symbol());

                if options.include_cells {
                    cells.push(SnapshotCell {
                        x,
                        y,
                        ch: cell.symbol().to_string(),
                        fg: color_name(cell.fg),
                        bg: color_name(cell.bg),
                        mods: modifier_names(cell.modifier),
                    });
                }
            }
            lines.push(line.trim_end().to_string());
        }

        Ok(Snapshot {
            width: area.width,
            height: area.height,
            lines,
            cursor: self.cursor_position_for_snapshot(),
            cells,
        })
    }

    /// Renders a snapshot at the app's current size.
    ///
    /// # Errors
    /// Returns an error if the off-screen terminal cannot be created or drawn.
    pub fn snapshot_current(&mut self) -> std::io::Result<Snapshot> {
        let (width, height) = self.last_screen_size;
        self.snapshot(SnapshotOptions::sized(width, height))
    }

    /// Returns where the editor cursor would be drawn, if the editor is shown.
    fn cursor_position_for_snapshot(&self) -> Option<(u16, u16)> {
        if !self.layout.ide_visible() {
            return None;
        }
        let position = self.editor.cursor_position();
        self.editor.view().buffer_to_screen(position)
    }

    /// Feeds a key event into the application.
    ///
    /// Goes through the same entry point as a real keystroke, so anything
    /// reachable by hand is reachable here.
    pub fn inject_key(&mut self, key: KeyEvent) {
        self.handle_key(key);
    }

    /// Feeds a mouse event into the application.
    pub fn inject_mouse(&mut self, mouse: MouseEvent) {
        self.handle_mouse(mouse);
    }

    /// Parses a key description and feeds it in.
    ///
    /// Accepts the spelling used in `.ratrc` and in scenario files:
    /// `ctrl+q`, `alt+shift+right`, `F2`, `enter`, `a`.
    ///
    /// # Errors
    /// Returns a message naming the part that could not be parsed.
    pub fn inject_key_str(&mut self, description: &str) -> Result<(), String> {
        let key = parse_key(description)?;
        self.inject_key(key);
        Ok(())
    }

    /// Types a string one character at a time.
    pub fn inject_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.inject_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
    }
}

/// Parses a key description such as `ctrl+shift+f5`.
///
/// # Errors
/// Returns a message naming the unrecognised part.
pub fn parse_key(description: &str) -> Result<KeyEvent, String> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return Err("empty key description".to_string());
    }

    // A lone character is itself, so `+` can be written as `+` rather than
    // needing an escape.
    let mut chars = trimmed.chars();
    if let (Some(only), None) = (chars.next(), chars.next()) {
        return Ok(KeyEvent {
            code: KeyCode::Char(only),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        });
    }

    let mut modifiers = KeyModifiers::NONE;
    let parts: Vec<&str> = trimmed.split('+').collect();
    let (code_part, modifier_parts) = parts
        .split_last()
        .ok_or_else(|| "empty key description".to_string())?;

    for part in modifier_parts {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" | "meta" | "option" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "super" | "cmd" | "win" => modifiers |= KeyModifiers::SUPER,
            other => return Err(format!("unknown modifier {other:?}")),
        }
    }

    let code = parse_key_code(code_part.trim())?;

    Ok(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}

/// Parses the key itself, without modifiers.
fn parse_key_code(name: &str) -> Result<KeyCode, String> {
    // A single character is itself, so `+` can be written as a lone `+`.
    let mut chars = name.chars();
    if let (Some(only), None) = (chars.next(), chars.next()) {
        return Ok(KeyCode::Char(only));
    }

    let lower = name.to_ascii_lowercase();

    if let Some(number) = lower.strip_prefix('f')
        && let Ok(n) = number.parse::<u8>()
    {
        if (1..=24).contains(&n) {
            return Ok(KeyCode::F(n));
        }
        return Err(format!("function key out of range: {name}"));
    }

    Ok(match lower.as_str() {
        "enter" | "return" | "cr" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backtab" | "shifttab" => KeyCode::BackTab,
        "backspace" | "bs" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "escape" | "esc" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        other => return Err(format!("unknown key {other:?}")),
    })
}

/// Parses a mouse event description such as `left_down@10,4`.
///
/// # Errors
/// Returns a message naming the part that could not be parsed.
pub fn parse_mouse(description: &str) -> Result<MouseEvent, String> {
    let (kind_part, position_part) = description
        .trim()
        .split_once('@')
        .ok_or_else(|| format!("mouse event {description:?} needs an @column,row suffix"))?;

    let (column, row) = position_part
        .split_once(',')
        .ok_or_else(|| format!("mouse position {position_part:?} needs a comma"))?;

    let column: u16 = column
        .trim()
        .parse()
        .map_err(|_| format!("bad column {column:?}"))?;
    let row: u16 = row.trim().parse().map_err(|_| format!("bad row {row:?}"))?;

    let kind = match kind_part.trim().to_ascii_lowercase().as_str() {
        "left_down" => MouseEventKind::Down(MouseButton::Left),
        "left_up" => MouseEventKind::Up(MouseButton::Left),
        "left_drag" => MouseEventKind::Drag(MouseButton::Left),
        "right_down" => MouseEventKind::Down(MouseButton::Right),
        "right_up" => MouseEventKind::Up(MouseButton::Right),
        "middle_down" => MouseEventKind::Down(MouseButton::Middle),
        "middle_up" => MouseEventKind::Up(MouseButton::Middle),
        "moved" => MouseEventKind::Moved,
        "scroll_up" => MouseEventKind::ScrollUp,
        "scroll_down" => MouseEventKind::ScrollDown,
        other => return Err(format!("unknown mouse event {other:?}")),
    };

    Ok(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn app() -> App {
        App::isolated(80, 24).expect("app")
    }

    #[test]
    fn a_snapshot_has_the_requested_geometry() {
        let mut app = app();
        let snapshot = app
            .snapshot(SnapshotOptions::sized(100, 30))
            .expect("snapshot");
        assert_eq!(snapshot.width, 100);
        assert_eq!(snapshot.height, 30);
        assert_eq!(snapshot.lines.len(), 30);
    }

    #[test]
    fn a_snapshot_omits_styled_cells_unless_asked() {
        let mut app = app();
        let plain = app
            .snapshot(SnapshotOptions::sized(40, 10))
            .expect("snapshot");
        assert!(plain.cells.is_empty());

        let styled = app
            .snapshot(SnapshotOptions::sized(40, 10).with_cells())
            .expect("snapshot");
        assert_eq!(styled.cells.len(), 40 * 10);
    }

    #[test]
    fn styled_cells_carry_position_and_colour() {
        let mut app = app();
        let snapshot = app
            .snapshot(SnapshotOptions::sized(20, 5).with_cells())
            .expect("snapshot");

        let cell = snapshot.cell_at(0, 0).expect("a cell at the origin");
        assert_eq!((cell.x, cell.y), (0, 0));
        assert!(!cell.fg.is_empty());
        assert!(!cell.bg.is_empty());
    }

    #[test]
    fn zero_dimensions_are_raised_rather_than_panicking() {
        let mut app = app();
        let snapshot = app
            .snapshot(SnapshotOptions::sized(0, 0))
            .expect("snapshot");
        assert_eq!(snapshot.width, 1);
        assert_eq!(snapshot.height, 1);
    }

    #[test]
    fn the_status_bar_text_appears_in_the_snapshot() {
        let mut app = app();
        app.set_status("SNAPSHOT MARKER");
        let snapshot = app
            .snapshot(SnapshotOptions::sized(120, 30))
            .expect("snapshot");
        assert!(
            snapshot.contains("SNAPSHOT MARKER"),
            "status text is missing:\n{}",
            snapshot.text()
        );
        assert!(snapshot.find_line("SNAPSHOT MARKER").is_some());
    }

    #[test]
    fn snapshots_reflect_state_changes() {
        let mut app = app();
        app.set_status("before");
        let first = app.snapshot(SnapshotOptions::sized(80, 20)).expect("first");
        app.set_status("after");
        let second = app
            .snapshot(SnapshotOptions::sized(80, 20))
            .expect("second");

        assert!(first.contains("before"));
        assert!(second.contains("after"));
        assert_ne!(first, second);
    }

    #[test]
    fn snapshot_current_uses_the_apps_own_size() {
        let mut app = app();
        app.resize(64, 18);
        let snapshot = app.snapshot_current().expect("snapshot");
        assert_eq!((snapshot.width, snapshot.height), (64, 18));
    }

    #[test]
    fn key_descriptions_parse() {
        let key = parse_key("ctrl+q").expect("parse");
        assert_eq!(key.code, KeyCode::Char('q'));
        assert!(key.modifiers.contains(KeyModifiers::CONTROL));

        let key = parse_key("alt+shift+right").expect("parse");
        assert_eq!(key.code, KeyCode::Right);
        assert!(key.modifiers.contains(KeyModifiers::ALT));
        assert!(key.modifiers.contains(KeyModifiers::SHIFT));

        assert_eq!(parse_key("F2").expect("parse").code, KeyCode::F(2));
        assert_eq!(parse_key("f12").expect("parse").code, KeyCode::F(12));
        assert_eq!(parse_key("enter").expect("parse").code, KeyCode::Enter);
        assert_eq!(parse_key("esc").expect("parse").code, KeyCode::Esc);
        assert_eq!(parse_key("space").expect("parse").code, KeyCode::Char(' '));
        assert_eq!(parse_key("a").expect("parse").code, KeyCode::Char('a'));
        assert_eq!(parse_key("+").expect("parse").code, KeyCode::Char('+'));
    }

    #[test]
    fn key_parsing_reports_what_it_did_not_understand() {
        let err = parse_key("hyper+a").expect_err("must fail");
        assert!(err.contains("hyper"), "{err}");

        let err = parse_key("wobble").expect_err("must fail");
        assert!(err.contains("wobble"), "{err}");

        let err = parse_key("f99").expect_err("must fail");
        assert!(err.contains("range"), "{err}");

        assert!(parse_key("").is_err());
        assert!(parse_key("   ").is_err());
    }

    #[test]
    fn mouse_descriptions_parse() {
        let event = parse_mouse("left_down@10,4").expect("parse");
        assert_eq!(event.column, 10);
        assert_eq!(event.row, 4);
        assert_eq!(event.kind, MouseEventKind::Down(MouseButton::Left));

        let event = parse_mouse("scroll_up@0,0").expect("parse");
        assert_eq!(event.kind, MouseEventKind::ScrollUp);
    }

    #[test]
    fn mouse_parsing_reports_what_it_did_not_understand() {
        assert!(parse_mouse("left_down").is_err());
        assert!(parse_mouse("left_down@10").is_err());
        assert!(parse_mouse("left_down@x,4").is_err());
        assert!(parse_mouse("nonsense@1,1").is_err());
    }

    #[test]
    fn injected_text_reaches_the_editor() {
        let mut app = app();
        app.new_editor_tab();
        app.editor_mut().set_mode(crate::editor::EditorMode::Insert);
        app.layout_mut()
            .set_focused(crate::ui::layout::FocusedPane::Editor);

        app.inject_text("hello");
        assert_eq!(app.editor().buffer().text(), "hello");
    }

    #[test]
    fn an_injected_key_string_reaches_the_editor() {
        let mut app = app();
        app.new_editor_tab();
        app.editor_mut().set_mode(crate::editor::EditorMode::Insert);
        app.layout_mut()
            .set_focused(crate::ui::layout::FocusedPane::Editor);

        app.inject_key_str("x").expect("inject");
        assert_eq!(app.editor().buffer().text(), "x");
    }

    #[test]
    fn an_unparseable_key_string_is_reported_and_changes_nothing() {
        let mut app = app();
        app.new_editor_tab();
        assert!(app.inject_key_str("not-a-key").is_err());
        assert!(app.editor().buffer().is_empty());
    }

    #[test]
    fn colour_names_cover_the_palette() {
        assert_eq!(color_name(Color::Red), "red");
        assert_eq!(color_name(Color::Reset), "reset");
        assert_eq!(color_name(Color::Rgb(1, 2, 3)), "#010203");
        assert_eq!(color_name(Color::Indexed(42)), "indexed(42)");
    }

    #[test]
    fn modifier_names_list_only_what_is_set() {
        assert!(modifier_names(Modifier::empty()).is_empty());
        assert_eq!(modifier_names(Modifier::BOLD), vec!["bold"]);
        let both = modifier_names(Modifier::BOLD | Modifier::REVERSED);
        assert!(both.contains(&"bold".to_string()));
        assert!(both.contains(&"reversed".to_string()));
    }

    #[test]
    fn a_snapshot_survives_a_json_round_trip() {
        let mut app = app();
        let snapshot = app
            .snapshot(SnapshotOptions::sized(30, 8).with_cells())
            .expect("snapshot");
        let encoded = serde_json::to_string(&snapshot).expect("serialise");
        let decoded: Snapshot = serde_json::from_str(&encoded).expect("deserialise");
        assert_eq!(decoded, snapshot);
    }
}
