//! Language Server Protocol (LSP) integration.
//!
//! Provides intelligent completion through LSP language servers.
//!
//! ## Supported Language Servers
//!
//! | Language | Server | Command |
//! |----------|--------|---------|
//! | Rust | rust-analyzer | `rust-analyzer` |
//! | Python | pylsp | `pylsp` |
//! | JavaScript | typescript-language-server | `typescript-language-server --stdio` |
//! | TypeScript | typescript-language-server | `typescript-language-server --stdio` |
//! | Java | jdtls | `jdtls` |
//! | C# | omnisharp | `omnisharp -lsp` |
//! | PHP | intelephense | `intelephense --stdio` |
//! | SQL | sql-language-server | `sql-language-server up --method stdio` |
//! | HTML | vscode-html-language-server | `vscode-html-language-server --stdio` |
//! | CSS | vscode-css-language-server | `vscode-css-language-server --stdio` |
//! | Go | gopls | `gopls` |
//! | C/C++ | clangd | `clangd` |
//!
//! ## Usage
//!
//! ```ignore
//! use ratterm::completion::lsp::LspProvider;
//!
//! let provider = LspProvider::new(PathBuf::from("."));
//!
//! // The provider will lazily start servers as needed
//! let result = provider.complete(&context).await;
//! ```

pub mod client;
pub mod config;
pub mod manager;

pub use client::{LspClient, LspError};
pub use config::{LspConfig, LspConfigRegistry, detect_language};
pub use manager::{LspManager, LspProvider};
