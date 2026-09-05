# Implementation Plan: Theming System & Terminal Selection

## Overview

Two major features to implement:
1. **Theming/Customization System** - Full color, font, and layout customization via `.ratrc` and command palette
2. **Terminal Selection** - Mouse click+drag and Shift+Arrow keyboard selection in terminal

---

## Feature 1: Theming/Customization System

### Architecture

Create a new `src/theme/` module with the following structure:

```
src/theme/
├── mod.rs           # Theme system core, theme manager
├── colors.rs        # Color definitions, parsing, RGB/named colors
├── component.rs     # Per-component theme settings (terminal, editor, statusbar, etc.)
├── preset.rs        # Built-in theme presets (dark, light, dracula, etc.)
└── persistence.rs   # Save/load themes to .ratrc
```

### Core Types

#### `src/theme/colors.rs`
```rust
pub struct ThemeColor {
    pub foreground: Color,
    pub background: Color,
    pub cursor: Color,
    pub selection: Color,
    // ANSI 16 colors (customizable)
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright_black: Color,
    pub bright_red: Color,
    // ... etc
}
```

#### `src/theme/component.rs`
```rust
pub struct TerminalTheme {
    pub colors: ThemeColor,
    pub border_color: Color,
    pub border_color_focused: Color,
    pub tab_active_bg: Color,
    pub tab_inactive_bg: Color,
}

pub struct EditorTheme {
    pub colors: ThemeColor,
    pub line_numbers_fg: Color,
    pub line_numbers_bg: Color,
    pub current_line_bg: Color,
    pub border_color: Color,
    pub border_color_focused: Color,
}

pub struct StatusBarTheme {
    pub background: Color,
    pub foreground: Color,
    pub mode_normal_bg: Color,
    pub mode_insert_bg: Color,
    pub mode_visual_bg: Color,
    pub mode_command_bg: Color,
}

pub struct Theme {
    pub name: String,
    pub terminal: TerminalTheme,
    pub editor: EditorTheme,
    pub statusbar: StatusBarTheme,
    pub file_browser: FileBrowserTheme,
    pub popup: PopupTheme,
}
```

#### Per-Shell Themes
```rust
pub struct ShellThemeOverride {
    pub shell_type: ShellType,
    pub theme: Option<String>,  // Theme name or custom colors
    pub colors: Option<ThemeColor>,  // Inline color overrides
}
```

#### Per-Tab Themes
```rust
pub struct TabThemeConfig {
    pub pattern: TabPattern,  // Sequential, ByShell, Custom
    pub themes: Vec<String>,  // Theme names to cycle through
}

pub enum TabPattern {
    Sequential,     // Each new tab gets next theme in list
    ByShell,        // Theme based on shell type
    Random,         // Random from list
    Custom(String), // User-defined logic name
}
```

### .ratrc Configuration Format

```ini
# Theme Configuration
# -------------------

# Global theme (built-in: dark, light, dracula, gruvbox, nord)
theme = dark

# Terminal colors
terminal.foreground = #ffffff
terminal.background = #1e1e1e
terminal.cursor = #f0f0f0
terminal.selection = #264f78
terminal.border = #444444
terminal.border_focused = #00ff00

# Editor colors
editor.foreground = #d4d4d4
editor.background = #1e1e1e
editor.line_numbers = #858585
editor.current_line = #2a2a2a
editor.border = #444444
editor.border_focused = #569cd6

# Status bar colors
statusbar.background = #007acc
statusbar.foreground = #ffffff
statusbar.mode_normal = #007acc
statusbar.mode_insert = #4ec9b0
statusbar.mode_visual = #c586c0
statusbar.mode_command = #ce9178

# Tab colors
tabs.active_bg = #1e1e1e
tabs.inactive_bg = #2d2d2d
tabs.active_fg = #ffffff
tabs.inactive_fg = #808080

# Per-shell themes
[shell.powershell]
terminal.background = #012456
terminal.foreground = #eeedf0

[shell.bash]
terminal.background = #300a24
terminal.foreground = #ffffff

# Per-tab cycling
tab_theme_pattern = sequential
tab_themes = dark, dracula, nord

# Popup/dialog colors
popup.background = #252526
popup.border = #3c3c3c
popup.selected = #094771
```

### Command Palette Integration

Add new commands to `CommandPalette`:

