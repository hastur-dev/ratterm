//! Lua API for editor operations.
//!
//! Provides `ratterm.editor.*` functions for manipulating the code editor.

use std::sync::{Arc, Mutex};

use mlua::{Lua, MultiValue, Result as LuaResult, Table};

use super::{EditorOp, LuaContext, LuaState};

/// Editor API wrapper.
pub struct LuaEditor;

impl LuaEditor {
    /// Creates the editor API table.
    pub fn create_table(
        lua: &Lua,
        state: Arc<Mutex<LuaState>>,
        context: Arc<Mutex<LuaContext>>,
    ) -> LuaResult<Table> {
        let editor = lua.create_table()?;

        // ratterm.editor.open(path)
        let state_clone = state.clone();
        let open = lua.create_function(move |_, path: String| {
            if let Ok(mut s) = state_clone.lock() {
                s.editor_ops.push(EditorOp::Open(path));
            }
            Ok(())
        })?;
        editor.set("open", open)?;

        // ratterm.editor.save()
        let state_clone = state.clone();
        let save = lua.create_function(move |_, ()| {
            if let Ok(mut s) = state_clone.lock() {
                s.editor_ops.push(EditorOp::Save);
            }
            Ok(())
        })?;
        editor.set("save", save)?;

        // ratterm.editor.get_content()
        let context_clone = context.clone();
        let get_content = lua.create_function(move |_, ()| {
            let result = context_clone
                .lock()
                .ok()
                .and_then(|c| c.editor_content.clone());
            Ok(result)
        })?;
        editor.set("get_content", get_content)?;

        // ratterm.editor.set_content(text)
        let state_clone = state.clone();
        let set_content = lua.create_function(move |_, text: String| {
            if let Ok(mut s) = state_clone.lock() {
                s.editor_ops.push(EditorOp::SetContent(text));
            }
            Ok(())
        })?;
        editor.set("set_content", set_content)?;

        // ratterm.editor.insert_at(line, col, text)
        let state_clone = state.clone();
        let insert_at =
            lua.create_function(move |_, (line, col, text): (usize, usize, String)| {
                if let Ok(mut s) = state_clone.lock() {
                    s.editor_ops.push(EditorOp::InsertAt { line, col, text });
                }
                Ok(())
            })?;
        editor.set("insert_at", insert_at)?;

        // ratterm.editor.get_cursor() -> line, col
        let context_clone = context.clone();
        let get_cursor = lua.create_function(move |_lua, ()| {
            let (line, col) = context_clone.lock().map(|c| c.cursor_pos).unwrap_or((0, 0));
            Ok(MultiValue::from_vec(vec![
                mlua::Value::Integer(line as i64),
                mlua::Value::Integer(col as i64),
            ]))
        })?;
        editor.set("get_cursor", get_cursor)?;

        // ratterm.editor.set_cursor(line, col)
        let state_clone = state.clone();
        let set_cursor = lua.create_function(move |_, (line, col): (usize, usize)| {
            if let Ok(mut s) = state_clone.lock() {
                s.editor_ops.push(EditorOp::SetCursor { line, col });
            }
            Ok(())
        })?;
        editor.set("set_cursor", set_cursor)?;

        // ratterm.editor.get_file()
        let context_clone = context;
        let get_file = lua.create_function(move |_, ()| {
            let result = context_clone
                .lock()
                .ok()
                .and_then(|c| c.current_file.clone());
            Ok(result)
        })?;
        editor.set("get_file", get_file)?;

        Ok(editor)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_open() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        let editor = LuaEditor::create_table(&lua, state.clone(), context).expect("create table");
        lua.globals().set("editor", editor).expect("set global");

        lua.load(r#"editor.open("/path/to/file.rs")"#)
            .exec()
            .expect("exec");

        let ops = &state.lock().expect("lock").editor_ops;
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            EditorOp::Open(path) => assert_eq!(path, "/path/to/file.rs"),
            _ => panic!("Expected Open op"),
        }
    }

    #[test]
    fn test_editor_get_content() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        // Set some content
        context.lock().expect("lock").editor_content = Some("Hello, World!".to_string());

        let editor = LuaEditor::create_table(&lua, state, context).expect("create table");
        lua.globals().set("editor", editor).expect("set global");

        let result: Option<String> = lua
            .load("return editor.get_content()")
            .eval()
            .expect("eval");

        assert_eq!(result, Some("Hello, World!".to_string()));
    }

    #[test]
    fn test_editor_cursor() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));
        let context = Arc::new(Mutex::new(LuaContext::default()));

        // Set cursor position
        context.lock().expect("lock").cursor_pos = (10, 5);

        let editor = LuaEditor::create_table(&lua, state.clone(), context).expect("create table");
        lua.globals().set("editor", editor).expect("set global");

        // Get cursor
        let result: (i64, i64) = lua.load("return editor.get_cursor()").eval().expect("eval");
        assert_eq!(result, (10, 5));

        // Set cursor
        lua.load("editor.set_cursor(20, 15)").exec().expect("exec");

        let ops = &state.lock().expect("lock").editor_ops;
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            EditorOp::SetCursor { line, col } => {
                assert_eq!(*line, 20);
                assert_eq!(*col, 15);
            }
            _ => panic!("Expected SetCursor op"),
        }
    }
}
