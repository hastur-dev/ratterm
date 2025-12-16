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

// Clippy configuration - allow common patterns
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::struct_excessive_bools)]

pub mod api;
pub mod app;
pub mod clipboard;
pub mod config;
pub mod editor;
pub mod extension;
pub mod filebrowser;
pub mod session;
pub mod terminal;
pub mod theme;
pub mod ui;
pub mod updater;

// Re-export main types
pub use app::App;
pub use clipboard::Clipboard;
pub use config::Config;
pub use editor::Editor;
pub use extension::ExtensionManager;
pub use filebrowser::FileBrowser;
pub use terminal::Terminal;
pub use theme::{Theme, ThemeManager, ThemePreset};
