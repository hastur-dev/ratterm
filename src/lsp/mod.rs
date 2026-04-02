//! Language Server Protocol integration.
//!
//! Provides a full LSP client with support for:
//! - Completion (delegated to `crate::completion`)
//! - Hover information
//! - Go-to-definition, type definition, implementation
//! - Find references
//! - Symbol rename
//! - Diagnostics (errors, warnings)
//! - Code actions (quick fixes)
//! - Signature help
//! - Document and workspace symbols
//! - Document formatting

pub mod actions;
pub mod client;
pub mod config;
pub mod definition;
pub mod diagnostics;
pub mod formatting;
pub mod hover;
pub mod manager;
pub mod references;
pub mod rename;
pub mod signature;
pub mod symbols;

pub use actions::CodeActionResult;
pub use client::{LspClient, LspError, LspNotificationMessage};
pub use config::{LspConfig, LspConfigRegistry, detect_language};
pub use definition::LocationResult;
pub use diagnostics::{DiagnosticInfo, DiagnosticSeverity, DiagnosticStore};
pub use formatting::TextEditResult;
pub use hover::HoverResult;
pub use manager::LspManager;
pub use rename::WorkspaceEditResult;
pub use signature::SignatureHelpResult;
pub use symbols::{DocumentSymbolResult, SymbolInfoResult};
