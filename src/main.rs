// Ratterm - A split-terminal TUI with PTY terminal emulator and code editor
// Copyright (C) 2024 hastur-dev
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Ratterm - Main entry point.
//!
//! A split-terminal TUI application with PTY terminal emulator and
//! code editor.
//!
//! Usage: rat \[OPTIONS\] \[FILE\]
//!
//! Options:
//!   --version, -v    Show version
//!   --verify         Verify binary is valid (used by updater)
//!   --update         Check and install updates
//!   --no-update      Skip update check
//!
//! Subcommands:
//!   uninstall        Uninstall ratterm from the system
//!   ext              Extension manager
//!
//! Opens ratterm, optionally with a file loaded in the editor.

use std::env;
use std::io;
use std::panic;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use ratterm::app::App;
use ratterm::config::Config;
use ratterm::extension::{ExtensionManager, installer::Installer};
use ratterm::logging::{self, LogConfig};
#[cfg(not(windows))]
use ratterm::updater::restart_application;
use ratterm::updater::{self, StartupUpdateResult, UpdateStatus, Updater, VERSION};

/// Maximum iterations for main loop (safety bound).
const MAX_MAIN_ITERATIONS: usize = 10_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    // Handle --version flag
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("ratterm v{VERSION}");
        return Ok(());
    }

    // Handle --verify flag (used by updater to validate downloaded binaries)
    if args.iter().any(|a| a == "--verify") {
        println!("ratterm v{VERSION} verify-ok");
        return Ok(());
    }

    // Handle --update flag
    if args.iter().any(|a| a == "--update") {
        let updater = Updater::new();
        match updater.check() {
            UpdateStatus::Available(version) => {
                println!("Updating to v{version}...");
                match updater.update_and_restart(&version) {
                    Ok(true) => {
                        // On Windows, the batch script handles restart
                        // On Unix, we restart here
                        #[cfg(not(windows))]
                        {
                            println!("Update complete! Restarting...");
                            restart_application();
                        }
                        #[cfg(windows)]
                        {
                            println!("Update prepared. Application will restart automatically.");
                        }
                    }
                    Ok(false) => {
                        println!("ratterm v{VERSION} is already up to date.");
                    }
                    Err(e) => {
                        eprintln!("Update failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
            UpdateStatus::UpToDate => {
                println!("ratterm v{VERSION} is up to date.");
            }
            UpdateStatus::Failed(e) => {
                eprintln!("Update check failed: {e}");
                std::process::exit(1);
            }
            UpdateStatus::Disabled => {
                println!("Updates are disabled.");
            }
        }
        return Ok(());
    }

    // Handle extension subcommand: rat ext <command>
    if args.get(1).map(|s| s.as_str()) == Some("ext") {
        return handle_extension_command(&args[2..]);
    }

    // Handle uninstall subcommand: rat uninstall
    if args.get(1).map(|s| s.as_str()) == Some("uninstall") {
        return handle_uninstall();
    }

    // Handle --test mode for automated testing
    if args.iter().any(|a| a == "--test") {
        return run_test_mode();
    }

    // Check for updates on startup (unless --no-update)
    let update_result = if args.iter().any(|a| a == "--no-update") {
        StartupUpdateResult::None
    } else {
        updater::check_for_updates()
    };

    // If update was performed, handle restart
    if let StartupUpdateResult::UpdatePerformed { version } = &update_result {
        // On Windows, the batch script handles restart - just exit
        // On Unix, we restart here
        #[cfg(not(windows))]
        {
            eprintln!("Update to v{version} complete! Restarting...");
            restart_application();
        }
        #[cfg(windows)]
        {
            eprintln!("Update to v{version} prepared. Restarting automatically...");
            return Ok(());
        }
    }

    // Get file path (skip flags)
    let file_path = args.iter().skip(1).find(|a| !a.starts_with('-')).cloned();

    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Load config early so we can use log settings
    let config = Config::load().unwrap_or_default();

    // Initialize logging with configurable retention
    setup_logging(&config.log_config);

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Get terminal size
    let size = terminal.size()?;

    // Create application
    let mut app = App::new(size.width, size.height)?;

    // Immediately resize to ensure proper layout synchronization
    // This fixes visual artifacts that occur before the first resize event
    app.resize(size.width, size.height);

    // Try to restore previous session (e.g., after an update)
    match app.restore_session() {
        Ok(true) => {
            // Session restored successfully
        }
        Ok(false) => {
            // No session to restore - this is normal
        }
        Err(e) => {
            tracing::warn!("Failed to restore session: {}", e);
        }
    }

    // Open file if provided (overrides session restore for this file)
    if let Some(path) = file_path {
        if let Err(e) = app.open_file(&path) {
            app.set_status(format!("Error opening {path}: {e}"));
        }
    }

    // Initialize extensions
    app.init_extensions();

    // Check for Windows 11 keybinding notification
    app.check_win11_notification();

    // Show update status in the app
    match update_result {
        StartupUpdateResult::DevModeUpdateAvailable { current, latest } => {
            app.set_status(format!(
                "[Dev] Update: v{} -> v{} (run 'rat --update')",
                current, latest
            ));
        }
        StartupUpdateResult::DevModeUpToDate { current } => {
            app.set_status(format!("[Dev] v{} (up to date)", current));
        }
        StartupUpdateResult::DevModeCheckFailed { current, error } => {
            app.set_status(format!(
                "[Dev] v{} (update check failed: {})",
                current, error
            ));
        }
        StartupUpdateResult::UpdateAvailable { current, latest } => {
            app.set_status(format!(
                "Update available: v{} -> v{} (run 'rat --update')",
                current, latest
            ));
        }
        _ => {}
    }

    // Main event loop
    let mut iterations = 0;
    while app.is_running() && iterations < MAX_MAIN_ITERATIONS {
        // Check if app requests a full redraw (e.g., after mode change)
        // This clears the terminal buffer to prevent ghost artifacts
        if app.take_redraw_request() {
            // Force complete terminal reset
            terminal.clear()?;
            // Also send raw clear screen escape sequence
            execute!(
                io::stdout(),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
            )?;
        }

        // Render
        terminal.draw(|frame| {
            app.render(frame);
        })?;

        // Update
        app.update()?;

        iterations += 1;
    }

    // Check if restart was requested (in-app update)
    let needs_restart = app.needs_restart_after_update();

    // Shutdown
    app.shutdown();

    // Restore terminal
    restore_terminal()?;

    // If update was performed in-app, handle restart
    if needs_restart {
        // On Windows, the batch script handles restart
        // On Unix, restart here
        #[cfg(not(windows))]
        {
            restart_application();
        }
        // On Windows, just exit - batch script will restart
    }

    // Force exit to avoid waiting for background threads
    std::process::exit(0);
}

/// Restores the terminal to its original state.
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

/// Sets up logging using the logging module with configurable retention.
///
/// Logs are written to `~/.ratterm/logs/` with automatic cleanup of old logs.
fn setup_logging(log_config: &LogConfig) {
    if let Err(e) = logging::init(log_config) {
        // Fall back to stderr-only logging if file logging fails
        eprintln!("Warning: Failed to initialize file logging: {}", e);

        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        let env_filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into());

        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_ansi(true),
            )
            .init();
    }
}

/// Handles extension subcommands: `rat ext <command>`
fn handle_extension_command(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let installer = Installer::new();
    let mut manager = ExtensionManager::new();

    // Initialize extension directories
    manager.init()?;

    match args.first().map(|s| s.as_str()) {
        Some("install") => {
            let repo = args.get(1).ok_or("Usage: rat ext install <user/repo>")?;
            println!("Installing extension from {}...", repo);

            match installer.install_from_github(repo) {
                Ok(manifest) => {
                    println!(
                        "Installed {} v{}",
                        manifest.extension.name, manifest.extension.version
                    );
                    println!("Restart ratterm to load the extension.");
                }
                Err(e) => {
                    eprintln!("Installation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("remove") => {
            let name = args.get(1).ok_or("Usage: rat ext remove <name>")?;
            println!("Removing extension {}...", name);

            match manager.remove(name) {
                Ok(()) => {
                    println!("Removed {}", name);
                }
                Err(e) => {
                    eprintln!("Removal failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("list") => {
            // Discover installed extensions
            let _ = manager.discover_extensions();

            let extensions = manager.installed();
            if extensions.is_empty() {
                println!("No extensions installed.");
                println!("\nInstall extensions with: rat ext install <user/repo>");
            } else {
                println!("Installed extensions:\n");
                for ext in extensions.values() {
                    println!("  {} v{}", ext.name, ext.version);
                    let desc = &ext.manifest.extension.description;
                    if !desc.is_empty() {
                        println!("    {}", desc);
                    }
                }
            }
        }
        Some("update") => {
            let name = args.get(1);

            if let Some(name) = name {
                // Update specific extension
                println!("Updating {}...", name);
                match installer.update(name) {
                    Ok(manifest) => {
                        println!(
                            "Updated {} to v{}",
                            manifest.extension.name, manifest.extension.version
                        );
                    }
                    Err(e) => {
                        eprintln!("Update failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Update all extensions
                let _ = manager.discover_extensions();
                let extensions: Vec<_> = manager
                    .installed()
                    .values()
                    .map(|e| e.name.clone())
                    .collect();

                if extensions.is_empty() {
                    println!("No extensions installed.");
                    return Ok(());
                }

                println!("Updating {} extensions...", extensions.len());
                let mut updated = 0;
                let mut failed = 0;

                for name in &extensions {
                    print!("  {} ... ", name);
                    match installer.update(name) {
                        Ok(manifest) => {
                            println!("v{}", manifest.extension.version);
                            updated += 1;
                        }
                        Err(e) => {
                            println!("failed: {}", e);
                            failed += 1;
                        }
                    }
                }

                println!("\nUpdated: {}, Failed: {}", updated, failed);
            }
        }
        Some("help") | None => {
            println!("Ratterm Extension Manager\n");
            println!("Usage: rat ext <command> [args]\n");
            println!("Commands:");
            println!("  install <user/repo>   Install extension from GitHub");
            println!("  install <user/repo>@v1.0.0  Install specific version");
            println!("  remove <name>         Remove installed extension");
            println!("  list                  List installed extensions");
            println!("  update                Update all extensions");
            println!("  update <name>         Update specific extension");
            println!("  help                  Show this help message");
        }
        Some(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            eprintln!("Run 'rat ext help' for usage.");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Runs automated test mode to diagnose rendering issues.
fn run_test_mode() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;

    println!("=== Ratterm Test Mode ===");
    println!("This mode tests the file browser open/close cycle.");
    println!();

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let mut app = App::new(size.width, size.height)?;
    app.resize(size.width, size.height);

    let mut log = File::create("test_output.txt")?;
    writeln!(log, "=== Test Mode Started ===")?;
    writeln!(log, "Terminal size: {}x{}", size.width, size.height)?;

    // Step 1: Initial render
    writeln!(log, "\n--- Step 1: Initial render ---")?;
    terminal.draw(|frame| app.render(frame))?;
    writeln!(log, "Initial render complete")?;

    // Step 2: Show file browser
    writeln!(log, "\n--- Step 2: Show file browser ---")?;
    app.show_file_browser();
    let redraw1 = app.take_redraw_request();
    writeln!(log, "Redraw requested after show_file_browser: {}", redraw1)?;
    if redraw1 {
        terminal.clear()?;
        writeln!(log, "Terminal cleared")?;
    }
    terminal.draw(|frame| app.render(frame))?;
    writeln!(log, "File browser render complete")?;

    // Step 3: Open a file (install.sh)
    writeln!(log, "\n--- Step 3: Open install.sh ---")?;
    let test_file = std::env::current_dir()?.join("install.sh");
    writeln!(log, "Test file path: {:?}", test_file)?;
    if test_file.exists() {
        writeln!(log, "File exists, opening...")?;
        let result = app.open_file(&test_file);
        writeln!(log, "open_file result: {:?}", result.is_ok())?;
    } else {
        writeln!(log, "install.sh not found, skipping file open")?;
    }

    // Check redraw flag
    let redraw2 = app.take_redraw_request();
    writeln!(log, "Redraw requested after open_file: {}", redraw2)?;

    if redraw2 {
        writeln!(log, "Clearing terminal...")?;
        terminal.clear()?;
        execute!(
            io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )?;
        writeln!(log, "Terminal cleared with both methods")?;
    }

    terminal.draw(|frame| app.render(frame))?;
    writeln!(log, "Post-open render complete")?;

    // Step 4: Another render cycle
    writeln!(log, "\n--- Step 4: Second render after file open ---")?;
    terminal.draw(|frame| app.render(frame))?;
    writeln!(log, "Second render complete")?;

    // Wait a bit to let user see the result
    std::thread::sleep(std::time::Duration::from_secs(2));

    writeln!(log, "\n=== Test Complete ===")?;

    // Cleanup
    app.shutdown();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    println!("Test complete. Check test_output.txt for results.");
    Ok(())
}

/// Handles the uninstall subcommand: `rat uninstall`
fn handle_uninstall() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    println!("Ratterm Uninstaller\n");

    // Get the current executable path
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or("Cannot determine executable directory")?;

    println!("Executable location: {}", exe_path.display());

    // Determine config directories
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let ratrc_path = home.join(".ratrc");
    let ratterm_dir = home.join(".ratterm");

    // Show what will be removed
    println!("\nThe following will be removed:");
    println!("  - {}", exe_path.display());

    if ratrc_path.exists() {
        println!("  - {} (config file)", ratrc_path.display());
    }
    if ratterm_dir.exists() {
        println!("  - {} (data directory)", ratterm_dir.display());
    }

    // Ask for confirmation
    println!("\nAre you sure you want to uninstall ratterm? [y/N] ");

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Uninstall cancelled.");
        return Ok(());
    }

    // Remove config files
    if ratrc_path.exists() {
        fs::remove_file(&ratrc_path)?;
        println!("Removed {}", ratrc_path.display());
    }

    if ratterm_dir.exists() {
        fs::remove_dir_all(&ratterm_dir)?;
        println!("Removed {}", ratterm_dir.display());
    }

    // Platform-specific binary removal
    #[cfg(not(windows))]
    {
        // On Unix, we can delete the running executable
        fs::remove_file(&exe_path)?;
        println!("Removed {}", exe_path.display());

        // Try to remove from PATH by showing instructions
        println!("\nratterm has been uninstalled.");
        println!("\nTo complete the uninstallation, remove the following from your shell config:");
        println!("  export PATH=\"{}:$PATH\"", exe_dir.display());
        println!("\nOr run:");
        println!("  sed -i '/ratterm/d' ~/.bashrc ~/.zshrc 2>/dev/null");
    }

    #[cfg(windows)]
    {
        // On Windows, we cannot delete the running executable
        // Create a batch script that will run after we exit
        let batch_path = env::temp_dir().join("ratterm_uninstall.bat");
        let batch_content = format!(
            r#"@echo off
echo Completing ratterm uninstallation...
:waitloop
tasklist /FI "IMAGENAME eq rat.exe" 2>NUL | find /I /N "rat.exe">NUL
if "%ERRORLEVEL%"=="0" (
    timeout /t 1 /nobreak >NUL
    goto waitloop
)
del /f /q "{exe_path}"
rmdir /s /q "{exe_dir}" 2>NUL
echo.
echo ratterm has been uninstalled.
echo.
echo To complete the uninstallation, remove ratterm from your PATH:
echo   1. Open System Properties ^> Environment Variables
echo   2. Remove "{exe_dir}" from the PATH variable
echo.
pause
del "%~f0"
"#,
            exe_path = exe_path.display(),
            exe_dir = exe_dir.display(),
        );

        fs::write(&batch_path, batch_content)?;

        println!("\nStarting uninstall script...");
        println!("The uninstaller will complete after this process exits.");

        // Start the batch script
        let batch_path_str = batch_path
            .to_str()
            .ok_or("Batch path contains invalid UTF-8")?;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", "/MIN", batch_path_str])
            .spawn()?;

        println!("\nratterm will be uninstalled when this window closes.");
        println!("Please manually remove ratterm from your PATH environment variable.");
    }

    Ok(())
}
