//! Native plugin loading using libloading.
//!
//! Loads compiled native plugins (.dll/.so/.dylib) with security confirmations.
//!
//! Note: Native plugin support is experimental. Due to FFI safety constraints,
//! plugins must export C-compatible functions rather than Rust trait objects.

use std::ffi::c_void;
use std::path::Path;

use libloading::{Library, Symbol};

use super::ExtensionError;
use super::api::{
    PluginCapability, PluginError, PluginHost, PluginInfo, PluginType, RattermPlugin, WidgetCell,
};

/// Function signature for plugin initialization.
/// Returns an opaque pointer to the plugin instance.
type PluginInitFn = unsafe extern "C" fn() -> *mut c_void;

/// Function signature for plugin destruction.
/// Takes an opaque pointer to the plugin instance.
type PluginDestroyFn = unsafe extern "C" fn(*mut c_void);

/// Native plugin instance.
pub struct NativePlugin {
    /// Plugin info.
    info: PluginInfo,
    /// Loaded library (kept alive for symbol validity).
    _library: Library,
    /// Plugin instance pointer (opaque for FFI safety).
    _instance: Option<*mut c_void>,
    /// Destroy function.
    destroy_fn: Option<PluginDestroyFn>,
    /// Whether the plugin is loaded.
    loaded: bool,
}

// Safety: Native plugins are loaded in a controlled manner
unsafe impl Send for NativePlugin {}
unsafe impl Sync for NativePlugin {}

impl NativePlugin {
    /// Loads a native plugin from a library file.
    ///
    /// # Safety
    /// This loads and executes arbitrary native code. Only load trusted plugins.
    pub unsafe fn load(
        path: &Path,
        name: &str,
        version: &str,
        capabilities: Vec<PluginCapability>,
    ) -> Result<Self, ExtensionError> {
        // SAFETY: Caller guarantees the library is trusted
        let library = unsafe {
            Library::new(path)
                .map_err(|e| ExtensionError::PluginLoad(format!("Failed to load library: {}", e)))?
        };

        // SAFETY: Looking up a symbol from the loaded library
        let init_fn: Symbol<PluginInitFn> = unsafe {
            library
                .get(b"ratterm_plugin_init")
                .map_err(|e| ExtensionError::PluginLoad(format!("Missing init function: {}", e)))?
        };

        // SAFETY: Looking up optional destroy function
        let destroy_fn: Option<PluginDestroyFn> = unsafe {
            library
                .get::<PluginDestroyFn>(b"ratterm_plugin_destroy")
                .ok()
                .map(|s| *s)
        };

        // SAFETY: Calling the plugin's init function
        let instance = unsafe { init_fn() };
        if instance.is_null() {
            return Err(ExtensionError::PluginLoad(
                "Plugin init returned null".to_string(),
            ));
        }

        Ok(Self {
            info: PluginInfo {
                name: name.to_string(),
                version: version.to_string(),
                plugin_type: PluginType::Native,
                capabilities,
            },
            _library: library,
            _instance: Some(instance),
            destroy_fn,
            loaded: false,
        })
    }

    /// Returns the instance pointer for cleanup.
    fn take_instance(&mut self) -> Option<*mut c_void> {
        self._instance.take()
    }
}

impl Drop for NativePlugin {
    fn drop(&mut self) {
        if let Some(instance) = self.take_instance() {
            if let Some(destroy_fn) = self.destroy_fn {
                unsafe {
                    destroy_fn(instance);
                }
            }
        }
    }
}

impl RattermPlugin for NativePlugin {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn on_load(&mut self, _host: &dyn PluginHost) -> Result<(), PluginError> {
        // Native plugins handle their own initialization in ratterm_plugin_init
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
        // Native command execution would require additional FFI functions
        // This is a placeholder for future implementation
        Err(PluginError::Other(
            "Native command execution not yet implemented".to_string(),
        ))
    }

    fn render_widget(&self, _area: ratatui::layout::Rect) -> Option<Vec<WidgetCell>> {
        if !self.loaded {
            return None;
        }
        // Native widget rendering would require additional FFI functions
        None
    }
}

/// Native plugin loader with security checks.
pub struct NativeLoader {
    /// Whether to require user confirmation.
    require_confirmation: bool,
}

impl Default for NativeLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeLoader {
    /// Creates a new native loader.
    #[must_use]
    pub fn new() -> Self {
        Self {
            require_confirmation: true,
        }
    }

    /// Creates a loader that doesn't require confirmation (for trusted plugins).
    #[must_use]
    pub fn trusted() -> Self {
        Self {
            require_confirmation: false,
        }
    }

    /// Checks if the plugin file exists for the current platform.
    #[must_use]
    pub fn plugin_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    /// Returns the expected extension for native plugins on this platform.
    #[must_use]
    pub fn platform_extension() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "dll"
        }
        #[cfg(target_os = "linux")]
        {
            "so"
        }
        #[cfg(target_os = "macos")]
        {
            "dylib"
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            "so"
        }
    }

    /// Loads a native plugin with safety checks.
    ///
    /// # Safety
    /// This loads and executes native code. The caller must ensure the plugin is trusted.
    pub unsafe fn load(
        &self,
        path: &Path,
        name: &str,
        version: &str,
        capabilities: Vec<PluginCapability>,
        is_trusted: bool,
    ) -> Result<NativePlugin, ExtensionError> {
        if self.require_confirmation && !is_trusted {
            return Err(ExtensionError::PluginLoad(
                "Native plugin requires user confirmation".to_string(),
            ));
        }

        if !self.plugin_exists(path) {
            return Err(ExtensionError::PluginLoad(format!(
                "Plugin file not found: {:?}",
                path
            )));
        }

        tracing::warn!(
            "Loading native plugin '{}' from {:?} - this has full system access",
            name,
            path
        );

        // SAFETY: Caller has verified the plugin is trusted
        unsafe { NativePlugin::load(path, name, version, capabilities) }
    }
}

/// Security warning message for native plugins.
pub const NATIVE_PLUGIN_WARNING: &str = r#"
WARNING: Native Plugin Security Notice

This extension contains a native plugin that has FULL ACCESS to your system.
Native plugins can:
- Read and write any files
- Access the network
- Execute system commands
- Access all memory

Only install native plugins from sources you trust completely.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_loader_new() {
        let loader = NativeLoader::new();
        assert!(loader.require_confirmation);
    }

    #[test]
    fn test_native_loader_trusted() {
        let loader = NativeLoader::trusted();
        assert!(!loader.require_confirmation);
    }

    #[test]
    fn test_platform_extension() {
        let ext = NativeLoader::platform_extension();
        #[cfg(target_os = "windows")]
        assert_eq!(ext, "dll");
        #[cfg(target_os = "linux")]
        assert_eq!(ext, "so");
        #[cfg(target_os = "macos")]
        assert_eq!(ext, "dylib");
    }
}
