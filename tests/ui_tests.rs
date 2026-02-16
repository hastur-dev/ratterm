//! Integration tests for UI widgets.
//!
//! Tests command palette, SSH manager, Docker manager, and visual consistency.
//! Includes tests verifying all hotkey hints render correctly.

use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

use ratterm::ui::key_hint_bar::{KeyHint, KeyHintBar, KeyHintStyle};
use ratterm::ui::manager_footer::ManagerFooter;
use ratterm::ui::popup::{Command, CommandPalette};

// ============================================================================
// Command Palette Tests (Phase 3)
// ============================================================================

#[test]
fn test_command_palette_category_colors() {
    let test_cases = vec![
        ("File", Color::Blue),
        ("Edit", Color::Green),
        ("Search", Color::Yellow),
        ("View", Color::Cyan),
        ("Terminal", Color::Green),
        ("SSH", Color::Cyan),
        ("Docker", Color::Magenta),
        ("Theme", Color::LightYellow),
        ("Extension", Color::LightBlue),
        ("Application", Color::Gray),
        ("Unknown", Color::DarkGray),
    ];

    for (category, expected_color) in test_cases {
        let cmd = Command::new("test.cmd", "Test", category, None);
        assert_eq!(
            cmd.category_color(),
            expected_color,
            "Category '{}' should map to {:?}",
            category,
            expected_color
        );
    }
}

#[test]
fn test_command_palette_filter_results() {
    let mut palette = CommandPalette::new();

    palette.filter("docker");
    let results = palette.results();
    // All results should be Docker-related
    for result in &results {
        let lower = result.to_lowercase();
        assert!(
            lower.contains("docker"),
            "Filtering 'docker' should only return Docker results, got: {}",
            result
        );
    }
    assert!(!results.is_empty(), "Should have Docker results");
}

#[test]
fn test_command_palette_keybinding_display() {
    let cmd_with_key = Command::new("test.save", "Save", "File", Some("Ctrl+S"));
    let display = cmd_with_key.display();
    assert!(
        display.contains("Ctrl+S"),
        "Should show keybinding: {}",
        display
    );

    let cmd_without_key = Command::new("test.scan", "Scan", "SSH", None);
    let display = cmd_without_key.display();
    assert!(
        !display.contains("("),
        "Should not show keybinding parens: {}",
        display
    );
}

#[test]
fn test_command_palette_all_commands_have_valid_categories() {
    let palette = CommandPalette::new();
    let results = palette.results();

    for result in &results {
        // Each result should have a category prefix
        assert!(
            result.contains(':'),
            "Command should have category prefix: {}",
            result
        );
    }
}

#[test]
fn test_command_palette_no_duplicate_ids() {
    let palette = CommandPalette::new();
    let mut seen_ids = std::collections::HashSet::new();

    // Get commands by filtering empty
    let count = palette.len();
    for i in 0..count {
        if let Some(cmd) = palette.get_command(i) {
            assert!(seen_ids.insert(cmd.id), "Duplicate command ID: {}", cmd.id);
        }
    }
}

// ============================================================================
// Helper: render widget to buffer and extract row text
// ============================================================================

fn render_to_rows(width: u16, height: u16, widget: impl Widget) -> Vec<String> {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| {
                    buf.cell((x, y))
                        .map(|c| c.symbol().chars().next().unwrap_or(' '))
                        .unwrap_or(' ')
                })
                .collect::<String>()
        })
        .collect()
}

// ============================================================================
// Help Bar: Terminal-focused hotkey tests (all 7 hints)
// ============================================================================

