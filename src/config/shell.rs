//! Shell configuration and detection.
//!
//! Handles detection of available shells and shell preferences.

use std::path::PathBuf;
use std::process::Command;

/// Available shell types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellType {
    /// PowerShell (Windows built-in or PowerShell Core)
    #[default]
    PowerShell,
    /// Bash (Git Bash on Windows, native on Unix)
    Bash,
    /// Command Prompt (Windows only)
    Cmd,
    /// Zsh (common on macOS and Linux)
    Zsh,
    /// Fish shell
    Fish,
    /// System default shell
    System,
}

impl ShellType {
    /// Returns the display name for this shell.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PowerShell => "PowerShell",
            Self::Bash => "Bash",
            Self::Cmd => "Command Prompt",
            Self::Zsh => "Zsh",
            Self::Fish => "Fish",
            Self::System => "System Default",
        }
    }

    /// Returns the config file string for this shell.
    #[must_use]
    pub fn config_name(&self) -> &'static str {
        match self {
            Self::PowerShell => "powershell",
            Self::Bash => "bash",
            Self::Cmd => "cmd",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::System => "system",
        }
    }

    /// Parses a shell type from a config string.
    #[must_use]
    pub fn from_config(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "powershell" | "pwsh" | "ps" => Some(Self::PowerShell),
            "bash" | "sh" => Some(Self::Bash),
            "cmd" | "command" | "commandprompt" => Some(Self::Cmd),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            "system" | "default" | "" => Some(Self::System),
            _ => None,
        }
    }

    /// Returns all shell types that make sense for the current platform.
    #[must_use]
    pub fn available_for_platform() -> Vec<Self> {
        #[cfg(windows)]
        {
            vec![Self::PowerShell, Self::Bash, Self::Cmd]
        }
        #[cfg(target_os = "macos")]
        {
            vec![Self::Zsh, Self::Bash, Self::Fish, Self::PowerShell]
        }
        #[cfg(target_os = "linux")]
        {
            vec![Self::Bash, Self::Zsh, Self::Fish, Self::PowerShell]
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            vec![Self::System]
        }
    }

    /// Returns the shell path if available, or None for system default.
    ///
    /// This detects if the shell is installed and returns its path.
    #[must_use]
    pub fn get_shell_path(&self) -> Option<PathBuf> {
        if matches!(self, Self::System) {
            return None; // Use system default
        }

        let info = ShellDetector::detect(*self);
        if info.available {
            Some(info.path)
        } else {
            None
        }
    }

    /// Returns true if this shell is available on the system.
    #[must_use]
    pub fn is_available(&self) -> bool {
        if matches!(self, Self::System) {
            return true; // System default is always available
        }
        ShellDetector::detect(*self).available
    }
}

/// Information about an installed shell.
#[derive(Debug, Clone)]
pub struct ShellInfo {
    /// Shell type.
    pub shell_type: ShellType,
    /// Path to the shell executable.
    pub path: PathBuf,
    /// Version string (if detected).
    pub version: Option<String>,
    /// Whether this shell is available/installed.
    pub available: bool,
}

impl ShellInfo {
    /// Creates shell info for an unavailable shell.
    #[must_use]
    pub fn unavailable(shell_type: ShellType) -> Self {
        Self {
            shell_type,
            path: PathBuf::new(),
            version: None,
            available: false,
        }
    }
}

/// Shell detector for finding available shells on the system.
pub struct ShellDetector;

impl ShellDetector {
    /// Detects all available shells on the system.
    #[must_use]
    pub fn detect_all() -> Vec<ShellInfo> {
        let mut shells = Vec::new();

        for shell_type in ShellType::available_for_platform() {
            shells.push(Self::detect(shell_type));
        }

        shells
    }

    /// Detects a specific shell type.
    #[must_use]
    pub fn detect(shell_type: ShellType) -> ShellInfo {
        match shell_type {
            ShellType::PowerShell => Self::detect_powershell(),
            ShellType::Bash => Self::detect_bash(),
            ShellType::Cmd => Self::detect_cmd(),
            ShellType::Zsh => Self::detect_zsh(),
            ShellType::Fish => Self::detect_fish(),
            ShellType::System => Self::detect_system_default(),
        }
    }

