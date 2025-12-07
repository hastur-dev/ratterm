//! Key to terminal bytes conversion.
//!
//! Converts crossterm key events to terminal escape sequences.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Converts a key event to bytes for the terminal.
pub fn key_to_bytes(key: KeyEvent) -> Vec<u8> {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            c.to_string().into_bytes()
        }
        (KeyModifiers::CONTROL, KeyCode::Char(c)) => {
            let ctrl = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a' - 1);
            vec![ctrl]
        }
        (KeyModifiers::NONE, KeyCode::Enter) => vec![b'\r'],
        (KeyModifiers::NONE, KeyCode::Backspace) => vec![0x7f],
        (KeyModifiers::NONE, KeyCode::Tab) => vec![b'\t'],
        (KeyModifiers::NONE, KeyCode::Esc) => vec![0x1b],
        (KeyModifiers::NONE, KeyCode::Up) => b"\x1b[A".to_vec(),
        (KeyModifiers::NONE, KeyCode::Down) => b"\x1b[B".to_vec(),
        (KeyModifiers::NONE, KeyCode::Right) => b"\x1b[C".to_vec(),
        (KeyModifiers::NONE, KeyCode::Left) => b"\x1b[D".to_vec(),
        (KeyModifiers::NONE, KeyCode::Home) => b"\x1b[H".to_vec(),
        (KeyModifiers::NONE, KeyCode::End) => b"\x1b[F".to_vec(),
        (KeyModifiers::NONE, KeyCode::PageUp) => b"\x1b[5~".to_vec(),
        (KeyModifiers::NONE, KeyCode::PageDown) => b"\x1b[6~".to_vec(),
        (KeyModifiers::NONE, KeyCode::Insert) => b"\x1b[2~".to_vec(),
        (KeyModifiers::NONE, KeyCode::Delete) => b"\x1b[3~".to_vec(),
        (KeyModifiers::NONE, KeyCode::F(n)) => function_key_bytes(n),
        _ => Vec::new(),
    }
}

/// Returns bytes for function key.
fn function_key_bytes(n: u8) -> Vec<u8> {
    match n {
        1 => b"\x1bOP".to_vec(),
        2 => b"\x1bOQ".to_vec(),
        3 => b"\x1bOR".to_vec(),
        4 => b"\x1bOS".to_vec(),
        5 => b"\x1b[15~".to_vec(),
        6 => b"\x1b[17~".to_vec(),
        7 => b"\x1b[18~".to_vec(),
        8 => b"\x1b[19~".to_vec(),
        9 => b"\x1b[20~".to_vec(),
        10 => b"\x1b[21~".to_vec(),
        11 => b"\x1b[23~".to_vec(),
        12 => b"\x1b[24~".to_vec(),
        _ => Vec::new(),
    }
}