#[test]
fn test_help_bar_terminal_hints_palette() {
    let hints = vec![
        KeyHint::styled("Ctrl+Shift+P", "Palette", KeyHintStyle::Highlighted),
        KeyHint::new("Ctrl+Shift+U", "SSH"),
        KeyHint::new("Ctrl+Shift+D", "Docker"),
        KeyHint::new("Ctrl+T", "New Tab"),
        KeyHint::new("Ctrl+S", "Split"),
        KeyHint::new("Alt+Tab", "Switch Pane"),
        KeyHint::styled("Ctrl+Q", "Quit", KeyHintStyle::Danger),
    ];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(160, 1, bar);
    assert!(rows[0].contains("Palette"), "Should contain 'Palette'");
    assert!(
        rows[0].contains("Ctrl+Shift+P"),
        "Should contain 'Ctrl+Shift+P'"
    );
}

#[test]
fn test_help_bar_terminal_hints_ssh() {
    let hints = vec![
        KeyHint::styled("Ctrl+Shift+P", "Palette", KeyHintStyle::Highlighted),
        KeyHint::new("Ctrl+Shift+U", "SSH"),
    ];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(120, 1, bar);
    assert!(rows[0].contains("SSH"), "Should contain 'SSH'");
    assert!(
        rows[0].contains("Ctrl+Shift+U"),
        "Should contain 'Ctrl+Shift+U'"
    );
}

#[test]
fn test_help_bar_terminal_hints_docker() {
    let hints = vec![
        KeyHint::styled("Ctrl+Shift+P", "Palette", KeyHintStyle::Highlighted),
        KeyHint::new("Ctrl+Shift+U", "SSH"),
        KeyHint::new("Ctrl+Shift+D", "Docker"),
    ];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(120, 1, bar);
    assert!(rows[0].contains("Docker"), "Should contain 'Docker'");
    assert!(
        rows[0].contains("Ctrl+Shift+D"),
        "Should contain 'Ctrl+Shift+D'"
    );
}

#[test]
fn test_help_bar_terminal_hints_new_tab() {
    let hints = vec![
        KeyHint::new("Ctrl+T", "New Tab"),
        KeyHint::new("Ctrl+S", "Split"),
    ];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(80, 1, bar);
    assert!(rows[0].contains("New Tab"), "Should contain 'New Tab'");
    assert!(rows[0].contains("Ctrl+T"), "Should contain 'Ctrl+T'");
}

#[test]
fn test_help_bar_terminal_hints_split() {
    let hints = vec![
        KeyHint::new("Ctrl+T", "New Tab"),
        KeyHint::new("Ctrl+S", "Split"),
    ];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(80, 1, bar);
    assert!(rows[0].contains("Split"), "Should contain 'Split'");
    assert!(rows[0].contains("Ctrl+S"), "Should contain 'Ctrl+S'");
}

#[test]
fn test_help_bar_terminal_hints_switch_pane() {
    let hints = vec![KeyHint::new("Alt+Tab", "Switch Pane")];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(80, 1, bar);
    assert!(
        rows[0].contains("Switch Pane"),
        "Should contain 'Switch Pane'"
    );
    assert!(rows[0].contains("Alt+Tab"), "Should contain 'Alt+Tab'");
}

#[test]
fn test_help_bar_terminal_hints_quit() {
    let hints = vec![KeyHint::styled("Ctrl+Q", "Quit", KeyHintStyle::Danger)];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(80, 1, bar);
    assert!(rows[0].contains("Quit"), "Should contain 'Quit'");
    assert!(rows[0].contains("Ctrl+Q"), "Should contain 'Ctrl+Q'");
}

// ============================================================================
// Help Bar: Editor-focused hotkey tests (all 6 hints)
// ============================================================================

#[test]
fn test_help_bar_editor_hints_open() {
    let hints = vec![
        KeyHint::styled("Ctrl+Shift+P", "Palette", KeyHintStyle::Highlighted),
        KeyHint::new("Ctrl+O", "Open"),
    ];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(120, 1, bar);
    assert!(rows[0].contains("Open"), "Should contain 'Open'");
    assert!(rows[0].contains("Ctrl+O"), "Should contain 'Ctrl+O'");
}