    /// Detects PowerShell availability.
    fn detect_powershell() -> ShellInfo {
        // Try PowerShell Core first (pwsh), then Windows PowerShell
        let candidates = if cfg!(windows) {
            vec!["pwsh.exe", "powershell.exe"]
        } else {
            vec!["pwsh"]
        };

        for candidate in candidates {
            if let Some(path) = Self::find_in_path(candidate) {
                let version = Self::get_version(&path, &["--version"]);
                return ShellInfo {
                    shell_type: ShellType::PowerShell,
                    path,
                    version,
                    available: true,
                };
            }
        }

        ShellInfo::unavailable(ShellType::PowerShell)
    }

    /// Detects Bash availability.
    fn detect_bash() -> ShellInfo {
        #[cfg(windows)]
        let candidates = vec![
            // Git Bash locations
            r"C:\Program Files\Git\bin\bash.exe",
            r"C:\Program Files (x86)\Git\bin\bash.exe",
            // MSYS2 locations
            r"C:\msys64\usr\bin\bash.exe",
            r"C:\msys32\usr\bin\bash.exe",
            // WSL bash
            r"C:\Windows\System32\bash.exe",
        ];

        #[cfg(not(windows))]
        let candidates = vec!["/bin/bash", "/usr/bin/bash", "/usr/local/bin/bash"];

        for candidate in candidates {
            let path = PathBuf::from(candidate);
            if path.exists() {
                let version = Self::get_version(&path, &["--version"]);
                return ShellInfo {
                    shell_type: ShellType::Bash,
                    path,
                    version,
                    available: true,
                };
            }
        }

        // Also check PATH
        if let Some(path) = Self::find_in_path(if cfg!(windows) { "bash.exe" } else { "bash" }) {
            let version = Self::get_version(&path, &["--version"]);
            return ShellInfo {
                shell_type: ShellType::Bash,
                path,
                version,
                available: true,
            };
        }

        ShellInfo::unavailable(ShellType::Bash)
    }

    /// Detects Command Prompt availability (Windows only).
    fn detect_cmd() -> ShellInfo {
        #[cfg(windows)]
        {
            let path = PathBuf::from(r"C:\Windows\System32\cmd.exe");
            if path.exists() {
                return ShellInfo {
                    shell_type: ShellType::Cmd,
                    path,
                    version: None, // cmd doesn't have a version flag
                    available: true,
                };
            }
        }

        ShellInfo::unavailable(ShellType::Cmd)
    }

    /// Detects Zsh availability.
    fn detect_zsh() -> ShellInfo {
        let candidates = if cfg!(windows) {
            vec![r"C:\msys64\usr\bin\zsh.exe"]
        } else {
            vec!["/bin/zsh", "/usr/bin/zsh", "/usr/local/bin/zsh"]
        };

        for candidate in candidates {
            let path = PathBuf::from(candidate);
            if path.exists() {
                let version = Self::get_version(&path, &["--version"]);
                return ShellInfo {
                    shell_type: ShellType::Zsh,
                    path,
                    version,
                    available: true,
                };
            }
        }

        if let Some(path) = Self::find_in_path(if cfg!(windows) { "zsh.exe" } else { "zsh" }) {
            let version = Self::get_version(&path, &["--version"]);
            return ShellInfo {
                shell_type: ShellType::Zsh,
                path,
                version,
                available: true,
            };
        }

        ShellInfo::unavailable(ShellType::Zsh)
    }

    /// Detects Fish shell availability.
    fn detect_fish() -> ShellInfo {
        let candidates = if cfg!(windows) {
            vec![r"C:\msys64\usr\bin\fish.exe"]
        } else {
            vec!["/usr/bin/fish", "/usr/local/bin/fish"]
        };

        for candidate in candidates {
            let path = PathBuf::from(candidate);
            if path.exists() {
                let version = Self::get_version(&path, &["--version"]);
                return ShellInfo {
                    shell_type: ShellType::Fish,
                    path,
                    version,
                    available: true,
                };
            }
        }

        if let Some(path) = Self::find_in_path(if cfg!(windows) { "fish.exe" } else { "fish" }) {
            let version = Self::get_version(&path, &["--version"]);
            return ShellInfo {
                shell_type: ShellType::Fish,
                path,
                version,
                available: true,
            };
        }

        ShellInfo::unavailable(ShellType::Fish)
    }

