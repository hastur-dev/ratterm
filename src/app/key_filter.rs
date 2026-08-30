//! Key event de-duplication filter.
//!
//! Windows consoles report every keystroke twice: once as
//! [`KeyEventKind::Press`] and once as [`KeyEventKind::Release`]. The same is
//! true on Unix terminals that negotiate the kitty keyboard protocol with
//! event-type reporting enabled. Dispatching both halves makes every hotkey
//! fire twice.
//!
//! Release events cannot simply be discarded, though: on Windows, spawning a
//! child process (`plink.exe` in particular) can corrupt the console input
//! mode so that some keys — Escape and modified keys especially — only ever
//! produce a Release event, with no matching Press.
//!
//! [`KeyEventFilter`] resolves both cases by pairing releases with the presses
//! that preceded them. A Release whose Press was already dispatched is a
//! duplicate and is dropped; an *orphan* Release (no Press was seen) is
//! dispatched so the corrupted-console workaround keeps working.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Number of simultaneously-held keys tracked.
///
/// Acts as a bounded ring buffer: the oldest entry is evicted once full, so a
/// press whose release never arrives (window focus loss, for example) cannot
/// occupy a slot forever.
const PRESSED_SLOTS: usize = 16;

/// What the caller should do with a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDecision {
    /// Route the event to the application's key handlers.
    Dispatch,
    /// Discard the event.
    Drop,
}

/// Pairs key releases with their presses to suppress duplicate dispatches.
#[derive(Debug)]
pub struct KeyEventFilter {
    /// Codes whose Press has been dispatched but whose Release has not arrived.
    pressed: [Option<KeyCode>; PRESSED_SLOTS],
    /// Next slot to overwrite when the ring is full.
    next: usize,
}

impl Default for KeyEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyEventFilter {
    /// Creates an empty filter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pressed: [None; PRESSED_SLOTS],
            next: 0,
        }
    }

    /// Decides whether `key` should be dispatched.
    ///
    /// Press events are always dispatched and remembered. Repeat events are
    /// dropped. Release events are dropped when they pair with a remembered
    /// Press, and dispatched only when they are orphans of a key that would
    /// otherwise be unreachable (see [`is_orphan_recoverable`]).
    pub fn decide(&mut self, key: &KeyEvent) -> KeyDecision {
        match key.kind {
            KeyEventKind::Press => {
                self.remember_press(key.code);
                KeyDecision::Dispatch
            }
            KeyEventKind::Repeat => KeyDecision::Drop,
            KeyEventKind::Release => {
                if self.take_press(key.code) {
                    // Paired release: its press was already dispatched.
                    KeyDecision::Drop
                } else if is_orphan_recoverable(key) {
                    KeyDecision::Dispatch
                } else {
                    KeyDecision::Drop
                }
            }
        }
    }

    /// Records a pressed key, ignoring a code that is already recorded.
    fn remember_press(&mut self, code: KeyCode) {
        let code = normalize(code);
        if self.find(code).is_some() {
            return;
        }
        self.pressed[self.next] = Some(code);
        self.next = (self.next + 1) % PRESSED_SLOTS;
    }

    /// Removes a recorded press, returning whether one was found.
    fn take_press(&mut self, code: KeyCode) -> bool {
        let code = normalize(code);
        match self.find(code) {
            Some(idx) => {
                self.pressed[idx] = None;
                true
            }
            None => false,
        }
    }

    /// Returns the slot holding `code`, if any.
    fn find(&self, code: KeyCode) -> Option<usize> {
        self.pressed.iter().position(|slot| *slot == Some(code))
    }
}

/// Normalizes a key code so a press and its release compare equal.
///
/// Windows reports `Shift+p` as `Char('P')` while the release may report
/// `Char('p')` (or the reverse) depending on modifier release order.
fn normalize(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other => other,
    }
}

