//! Tests for ANSI/VTE escape sequence parsing.
//!
//! Tests cover: cursor movement, colors, text attributes, screen operations.

use ratterm::terminal::ParsedAction;
use ratterm::terminal::parser::AnsiParser;
use ratterm::terminal::style::{Attr, Color};

/// Test parsing plain text (no escape sequences).
#[test]
fn test_parse_plain_text() {
    let mut parser = AnsiParser::new();
    let actions = parser.parse(b"Hello World");

    assert_eq!(actions.len(), 1, "Should produce one action");

    match &actions[0] {
        ParsedAction::Print(text) => {
            assert_eq!(text, "Hello World", "Text content mismatch");
        }
        _ => panic!("Expected Print action"),
    }
}

/// Test parsing mixed text and escape sequences.
#[test]
fn test_parse_mixed_content() {
    let mut parser = AnsiParser::new();
    // "Hello" + cursor forward 5 + "World"
    let input = b"Hello\x1b[5CWorld";
    let actions = parser.parse(input);

    assert!(actions.len() >= 3, "Should have text + escape + text");

    let mut found_hello = false;
    let mut found_cursor = false;
    let mut found_world = false;

    for action in &actions {
        match action {
            ParsedAction::Print(text) if text.contains("Hello") => found_hello = true,
            ParsedAction::Print(text) if text.contains("World") => found_world = true,
            ParsedAction::CursorForward(5) => found_cursor = true,
            _ => {}
        }
    }

    assert!(found_hello, "Should find 'Hello'");
    assert!(found_cursor, "Should find cursor forward");
    assert!(found_world, "Should find 'World'");
}

/// Test cursor movement escape sequences.
#[test]
fn test_cursor_movement_sequences() {
    let mut parser = AnsiParser::new();

    // Cursor Up: ESC[A or ESC[nA
    let actions = parser.parse(b"\x1b[A");
    assert!(matches!(actions.as_slice(), [ParsedAction::CursorUp(1)]));

    let actions = parser.parse(b"\x1b[5A");
    assert!(matches!(actions.as_slice(), [ParsedAction::CursorUp(5)]));

    // Cursor Down: ESC[B or ESC[nB
    let actions = parser.parse(b"\x1b[B");
    assert!(matches!(actions.as_slice(), [ParsedAction::CursorDown(1)]));

    let actions = parser.parse(b"\x1b[3B");
    assert!(matches!(actions.as_slice(), [ParsedAction::CursorDown(3)]));

    // Cursor Forward: ESC[C or ESC[nC
    let actions = parser.parse(b"\x1b[C");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::CursorForward(1)]
    ));

    let actions = parser.parse(b"\x1b[10C");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::CursorForward(10)]
    ));

    // Cursor Back: ESC[D or ESC[nD
    let actions = parser.parse(b"\x1b[D");
    assert!(matches!(actions.as_slice(), [ParsedAction::CursorBack(1)]));

    let actions = parser.parse(b"\x1b[7D");
    assert!(matches!(actions.as_slice(), [ParsedAction::CursorBack(7)]));
}

/// Test cursor position (CUP) escape sequence.
#[test]
fn test_cursor_position() {
    let mut parser = AnsiParser::new();

    // ESC[H - home position (1,1)
    let actions = parser.parse(b"\x1b[H");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::CursorPosition(1, 1)]
    ));

    // ESC[;H - also home
    let actions = parser.parse(b"\x1b[;H");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::CursorPosition(1, 1)]
    ));

    // ESC[5;10H - row 5, column 10
    let actions = parser.parse(b"\x1b[5;10H");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::CursorPosition(5, 10)]
    ));

    // ESC[12;1H - row 12, column 1
    let actions = parser.parse(b"\x1b[12;1H");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::CursorPosition(12, 1)]
    ));
}