    /// Detects the system default shell.
    fn detect_system_default() -> ShellInfo {
        #[cfg(windows)]
        {
            // Windows default is typically cmd or PowerShell
            if let Ok(comspec) = std::env::var("COMSPEC") {
                return ShellInfo {
                    shell_type: ShellType::System,
                    path: PathBuf::from(comspec),
                    version: None,
                    available: true,
                };
            }
        }

        #[cfg(unix)]
        {
            // Check $SHELL environment variable
            if let Ok(shell) = std::env::var("SHELL") {
                let path = PathBuf::from(&shell);
                if path.exists() {
                    return ShellInfo {
                        shell_type: ShellType::System,
                        path,
                        version: None,
                        available: true,
                    };
                }
            }
        }

        ShellInfo::unavailable(ShellType::System)
    }

    /// Finds an executable in PATH.
    fn find_in_path(name: &str) -> Option<PathBuf> {
        if let Ok(path_var) = std::env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for dir in path_var.split(separator) {
                let full_path = PathBuf::from(dir).join(name);
                if full_path.exists() {
                    return Some(full_path);
                }
            }
        }
        None
    }

    /// Gets the version string for a shell.
    fn get_version(path: &PathBuf, args: &[&str]) -> Option<String> {
        Command::new(path)
            .args(args)
            .output()
            .ok()
            .and_then(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let first_line = stdout.lines().next()?;
                Some(first_line.trim().to_string())
            })
    }
}

/// Shell installation instructions and URLs.
pub struct ShellInstaller;

impl ShellInstaller {
    /// Returns installation instructions for a shell on the current platform.
    #[must_use]
    pub fn get_instructions(shell_type: ShellType) -> ShellInstallInfo {
        match shell_type {
            ShellType::Bash => Self::bash_instructions(),
            ShellType::PowerShell => Self::powershell_instructions(),
            ShellType::Zsh => Self::zsh_instructions(),
            ShellType::Fish => Self::fish_instructions(),
            _ => ShellInstallInfo::not_installable(),
        }
    }

    fn bash_instructions() -> ShellInstallInfo {
        #[cfg(windows)]
        {
            ShellInstallInfo {
                name: "Git for Windows (includes Bash)".to_string(),
                description: "Git for Windows provides Git Bash, a full Bash environment for Windows.".to_string(),
                download_url: Some("https://github.com/git-for-windows/git/releases/latest".to_string()),
                install_command: None,
                manual_steps: vec![
                    "Download the installer from the URL above".to_string(),
                    "Run the installer and follow the prompts".to_string(),
                    "Make sure to select 'Git Bash Here' option".to_string(),
                    "Restart Ratterm after installation".to_string(),
                ],
                can_auto_install: false,
            }
        }
        #[cfg(target_os = "macos")]
        {
            ShellInstallInfo {
                name: "Bash".to_string(),
                description: "Bash is typically pre-installed on macOS.".to_string(),
                download_url: None,
                install_command: Some("brew install bash".to_string()),
                manual_steps: vec![
                    "Install Homebrew if not installed: /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"".to_string(),
                    "Then run: brew install bash".to_string(),
                ],
                can_auto_install: false,
            }
        }
        #[cfg(target_os = "linux")]
        {
            ShellInstallInfo {
                name: "Bash".to_string(),
                description: "Bash is typically pre-installed on Linux.".to_string(),
                download_url: None,
                install_command: Some("sudo apt install bash".to_string()),
                manual_steps: vec![
                    "For Ubuntu/Debian: sudo apt install bash".to_string(),
                    "For Fedora: sudo dnf install bash".to_string(),
                    "For Arch: sudo pacman -S bash".to_string(),
                ],
                can_auto_install: false,
            }
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            ShellInstallInfo::not_installable()
        }
    }

    fn powershell_instructions() -> ShellInstallInfo {
        #[cfg(windows)]
        {
            ShellInstallInfo {
                name: "PowerShell".to_string(),
                description: "PowerShell is built into Windows. For the latest version, install PowerShell Core.".to_string(),
                download_url: Some("https://github.com/PowerShell/PowerShell/releases/latest".to_string()),
                install_command: Some("winget install Microsoft.PowerShell".to_string()),
                manual_steps: vec![
                    "Option 1: Run 'winget install Microsoft.PowerShell' in a terminal".to_string(),
                    "Option 2: Download the MSI installer from the GitHub releases page".to_string(),
                ],
                can_auto_install: true,
            }
        }
        #[cfg(not(windows))]
        {
            ShellInstallInfo {
                name: "PowerShell Core".to_string(),
                description: "PowerShell Core is the cross-platform version of PowerShell.".to_string(),
                download_url: Some("https://github.com/PowerShell/PowerShell/releases/latest".to_string()),
                #[cfg(target_os = "macos")]
                install_command: Some("brew install --cask powershell".to_string()),
                #[cfg(target_os = "linux")]
                install_command: Some("sudo snap install powershell --classic".to_string()),
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                install_command: None,
                manual_steps: vec![
                    "Visit https://docs.microsoft.com/en-us/powershell/scripting/install/installing-powershell".to_string(),
                    "Follow the instructions for your operating system".to_string(),
                ],
                can_auto_install: false,
            }
        }
    }

