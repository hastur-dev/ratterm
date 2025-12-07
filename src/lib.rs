//! Ratterm
//!
//! A split-terminal TUI application with PTY terminal emulator and
//! code editor.
//!
//! # Architecture
//!
//! - **Terminal Module**: PTY-based terminal emulator with ANSI parsing
//! - **Editor Module**: Code editor with ropey text buffer
//! - **UI Module**: Ratatui widgets and split-pane layout
//! - **File Browser Module**: File system navigation
//!
//! # Usage
//!
//! ```no_run
//! use ratterm::app::App;
//!
//! let mut app = App::new(80, 24).expect("Failed to create app");
//! // Run event loop...
//! ```

#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

pub mod app;
pub mod clipboard;
pub mod config;
pub mod editor;
pub mod filebrowser;
pub mod terminal;
pub mod ui;
pub mod updater;

// Re-export main types
pub use app::App;
pub use clipboard::Clipboard;
pub use config::Config;
pub use editor::Editor;
pub use filebrowser::FileBrowser;
pub use terminal::Terminal;
