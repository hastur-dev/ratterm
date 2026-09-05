//! Platform detection utilities.
//!
//! Provides functions to detect the current operating system and version.

use std::sync::OnceLock;

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
/// On Windows 11, returns "F1" to avoid conflict with the Windows command palette.
/// On other platforms, returns "Ctrl+Shift+P".
#[must_use]
pub fn command_palette_hotkey() -> &'static str {
    if is_windows_11() {
        "F1"
    } else {
        "Ctrl+Shift+P"
    }
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
    /// Windows 11 reserves `Ctrl+Shift+P` for its own terminal command
    /// palette, so the binding — and therefore the label — becomes `F1`.
    #[must_use]
    pub const fn command_palette(self) -> &'static str {
        if self.windows_11 {
            "F1"
        } else {
            "Ctrl+Shift+P"
        }
    }

    /// Label for the pane-switching hotkey.
    ///
    /// Windows reserves `Alt+Tab` for the system window switcher, so the
    /// application never receives it. `Alt+Left` / `Alt+Right` focus the
    /// terminal and editor panes directly and do reach the application.
    #[must_use]
    pub const fn switch_pane(self) -> &'static str {
        if self.windows {
            "Alt+Arrows"
        } else {
            "Alt+Tab"
        }
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
        assert_eq!(WIN10.command_palette(), "Ctrl+Shift+P");
        assert_eq!(WIN11.command_palette(), "F1");
    }

    #[test]
    fn test_switch_pane_label_per_platform() {
        assert_eq!(UNIX.switch_pane(), "Alt+Tab");
        assert_eq!(
            WIN10.switch_pane(),
            "Alt+Arrows",
            "Windows steals Alt+Tab, so the hint must not advertise it"
        );
        assert_eq!(WIN11.switch_pane(), "Alt+Arrows");
    }

    #[test]
    fn test_labels_are_ascii_for_width_math() {
        // The hint bar measures badge width in bytes, so labels must be ASCII.
        for keys in [UNIX, WIN10, WIN11] {
            assert!(keys.command_palette().is_ascii());
            assert!(keys.switch_pane().is_ascii());
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
