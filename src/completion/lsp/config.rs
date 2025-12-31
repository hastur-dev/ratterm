//! Language server configurations.
//!
//! Defines the configuration for each supported language server
//! including command, arguments, and file extensions.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Configuration for a language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// Language identifier (e.g., "rust", "python").
    pub language_id: String,

    /// Server executable command.
    pub command: String,

    /// Command-line arguments.
    pub args: Vec<String>,

    /// File extensions this server handles.
    pub extensions: Vec<String>,

    /// Whether to use stdio for communication.
    pub use_stdio: bool,

    /// Initialization options (passed to LSP initialize).
    pub init_options: Option<serde_json::Value>,

    /// Root URI patterns for workspace detection.
    pub root_patterns: Vec<String>,
}

impl LspConfig {
    /// Creates a new LSP configuration.
    #[must_use]
    pub fn new(language_id: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            language_id: language_id.into(),
            command: command.into(),
            args: Vec::new(),
            extensions: Vec::new(),
            use_stdio: true,
            init_options: None,
            root_patterns: Vec::new(),
        }
    }

    /// Adds command-line arguments.
    #[must_use]
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Adds file extensions.
    #[must_use]
    pub fn with_extensions(mut self, exts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.extensions = exts.into_iter().map(Into::into).collect();
        self
    }

    /// Adds root patterns for workspace detection.
    #[must_use]
    pub fn with_root_patterns(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.root_patterns = patterns.into_iter().map(Into::into).collect();
        self
    }

    /// Sets initialization options.
    #[must_use]
    pub fn with_init_options(mut self, options: serde_json::Value) -> Self {
        self.init_options = Some(options);
        self
    }

    /// Returns whether this config handles the given file extension.
    #[must_use]
    pub fn handles_extension(&self, ext: &str) -> bool {
        self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
    }

    /// Returns the command with platform-specific adjustments.
    #[must_use]
    pub fn platform_command(&self) -> String {
        #[cfg(windows)]
        {
            if !self.command.ends_with(".exe") && !self.command.contains('.') {
                format!("{}.exe", self.command)
            } else {
                self.command.clone()
            }
        }
        #[cfg(not(windows))]
        {
            self.command.clone()
        }
    }
}

/// Registry of language server configurations.
#[derive(Debug)]
pub struct LspConfigRegistry {
    /// Configurations by language ID.
    configs: HashMap<String, LspConfig>,

    /// Extension to language ID mapping.
    ext_to_lang: HashMap<String, String>,
}

