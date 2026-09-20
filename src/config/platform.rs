//! Platform detection utilities.
//!
//! Provides functions to detect the current operating system and version.

use std::sync::OnceLock;

use crossterm::event::{KeyCode, KeyModifiers};

use super::keybindings::KeyBinding;

/// Cached result of Windows 11 detection.
static IS_WINDOWS_11: OnceLock<bool> = OnceLock::new();

/// Returns true if running on Windows 11.
///
/// Windows 11 has build number >= 22000. This function caches the result
/// for subsequent calls.
#[must_use]
pub fn is_windows_11() -> bool {
    *IS_WINDOWS_11.get_or_init(detect_windows_11)
}

/// Detects if the current OS is Windows 11.
#[cfg(windows)]
fn detect_windows_11() -> bool {
    use std::process::Command;

    // Try to get Windows build number via PowerShell
    // Windows 11 has build number >= 22000
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_OperatingSystem).BuildNumber",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let build_str = String::from_utf8_lossy(&output.stdout);
            let build_num: u32 = build_str.trim().parse().unwrap_or(0);
            build_num >= 22000
        }
        _ => {
            // Fallback: try registry via reg query
            let reg_output = Command::new("reg")
                .args([
                    "query",
                    r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                    "/v",
                    "CurrentBuildNumber",
                ])
                .output();

            match reg_output {
                Ok(output) if output.status.success() => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    // Parse output like: "CurrentBuildNumber    REG_SZ    22631"
                    for line in output_str.lines() {
                        if line.contains("CurrentBuildNumber")
                            && let Some(build_str) = line.split_whitespace().last()
                        {
                            let build_num: u32 = build_str.parse().unwrap_or(0);
                            return build_num >= 22000;
                        }
                    }
                    false
                }
                _ => false,
            }
        }
    }
}

/// Non-Windows platforms are never Windows 11.
#[cfg(not(windows))]
fn detect_windows_11() -> bool {
    false
}

/// Returns the command palette hotkey string for the current platform.
///
/// On Windows, returns "F1" to avoid Windows Terminal's command palette binding.
/// On other platforms, returns "Ctrl+Shift+P".
#[must_use]
pub fn command_palette_hotkey() -> &'static str {
    if cfg!(windows) { "F1" } else { "Ctrl+Shift+P" }
}

/// Platform facts that change which hotkey labels the UI must display.
///
/// Held as data rather than queried ad hoc so the label logic is testable
/// without pretending to run on another operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformKeys {
    /// Whether the host is Windows (any version).
    pub windows: bool,
    /// Whether the host is Windows 11 or newer.
    pub windows_11: bool,
}

impl PlatformKeys {
    /// Returns the facts for the host this binary is running on.
    #[must_use]
    pub fn detect() -> Self {
        Self {
            windows: cfg!(windows),
            windows_11: is_windows_11(),
        }
    }

