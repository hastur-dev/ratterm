//! Parsed terminal actions from ANSI escape sequences.
//!
//! Defines all possible actions that can result from parsing terminal input.

use super::style::{Attr, Color};

/// Parsed terminal action.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedAction {
    /// Print text.
    Print(String),
    /// Cursor up by n rows.
    CursorUp(u16),
    /// Cursor down by n rows.
    CursorDown(u16),
    /// Cursor forward by n columns.
    CursorForward(u16),
    /// Cursor back by n columns.
    CursorBack(u16),
    /// Set cursor position (1-indexed row, col).
    CursorPosition(u16, u16),
    /// Set text attributes.
    SetAttr(Vec<Attr>),
    /// Set foreground color.
    SetFg(Color),
    /// Set background color.
    SetBg(Color),
    /// Erase display (mode: 0=to end, 1=to start, 2=all, 3=all+scrollback).
    EraseDisplay(u8),
    /// Erase line (mode: 0=to end, 1=to start, 2=all).
    EraseLine(u8),
    /// Scroll up by n lines.
    ScrollUp(u16),
    /// Scroll down by n lines.
    ScrollDown(u16),
    /// Save cursor position.
    SaveCursor,
    /// Restore cursor position.
    RestoreCursor,
    /// Hide cursor.
    HideCursor,
    /// Show cursor.
    ShowCursor,
    /// Enter alternate screen.
    EnterAlternateScreen,
    /// Exit alternate screen.
    ExitAlternateScreen,
    /// Bell.
    Bell,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
    /// Line feed.
    LineFeed,
    /// Carriage return.
    CarriageReturn,
    /// Set window title.
    SetTitle(String),
    /// Set cursor shape (DECSCUSR).
    SetCursorShape(u8),
    /// Insert lines.
    InsertLines(u16),
    /// Delete lines.
    DeleteLines(u16),
    /// Insert characters.
    InsertChars(u16),
    /// Delete characters.
    DeleteChars(u16),
    /// Device status report request.
    DeviceStatusReport,
    /// Hyperlink.
    Hyperlink { url: String, id: Option<String> },
    /// Unknown sequence.
    Unknown(String),
}