/// Test SGR (Select Graphic Rendition) - text attributes.
#[test]
fn test_sgr_attributes() {
    let mut parser = AnsiParser::new();

    // Reset: ESC[0m or ESC[m
    let actions = parser.parse(b"\x1b[0m");
    assert!(matches!(actions.as_slice(), [ParsedAction::SetAttr(attr)] if attr.is_empty()));

    let actions = parser.parse(b"\x1b[m");
    assert!(matches!(actions.as_slice(), [ParsedAction::SetAttr(attr)] if attr.is_empty()));

    // Bold: ESC[1m
    let actions = parser.parse(b"\x1b[1m");
    match &actions[0] {
        ParsedAction::SetAttr(attrs) => {
            assert!(attrs.contains(&Attr::Bold), "Should set bold");
        }
        _ => panic!("Expected SetAttr"),
    }

    // Dim: ESC[2m
    let actions = parser.parse(b"\x1b[2m");
    match &actions[0] {
        ParsedAction::SetAttr(attrs) => {
            assert!(attrs.contains(&Attr::Dim), "Should set dim");
        }
        _ => panic!("Expected SetAttr"),
    }

    // Italic: ESC[3m
    let actions = parser.parse(b"\x1b[3m");
    match &actions[0] {
        ParsedAction::SetAttr(attrs) => {
            assert!(attrs.contains(&Attr::Italic), "Should set italic");
        }
        _ => panic!("Expected SetAttr"),
    }

    // Underline: ESC[4m
    let actions = parser.parse(b"\x1b[4m");
    match &actions[0] {
        ParsedAction::SetAttr(attrs) => {
            assert!(attrs.contains(&Attr::Underline), "Should set underline");
        }
        _ => panic!("Expected SetAttr"),
    }

    // Blink: ESC[5m
    let actions = parser.parse(b"\x1b[5m");
    match &actions[0] {
        ParsedAction::SetAttr(attrs) => {
            assert!(attrs.contains(&Attr::Blink), "Should set blink");
        }
        _ => panic!("Expected SetAttr"),
    }

    // Reverse: ESC[7m
    let actions = parser.parse(b"\x1b[7m");
    match &actions[0] {
        ParsedAction::SetAttr(attrs) => {
            assert!(attrs.contains(&Attr::Reverse), "Should set reverse");
        }
        _ => panic!("Expected SetAttr"),
    }
}

/// Test SGR foreground colors (30-37, 90-97).
#[test]
fn test_sgr_foreground_colors() {
    let mut parser = AnsiParser::new();

    // Standard colors 30-37
    let colors = [
        (30, Color::Black),
        (31, Color::Red),
        (32, Color::Green),
        (33, Color::Yellow),
        (34, Color::Blue),
        (35, Color::Magenta),
        (36, Color::Cyan),
        (37, Color::White),
    ];

    for (code, expected_color) in colors {
        let input = format!("\x1b[{code}m");
        let actions = parser.parse(input.as_bytes());
        match &actions[0] {
            ParsedAction::SetFg(color) => {
                assert_eq!(*color, expected_color, "Color {code} mismatch");
            }
            _ => panic!("Expected SetFg for code {code}"),
        }
    }

    // Bright colors 90-97
    let bright_colors = [
        (90, Color::BrightBlack),
        (91, Color::BrightRed),
        (92, Color::BrightGreen),
        (93, Color::BrightYellow),
        (94, Color::BrightBlue),
        (95, Color::BrightMagenta),
        (96, Color::BrightCyan),
        (97, Color::BrightWhite),
    ];

    for (code, expected_color) in bright_colors {
        let input = format!("\x1b[{code}m");
        let actions = parser.parse(input.as_bytes());
        match &actions[0] {
            ParsedAction::SetFg(color) => {
                assert_eq!(*color, expected_color, "Bright color {code} mismatch");
            }
            _ => panic!("Expected SetFg for code {code}"),
        }
    }
}

/// Test SGR background colors (40-47, 100-107).
#[test]
fn test_sgr_background_colors() {
    let mut parser = AnsiParser::new();

    // Standard background 40-47
    let colors = [
        (40, Color::Black),
        (41, Color::Red),
        (42, Color::Green),
        (43, Color::Yellow),
        (44, Color::Blue),
        (45, Color::Magenta),
        (46, Color::Cyan),
        (47, Color::White),
    ];

    for (code, expected_color) in colors {
        let input = format!("\x1b[{code}m");
        let actions = parser.parse(input.as_bytes());
        match &actions[0] {
            ParsedAction::SetBg(color) => {
                assert_eq!(*color, expected_color, "Background {code} mismatch");
            }
            _ => panic!("Expected SetBg for code {code}"),
        }
    }
}