/// Returns true if an orphan Release for this key should still be dispatched.
///
/// Only keys affected by the corrupted-console-mode bug qualify: Escape, and
/// keys held with Ctrl or Alt. Shift is included only for non-character keys,
/// because accepting a release for `Shift+Char` would duplicate typed text on
/// terminals that report both event kinds for plain typing.
fn is_orphan_recoverable(key: &KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || key.modifiers.contains(KeyModifiers::CONTROL)
        || key.modifiers.contains(KeyModifiers::ALT)
        || (key.modifiers.contains(KeyModifiers::SHIFT) && !matches!(key.code, KeyCode::Char(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn key(code: KeyCode, mods: KeyModifiers, kind: KeyEventKind) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: mods,
            kind,
            state: KeyEventState::NONE,
        }
    }

    fn press(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        key(code, mods, KeyEventKind::Press)
    }

    fn release(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        key(code, mods, KeyEventKind::Release)
    }

    // ================================================================
    // Press events are always dispatched
    // ================================================================

    #[test]
    fn test_press_plain_char_dispatched() {
        let mut f = KeyEventFilter::new();
        let ev = press(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(f.decide(&ev), KeyDecision::Dispatch);
    }

    #[test]
    fn test_press_shifted_char_dispatched() {
        let mut f = KeyEventFilter::new();
        let ev = press(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(f.decide(&ev), KeyDecision::Dispatch);
    }

    #[test]
    fn test_press_ctrl_char_dispatched() {
        let mut f = KeyEventFilter::new();
        let ev = press(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(f.decide(&ev), KeyDecision::Dispatch);
    }

    #[test]
    fn test_press_esc_dispatched() {
        let mut f = KeyEventFilter::new();
        let ev = press(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(f.decide(&ev), KeyDecision::Dispatch);
    }

    // ================================================================
    // Paired Press+Release dispatches exactly once (the Windows bug)
    // ================================================================

    #[test]
    fn test_ctrl_char_press_then_release_dispatches_once() {
        let mut f = KeyEventFilter::new();
        let mods = KeyModifiers::CONTROL;
        assert_eq!(
            f.decide(&press(KeyCode::Char('q'), mods)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('q'), mods)),
            KeyDecision::Drop,
            "Release paired with a dispatched Press must not fire the hotkey twice"
        );
    }

    #[test]
    fn test_ctrl_shift_char_press_then_release_dispatches_once() {
        let mut f = KeyEventFilter::new();
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(
            f.decide(&press(KeyCode::Char('P'), mods)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('P'), mods)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_esc_press_then_release_dispatches_once() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&press(KeyCode::Esc, KeyModifiers::NONE)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Esc, KeyModifiers::NONE)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_alt_char_press_then_release_dispatches_once() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&press(KeyCode::Char('x'), KeyModifiers::ALT)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('x'), KeyModifiers::ALT)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_shift_tab_press_then_release_dispatches_once() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&press(KeyCode::BackTab, KeyModifiers::SHIFT)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::BackTab, KeyModifiers::SHIFT)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_release_case_differs_from_press_still_pairs() {
        // Press reports the shifted glyph, release reports the unshifted one.
        let mut f = KeyEventFilter::new();
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(
            f.decide(&press(KeyCode::Char('P'), mods)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('p'), mods)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_release_after_modifier_lifted_still_pairs() {
        // User lifts Ctrl before the letter, so the release carries no modifier.
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&press(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('g'), KeyModifiers::NONE)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_repeated_hotkey_dispatches_once_each_time() {
        let mut f = KeyEventFilter::new();
        let mods = KeyModifiers::CONTROL;
        for _ in 0..5 {
            assert_eq!(
                f.decide(&press(KeyCode::Char('t'), mods)),
                KeyDecision::Dispatch
            );
            assert_eq!(
                f.decide(&release(KeyCode::Char('t'), mods)),
                KeyDecision::Drop
            );
        }
    }

    #[test]
    fn test_interleaved_hotkeys_each_dispatch_once() {
        let mut f = KeyEventFilter::new();
        let mods = KeyModifiers::CONTROL;
        assert_eq!(
            f.decide(&press(KeyCode::Char('a'), mods)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&press(KeyCode::Char('b'), mods)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('a'), mods)),
            KeyDecision::Drop
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('b'), mods)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_duplicate_press_without_release_still_pairs_once() {
        // Some consoles emit repeated Press events while a key is held.
        let mut f = KeyEventFilter::new();
        let mods = KeyModifiers::CONTROL;
        assert_eq!(
            f.decide(&press(KeyCode::Char('s'), mods)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&press(KeyCode::Char('s'), mods)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::Char('s'), mods)),
            KeyDecision::Drop
        );
        // The slot is now free, so a genuine orphan release is recoverable.
        assert_eq!(
            f.decide(&release(KeyCode::Char('s'), mods)),
            KeyDecision::Dispatch
        );
    }

    // ================================================================
    // Orphan releases (corrupted console mode) are still dispatched
    // ================================================================

    #[test]
    fn test_orphan_release_esc_dispatched() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&release(KeyCode::Esc, KeyModifiers::NONE)),
            KeyDecision::Dispatch,
            "Release of Esc with no preceding Press keeps the plink workaround"
        );
    }

    #[test]
    fn test_orphan_release_ctrl_char_dispatched() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&release(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            KeyDecision::Dispatch
        );
    }

    #[test]
    fn test_orphan_release_alt_char_dispatched() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&release(KeyCode::Char('x'), KeyModifiers::ALT)),
            KeyDecision::Dispatch
        );
    }

    #[test]
    fn test_orphan_release_shift_tab_dispatched() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&release(KeyCode::Tab, KeyModifiers::SHIFT)),
            KeyDecision::Dispatch
        );
        assert_eq!(
            f.decide(&release(KeyCode::BackTab, KeyModifiers::SHIFT)),
            KeyDecision::Dispatch
        );
    }

    // ================================================================
    // Releases that are never recoverable
    // ================================================================

    #[test]
    fn test_orphan_release_plain_char_dropped() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&release(KeyCode::Char('a'), KeyModifiers::NONE)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_orphan_release_shifted_char_dropped() {
        let mut f = KeyEventFilter::new();
        assert_eq!(
            f.decide(&release(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            KeyDecision::Drop,
            "Release of Shift+Char must be filtered to prevent double input"
        );
    }

    // ================================================================
    // Repeat events
    // ================================================================

    #[test]
    fn test_repeat_plain_char_dropped() {
        let mut f = KeyEventFilter::new();
        let ev = key(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Repeat);
        assert_eq!(f.decide(&ev), KeyDecision::Drop);
    }

    #[test]
    fn test_repeat_shifted_char_dropped() {
        let mut f = KeyEventFilter::new();
        let ev = key(
            KeyCode::Char('A'),
            KeyModifiers::SHIFT,
            KeyEventKind::Repeat,
        );
        assert_eq!(f.decide(&ev), KeyDecision::Drop);
    }

    // ================================================================
    // Bounded state
    // ================================================================

    #[test]
    fn test_press_ring_evicts_oldest_when_full() {
        let mut f = KeyEventFilter::new();
        let mods = KeyModifiers::CONTROL;
        // Fill every slot plus one, without any releases.
        for byte in (b'a'..=b'z').take(PRESSED_SLOTS + 1) {
            let c = char::from(byte);
            assert_eq!(
                f.decide(&press(KeyCode::Char(c), mods)),
                KeyDecision::Dispatch
            );
        }
        // 'a' was evicted, so its release now looks like an orphan.
        assert_eq!(
            f.decide(&release(KeyCode::Char('a'), mods)),
            KeyDecision::Dispatch
        );
        // 'b' is still tracked, so its release is a duplicate.
        assert_eq!(
            f.decide(&release(KeyCode::Char('b'), mods)),
            KeyDecision::Drop
        );
    }

    #[test]
    fn test_default_matches_new() {
        let mut a = KeyEventFilter::default();
        let mut b = KeyEventFilter::new();
        let ev = release(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(a.decide(&ev), b.decide(&ev));
    }

    #[test]
    fn test_normalize_lowercases_chars_only() {
        assert_eq!(normalize(KeyCode::Char('Z')), KeyCode::Char('z'));
        assert_eq!(normalize(KeyCode::Char('z')), KeyCode::Char('z'));
        assert_eq!(normalize(KeyCode::F(5)), KeyCode::F(5));
        assert_eq!(normalize(KeyCode::Esc), KeyCode::Esc);
    }
}
