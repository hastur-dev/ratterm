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
                        if line.contains("CurrentBuildNumber") {
                            if let Some(build_str) = line.split_whitespace().last() {
                                let build_num: u32 = build_str.parse().unwrap_or(0);
                                return build_num >= 22000;
                            }
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

#[cfg(test)]
mod tests {
    use super::*;

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