/// Test 256-color mode (ESC[38;5;Nm and ESC[48;5;Nm).
#[test]
fn test_256_color_mode() {
    let mut parser = AnsiParser::new();

    // Foreground 256-color: ESC[38;5;Nm
    let actions = parser.parse(b"\x1b[38;5;196m");
    match &actions[0] {
        ParsedAction::SetFg(Color::Indexed(196)) => {}
        other => panic!("Expected SetFg(Indexed(196)), got {other:?}"),
    }

    // Background 256-color: ESC[48;5;Nm
    let actions = parser.parse(b"\x1b[48;5;21m");
    match &actions[0] {
        ParsedAction::SetBg(Color::Indexed(21)) => {}
        other => panic!("Expected SetBg(Indexed(21)), got {other:?}"),
    }
}

/// Test true color (24-bit) mode.
#[test]
fn test_true_color_mode() {
    let mut parser = AnsiParser::new();

    // Foreground RGB: ESC[38;2;R;G;Bm
    let actions = parser.parse(b"\x1b[38;2;255;128;64m");
    match &actions[0] {
        ParsedAction::SetFg(Color::Rgb(255, 128, 64)) => {}
        other => panic!("Expected SetFg(Rgb(255,128,64)), got {other:?}"),
    }

    // Background RGB: ESC[48;2;R;G;Bm
    let actions = parser.parse(b"\x1b[48;2;0;255;0m");
    match &actions[0] {
        ParsedAction::SetBg(Color::Rgb(0, 255, 0)) => {}
        other => panic!("Expected SetBg(Rgb(0,255,0)), got {other:?}"),
    }
}

/// Test combined SGR parameters.
#[test]
fn test_combined_sgr() {
    let mut parser = AnsiParser::new();

    // Bold + Red foreground: ESC[1;31m
    let actions = parser.parse(b"\x1b[1;31m");

    let mut found_bold = false;
    let mut found_red = false;

    for action in &actions {
        match action {
            ParsedAction::SetAttr(attrs) if attrs.contains(&Attr::Bold) => found_bold = true,
            ParsedAction::SetFg(Color::Red) => found_red = true,
            _ => {}
        }
    }

    assert!(found_bold, "Should set bold attribute");
    assert!(found_red, "Should set red foreground");
}

/// Test erase display sequences.
#[test]
fn test_erase_display() {
    let mut parser = AnsiParser::new();

    // ESC[J or ESC[0J - clear from cursor to end of screen
    let actions = parser.parse(b"\x1b[J");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::EraseDisplay(0)]
    ));

    let actions = parser.parse(b"\x1b[0J");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::EraseDisplay(0)]
    ));

    // ESC[1J - clear from cursor to beginning of screen
    let actions = parser.parse(b"\x1b[1J");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::EraseDisplay(1)]
    ));

    // ESC[2J - clear entire screen
    let actions = parser.parse(b"\x1b[2J");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::EraseDisplay(2)]
    ));

    // ESC[3J - clear entire screen + scrollback
    let actions = parser.parse(b"\x1b[3J");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::EraseDisplay(3)]
    ));
}

/// Test erase line sequences.
#[test]
fn test_erase_line() {
    let mut parser = AnsiParser::new();

    // ESC[K or ESC[0K - clear from cursor to end of line
    let actions = parser.parse(b"\x1b[K");
    assert!(matches!(actions.as_slice(), [ParsedAction::EraseLine(0)]));

    // ESC[1K - clear from cursor to beginning of line
    let actions = parser.parse(b"\x1b[1K");
    assert!(matches!(actions.as_slice(), [ParsedAction::EraseLine(1)]));

    // ESC[2K - clear entire line
    let actions = parser.parse(b"\x1b[2K");
    assert!(matches!(actions.as_slice(), [ParsedAction::EraseLine(2)]));
}