    /// Label for the command palette hotkey.
    ///
    /// Windows Terminal uses `Ctrl+Shift+P` for its command palette, so the
    /// binding — and therefore the label — becomes `F1` on Windows.
    #[must_use]
    pub const fn command_palette(self) -> &'static str {
        if self.windows { "F1" } else { "Ctrl+Shift+P" }
    }

    /// Label for the pane-switching hotkey.
    ///
    /// Windows reserves `Alt+Tab` for the system window switcher and Windows
    /// Terminal uses `Alt+Arrow` for host-pane focus, so Windows uses `Ctrl+F6`.
    #[must_use]
    pub const fn switch_pane(self) -> &'static str {
        if self.windows { "Ctrl+F6" } else { "Alt+Tab" }
    }

    /// Binding for opening the command palette.
    #[must_use]
    pub const fn command_palette_binding(self) -> KeyBinding {
        if self.windows {
            KeyBinding::new(KeyModifiers::NONE, KeyCode::F(1))
        } else {
            KeyBinding::new(
                KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                KeyCode::Char('p'),
            )
        }
    }

    /// Binding for opening the editor-mode switcher.
    #[must_use]
    pub const fn mode_switcher_binding(self) -> KeyBinding {
        if self.windows {
            KeyBinding::new(KeyModifiers::CONTROL, KeyCode::F(1))
        } else {
            KeyBinding::new(
                KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                KeyCode::Tab,
            )
        }
    }

    /// Binding for closing a terminal split.
    #[must_use]
    pub const fn close_split_binding(self) -> KeyBinding {
        if self.windows {
            KeyBinding::new(
                KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                KeyCode::F(4),
            )
        } else {
            KeyBinding::new(
                KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                KeyCode::Char('w'),
            )
        }
    }

    /// Binding for cycling focus between terminal grid panes.
    #[must_use]
    pub const fn split_focus_binding(self) -> KeyBinding {
        if self.windows {
            KeyBinding::new(KeyModifiers::ALT, KeyCode::F(6))
        } else {
            KeyBinding::new(KeyModifiers::CONTROL, KeyCode::Tab)
        }
    }

    /// Binding for the Docker fleet view.
    #[must_use]
    pub const fn docker_fleet_binding(self) -> KeyBinding {
        if self.windows {
            KeyBinding::new(
                KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                KeyCode::F(7),
            )
        } else {
            KeyBinding::new(
                KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                KeyCode::Char('m'),
            )
        }
    }

    /// Binding for debugger step-into.
    #[must_use]
    pub const fn debug_step_into_binding(self) -> KeyBinding {
        if self.windows {
            KeyBinding::new(KeyModifiers::CONTROL, KeyCode::F(11))
        } else {
            KeyBinding::new(KeyModifiers::NONE, KeyCode::F(11))
        }
    }

    /// Binding for debugger step-out.
    #[must_use]
    pub const fn debug_step_out_binding(self) -> KeyBinding {
        if self.windows {
            KeyBinding::new(
                KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
                KeyCode::F(11),
            )
        } else {
            KeyBinding::new(KeyModifiers::SHIFT, KeyCode::F(11))
        }
    }

    /// Label for the editor-mode switcher.
    #[must_use]
    pub const fn mode_switcher(self) -> &'static str {
        if self.windows {
            "Ctrl+F1"
        } else {
            "Ctrl+Shift+Tab"
        }
    }

    /// Label for closing a terminal split.
    #[must_use]
    pub const fn close_split(self) -> &'static str {
        if self.windows {
            "Ctrl+Shift+F4"
        } else {
            "Ctrl+Shift+W"
        }
    }

    /// Label for cycling focus between terminal grid panes.
    #[must_use]
    pub const fn split_focus(self) -> &'static str {
        if self.windows { "Alt+F6" } else { "Ctrl+Tab" }
    }

    /// Label for opening the Docker fleet view.
    #[must_use]
    pub const fn docker_fleet(self) -> &'static str {
        if self.windows {
            "Ctrl+Shift+F7"
        } else {
            "Ctrl+Shift+M"
        }
    }

    /// Label for debugger step-into.
    #[must_use]
    pub const fn debug_step_into(self) -> &'static str {
        if self.windows { "Ctrl+F11" } else { "F11" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Non-Windows host facts.
    const UNIX: PlatformKeys = PlatformKeys {
        windows: false,
        windows_11: false,
    };
    /// Windows 10 host facts.
    const WIN10: PlatformKeys = PlatformKeys {
        windows: true,
        windows_11: false,
    };
    /// Windows 11 host facts.
    const WIN11: PlatformKeys = PlatformKeys {
        windows: true,
        windows_11: true,
    };

    #[test]
    fn test_command_palette_label_per_platform() {
        assert_eq!(UNIX.command_palette(), "Ctrl+Shift+P");
        assert_eq!(WIN10.command_palette(), "F1");
        assert_eq!(WIN11.command_palette(), "F1");
    }

    #[test]
    fn test_switch_pane_label_per_platform() {
        assert_eq!(UNIX.switch_pane(), "Alt+Tab");
        assert_eq!(WIN10.switch_pane(), "Ctrl+F6");
        assert_eq!(WIN11.switch_pane(), "Ctrl+F6");
    }

    #[test]
    fn windows_direct_shortcuts_avoid_terminal_defaults() {
        assert_eq!(WIN11.command_palette(), "F1");
        assert_eq!(WIN11.mode_switcher(), "Ctrl+F1");
        assert_eq!(WIN11.close_split(), "Ctrl+Shift+F4");
        assert_eq!(WIN11.split_focus(), "Alt+F6");
        assert_eq!(WIN11.docker_fleet(), "Ctrl+Shift+F7");
        assert_eq!(WIN11.debug_step_into(), "Ctrl+F11");
    }

    #[test]
    fn non_windows_direct_shortcuts_remain_unchanged() {
        assert_eq!(UNIX.command_palette(), "Ctrl+Shift+P");
        assert_eq!(UNIX.mode_switcher(), "Ctrl+Shift+Tab");
        assert_eq!(UNIX.close_split(), "Ctrl+Shift+W");
        assert_eq!(UNIX.split_focus(), "Ctrl+Tab");
        assert_eq!(UNIX.docker_fleet(), "Ctrl+Shift+M");
        assert_eq!(UNIX.debug_step_into(), "F11");
    }

    #[test]
    fn test_labels_are_ascii_for_width_math() {
        // The hint bar measures badge width in bytes, so labels must be ASCII.
        for keys in [UNIX, WIN10, WIN11] {
            assert!(keys.command_palette().is_ascii());
            assert!(keys.switch_pane().is_ascii());
            assert!(keys.split_focus().is_ascii());
        }
    }

    #[test]
    fn test_detect_matches_compiled_target() {
        let keys = PlatformKeys::detect();
        assert_eq!(keys.windows, cfg!(windows));
        assert_eq!(keys.windows_11, is_windows_11());
        if keys.windows_11 {
            assert!(keys.windows, "Windows 11 implies Windows");
        }
    }

    #[test]
    fn test_detect_agrees_with_free_function() {
        assert_eq!(
            PlatformKeys::detect().command_palette(),
            command_palette_hotkey()
        );
    }

    #[test]
    fn test_is_windows_11_returns_bool() {
        // Just ensure the function runs without panicking
        let _ = is_windows_11();
    }

    #[test]
    fn test_command_palette_hotkey_not_empty() {
        let hotkey = command_palette_hotkey();
        assert!(!hotkey.is_empty());
        assert!(hotkey == "F1" || hotkey == "Ctrl+Shift+P");
    }
}