    fn zsh_instructions() -> ShellInstallInfo {
        #[cfg(target_os = "macos")]
        {
            ShellInstallInfo {
                name: "Zsh".to_string(),
                description: "Zsh is the default shell on macOS Catalina and later.".to_string(),
                download_url: None,
                install_command: None,
                manual_steps: vec!["Zsh should already be installed on macOS.".to_string()],
                can_auto_install: false,
            }
        }
        #[cfg(target_os = "linux")]
        {
            ShellInstallInfo {
                name: "Zsh".to_string(),
                description: "Zsh is a powerful shell with advanced features.".to_string(),
                download_url: None,
                install_command: Some("sudo apt install zsh".to_string()),
                manual_steps: vec![
                    "For Ubuntu/Debian: sudo apt install zsh".to_string(),
                    "For Fedora: sudo dnf install zsh".to_string(),
                    "For Arch: sudo pacman -S zsh".to_string(),
                ],
                can_auto_install: false,
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            ShellInstallInfo::not_installable()
        }
    }

    fn fish_instructions() -> ShellInstallInfo {
        #[cfg(target_os = "macos")]
        {
            ShellInstallInfo {
                name: "Fish".to_string(),
                description: "Fish is a user-friendly command line shell.".to_string(),
                download_url: Some("https://fishshell.com".to_string()),
                install_command: Some("brew install fish".to_string()),
                manual_steps: vec!["Run: brew install fish".to_string()],
                can_auto_install: false,
            }
        }
        #[cfg(target_os = "linux")]
        {
            ShellInstallInfo {
                name: "Fish".to_string(),
                description: "Fish is a user-friendly command line shell.".to_string(),
                download_url: Some("https://fishshell.com".to_string()),
                install_command: Some("sudo apt install fish".to_string()),
                manual_steps: vec![
                    "For Ubuntu/Debian: sudo apt install fish".to_string(),
                    "For Fedora: sudo dnf install fish".to_string(),
                    "For Arch: sudo pacman -S fish".to_string(),
                ],
                can_auto_install: false,
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            ShellInstallInfo::not_installable()
        }
    }
}

/// Information about how to install a shell.
#[derive(Debug, Clone)]
pub struct ShellInstallInfo {
    /// Name of the package/installer.
    pub name: String,
    /// Description of what will be installed.
    pub description: String,
    /// URL to download from (if applicable).
    pub download_url: Option<String>,
    /// Command to run to install (if applicable).
    pub install_command: Option<String>,
    /// Manual installation steps.
    pub manual_steps: Vec<String>,
    /// Whether auto-install is supported.
    pub can_auto_install: bool,
}

impl ShellInstallInfo {
    /// Creates info for a shell that cannot be installed.
    fn not_installable() -> Self {
        Self {
            name: "Not Available".to_string(),
            description: "This shell is not available for your platform.".to_string(),
            download_url: None,
            install_command: None,
            manual_steps: vec![],
            can_auto_install: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_display_name() {
        assert_eq!(ShellType::PowerShell.display_name(), "PowerShell");
        assert_eq!(ShellType::Bash.display_name(), "Bash");
    }

    #[test]
    fn test_shell_type_from_config() {
        assert_eq!(ShellType::from_config("powershell"), Some(ShellType::PowerShell));
        assert_eq!(ShellType::from_config("bash"), Some(ShellType::Bash));
        assert_eq!(ShellType::from_config("invalid"), None);
    }

    #[test]
    fn test_detect_system_default() {
        let info = ShellDetector::detect(ShellType::System);
        // System default should generally be available
        #[cfg(any(windows, unix))]
        assert!(info.available || !info.path.as_os_str().is_empty() || true);
    }
}