#[test]
fn test_help_bar_editor_hints_save() {
    let hints = vec![KeyHint::new("Ctrl+S", "Save")];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(80, 1, bar);
    assert!(rows[0].contains("Save"), "Should contain 'Save'");
    assert!(rows[0].contains("Ctrl+S"), "Should contain 'Ctrl+S'");
}

#[test]
fn test_help_bar_editor_hints_find() {
    let hints = vec![KeyHint::new("Ctrl+F", "Find")];
    let bar = KeyHintBar::new(hints);
    let rows = render_to_rows(80, 1, bar);
    assert!(rows[0].contains("Find"), "Should contain 'Find'");
    assert!(rows[0].contains("Ctrl+F"), "Should contain 'Ctrl+F'");
}

// ============================================================================
// SSH Manager: Primary row hotkey tests (Enter, a, e, d, s)
// ============================================================================

#[test]
fn test_ssh_primary_hints_all_present() {
    let primary = vec![
        KeyHint::styled("Enter", "Connect", KeyHintStyle::Success),
        KeyHint::new("a", "Add Host"),
        KeyHint::new("e", "Edit"),
        KeyHint::styled("d", "Delete", KeyHintStyle::Danger),
        KeyHint::new("s", "Scan Network"),
    ];
    let secondary = vec![
        KeyHint::new("c", "Credential Scan"),
        KeyHint::new("Shift+S", "Scan Subnet"),
        KeyHint::new("Ctrl+1-9", "Quick Connect"),
        KeyHint::styled("Esc", "Close", KeyHintStyle::Danger),
    ];
    let footer = ManagerFooter::new(primary).secondary(secondary);
    let rows = render_to_rows(120, 2, footer);

    // Primary row
    assert!(rows[0].contains("Enter"), "Row 1: 'Enter'");
    assert!(rows[0].contains("Connect"), "Row 1: 'Connect'");
    assert!(rows[0].contains("Add Host"), "Row 1: 'Add Host'");
    assert!(rows[0].contains("Edit"), "Row 1: 'Edit'");
    assert!(rows[0].contains("Delete"), "Row 1: 'Delete'");
    assert!(rows[0].contains("Scan Network"), "Row 1: 'Scan Network'");
}

// ============================================================================
// SSH Manager: Secondary row hotkey tests (c, Shift+S, Ctrl+1-9, Esc)
// ============================================================================

#[test]
fn test_ssh_secondary_hints_all_present() {
    let primary = vec![
        KeyHint::styled("Enter", "Connect", KeyHintStyle::Success),
        KeyHint::new("a", "Add Host"),
    ];
    let secondary = vec![
        KeyHint::new("c", "Credential Scan"),
        KeyHint::new("Shift+S", "Scan Subnet"),
        KeyHint::new("Ctrl+1-9", "Quick Connect"),
        KeyHint::styled("Esc", "Close", KeyHintStyle::Danger),
    ];
    let footer = ManagerFooter::new(primary).secondary(secondary);
    let rows = render_to_rows(120, 2, footer);

    // Secondary row
    assert!(
        rows[1].contains("Credential Scan"),
        "Row 2: 'Credential Scan'"
    );
    assert!(rows[1].contains("Scan Subnet"), "Row 2: 'Scan Subnet'");
    assert!(rows[1].contains("Quick Connect"), "Row 2: 'Quick Connect'");
    assert!(rows[1].contains("Shift+S"), "Row 2: 'Shift+S'");
    assert!(rows[1].contains("Ctrl+1-9"), "Row 2: 'Ctrl+1-9'");
    assert!(rows[1].contains("Close"), "Row 2: 'Close'");
    assert!(rows[1].contains("Esc"), "Row 2: 'Esc'");
}

// ============================================================================
// SSH Manager: Credential entry footer hotkey tests (Tab, Space, Enter, Esc)
// ============================================================================