```rust
// Theme commands
Command::new("theme.select", "Select Theme", "Appearance", None),
Command::new("theme.customize", "Customize Colors", "Appearance", None),
Command::new("theme.terminal.background", "Set Terminal Background", "Appearance", None),
Command::new("theme.terminal.foreground", "Set Terminal Foreground", "Appearance", None),
Command::new("theme.editor.background", "Set Editor Background", "Appearance", None),
Command::new("theme.editor.foreground", "Set Editor Foreground", "Appearance", None),
Command::new("theme.reset", "Reset to Default Theme", "Appearance", None),
Command::new("theme.export", "Export Current Theme", "Appearance", None),
```

### New Popup Types

```rust
pub enum PopupKind {
    // ... existing
    ThemeSelector,       // Select from preset themes
    ColorPicker,         // Pick a color (hex input, named colors)
    ComponentCustomizer, // Customize specific component
}
```

### Config Module Updates

Extend `src/config/mod.rs`:
- Add `theme: Theme` field to `Config`
- Add parsing for theme-related settings in `.ratrc`
- Add `save_setting()` method to write individual settings back to `.ratrc`
- Preserve comments and formatting when modifying `.ratrc`

### Files to Modify

| File | Changes |
|------|---------|
| `src/config/mod.rs` | Add theme config parsing, add `save_setting()` method |
| `src/app/mod.rs` | Add `theme` field, pass theme to widgets |
| `src/ui/terminal_widget.rs` | Accept theme, apply colors |
| `src/ui/editor_widget.rs` | Accept theme, apply colors |
| `src/ui/statusbar.rs` | Accept theme, apply colors |
| `src/ui/popup.rs` | Add ThemeSelector, ColorPicker popups, accept theme |
| `src/ui/terminal_tabs.rs` | Accept theme, apply colors |
| `src/ui/editor_tabs.rs` | Accept theme, apply colors |
| `src/ui/file_picker.rs` | Accept theme, apply colors |
| `src/terminal/multiplexer.rs` | Support per-tab theme assignment |

### New Files

| File | Purpose |
|------|---------|
| `src/theme/mod.rs` | Theme module, ThemeManager |
| `src/theme/colors.rs` | Color types, parsing hex/named |
| `src/theme/component.rs` | Component-specific themes |
| `src/theme/preset.rs` | Built-in presets |
| `src/theme/persistence.rs` | Save theme changes to .ratrc |

---

## Feature 2: Terminal Selection

### Architecture

Add selection state to the terminal grid and handle mouse/keyboard events.

### Selection State

Add to `src/terminal/grid.rs`:

```rust
pub struct Selection {
    /// Start position (col, row) in grid coordinates
    pub start: (u16, u16),
    /// End position (col, row) in grid coordinates
    pub end: (u16, u16),
    /// Whether selection is active (mouse button held)
    pub active: bool,
    /// Selection mode
    pub mode: SelectionMode,
}

pub enum SelectionMode {
    /// Character-by-character selection
    Normal,
    /// Select entire lines
    Line,
    /// Select rectangular block
    Block,
}
```

### Grid Updates

Add to `Grid`:
```rust
impl Grid {
    pub fn start_selection(&mut self, col: u16, row: u16);
    pub fn update_selection(&mut self, col: u16, row: u16);
    pub fn clear_selection(&mut self);
    pub fn get_selection(&self) -> Option<&Selection>;
    pub fn selected_text(&self) -> Option<String>;
    pub fn is_cell_selected(&self, col: u16, row: u16) -> bool;
}
```

### Terminal Module Updates

Add to `src/terminal/mod.rs`:
```rust
impl Terminal {
    /// Start selection at grid position
    pub fn start_selection(&mut self, col: u16, row: u16);

    /// Update selection end position
    pub fn update_selection(&mut self, col: u16, row: u16);

    /// Clear current selection
    pub fn clear_selection(&mut self);

    /// Get selected text
    pub fn selected_text(&self) -> Option<String>;

    /// Check if selection exists
    pub fn has_selection(&self) -> bool;

    /// Handle shift+arrow selection from cursor
    pub fn select_left(&mut self);
    pub fn select_right(&mut self);
    pub fn select_up(&mut self);
    pub fn select_down(&mut self);
}
```

### Mouse Event Handling

Update `src/app/mod.rs` to handle mouse events:

