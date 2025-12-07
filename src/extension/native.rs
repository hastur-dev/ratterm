//! Native plugin loading using libloading.
//!
//! Loads compiled native plugins (.dll/.so/.dylib) with security confirmations.

use std::path::Path;

use libloading::{Library, Symbol};

use super::api::{PluginCapability, PluginError, PluginHost, PluginInfo, PluginType, RattermPlugin, WidgetCell};
use super::ExtensionError;

/// Function signature for plugin initialization.
type PluginInitFn = unsafe extern "C" fn() -> *mut dyn RattermPlugin;

/// Function signature for plugin destruction.
type PluginDestroyFn = unsafe extern "C" fn(*mut dyn RattermPlugin);

/// Native plugin instance.
pub struct NativePlugin {
    /// Plugin info.
    info: PluginInfo,
    /// Loaded library.
    _library: Library,
    /// Plugin instance pointer.
    instance: Option<*mut dyn RattermPlugin>,
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
            instance: Some(instance),
            destroy_fn,
            loaded: false,
        })
    }
}

impl Drop for NativePlugin {
    fn drop(&mut self) {
        if let Some(instance) = self.instance.take() {
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

    fn on_load(&mut self, host: &dyn PluginHost) -> Result<(), PluginError> {
        if let Some(instance) = self.instance {
            unsafe {
                let plugin = &mut *instance;
                plugin.on_load(host)?;
            }
        }
        self.loaded = true;
        Ok(())
    }

    fn on_unload(&mut self) {
        if let Some(instance) = self.instance {
            unsafe {
                let plugin = &mut *instance;
                plugin.on_unload();
            }
        }
        self.loaded = false;
    }

    fn execute_command(&mut self, cmd: &str, args: &[&str]) -> Result<(), PluginError> {
        if !self.loaded {
            return Err(PluginError::Other("Plugin not loaded".to_string()));
        }
        if let Some(instance) = self.instance {
            unsafe {
                let plugin = &mut *instance;
                return plugin.execute_command(cmd, args);
            }
        }
        Ok(())
    }

    fn render_widget(&self, area: ratatui::layout::Rect) -> Option<Vec<WidgetCell>> {
        if !self.loaded {
            return None;
        }
        if let Some(instance) = self.instance {
            unsafe {
                let plugin = &*instance;
                return plugin.render_widget(area);
            }
        }
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