/// Test scroll up/down sequences.
#[test]
fn test_scroll_sequences() {
    let mut parser = AnsiParser::new();

    // Scroll up: ESC[S or ESC[nS
    let actions = parser.parse(b"\x1b[S");
    assert!(matches!(actions.as_slice(), [ParsedAction::ScrollUp(1)]));

    let actions = parser.parse(b"\x1b[5S");
    assert!(matches!(actions.as_slice(), [ParsedAction::ScrollUp(5)]));

    // Scroll down: ESC[T or ESC[nT
    let actions = parser.parse(b"\x1b[T");
    assert!(matches!(actions.as_slice(), [ParsedAction::ScrollDown(1)]));

    let actions = parser.parse(b"\x1b[3T");
    assert!(matches!(actions.as_slice(), [ParsedAction::ScrollDown(3)]));
}

/// Test save/restore cursor sequences.
#[test]
fn test_cursor_save_restore() {
    let mut parser = AnsiParser::new();

    // Save cursor: ESC[s or ESC7
    let actions = parser.parse(b"\x1b[s");
    assert!(matches!(actions.as_slice(), [ParsedAction::SaveCursor]));

    let actions = parser.parse(b"\x1b7");
    assert!(matches!(actions.as_slice(), [ParsedAction::SaveCursor]));

    // Restore cursor: ESC[u or ESC8
    let actions = parser.parse(b"\x1b[u");
    assert!(matches!(actions.as_slice(), [ParsedAction::RestoreCursor]));

    let actions = parser.parse(b"\x1b8");
    assert!(matches!(actions.as_slice(), [ParsedAction::RestoreCursor]));
}

/// Test show/hide cursor sequences.
#[test]
fn test_cursor_visibility() {
    let mut parser = AnsiParser::new();

    // Hide cursor: ESC[?25l
    let actions = parser.parse(b"\x1b[?25l");
    assert!(matches!(actions.as_slice(), [ParsedAction::HideCursor]));

    // Show cursor: ESC[?25h
    let actions = parser.parse(b"\x1b[?25h");
    assert!(matches!(actions.as_slice(), [ParsedAction::ShowCursor]));
}

/// Test alternate screen buffer sequences.
#[test]
fn test_alternate_screen() {
    let mut parser = AnsiParser::new();

    // Enter alternate screen: ESC[?1049h
    let actions = parser.parse(b"\x1b[?1049h");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::EnterAlternateScreen]
    ));

    // Exit alternate screen: ESC[?1049l
    let actions = parser.parse(b"\x1b[?1049l");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::ExitAlternateScreen]
    ));
}

/// Test control characters (C0).
#[test]
fn test_control_characters() {
    let mut parser = AnsiParser::new();

    // Bell (BEL, 0x07)
    let actions = parser.parse(b"\x07");
    assert!(matches!(actions.as_slice(), [ParsedAction::Bell]));

    // Backspace (BS, 0x08)
    let actions = parser.parse(b"\x08");
    assert!(matches!(actions.as_slice(), [ParsedAction::Backspace]));

    // Tab (HT, 0x09)
    let actions = parser.parse(b"\x09");
    assert!(matches!(actions.as_slice(), [ParsedAction::Tab]));

    // Newline (LF, 0x0A)
    let actions = parser.parse(b"\x0A");
    assert!(matches!(actions.as_slice(), [ParsedAction::LineFeed]));

    // Carriage return (CR, 0x0D)
    let actions = parser.parse(b"\x0D");
    assert!(matches!(actions.as_slice(), [ParsedAction::CarriageReturn]));
}

/// Test OSC (Operating System Command) sequences.
#[test]
fn test_osc_sequences() {
    let mut parser = AnsiParser::new();

    // Set window title: OSC 0;title ST or OSC 2;title ST
    let actions = parser.parse(b"\x1b]0;My Title\x07");
    match &actions[0] {
        ParsedAction::SetTitle(title) => {
            assert_eq!(title, "My Title", "Title mismatch");
        }
        _ => panic!("Expected SetTitle"),
    }

    // Using ST (ESC \) instead of BEL
    let actions = parser.parse(b"\x1b]2;Another Title\x1b\\");
    match &actions[0] {
        ParsedAction::SetTitle(title) => {
            assert_eq!(title, "Another Title", "Title mismatch");
        }
        _ => panic!("Expected SetTitle"),
    }
}