```rust
pub fn update(&mut self) -> io::Result<()> {
    // ... existing code ...

    if event::poll(Duration::from_millis(POLL_TIMEOUT_MS))? {
        match event::read()? {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(w, h) => self.resize(w, h),
            _ => {}
        }
    }

    Ok(())
}
```

Add mouse handler in `src/app/input.rs`:

```rust
use crossterm::event::{MouseEvent, MouseEventKind, MouseButton};

impl App {
    pub fn handle_mouse(&mut self, event: MouseEvent) {
        // Check if click is within terminal area
        let areas = self.layout.calculate(/* frame size */);

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.is_in_terminal_area(event.column, event.row, &areas) {
                    // Convert to terminal-local coordinates
                    let (local_col, local_row) = self.to_terminal_coords(event.column, event.row, &areas);
                    self.start_terminal_selection(local_col, local_row);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.is_in_terminal_area(event.column, event.row, &areas) {
                    let (local_col, local_row) = self.to_terminal_coords(event.column, event.row, &areas);
                    self.update_terminal_selection(local_col, local_row);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.finalize_terminal_selection();
            }
            MouseEventKind::ScrollUp => {
                self.handle_scroll_up();
            }
            MouseEventKind::ScrollDown => {
                self.handle_scroll_down();
            }
            _ => {}
        }
    }
}
```

### Keyboard Selection (Shift+Arrow)

Update `src/app/input.rs` - add to `handle_terminal_key`:

```rust
// Shift+Arrow for selection (starts from cursor position)
(KeyModifiers::SHIFT, KeyCode::Left) => {
    if let Some(ref mut terminals) = self.terminals {
        if let Some(terminal) = terminals.active_terminal_mut() {
            terminal.select_left();
        }
    }
    return;
}
(KeyModifiers::SHIFT, KeyCode::Right) => {
    if let Some(ref mut terminals) = self.terminals {
        if let Some(terminal) = terminals.active_terminal_mut() {
            terminal.select_right();
        }
    }
    return;
}
(KeyModifiers::SHIFT, KeyCode::Up) => {
    // Selects from current position to same column on line above
    // OR extends selection upward, selecting everything between
    if let Some(ref mut terminals) = self.terminals {
        if let Some(terminal) = terminals.active_terminal_mut() {
            terminal.select_up();
        }
    }
    return;
}
(KeyModifiers::SHIFT, KeyCode::Down) => {
    if let Some(ref mut terminals) = self.terminals {
        if let Some(terminal) = terminals.active_terminal_mut() {
            terminal.select_down();
        }
    }
    return;
}
```

### Selection Text Extraction Logic

When selection spans multiple lines:
```rust
fn selected_text(&self) -> Option<String> {
    let selection = self.selection.as_ref()?;

    // Normalize: ensure start is before end
    let (start, end) = if selection.start.1 < selection.end.1
        || (selection.start.1 == selection.end.1 && selection.start.0 <= selection.end.0) {
        (selection.start, selection.end)
    } else {
        (selection.end, selection.start)
    };

    let mut result = String::new();

    for row in start.1..=end.1 {
        let row_data = self.row(row as usize)?;

        let col_start = if row == start.1 { start.0 } else { 0 };
        let col_end = if row == end.1 { end.0 } else { self.cols - 1 };

        for col in col_start..=col_end {
            if let Some(cell) = row_data.cell(col) {
                result.push(cell.character());
            }
        }

        // Add newline between lines (not after last line)
        if row < end.1 {
            result.push('\n');
        }
    }

    // Trim trailing whitespace from each line
    Some(result.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n"))
}
```

### Terminal Widget Rendering

Update `src/ui/terminal_widget.rs` to render selection:

```rust
fn render_row_cells(&self, ...) {
    let selection = self.terminal.grid().get_selection();

    for col in 0..cols {
        let cell = row.cell(col as u16)?;
        let x = area.x + col as u16;

        if let Some(ratatui_cell) = buf.cell_mut((x, y)) {
            ratatui_cell.set_char(cell.character());

            // Check if this cell is selected
            let is_selected = selection
                .map(|s| self.is_cell_in_selection(col as u16, screen_row as u16, s))
                .unwrap_or(false);

            if is_selected {
                // Apply selection highlight (inverted or custom color)
                let selection_style = Style::default()
                    .bg(Color::Rgb(38, 79, 120))  // Selection blue
                    .fg(Color::White);
                ratatui_cell.set_style(selection_style);
            } else {
                ratatui_cell.set_style(cell.style().to_ratatui());
            }
        }
    }
}
```

