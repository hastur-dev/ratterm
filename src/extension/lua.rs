//! Lua plugin system.
//!
//! Provides loading and management of Lua extensions with full system access.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mlua::{Function, Lua};

use super::manifest::ExtensionManifest;
use super::ExtensionError;
use crate::extension::lua_api::{
    self, EditorOp, LuaContext, LuaState, TerminalOp,
};
use crate::extension::lua_api::events::{dispatch_event, EventType};
use crate::extension::lua_api::timers::process_timers;

/// A loaded Lua plugin.
pub struct LuaPlugin {
    /// Plugin name.
    name: String,
    /// Plugin version.
    version: String,
    /// Plugin directory.
    dir: PathBuf,
    /// Lua runtime.
    lua: Lua,
    /// Shared state.
    state: Arc<Mutex<LuaState>>,
    /// Context for read operations.
    context: Arc<Mutex<LuaContext>>,
}

impl LuaPlugin {
    /// Creates a new Lua plugin from a manifest and directory.
    pub fn new(manifest: &ExtensionManifest, dir: &Path) -> Result<Self, ExtensionError> {
        let lua_config = manifest.lua.as_ref().ok_or_else(|| {
            ExtensionError::PluginLoad("Missing [lua] section in manifest".to_string())
        })?;

        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        // Register the ratterm API
        lua_api::register_api(&lua, state.clone(), context.clone())
            .map_err(|e| ExtensionError::PluginLoad(format!("Failed to register API: {}", e)))?;

        let plugin = Self {
            name: manifest.extension.name.clone(),
            version: manifest.extension.version.clone(),
            dir: dir.to_path_buf(),
            lua,
            state,
            context,
        };

        // Load preload files
        for preload in &lua_config.preload {
            plugin.load_file(preload)?;
        }

        // Load main file
        plugin.load_file(&lua_config.main)?;

        Ok(plugin)
    }

    /// Loads and executes a Lua file.
    fn load_file(&self, filename: &str) -> Result<(), ExtensionError> {
        let path = self.dir.join(filename);
        let content = fs::read_to_string(&path).map_err(|e| {
            ExtensionError::PluginLoad(format!("Failed to read {}: {}", filename, e))
        })?;

        self.lua
            .load(&content)
            .set_name(filename)
            .exec()
            .map_err(|e| ExtensionError::PluginLoad(format!("Lua error in {}: {}", filename, e)))?;

        Ok(())
    }

    /// Calls the `on_load()` lifecycle hook.
    pub fn call_on_load(&self) -> Result<(), ExtensionError> {
        if let Ok(on_load) = self.lua.globals().get::<Function>("on_load") {
            on_load
                .call::<()>(())
                .map_err(|e| ExtensionError::PluginLoad(format!("on_load error: {}", e)))?;
        }
        Ok(())
    }

    /// Calls the `on_unload()` lifecycle hook.
    pub fn call_on_unload(&self) {
        if let Ok(on_unload) = self.lua.globals().get::<Function>("on_unload") {
            if let Err(e) = on_unload.call::<()>(()) {
                tracing::warn!("on_unload error for {}: {}", self.name, e);
            }
        }
    }

    /// Updates the context with current application state.
    pub fn update_context(&self, ctx: LuaContext) {
        if let Ok(mut c) = self.context.lock() {
            *c = ctx;
        }
    }

    /// Takes pending notifications.
    pub fn take_notifications(&self) -> Vec<String> {
        if let Ok(mut s) = self.state.lock() {
            std::mem::take(&mut s.notifications)
        } else {
            Vec::new()
        }
    }

    /// Takes pending editor operations.
    pub fn take_editor_ops(&self) -> Vec<EditorOp> {
        if let Ok(mut s) = self.state.lock() {
            std::mem::take(&mut s.editor_ops)
        } else {
            Vec::new()
        }
    }

    /// Takes pending terminal operations.
    pub fn take_terminal_ops(&self) -> Vec<TerminalOp> {
        if let Ok(mut s) = self.state.lock() {
            std::mem::take(&mut s.terminal_ops)
        } else {
            Vec::new()
        }
    }

    /// Dispatches an event to this plugin.
    pub fn dispatch_event(&self, event_type: EventType, arg: &str) {
        if let Err(e) = dispatch_event(&self.lua, &self.state, event_type, arg.to_string()) {
            tracing::warn!("Event dispatch error for {}: {}", self.name, e);
        }
    }