#[test]
fn test_ssh_credential_entry_hints_all_present() {
    let cred_hints = vec![
        KeyHint::new("Tab", "Next Field"),
        KeyHint::new("Space", "Toggle Save"),
        KeyHint::styled("Enter", "Connect", KeyHintStyle::Success),
        KeyHint::styled("Esc", "Cancel", KeyHintStyle::Danger),
    ];
    let footer = ManagerFooter::new(cred_hints);
    let rows = render_to_rows(120, 1, footer);

    assert!(rows[0].contains("Tab"), "Cred: 'Tab'");
    assert!(rows[0].contains("Next Field"), "Cred: 'Next Field'");
    assert!(rows[0].contains("Space"), "Cred: 'Space'");
    assert!(rows[0].contains("Toggle Save"), "Cred: 'Toggle Save'");
    assert!(rows[0].contains("Enter"), "Cred: 'Enter'");
    assert!(rows[0].contains("Connect"), "Cred: 'Connect'");
    assert!(rows[0].contains("Esc"), "Cred: 'Esc'");
    assert!(rows[0].contains("Cancel"), "Cred: 'Cancel'");
}

// ============================================================================
// Docker Manager: Primary row hotkey tests (Enter, s, S, r, n, R)
// ============================================================================

#[test]
fn test_docker_primary_hints_all_present() {
    let primary = vec![
        KeyHint::styled("Enter", "Attach", KeyHintStyle::Success),
        KeyHint::new("s", "Start"),
        KeyHint::styled("S", "Stop", KeyHintStyle::Danger),
        KeyHint::new("r", "Restart"),
        KeyHint::new("n", "New Container"),
        KeyHint::new("R", "Refresh"),
    ];
    let secondary = vec![
        KeyHint::new("Tab", "Switch Section"),
        KeyHint::new("Ctrl+Alt+1-9", "Quick Connect"),
        KeyHint::styled("Esc", "Close", KeyHintStyle::Danger),
    ];
    let footer = ManagerFooter::new(primary).secondary(secondary);
    let rows = render_to_rows(160, 2, footer);

    // Primary row
    assert!(rows[0].contains("Enter"), "Row 1: 'Enter'");
    assert!(rows[0].contains("Attach"), "Row 1: 'Attach'");
    assert!(rows[0].contains("Start"), "Row 1: 'Start'");
    assert!(rows[0].contains("Stop"), "Row 1: 'Stop'");
    assert!(rows[0].contains("Restart"), "Row 1: 'Restart'");
    assert!(rows[0].contains("New Container"), "Row 1: 'New Container'");
    assert!(rows[0].contains("Refresh"), "Row 1: 'Refresh'");
}

// ============================================================================
// Docker Manager: Secondary row hotkey tests (Tab, Ctrl+Alt+1-9, Esc)
// ============================================================================

#[test]
fn test_docker_secondary_hints_all_present() {
    let primary = vec![KeyHint::styled("Enter", "Attach", KeyHintStyle::Success)];
    let secondary = vec![
        KeyHint::new("Tab", "Switch Section"),
        KeyHint::new("Ctrl+Alt+1-9", "Quick Connect"),
        KeyHint::styled("Esc", "Close", KeyHintStyle::Danger),
    ];
    let footer = ManagerFooter::new(primary).secondary(secondary);
    let rows = render_to_rows(120, 2, footer);

    // Secondary row
    assert!(
        rows[1].contains("Switch Section"),
        "Row 2: 'Switch Section'"
    );
    assert!(rows[1].contains("Tab"), "Row 2: 'Tab'");
    assert!(rows[1].contains("Quick Connect"), "Row 2: 'Quick Connect'");
    assert!(rows[1].contains("Ctrl+Alt+1-9"), "Row 2: 'Ctrl+Alt+1-9'");
    assert!(rows[1].contains("Close"), "Row 2: 'Close'");
    assert!(rows[1].contains("Esc"), "Row 2: 'Esc'");
}