/// Test cursor shape sequences.
#[test]
fn test_cursor_shape() {
    let mut parser = AnsiParser::new();

    // Block cursor: ESC[0 q or ESC[2 q
    let actions = parser.parse(b"\x1b[2 q");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::SetCursorShape(0 | 2)]
    ));

    // Underline cursor: ESC[4 q
    let actions = parser.parse(b"\x1b[4 q");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::SetCursorShape(4)]
    ));

    // Bar cursor: ESC[6 q
    let actions = parser.parse(b"\x1b[6 q");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::SetCursorShape(6)]
    ));
}

/// Test insert/delete line sequences.
#[test]
fn test_insert_delete_lines() {
    let mut parser = AnsiParser::new();

    // Insert lines: ESC[L or ESC[nL
    let actions = parser.parse(b"\x1b[L");
    assert!(matches!(actions.as_slice(), [ParsedAction::InsertLines(1)]));

    let actions = parser.parse(b"\x1b[5L");
    assert!(matches!(actions.as_slice(), [ParsedAction::InsertLines(5)]));

    // Delete lines: ESC[M or ESC[nM
    let actions = parser.parse(b"\x1b[M");
    assert!(matches!(actions.as_slice(), [ParsedAction::DeleteLines(1)]));

    let actions = parser.parse(b"\x1b[3M");
    assert!(matches!(actions.as_slice(), [ParsedAction::DeleteLines(3)]));
}

/// Test insert/delete characters sequences.
#[test]
fn test_insert_delete_chars() {
    let mut parser = AnsiParser::new();

    // Insert chars: ESC[@ or ESC[n@
    let actions = parser.parse(b"\x1b[@");
    assert!(matches!(actions.as_slice(), [ParsedAction::InsertChars(1)]));

    // Delete chars: ESC[P or ESC[nP
    let actions = parser.parse(b"\x1b[P");
    assert!(matches!(actions.as_slice(), [ParsedAction::DeleteChars(1)]));

    let actions = parser.parse(b"\x1b[10P");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::DeleteChars(10)]
    ));
}

/// Test reporting sequences.
#[test]
fn test_device_status_report() {
    let mut parser = AnsiParser::new();

    // Device status report: ESC[6n (request cursor position)
    let actions = parser.parse(b"\x1b[6n");
    assert!(matches!(
        actions.as_slice(),
        [ParsedAction::DeviceStatusReport]
    ));
}

/// Test partial sequence handling (incomplete escape sequence).
#[test]
fn test_partial_sequence() {
    let mut parser = AnsiParser::new();

    // Send incomplete sequence
    let actions1 = parser.parse(b"\x1b[");
    assert!(
        actions1.is_empty(),
        "Incomplete sequence should not produce actions"
    );

    // Complete the sequence
    let actions2 = parser.parse(b"5A");
    assert!(matches!(actions2.as_slice(), [ParsedAction::CursorUp(5)]));
}

/// Test invalid/unknown sequences.
#[test]
fn test_invalid_sequences() {
    let mut parser = AnsiParser::new();

    // Unknown CSI sequence should be ignored or produce Unknown action
    let actions = parser.parse(b"\x1b[999z");

    // Should not panic, may produce Unknown or be ignored
    for action in &actions {
        match action {
            ParsedAction::Unknown(_) => {} // OK
            _ => {}                        // Also OK if ignored
        }
    }
}

/// Test UTF-8 text parsing.
#[test]
fn test_utf8_text() {
    let mut parser = AnsiParser::new();

    let actions = parser.parse("Hello 世界 🌍".as_bytes());

    let mut text = String::new();
    for action in &actions {
        if let ParsedAction::Print(s) = action {
            text.push_str(s);
        }
    }

    assert_eq!(text, "Hello 世界 🌍", "UTF-8 text should be preserved");
}

/// Test hyperlink OSC sequence.
#[test]
fn test_hyperlink_osc() {
    let mut parser = AnsiParser::new();

    // OSC 8;;URL ST text OSC 8;; ST
    let input = b"\x1b]8;;https://example.com\x1b\\Click here\x1b]8;;\x1b\\";
    let actions = parser.parse(input);

    let mut found_link = false;
    for action in &actions {
        if let ParsedAction::Hyperlink { url, .. } = action {
            if url == "https://example.com" {
                found_link = true;
            }
        }
    }

    assert!(found_link, "Should parse hyperlink OSC");
}