    /// Processes timer callbacks.
    pub fn process_timers(&self) {
        if let Err(e) = process_timers(&self.lua, &self.state) {
            tracing::warn!("Timer processing error for {}: {}", self.name, e);
        }
    }

    /// Returns the next timer timeout for this plugin.
    pub fn next_timer_timeout(&self) -> Option<std::time::Duration> {
        self.state.lock().ok().and_then(|s| s.timers.next_timeout())
    }

    /// Gets registered commands from this plugin.
    pub fn get_commands(&self) -> Vec<(String, String, String)> {
        if let Ok(s) = self.state.lock() {
            s.commands
                .all()
                .iter()
                .map(|c| (c.id.clone(), c.name.clone(), c.description.clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Executes a registered command.
    pub fn execute_command(&self, id: &str, args: &[String]) -> Result<(), ExtensionError> {
        crate::extension::lua_api::commands::execute_command(&self.lua, &self.state, id, args)
            .map_err(|e| ExtensionError::PluginLoad(format!("Command error: {}", e)))
    }

    /// Returns the plugin name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the plugin version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

impl Drop for LuaPlugin {
    fn drop(&mut self) {
        self.call_on_unload();
    }
}

/// Manages all loaded Lua plugins.
#[derive(Default)]
pub struct LuaPluginManager {
    /// Loaded plugins by name.
    plugins: HashMap<String, LuaPlugin>,
}

impl LuaPluginManager {
    /// Creates a new plugin manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Loads a Lua plugin from a directory.
    pub fn load(&mut self, manifest: &ExtensionManifest, dir: &Path) -> Result<(), ExtensionError> {
        let plugin = LuaPlugin::new(manifest, dir)?;
        plugin.call_on_load()?;

        tracing::info!("Loaded Lua plugin: {} v{}", plugin.name(), plugin.version());
        self.plugins.insert(plugin.name().to_string(), plugin);

        Ok(())
    }

    /// Unloads a plugin by name.
    pub fn unload(&mut self, name: &str) -> bool {
        if self.plugins.remove(name).is_some() {
            tracing::info!("Unloaded Lua plugin: {}", name);
            true
        } else {
            false
        }
    }

    /// Gets a plugin by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&LuaPlugin> {
        self.plugins.get(name)
    }

    /// Gets a mutable reference to a plugin by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut LuaPlugin> {
        self.plugins.get_mut(name)
    }

    /// Returns all loaded plugins.
    #[must_use]
    pub fn all(&self) -> impl Iterator<Item = &LuaPlugin> {
        self.plugins.values()
    }

    /// Returns all loaded plugins mutably.
    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut LuaPlugin> {
        self.plugins.values_mut()
    }

    /// Updates context for all plugins.
    pub fn update_all_contexts(&mut self, ctx: LuaContext) {
        for plugin in self.plugins.values() {
            plugin.update_context(LuaContext {
                editor_content: ctx.editor_content.clone(),
                cursor_pos: ctx.cursor_pos,
                current_file: ctx.current_file.clone(),
                terminal_lines: ctx.terminal_lines.clone(),
                terminal_size: ctx.terminal_size,
                theme_name: ctx.theme_name.clone(),
                config: ctx.config.clone(),
            });
        }
    }

    /// Dispatches an event to all plugins.
    pub fn dispatch_event(&self, event_type: EventType, arg: &str) {
        for plugin in self.plugins.values() {
            plugin.dispatch_event(event_type, arg);
        }
    }

    /// Processes timers for all plugins.
    pub fn process_timers(&self) {
        for plugin in self.plugins.values() {
            plugin.process_timers();
        }
    }

    /// Returns the minimum timeout across all plugin timers.
    #[must_use]
    pub fn next_timer_timeout(&self) -> Option<std::time::Duration> {
        self.plugins
            .values()
            .filter_map(LuaPlugin::next_timer_timeout)
            .min()
    }

    /// Collects all pending notifications from all plugins.
    pub fn take_all_notifications(&mut self) -> Vec<String> {
        let mut notifications = Vec::new();
        for plugin in self.plugins.values() {
            notifications.extend(plugin.take_notifications());
        }
        notifications
    }

    /// Collects all pending editor operations from all plugins.
    pub fn take_all_editor_ops(&mut self) -> Vec<EditorOp> {
        let mut ops = Vec::new();
        for plugin in self.plugins.values() {
            ops.extend(plugin.take_editor_ops());
        }
        ops
    }

    /// Collects all pending terminal operations from all plugins.
    pub fn take_all_terminal_ops(&mut self) -> Vec<TerminalOp> {
        let mut ops = Vec::new();
        for plugin in self.plugins.values() {
            ops.extend(plugin.take_terminal_ops());
        }
        ops
    }

    /// Gets all registered commands from all plugins.
    pub fn get_all_commands(&self) -> Vec<(String, String, String)> {
        let mut commands = Vec::new();
        for plugin in self.plugins.values() {
            commands.extend(plugin.get_commands());
        }
        commands
    }

    /// Executes a command by ID (searches all plugins).
    pub fn execute_command(&self, id: &str, args: &[String]) -> Result<(), ExtensionError> {
        for plugin in self.plugins.values() {
            // Check if this plugin has the command
            let commands = plugin.get_commands();
            if commands.iter().any(|(cmd_id, _, _)| cmd_id == id) {
                return plugin.execute_command(id, args);
            }
        }
        Err(ExtensionError::NotFound(format!("Command not found: {}", id)))
    }

    /// Returns the number of loaded plugins.
    #[must_use]
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Returns true if no plugins are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_extension(dir: &Path, main_content: &str) -> ExtensionManifest {
        // Create extension.toml
        let manifest_content = r#"
[extension]
name = "test-lua-ext"
version = "1.0.0"
type = "lua"

[lua]
main = "init.lua"
"#;
        let mut manifest_file = fs::File::create(dir.join("extension.toml")).expect("create");
        manifest_file
            .write_all(manifest_content.as_bytes())
            .expect("write");

        // Create init.lua
        let mut lua_file = fs::File::create(dir.join("init.lua")).expect("create");
        lua_file.write_all(main_content.as_bytes()).expect("write");

        // Parse manifest
        super::super::manifest::load_manifest(&dir.join("extension.toml")).expect("load manifest")
    }

    #[test]
    fn test_lua_plugin_load() {
        let dir = TempDir::new().expect("temp dir");
        let manifest = create_test_extension(
            dir.path(),
            r#"
            function on_load()
                ratterm.notify("Plugin loaded!")
            end
            "#,
        );

        let plugin = LuaPlugin::new(&manifest, dir.path()).expect("create plugin");
        plugin.call_on_load().expect("on_load");

        let notifications = plugin.take_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0], "Plugin loaded!");
    }

    #[test]
    fn test_lua_plugin_commands() {
        let dir = TempDir::new().expect("temp dir");
        let manifest = create_test_extension(
            dir.path(),
            r#"
            function on_load()
                ratterm.commands.register("test.greet", function(args)
                    ratterm.notify("Hello, " .. (args[1] or "World") .. "!")
                end, {
                    name = "Greet",
                    description = "Says hello"
                })
            end
            "#,
        );

        let plugin = LuaPlugin::new(&manifest, dir.path()).expect("create plugin");
        plugin.call_on_load().expect("on_load");

        // Check command was registered
        let commands = plugin.get_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "test.greet");

        // Execute command
        plugin
            .execute_command("test.greet", &["Claude".to_string()])
            .expect("execute");

        let notifications = plugin.take_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0], "Hello, Claude!");
    }

    #[test]
    fn test_lua_plugin_manager() {
        let dir = TempDir::new().expect("temp dir");
        let manifest = create_test_extension(
            dir.path(),
            r#"
            function on_load()
                ratterm.notify("Loaded!")
            end
            "#,
        );

        let mut manager = LuaPluginManager::new();
        manager.load(&manifest, dir.path()).expect("load");

        assert_eq!(manager.count(), 1);
        assert!(!manager.is_empty());
        assert!(manager.get("test-lua-ext").is_some());

        let notifications: Vec<String> = manager.take_all_notifications();
        assert_eq!(notifications.len(), 1);
    }

    #[test]
    fn test_lua_plugin_events() {
        let dir = TempDir::new().expect("temp dir");
        let manifest = create_test_extension(
            dir.path(),
            r#"
            saved_path = nil
            function on_load()
                ratterm.events.on("file_save", function(path)
                    saved_path = path
                    ratterm.notify("Saved: " .. path)
                end)
            end
            "#,
        );

        let plugin = LuaPlugin::new(&manifest, dir.path()).expect("create plugin");
        plugin.call_on_load().expect("on_load");

        // Dispatch event
        plugin.dispatch_event(EventType::FileSave, "/test/file.rs");

        let notifications = plugin.take_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0], "Saved: /test/file.rs");
    }
}
