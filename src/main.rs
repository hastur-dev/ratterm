//! Ratterm - Main entry point.
//!
//! A split-terminal TUI application with PTY terminal emulator and
//! code editor.
//!
//! Usage: rat [OPTIONS] [FILE]
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
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use ratterm::app::App;
use ratterm::updater::{self, Updater, UpdateStatus, VERSION};

/// Maximum iterations for main loop (safety bound).
const MAX_MAIN_ITERATIONS: usize = 10_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    // Handle --version flag
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("ratterm v{}", VERSION);
        return Ok(());
    }

    // Handle --update flag
    if args.iter().any(|a| a == "--update") {
        let updater = Updater::new();
        match updater.check() {
            UpdateStatus::Available(version) => {
                println!("Updating to v{}...", version);
                if let Err(e) = updater.update(&version) {
                    eprintln!("Update failed: {}", e);
                    std::process::exit(1);
                }
                println!("Update complete! Please restart ratterm.");
            }
            UpdateStatus::UpToDate => {
                println!("ratterm v{} is up to date.", VERSION);
            }
            UpdateStatus::Failed(e) => {
                eprintln!("Update check failed: {}", e);
                std::process::exit(1);
            }
            UpdateStatus::Disabled => {
                println!("Updates are disabled.");
            }
        }
        return Ok(());
    }

    // Check for updates on startup (unless --no-update)
    if !args.iter().any(|a| a == "--no-update") {
        if updater::check_for_updates() {
            // User updated, exit so they can restart
            return Ok(());
        }
    }

    // Get file path (skip flags)
    let file_path = args.iter()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .cloned();

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

    // Open file if provided
    if let Some(path) = file_path {
        if let Err(e) = app.open_file(&path) {
            app.set_status(format!("Error opening {}: {}", path, e));
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
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(())
}
