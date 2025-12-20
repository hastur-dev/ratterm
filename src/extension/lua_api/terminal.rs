//! Lua API for terminal operations.
//!
//! Provides `ratterm.terminal.*` functions for interacting with the terminal.

use std::sync::{Arc, Mutex};

use mlua::{Lua, MultiValue, Result as LuaResult, Table};

use super::{LuaContext, LuaState, TerminalOp};

/// Terminal API wrapper.
pub struct LuaTerminal;

impl LuaTerminal {
    /// Creates the terminal API table.
    pub fn create_table(
        lua: &Lua,
        state: Arc<Mutex<LuaState>>,
        context: Arc<Mutex<LuaContext>>,
    ) -> LuaResult<Table> {
        let terminal = lua.create_table()?;

        // ratterm.terminal.send_keys(text)
        let state_clone = state.clone();
        let send_keys = lua.create_function(move |_, text: String| {
            if let Ok(mut s) = state_clone.lock() {
                s.terminal_ops.push(TerminalOp::SendKeys(text));
            }
            Ok(())
        })?;
        terminal.set("send_keys", send_keys)?;

        // ratterm.terminal.get_buffer() -> table of lines
        let context_clone = context.clone();
        let get_buffer = lua.create_function(move |lua, ()| {
            let lines = context_clone
                .lock()
                .map(|c| c.terminal_lines.clone())
                .unwrap_or_default();

            let table = lua.create_table()?;
            for (i, line) in lines.into_iter().enumerate() {
                table.set(i + 1, line)?; // Lua tables are 1-indexed
            }
            Ok(table)
        })?;
        terminal.set("get_buffer", get_buffer)?;

        // ratterm.terminal.get_size() -> cols, rows
        let context_clone = context;
        let get_size = lua.create_function(move |_, ()| {
            let (cols, rows) = context_clone
                .lock()
                .map(|c| c.terminal_size)
                .unwrap_or((80, 24));
            Ok(MultiValue::from_vec(vec![
                mlua::Value::Integer(cols as i64),
                mlua::Value::Integer(rows as i64),
            ]))
        })?;
        terminal.set("get_size", get_size)?;

        Ok(terminal)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_send_keys() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        let terminal =
            LuaTerminal::create_table(&lua, state.clone(), context).expect("create table");
        lua.globals().set("terminal", terminal).expect("set global");

        lua.load(r#"terminal.send_keys("ls -la\n")"#)
            .exec()
            .expect("exec");

        let ops = &state.lock().expect("lock").terminal_ops;
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TerminalOp::SendKeys(text) => assert_eq!(text, "ls -la\n"),
        }
    }

    #[test]
    fn test_terminal_get_buffer() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        // Set some terminal lines
        context.lock().expect("lock").terminal_lines = vec![
            "Line 1".to_string(),
            "Line 2".to_string(),
            "Line 3".to_string(),
        ];

        let terminal = LuaTerminal::create_table(&lua, state, context).expect("create table");
        lua.globals().set("terminal", terminal).expect("set global");

        let result: Table = lua
            .load("return terminal.get_buffer()")
            .eval()
            .expect("eval");

        assert_eq!(result.get::<String>(1).expect("get 1"), "Line 1");
        assert_eq!(result.get::<String>(2).expect("get 2"), "Line 2");
        assert_eq!(result.get::<String>(3).expect("get 3"), "Line 3");
    }

    #[test]
    fn test_terminal_get_size() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        // Set terminal size
        context.lock().expect("lock").terminal_size = (120, 40);

        let terminal = LuaTerminal::create_table(&lua, state, context).expect("create table");
        lua.globals().set("terminal", terminal).expect("set global");

        let result: (i64, i64) = lua.load("return terminal.get_size()").eval().expect("eval");

        assert_eq!(result, (120, 40));
    }
}
