//! ANSI escape sequences for keyboard input.
//!
//! These match what a real terminal emulator sends when keys are pressed.
//! crossterm parses these on the receiving end.

// === Modifier keys ===

/// Escape key
pub const ESC: &str = "\x1b";

/// Enter / Return
pub const ENTER: &str = "\r";

/// Tab
pub const TAB: &str = "\t";

/// Backspace (DEL byte)
pub const BACKSPACE: &str = "\x7f";

// === Control keys (Ctrl+<letter> = letter & 0x1f) ===

pub const CTRL_A: &str = "\x01";
pub const CTRL_B: &str = "\x02";
pub const CTRL_C: &str = "\x03";
pub const CTRL_D: &str = "\x04";
pub const CTRL_E: &str = "\x05";
pub const CTRL_F: &str = "\x06";
pub const CTRL_H: &str = "\x08";
pub const CTRL_I: &str = "\x09"; // Also Tab
pub const CTRL_K: &str = "\x0b";
pub const CTRL_L: &str = "\x0c";
pub const CTRL_N: &str = "\x0e";
pub const CTRL_O: &str = "\x0f";
pub const CTRL_P: &str = "\x10";
pub const CTRL_Q: &str = "\x11";
pub const CTRL_R: &str = "\x12";
pub const CTRL_S: &str = "\x13";
pub const CTRL_T: &str = "\x14";
pub const CTRL_V: &str = "\x16";
pub const CTRL_W: &str = "\x17";
pub const CTRL_X: &str = "\x18";
pub const CTRL_Y: &str = "\x19";
pub const CTRL_Z: &str = "\x1a";

// === Arrow keys (standard xterm sequences) ===

pub const UP: &str = "\x1b[A";
pub const DOWN: &str = "\x1b[B";
pub const RIGHT: &str = "\x1b[C";
pub const LEFT: &str = "\x1b[D";

// === Navigation keys ===

pub const HOME: &str = "\x1b[H";
pub const END: &str = "\x1b[F";
pub const PAGE_UP: &str = "\x1b[5~";
pub const PAGE_DOWN: &str = "\x1b[6~";
pub const DELETE: &str = "\x1b[3~";
pub const INSERT: &str = "\x1b[2~";

// === Shift+Arrow (xterm modifier encoding) ===

pub const SHIFT_UP: &str = "\x1b[1;2A";
pub const SHIFT_DOWN: &str = "\x1b[1;2B";
pub const SHIFT_RIGHT: &str = "\x1b[1;2C";
pub const SHIFT_LEFT: &str = "\x1b[1;2D";

// === Ctrl+Arrow (xterm modifier encoding) ===

pub const CTRL_LEFT: &str = "\x1b[1;5D";
pub const CTRL_RIGHT: &str = "\x1b[1;5C";
pub const CTRL_UP: &str = "\x1b[1;5A";
pub const CTRL_DOWN: &str = "\x1b[1;5B";

// === Alt+Arrow (xterm modifier encoding) ===

pub const ALT_LEFT: &str = "\x1b[1;3D";
pub const ALT_RIGHT: &str = "\x1b[1;3C";
pub const ALT_UP: &str = "\x1b[1;3A";
pub const ALT_DOWN: &str = "\x1b[1;3B";

// === Alt+Key combinations ===

pub const ALT_TAB: &str = "\x1b\t";
/// Alt+[ — note: raw ESC+[ is CSI prefix, so we use the xterm encoding
pub const ALT_OPEN_BRACKET: &str = "\x1b[91;3~";
/// Alt+]
pub const ALT_CLOSE_BRACKET: &str = "\x1b]";

// === Shift+PageUp/Down (for terminal scrollback) ===

pub const SHIFT_PAGE_UP: &str = "\x1b[5;2~";
pub const SHIFT_PAGE_DOWN: &str = "\x1b[6;2~";

// === Ctrl+Shift combinations ===
// These use CSI u encoding or xterm modifyOtherKeys

pub const CTRL_SHIFT_P: &str = "\x1b[112;6u";
pub const CTRL_SHIFT_C: &str = "\x1b[99;6u";
pub const CTRL_SHIFT_S: &str = "\x1b[115;6u";
pub const CTRL_SHIFT_W: &str = "\x1b[119;6u";
pub const CTRL_SHIFT_F: &str = "\x1b[102;6u";
pub const CTRL_SHIFT_D: &str = "\x1b[100;6u";
pub const CTRL_SHIFT_E: &str = "\x1b[101;6u";
pub const CTRL_SHIFT_N: &str = "\x1b[110;6u";
/// Shift+Tab / Ctrl+Shift+Tab (reverse tab)
pub const CTRL_SHIFT_TAB: &str = "\x1b[Z";

// === Alt+Shift combinations ===

pub const ALT_SHIFT_LEFT: &str = "\x1b[1;4D";
pub const ALT_SHIFT_RIGHT: &str = "\x1b[1;4C";

// === Function keys ===

pub const F1: &str = "\x1bOP";
pub const F2: &str = "\x1bOQ";
pub const F3: &str = "\x1bOR";
pub const F4: &str = "\x1bOS";
pub const F5: &str = "\x1b[15~";

// === Ctrl+Tab (requires special CSI encoding) ===

pub const CTRL_TAB: &str = "\x1b[9;5u";

/// Build a concatenated key sequence from multiple key strings.
pub fn key_sequence(keys: &[&str]) -> String {
    keys.join("")
}