### Copy Selection Update

Update existing `copy_terminal_selection()` in `src/app/input.rs`:

```rust
fn copy_terminal_selection(&mut self) {
    if let Some(ref mut terminals) = self.terminals {
        if let Some(terminal) = terminals.active_terminal_mut() {
            if let Some(text) = terminal.selected_text() {
                if !text.is_empty() {
                    self.copy_to_clipboard(&text);
                    self.set_status("Copied selection");
                    // Optionally clear selection after copy
                    terminal.clear_selection();
                }
            } else {
                // Fallback: copy current line at cursor (existing behavior)
                let grid = terminal.grid();
                let (_, row) = grid.cursor_pos();
                if let Some(line) = grid.row(row as usize) {
                    let text: String = line.cells().iter().map(|c| c.character()).collect();
                    let text = text.trim_end();
                    if !text.is_empty() {
                        self.copy_to_clipboard(text);
                    }
                }
            }
        }
    }
}
```

### Files to Modify

| File | Changes |
|------|---------|
| `src/terminal/grid.rs` | Add Selection struct, selection methods, `is_cell_selected()` |
| `src/terminal/mod.rs` | Add selection delegation methods, Shift+Arrow handlers |
| `src/app/mod.rs` | Add mouse event handling in `update()`, store layout areas |
| `src/app/input.rs` | Add `handle_mouse()`, Shift+Arrow handling, update copy |
| `src/ui/terminal_widget.rs` | Render selection highlight, accept selection state |

---

## Implementation Order

### Phase 1: Terminal Selection (Simpler, foundational) - ~300 lines new code
1. Add `Selection` struct to `src/terminal/grid.rs` (~50 lines)
2. Implement selection methods in `Grid` (~80 lines)
3. Add delegation methods in `Terminal` (~40 lines)
4. Add `handle_mouse()` to `src/app/input.rs` (~60 lines)
5. Add Shift+Arrow handling in `handle_terminal_key()` (~30 lines)
6. Update `TerminalWidget` to render selection (~40 lines)
7. Update `copy_terminal_selection()` (~20 lines)
8. Write tests for selection logic
9. Update documentation (`docs/hotkeys.md`)

### Phase 2: Theming System (Larger scope) - ~800-1000 lines new code
1. Create `src/theme/colors.rs` (~150 lines)
2. Create `src/theme/component.rs` (~200 lines)
3. Create `src/theme/preset.rs` (~150 lines)
4. Create `src/theme/persistence.rs` (~100 lines)
5. Create `src/theme/mod.rs` (~100 lines)
6. Extend `src/config/mod.rs` for theme parsing (~100 lines)
7. Update all UI widgets to accept theme (~150 lines across files)
8. Add command palette commands (~50 lines)
9. Implement ThemeSelector popup (~80 lines)
10. Implement ColorPicker popup (~100 lines)
11. Add per-shell theme support (~50 lines)
12. Add per-tab theme cycling (~50 lines)
13. Write tests
14. Update documentation (`docs/ratrc_docs.md`, `docs/hotkeys.md`)

---

## Documentation Updates

### `docs/ratrc_docs.md` - Add sections:
- Theme Configuration (global theme selection)
- Terminal Color Settings
- Editor Color Settings
- Status Bar Color Settings
- Tab Color Settings
- Per-Shell Theme Overrides
- Per-Tab Theme Patterns
- Color Format (hex, named colors)
- Examples for common customizations

### `docs/hotkeys.md` - Add sections:
- Terminal Selection
  - `Click+Drag` - Select text with mouse
  - `Shift+Left/Right` - Extend selection character by character
  - `Shift+Up/Down` - Extend selection by line
  - `Ctrl+Shift+C` - Copy selection
- Theme Commands (Command Palette)
  - Select Theme
  - Customize Colors
  - Reset Theme

---

## Line Count Compliance

Each new file will be kept under 500 lines:
- `src/theme/colors.rs` - ~150 lines
- `src/theme/component.rs` - ~200 lines
- `src/theme/preset.rs` - ~150 lines
- `src/theme/persistence.rs` - ~100 lines
- `src/theme/mod.rs` - ~100 lines

Modifications to existing files stay within limits by:
- Selection logic in grid.rs adds ~130 lines (grid.rs currently ~478 lines → ~480 after split)
- If grid.rs exceeds 500, split selection into `src/terminal/selection.rs`
