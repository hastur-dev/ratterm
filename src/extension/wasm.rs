//! WASM plugin runtime using Wasmtime.
//!
//! Provides sandboxed execution of WebAssembly plugins.

use std::path::Path;
use std::sync::Arc;

use wasmtime::{Config, Engine, Module};

use super::api::{PluginCapability, PluginError, PluginInfo, PluginType, RattermPlugin, WidgetCell};
use super::ExtensionError;

/// WASM plugin instance.
pub struct WasmPlugin {
    /// Plugin info.
    info: PluginInfo,
    /// Wasmtime engine.
    _engine: Engine,
    /// Compiled module.
    _module: Module,
    /// Whether the plugin is loaded.
    loaded: bool,
}

impl WasmPlugin {
    /// Loads a WASM plugin from a file.
    pub fn load(
        path: &Path,
        name: &str,
        version: &str,
        capabilities: Vec<PluginCapability>,
    ) -> Result<Self, ExtensionError> {
        // Configure the engine with safety limits
        let mut config = Config::new();
        config.wasm_memory64(false);
        config.max_wasm_stack(512 * 1024); // 512KB stack limit

        let engine = Engine::new(&config)
            .map_err(|e| ExtensionError::PluginLoad(format!("Failed to create engine: {}", e)))?;

        let module = Module::from_file(&engine, path)
            .map_err(|e| ExtensionError::PluginLoad(format!("Failed to load WASM: {}", e)))?;

        Ok(Self {
            info: PluginInfo {
                name: name.to_string(),
                version: version.to_string(),
                plugin_type: PluginType::Wasm,
                capabilities,
            },
            _engine: engine,
            _module: module,
            loaded: false,
        })
    }
}

impl RattermPlugin for WasmPlugin {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn on_load(&mut self, _host: &dyn super::api::PluginHost) -> Result<(), PluginError> {
        // TODO: Set up WASM imports and call plugin init function
        self.loaded = true;
        Ok(())
    }

    fn on_unload(&mut self) {
        self.loaded = false;
    }

    fn execute_command(&mut self, _cmd: &str, _args: &[&str]) -> Result<(), PluginError> {
        if !self.loaded {
            return Err(PluginError::Other("Plugin not loaded".to_string()));
        }
        // TODO: Call WASM function
        Ok(())
    }

    fn render_widget(&self, _area: ratatui::layout::Rect) -> Option<Vec<WidgetCell>> {
        if !self.loaded {
            return None;
        }
        // TODO: Call WASM render function
        None
    }
}

/// WASM plugin loader.
pub struct WasmLoader {
    /// Shared engine for all plugins.
    engine: Arc<Engine>,
}

impl Default for WasmLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmLoader {
    /// Creates a new WASM loader.
    #[must_use]
    pub fn new() -> Self {
        let mut config = Config::new();
        config.wasm_memory64(false);

        let engine = Engine::new(&config).unwrap_or_else(|_| Engine::default());

        Self {
            engine: Arc::new(engine),
        }
    }

    /// Loads a WASM plugin.
    pub fn load(
        &self,
        path: &Path,
        name: &str,
        version: &str,
        capabilities: Vec<PluginCapability>,
    ) -> Result<WasmPlugin, ExtensionError> {
        WasmPlugin::load(path, name, version, capabilities)
    }

    /// Validates a WASM file without loading it.
    pub fn validate(&self, path: &Path) -> Result<(), ExtensionError> {
        let bytes = std::fs::read(path)?;
        Module::validate(&self.engine, &bytes)
            .map_err(|e| ExtensionError::PluginLoad(format!("Invalid WASM: {}", e)))
    }
}

/// Host functions exposed to WASM plugins.
pub mod host_functions {
    /// Log a message from the plugin.
    pub fn log_message(level: i32, message: &str) {
        match level {
            0 => tracing::debug!(target: "wasm_plugin", "{}", message),
            1 => tracing::info!(target: "wasm_plugin", "{}", message),
            2 => tracing::warn!(target: "wasm_plugin", "{}", message),
            _ => tracing::error!(target: "wasm_plugin", "{}", message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_loader_new() {
        let loader = WasmLoader::new();
        // Just verify it creates without panic
        drop(loader);
    }
}
