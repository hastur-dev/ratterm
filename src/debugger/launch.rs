//! Launch configuration for debug sessions.
//!
//! Parses `.ratterm/launch.json` (VS Code-compatible subset) and provides
//! adapter auto-detection based on file extension.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Kind of debug adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterKind {
    /// CodeLLDB for Rust/C/C++.
    CodeLldb,
    /// Python debugpy.
    Debugpy,
    /// Node.js debug adapter.
    NodeDebug,
    /// Go delve debugger.
    Delve,
    /// Custom adapter with a given executable path.
    Custom(String),
}

impl AdapterKind {
    /// Returns the default adapter executable name.
    #[must_use]
    pub fn executable(&self) -> &str {
        match self {
            Self::CodeLldb => "codelldb",
            Self::Debugpy => "debugpy-adapter",
            Self::NodeDebug => "node-debug2",
            Self::Delve => "dlv",
            Self::Custom(path) => path,
        }
    }

    /// Returns a human-readable name for display.
    #[must_use]
    pub fn display_name(&self) -> &str {
        match self {
            Self::CodeLldb => "CodeLLDB",
            Self::Debugpy => "debugpy",
            Self::NodeDebug => "Node Debug",
            Self::Delve => "Delve",
            Self::Custom(_) => "Custom",
        }
    }
}

/// Detects the appropriate debug adapter for a file extension.
#[must_use]
pub fn detect_adapter(file_ext: &str) -> Option<AdapterKind> {
    match file_ext.trim_start_matches('.') {
        "rs" => Some(AdapterKind::CodeLldb),
        "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => Some(AdapterKind::CodeLldb),
        "py" => Some(AdapterKind::Debugpy),
        "js" | "ts" | "jsx" | "tsx" | "mjs" => Some(AdapterKind::NodeDebug),
        "go" => Some(AdapterKind::Delve),
        _ => None,
    }
}

/// A single launch configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchConfig {
    /// Display name for this configuration.
    #[serde(default)]
    pub name: String,
    /// Request type: "launch" or "attach".
    #[serde(default = "default_request")]
    pub request: String,
    /// Program to debug (executable path).
    #[serde(default)]
    pub program: String,
    /// Command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Debug adapter to use.
    #[serde(default)]
    pub adapter: Option<String>,
    /// Whether to stop at entry point.
    #[serde(default)]
    pub stop_on_entry: bool,
}

fn default_request() -> String {
    "launch".to_string()
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            name: "Debug".to_string(),
            request: "launch".to_string(),
            program: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            adapter: None,
            stop_on_entry: false,
        }
    }
}

impl LaunchConfig {
    /// Returns the adapter kind for this config.
    #[must_use]
    pub fn adapter_kind(&self) -> Option<AdapterKind> {
        if let Some(ref adapter) = self.adapter {
            match adapter.as_str() {
                "codelldb" => Some(AdapterKind::CodeLldb),
                "debugpy" => Some(AdapterKind::Debugpy),
                "node-debug" | "node-debug2" => Some(AdapterKind::NodeDebug),
                "dlv" | "delve" => Some(AdapterKind::Delve),
                other => Some(AdapterKind::Custom(other.to_string())),
            }
        } else {
            // Try to detect from program extension
            let ext = Path::new(&self.program)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            detect_adapter(ext)
        }
    }
}

/// On-disk format for launch.json.
#[derive(Debug, Serialize, Deserialize)]
struct LaunchFile {
    #[serde(default)]
    configurations: Vec<LaunchConfig>,
}

