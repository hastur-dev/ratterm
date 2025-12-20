//! Lua API module for ratterm extensions.
//!
//! Provides the `ratterm` table and all API functions available to Lua scripts.
//! This module gives full system access to extensions - no sandboxing.

pub mod commands;
pub mod editor;
pub mod events;
pub mod fs;
pub mod terminal;
pub mod timers;

use std::sync::{Arc, Mutex};

use mlua::{Lua, Result as LuaResult, Table};

use self::commands::LuaCommands;
use self::editor::LuaEditor;
use self::events::LuaEvents;
use self::fs::LuaFs;
use self::terminal::LuaTerminal;
use self::timers::LuaTimers;

/// Shared state accessible from Lua callbacks.
/// This is wrapped in Arc<Mutex<>> to be safely shared.
#[derive(Default)]
pub struct LuaState {
    /// Pending notifications to display.
    pub notifications: Vec<String>,
    /// Registered commands from Lua extensions.
    pub commands: LuaCommands,
    /// Event subscriptions.
    pub events: LuaEvents,
    /// Timer state.
    pub timers: LuaTimers,
    /// Editor operations buffer.
    pub editor_ops: Vec<EditorOp>,
    /// Terminal operations buffer.
    pub terminal_ops: Vec<TerminalOp>,
}

/// Editor operations requested by Lua scripts.
#[derive(Debug, Clone)]
pub enum EditorOp {
    /// Open a file.
    Open(String),
    /// Save current file.
    Save,
    /// Set editor content.
    SetContent(String),
    /// Insert text at position.
    InsertAt { line: usize, col: usize, text: String },
    /// Set cursor position.
    SetCursor { line: usize, col: usize },
}

/// Terminal operations requested by Lua scripts.
#[derive(Debug, Clone)]
pub enum TerminalOp {
    /// Send keys to terminal.
    SendKeys(String),
}

/// Context passed to Lua API for read operations.
pub struct LuaContext {
    /// Current editor content (if any).
    pub editor_content: Option<String>,
    /// Current cursor position (line, col).
    pub cursor_pos: (usize, usize),
    /// Current file path.
    pub current_file: Option<String>,
    /// Terminal buffer lines.
    pub terminal_lines: Vec<String>,
    /// Terminal size (cols, rows).
    pub terminal_size: (u16, u16),
    /// Current theme name.
    pub theme_name: String,
    /// Config values.
    pub config: std::collections::HashMap<String, String>,
}

impl Default for LuaContext {
    fn default() -> Self {
        Self {
            editor_content: None,
            cursor_pos: (0, 0),
            current_file: None,
            terminal_lines: Vec::new(),
            terminal_size: (80, 24),
            theme_name: "dark".to_string(),
            config: std::collections::HashMap::new(),
        }
    }
}

/// Registers the `ratterm` global table with all API functions.
pub fn register_api(
    lua: &Lua,
    state: Arc<Mutex<LuaState>>,
    context: Arc<Mutex<LuaContext>>,
) -> LuaResult<()> {
    let ratterm = lua.create_table()?;

    // Register sub-modules
    ratterm.set("editor", LuaEditor::create_table(lua, state.clone(), context.clone())?)?;
    ratterm.set("terminal", LuaTerminal::create_table(lua, state.clone(), context.clone())?)?;
    ratterm.set("fs", LuaFs::create_table(lua)?)?;
    ratterm.set("commands", commands::create_table(lua, state.clone())?)?;
    ratterm.set("events", events::create_table(lua, state.clone())?)?;
    ratterm.set("timer", timers::create_table(lua, state.clone())?)?;

    // Top-level functions
    register_toplevel_functions(lua, &ratterm, state, context)?;

    lua.globals().set("ratterm", ratterm)?;
    Ok(())
}

/// Registers top-level ratterm functions.
fn register_toplevel_functions(
    lua: &Lua,
    ratterm: &Table,
    state: Arc<Mutex<LuaState>>,
    context: Arc<Mutex<LuaContext>>,
) -> LuaResult<()> {
    // ratterm.notify(message)
    let state_clone = state.clone();
    let notify = lua.create_function(move |_, message: String| {
        if let Ok(mut s) = state_clone.lock() {
            s.notifications.push(message);
        }
        Ok(())
    })?;
    ratterm.set("notify", notify)?;

    // ratterm.get_config(key)
    let context_clone = context.clone();
    let get_config = lua.create_function(move |_, key: String| {
        let result = context_clone
            .lock()
            .ok()
            .and_then(|c| c.config.get(&key).cloned());
        Ok(result)
    })?;
    ratterm.set("get_config", get_config)?;

    // ratterm.get_theme()
    let context_clone = context;
    let get_theme = lua.create_function(move |_, ()| {
        let result = context_clone
            .lock()
            .map(|c| c.theme_name.clone())
            .unwrap_or_else(|_| "dark".to_string());
        Ok(result)
    })?;
    ratterm.set("get_theme", get_theme)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_state_default() {
        let state = LuaState::default();
        assert!(state.notifications.is_empty());
        assert!(state.editor_ops.is_empty());
        assert!(state.terminal_ops.is_empty());
    }

    #[test]
    fn test_lua_context_default() {
        let ctx = LuaContext::default();
        assert!(ctx.editor_content.is_none());
        assert_eq!(ctx.cursor_pos, (0, 0));
        assert_eq!(ctx.terminal_size, (80, 24));
    }

    #[test]
    fn test_register_api() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        let result = register_api(&lua, state, context);
        assert!(result.is_ok());

        // Check that ratterm table exists
        let globals = lua.globals();
        let ratterm: Table = globals.get("ratterm").expect("ratterm table");

        // Check sub-tables exist
        assert!(ratterm.get::<Table>("editor").is_ok());
        assert!(ratterm.get::<Table>("terminal").is_ok());
        assert!(ratterm.get::<Table>("fs").is_ok());
        assert!(ratterm.get::<Table>("commands").is_ok());
        assert!(ratterm.get::<Table>("events").is_ok());
        assert!(ratterm.get::<Table>("timer").is_ok());
    }
}
