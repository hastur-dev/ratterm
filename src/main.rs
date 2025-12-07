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
//!   --update         Check and install updates
//!   --no-update      Skip update check
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
use ratterm::extension::{ExtensionManager, installer::Installer};
use ratterm::updater::{self, UpdateStatus, Updater, VERSION};

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

    // Handle --update flag
    if args.iter().any(|a| a == "--update") {
        let updater = Updater::new();
        match updater.check() {
            UpdateStatus::Available(version) => {
                println!("Updating to v{version}...");
                if let Err(e) = updater.update(&version) {
                    eprintln!("Update failed: {e}");
                    std::process::exit(1);
                }
                println!("Update complete! Please restart ratterm.");
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

    // Check for updates on startup (unless --no-update)
    if !args.iter().any(|a| a == "--no-update") && updater::check_for_updates() {
        // User updated, exit so they can restart
        return Ok(());
    }

    // Get file path (skip flags)
    let file_path = args.iter().skip(1).find(|a| !a.starts_with('-')).cloned();

    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

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

    // Main event loop
    let mut iterations = 0;
    while app.is_running() && iterations < MAX_MAIN_ITERATIONS {
        // Render
        terminal.draw(|frame| {
            app.render(frame);
        })?;

        // Update
        app.update()?;

        iterations += 1;
    }

    // Shutdown
    app.shutdown();

    // Restore terminal
    restore_terminal()?;

    // Force exit to avoid waiting for background threads
    std::process::exit(0);
}

/// Restores the terminal to its original state.
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
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
                        "Installed {} v{} ({})",
                        manifest.extension.name,
                        manifest.extension.version,
                        manifest.extension.ext_type
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
                    println!("  {} v{} ({})", ext.name, ext.version, ext.ext_type);
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