/// Loads launch configurations from `.ratterm/launch.json`.
///
/// Returns an empty vec if the file doesn't exist or can't be parsed.
#[must_use]
pub fn load_launch_configs(project_root: &Path) -> Vec<LaunchConfig> {
    let path = project_root.join(".ratterm").join("launch.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(file) = serde_json::from_str::<LaunchFile>(&content) else {
        return Vec::new();
    };
    file.configurations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_adapter_rust() {
        assert_eq!(detect_adapter("rs"), Some(AdapterKind::CodeLldb));
        assert_eq!(detect_adapter(".rs"), Some(AdapterKind::CodeLldb));
    }

    #[test]
    fn test_detect_adapter_python() {
        assert_eq!(detect_adapter("py"), Some(AdapterKind::Debugpy));
    }

    #[test]
    fn test_detect_adapter_javascript() {
        assert_eq!(detect_adapter("js"), Some(AdapterKind::NodeDebug));
        assert_eq!(detect_adapter("ts"), Some(AdapterKind::NodeDebug));
        assert_eq!(detect_adapter("tsx"), Some(AdapterKind::NodeDebug));
    }

    #[test]
    fn test_detect_adapter_go() {
        assert_eq!(detect_adapter("go"), Some(AdapterKind::Delve));
    }

    #[test]
    fn test_detect_adapter_cpp() {
        assert_eq!(detect_adapter("cpp"), Some(AdapterKind::CodeLldb));
        assert_eq!(detect_adapter("cc"), Some(AdapterKind::CodeLldb));
        assert_eq!(detect_adapter("h"), Some(AdapterKind::CodeLldb));
    }

    #[test]
    fn test_detect_adapter_unknown() {
        assert_eq!(detect_adapter("txt"), None);
        assert_eq!(detect_adapter("md"), None);
    }

    #[test]
    fn test_adapter_kind_executable() {
        assert_eq!(AdapterKind::CodeLldb.executable(), "codelldb");
        assert_eq!(AdapterKind::Debugpy.executable(), "debugpy-adapter");
        assert_eq!(AdapterKind::NodeDebug.executable(), "node-debug2");
        assert_eq!(AdapterKind::Delve.executable(), "dlv");
        assert_eq!(
            AdapterKind::Custom("/usr/bin/my-adapter".to_string()).executable(),
            "/usr/bin/my-adapter"
        );
    }

    #[test]
    fn test_launch_config_default() {
        let config = LaunchConfig::default();
        assert_eq!(config.name, "Debug");
        assert_eq!(config.request, "launch");
        assert!(config.program.is_empty());
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert!(!config.stop_on_entry);
    }

    #[test]
    fn test_parse_launch_json() {
        let json = r#"{
            "configurations": [
                {
                    "name": "Debug Rust",
                    "request": "launch",
                    "program": "target/debug/myapp",
                    "args": ["--verbose"],
                    "env": {"RUST_LOG": "debug"},
                    "cwd": "/home/user/project",
                    "adapter": "codelldb",
                    "stop_on_entry": true
                }
            ]
        }"#;

        let file: LaunchFile = serde_json::from_str(json).expect("parse launch.json");
        assert_eq!(file.configurations.len(), 1);

        let config = &file.configurations[0];
        assert_eq!(config.name, "Debug Rust");
        assert_eq!(config.program, "target/debug/myapp");
        assert_eq!(config.args, vec!["--verbose"]);
        assert_eq!(config.env.get("RUST_LOG").unwrap(), "debug");
        assert_eq!(config.cwd.as_deref(), Some("/home/user/project"));
        assert_eq!(config.adapter.as_deref(), Some("codelldb"));
        assert!(config.stop_on_entry);
    }

    #[test]
    fn test_launch_config_adapter_kind() {
        let config = LaunchConfig {
            adapter: Some("codelldb".to_string()),
            ..LaunchConfig::default()
        };
        assert_eq!(config.adapter_kind(), Some(AdapterKind::CodeLldb));
    }

    #[test]
    fn test_launch_config_adapter_kind_from_program() {
        let config = LaunchConfig {
            program: "target/debug/myapp.rs".to_string(),
            ..LaunchConfig::default()
        };
        assert_eq!(config.adapter_kind(), Some(AdapterKind::CodeLldb));
    }

    #[test]
    fn test_load_launch_configs_nonexistent() {
        let configs = load_launch_configs(Path::new("/nonexistent/path"));
        assert!(configs.is_empty());
    }

    #[test]
    fn test_load_launch_configs_from_file() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let ratterm_dir = tmp.path().join(".ratterm");
        std::fs::create_dir_all(&ratterm_dir).expect("mkdir");
        std::fs::write(
            ratterm_dir.join("launch.json"),
            r#"{"configurations": [{"name": "test", "program": "a.out"}]}"#,
        )
        .expect("write");

        let configs = load_launch_configs(tmp.path());
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "test");
        assert_eq!(configs[0].program, "a.out");
    }
}
