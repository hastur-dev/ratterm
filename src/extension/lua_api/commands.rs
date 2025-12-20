//! Lua API for command registration.
//!
//! Provides `ratterm.commands.*` functions for registering custom commands.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mlua::{Function, Lua, RegistryKey, Result as LuaResult, Table};

use super::LuaState;

/// A registered Lua command.
#[derive(Debug)]
pub struct LuaCommand {
    /// Command identifier (e.g., "myext.hello").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Registry key for the callback function.
    pub callback_key: RegistryKey,
}

/// Manages registered Lua commands.
#[derive(Default)]
pub struct LuaCommands {
    /// Registered commands by ID.
    pub commands: HashMap<String, LuaCommand>,
}

impl LuaCommands {
    /// Creates a new command manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Registers a command.
    pub fn register(&mut self, cmd: LuaCommand) {
        self.commands.insert(cmd.id.clone(), cmd);
    }

    /// Gets a command by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&LuaCommand> {
        self.commands.get(id)
    }

    /// Returns all registered commands.
    #[must_use]
    pub fn all(&self) -> Vec<&LuaCommand> {
        self.commands.values().collect()
    }

    /// Removes a command.
    pub fn unregister(&mut self, id: &str) -> Option<LuaCommand> {
        self.commands.remove(id)
    }

    /// Clears all commands.
    pub fn clear(&mut self) {
        self.commands.clear();
    }
}

/// Creates the commands API table.
pub fn create_table(lua: &Lua, state: Arc<Mutex<LuaState>>) -> LuaResult<Table> {
    let commands = lua.create_table()?;

    // ratterm.commands.register(id, callback, options)
    // options = { name = "...", description = "..." }
    let state_clone = state.clone();
    let register = lua.create_function(
        move |lua, (id, callback, options): (String, Function, Option<Table>)| {
            let name = options
                .as_ref()
                .and_then(|o| o.get::<String>("name").ok())
                .unwrap_or_else(|| id.clone());

            let description = options
                .as_ref()
                .and_then(|o| o.get::<String>("description").ok())
                .unwrap_or_default();

            // Store callback in Lua registry
            let callback_key = lua.create_registry_value(callback)?;

            let cmd = LuaCommand {
                id: id.clone(),
                name,
                description,
                callback_key,
            };

            if let Ok(mut s) = state_clone.lock() {
                s.commands.register(cmd);
            }

            Ok(())
        },
    )?;
    commands.set("register", register)?;

    // ratterm.commands.unregister(id)
    let state_clone = state.clone();
    let unregister = lua.create_function(move |_, id: String| {
        if let Ok(mut s) = state_clone.lock() {
            s.commands.unregister(&id);
        }
        Ok(())
    })?;
    commands.set("unregister", unregister)?;

    // ratterm.commands.list() -> table of command info
    let state_clone = state;
    let list = lua.create_function(move |lua, ()| {
        let table = lua.create_table()?;
        if let Ok(s) = state_clone.lock() {
            for (i, cmd) in s.commands.all().iter().enumerate() {
                let cmd_table = lua.create_table()?;
                cmd_table.set("id", cmd.id.clone())?;
                cmd_table.set("name", cmd.name.clone())?;
                cmd_table.set("description", cmd.description.clone())?;
                table.set(i + 1, cmd_table)?;
            }
        }
        Ok(table)
    })?;
    commands.set("list", list)?;

    Ok(commands)
}

/// Executes a registered Lua command.
pub fn execute_command(
    lua: &Lua,
    state: &Arc<Mutex<LuaState>>,
    id: &str,
    args: &[String],
) -> LuaResult<()> {
    let callback_key = {
        let s = state.lock().map_err(|e| {
            mlua::Error::RuntimeError(format!("Failed to lock state: {}", e))
        })?;
        let cmd = s.commands.get(id).ok_or_else(|| {
            mlua::Error::RuntimeError(format!("Command not found: {}", id))
        })?;
        // We need to clone the key somehow - but RegistryKey isn't Clone
        // Instead, we'll get the function directly
        lua.registry_value::<Function>(&cmd.callback_key)?
    };

    // Create args table
    let args_table = lua.create_table()?;
    for (i, arg) in args.iter().enumerate() {
        args_table.set(i + 1, arg.clone())?;
    }

    callback_key.call::<()>(args_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_commands_register() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let commands = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("commands", commands).expect("set global");

        // Register a command
        lua.load(
            r#"
            commands.register("test.hello", function(args)
                -- callback
            end, {
                name = "Say Hello",
                description = "Greets the user"
            })
            "#,
        )
        .exec()
        .expect("exec");

        let s = state.lock().expect("lock");
        assert_eq!(s.commands.commands.len(), 1);
        let cmd = s.commands.get("test.hello").expect("get cmd");
        assert_eq!(cmd.name, "Say Hello");
        assert_eq!(cmd.description, "Greets the user");
    }

    #[test]
    fn test_lua_commands_unregister() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let commands = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("commands", commands).expect("set global");

        // Register and unregister
        lua.load(
            r#"
            commands.register("test.remove", function() end)
            commands.unregister("test.remove")
            "#,
        )
        .exec()
        .expect("exec");

        let s = state.lock().expect("lock");
        assert!(s.commands.get("test.remove").is_none());
    }

    #[test]
    fn test_execute_command() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let commands = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("commands", commands).expect("set global");

        // Register a command that modifies a global
        lua.load(
            r#"
            result = nil
            commands.register("test.setresult", function(args)
                result = args[1]
            end)
            "#,
        )
        .exec()
        .expect("exec");

        // Execute the command
        execute_command(&lua, &state, "test.setresult", &["Hello!".to_string()])
            .expect("execute");

        // Check result
        let result: String = lua.globals().get("result").expect("get result");
        assert_eq!(result, "Hello!");
    }
}