impl LspConfigRegistry {
    /// Creates a new registry with default configurations.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            configs: HashMap::new(),
            ext_to_lang: HashMap::new(),
        };

        // Register default language servers
        registry.register_defaults();
        registry
    }

    /// Registers a language server configuration.
    pub fn register(&mut self, config: LspConfig) {
        let lang_id = config.language_id.clone();

        // Update extension mapping
        for ext in &config.extensions {
            self.ext_to_lang.insert(ext.to_lowercase(), lang_id.clone());
        }

        self.configs.insert(lang_id, config);
    }

    /// Gets configuration for a language ID.
    #[must_use]
    pub fn get(&self, language_id: &str) -> Option<&LspConfig> {
        self.configs.get(language_id)
    }

    /// Gets configuration for a file path.
    #[must_use]
    pub fn get_for_file(&self, path: &Path) -> Option<&LspConfig> {
        let ext = path.extension()?.to_str()?;
        let lang_id = self.ext_to_lang.get(&ext.to_lowercase())?;
        self.configs.get(lang_id)
    }

    /// Gets the language ID for a file extension.
    #[must_use]
    pub fn language_id_for_extension(&self, ext: &str) -> Option<&str> {
        self.ext_to_lang
            .get(&ext.to_lowercase())
            .map(String::as_str)
    }

    /// Returns all registered configurations.
    pub fn all(&self) -> impl Iterator<Item = &LspConfig> {
        self.configs.values()
    }

    /// Registers default language server configurations.
    fn register_defaults(&mut self) {
        // Rust - rust-analyzer
        self.register(
            LspConfig::new("rust", "rust-analyzer")
                .with_extensions(["rs"])
                .with_root_patterns(["Cargo.toml", "rust-project.json"]),
        );

        // Python - pylsp or pyright
        self.register(
            LspConfig::new("python", "pylsp")
                .with_extensions(["py", "pyi", "pyw"])
                .with_root_patterns(["pyproject.toml", "setup.py", "requirements.txt"]),
        );

        // JavaScript/TypeScript - typescript-language-server
        self.register(
            LspConfig::new("javascript", "typescript-language-server")
                .with_args(["--stdio"])
                .with_extensions(["js", "jsx", "mjs", "cjs"])
                .with_root_patterns(["package.json", "tsconfig.json", "jsconfig.json"]),
        );

        self.register(
            LspConfig::new("typescript", "typescript-language-server")
                .with_args(["--stdio"])
                .with_extensions(["ts", "tsx", "mts", "cts"])
                .with_root_patterns(["package.json", "tsconfig.json"]),
        );

        // Java - jdtls
        self.register(
            LspConfig::new("java", "jdtls")
                .with_extensions(["java"])
                .with_root_patterns(["pom.xml", "build.gradle", "build.gradle.kts", ".project"]),
        );

        // C# - omnisharp
        self.register(
            LspConfig::new("csharp", "omnisharp")
                .with_args(["-lsp"])
                .with_extensions(["cs", "csx"])
                .with_root_patterns(["*.csproj", "*.sln"]),
        );

        // PHP - intelephense
        self.register(
            LspConfig::new("php", "intelephense")
                .with_args(["--stdio"])
                .with_extensions(["php", "phtml", "php3", "php4", "php5", "phps"])
                .with_root_patterns(["composer.json"]),
        );

        // SQL - sql-language-server
        self.register(
            LspConfig::new("sql", "sql-language-server")
                .with_args(["up", "--method", "stdio"])
                .with_extensions(["sql"]),
        );

        // HTML - vscode-html-languageserver
        self.register(
            LspConfig::new("html", "vscode-html-language-server")
                .with_args(["--stdio"])
                .with_extensions(["html", "htm", "xhtml"]),
        );

        // CSS - vscode-css-languageserver
        self.register(
            LspConfig::new("css", "vscode-css-language-server")
                .with_args(["--stdio"])
                .with_extensions(["css", "scss", "less"]),
        );

        // JSON - vscode-json-languageserver
        self.register(
            LspConfig::new("json", "vscode-json-language-server")
                .with_args(["--stdio"])
                .with_extensions(["json", "jsonc"]),
        );

        // YAML - yaml-language-server
        self.register(
            LspConfig::new("yaml", "yaml-language-server")
                .with_args(["--stdio"])
                .with_extensions(["yaml", "yml"]),
        );

        // Go - gopls
        self.register(
            LspConfig::new("go", "gopls")
                .with_extensions(["go", "mod"])
                .with_root_patterns(["go.mod", "go.work"]),
        );

        // C/C++ - clangd
        self.register(
            LspConfig::new("c", "clangd")
                .with_extensions(["c", "h"])
                .with_root_patterns(["compile_commands.json", "CMakeLists.txt", "Makefile"]),
        );

        self.register(
            LspConfig::new("cpp", "clangd")
                .with_extensions(["cpp", "cc", "cxx", "hpp", "hxx", "hh"])
                .with_root_patterns(["compile_commands.json", "CMakeLists.txt", "Makefile"]),
        );
    }
}

impl Default for LspConfigRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Detects the language ID from a file path.
#[must_use]
pub fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;

    Some(
        match ext.to_lowercase().as_str() {
            "rs" => "rust",
            "py" | "pyi" | "pyw" => "python",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "ts" | "tsx" | "mts" | "cts" => "typescript",
            "java" => "java",
            "cs" | "csx" => "csharp",
            "php" | "phtml" => "php",
            "sql" => "sql",
            "html" | "htm" | "xhtml" => "html",
            "css" | "scss" | "less" => "css",
            "json" | "jsonc" => "json",
            "yaml" | "yml" => "yaml",
            "go" | "mod" => "go",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
            "md" | "markdown" => "markdown",
            "toml" => "toml",
            "xml" => "xml",
            "sh" | "bash" | "zsh" => "shellscript",
            "ps1" | "psm1" | "psd1" => "powershell",
            _ => return None,
        }
        .to_string(),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_config_creation() {
        let config = LspConfig::new("rust", "rust-analyzer")
            .with_extensions(["rs"])
            .with_root_patterns(["Cargo.toml"]);

        assert_eq!(config.language_id, "rust");
        assert_eq!(config.command, "rust-analyzer");
        assert!(config.handles_extension("rs"));
        assert!(!config.handles_extension("py"));
    }

    #[test]
    fn test_registry_get_for_file() {
        let registry = LspConfigRegistry::new();

        let rust_config = registry.get_for_file(Path::new("src/main.rs"));
        assert!(rust_config.is_some());
        assert_eq!(rust_config.unwrap().language_id, "rust");

        let py_config = registry.get_for_file(Path::new("script.py"));
        assert!(py_config.is_some());
        assert_eq!(py_config.unwrap().language_id, "python");
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(
            detect_language(Path::new("main.rs")),
            Some("rust".to_string())
        );
        assert_eq!(
            detect_language(Path::new("script.py")),
            Some("python".to_string())
        );
        assert_eq!(
            detect_language(Path::new("app.tsx")),
            Some("typescript".to_string())
        );
        assert_eq!(detect_language(Path::new("noext")), None);
    }

    #[test]
    fn test_platform_command() {
        let config = LspConfig::new("rust", "rust-analyzer");

        #[cfg(windows)]
        assert_eq!(config.platform_command(), "rust-analyzer.exe");

        #[cfg(not(windows))]
        assert_eq!(config.platform_command(), "rust-analyzer");
    }
}
