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

use ratterm::api::ApiServerConfig;
use ratterm::app::{App, AppOptions};
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

    // Handle --help flag
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("ratterm v{VERSION}");
        println!();
        println!("Usage: rat [OPTIONS] [FILE]");
        println!();
        println!("Options:");
        println!("  --version, -v    Show version");
        println!("  --help, -h       Show this help");
        println!("  --verify         Verify the binary is valid (used by the updater)");
        println!("  --update         Check and install updates");
        println!("  --no-update      Skip the update check");
        println!();
        print!("{}", ratterm::cli::help_text());
        println!("Subcommands:");
        println!("  uninstall        Uninstall ratterm from the system");
        println!("  ext              Extension manager");
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

    // Automation flags: headless runs and scenarios never touch the terminal,
    // so they are handled before it is put into raw mode.
    let cli = match ratterm::cli::parse(&args[1..]) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("{message}");
            eprintln!();
            eprint!("{}", ratterm::cli::help_text());
            std::process::exit(2);
        }
    };

    if cli.is_scenario_run() || cli.is_headless() {
        setup_logging(&Config::load().unwrap_or_default().log_config);
        return if cli.is_scenario_run() {
            run_scenarios(&cli)
        } else {
            run_headless(&cli)
        };
    }

    // Check for updates on startup (unless --no-update)
    let update_result = if cli.no_update {
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

    let file_path = cli.file.clone();

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
    let options = match app_options_from(&cli) {
        Ok(options) => options,
        Err(message) => {
            let _ = restore_terminal();
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let mut app = App::with_options(size.width, size.height, options)?;

    // Enable test-keys mode if requested (F1/F2/F3 open palette/SSH/Docker)
    if cli.test_keys {
        app.enable_test_keys();
    }

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
    if let Some(path) = file_path
        && let Err(e) = app.open_file(&path)
    {
        app.set_status(format!("Error opening {path}: {e}"));
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

/// Builds the application options implied by the command line.
fn app_options_from(cli: &ratterm::cli::CliOptions) -> Result<AppOptions, String> {
    let mut options = AppOptions::interactive();

    if let Some(dir) = cli.fixtures.as_ref() {
        options = options.with_fixtures(dir.clone());
    }

    if cli.no_api {
        options.without_api = true;
    } else if let Some(endpoint) = cli.api_endpoint.clone() {
        let mut config = if cli.api_no_auth {
            ApiServerConfig::default()
        } else {
            ApiServerConfig::secure_default().map_err(|e| e.to_string())?
        };
        config.endpoint = endpoint;
        options = options.with_api(config);
    } else if cli.api_no_auth {
        options = options.with_api(ApiServerConfig::default());
    }

    Ok(options)
}

/// Runs the scenarios named on the command line.
///
/// Exits non-zero if any scenario fails, so CI can use it directly.
fn run_scenarios(cli: &ratterm::cli::CliOptions) -> Result<(), Box<dyn std::error::Error>> {
    use ratterm::scenario::{ScenarioRunner, load, load_dir};

    let mut scenarios = Vec::new();
    if let Some(path) = cli.scenario.as_ref() {
        scenarios.push(load(path)?);
    }
    if let Some(dir) = cli.scenario_dir.as_ref() {
        scenarios.extend(load_dir(dir)?);
    }

    if scenarios.is_empty() {
        eprintln!("no scenarios to run");
        std::process::exit(2);
    }

    let mut runner = ScenarioRunner::new();
    if let Some(dir) = cli.results_dir.as_ref() {
        runner = runner.with_results_dir(dir.clone());
    }

    let (width, height) = cli.headless_size();
    // A scenario run must not compete for the single IPC endpoint, and must
    // not spawn a shell it never uses.
    let mut options =
        app_options_from(cli).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    options.without_terminals = true;
    options.without_api = cli.api_endpoint.is_none();

    let mut failures = 0;
    let mut summaries = Vec::new();

    for scenario in &scenarios {
        let mut app = App::with_options(width, height, options.clone())?;
        let outcome = runner.run(scenario, &mut app);
        print!("{}", outcome.report());
        if !outcome.passed {
            failures += 1;
        }
        summaries.push(serde_json::json!({
            "name": outcome.name,
            "passed": outcome.passed,
            "steps": outcome.steps.len(),
            "passed_steps": outcome.passed_count(),
            "duration_ms": outcome.duration.as_millis() as u64,
            "snapshots": outcome.snapshots
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>(),
            "failure": outcome.first_failure().map(|f| serde_json::json!({
                "step": f.index,
                "description": f.description,
                "message": f.message,
            })),
        }));
        app.shutdown();
    }

    // A machine-readable summary next to the snapshots, for CI to upload.
    let results_dir = runner.results_dir().to_path_buf();
    if std::fs::create_dir_all(&results_dir).is_ok() {
        let report = serde_json::json!({
            "platform": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "total": scenarios.len(),
            "failed": failures,
            "scenarios": summaries,
        });
        let path = results_dir.join("scenarios.json");
        if let Ok(text) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(&path, text);
            println!("summary written to {}", path.display());
        }
    }

    println!("{} scenario(s), {} failed", scenarios.len(), failures);

    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Runs the application with no terminal attached.
///
/// The interface is still rendered on demand through `app.snapshot`, so a
/// client on the control API sees exactly what a terminal would show. Useful
/// under systemd, in a container, or over SSH where there is no PTY.
fn run_headless(cli: &ratterm::cli::CliOptions) -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = cli.headless_size();
    let options = app_options_from(cli).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    let mut app = App::with_options(width, height, options)?;
    app.resize(width, height);

    if cli.test_keys {
        app.enable_test_keys();
    }

    if let Some(path) = cli.file.as_ref()
        && let Err(e) = app.open_file(path)
    {
        eprintln!("could not open {path}: {e}");
    }

    eprintln!("ratterm running headless at {width}x{height}");
    if let Some(endpoint) = app.api_endpoint() {
        eprintln!("control API on {endpoint}");
    }

    // The loop only advances state; rendering happens when something asks for
    // a snapshot. Sleeping keeps an idle instance off the CPU.
    let mut iterations: u64 = 0;
    const MAX_HEADLESS_ITERATIONS: u64 = 10_000_000_000;

    while app.is_running() && iterations < MAX_HEADLESS_ITERATIONS {
        app.tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
        iterations += 1;
    }

    app.shutdown();
    Ok(())
}
