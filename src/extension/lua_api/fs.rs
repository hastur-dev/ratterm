//! Lua API for file system operations.
//!
//! Provides `ratterm.fs.*` functions for unrestricted file system access.
//! WARNING: This is intentionally unsandboxed - extensions have full system access.

use std::fs;
use std::path::Path;

use mlua::{Lua, Result as LuaResult, Table};

/// File system API wrapper.
pub struct LuaFs;

impl LuaFs {
    /// Creates the fs API table.
    pub fn create_table(lua: &Lua) -> LuaResult<Table> {
        let fs_table = lua.create_table()?;

        // ratterm.fs.read(path) -> string or nil
        let read = lua.create_function(|_, path: String| {
            match fs::read_to_string(&path) {
                Ok(content) => Ok(Some(content)),
                Err(_) => Ok(None),
            }
        })?;
        fs_table.set("read", read)?;

        // ratterm.fs.write(path, content) -> bool
        let write = lua.create_function(|_, (path, content): (String, String)| {
            match fs::write(&path, &content) {
                Ok(()) => Ok(true),
                Err(_) => Ok(false),
            }
        })?;
        fs_table.set("write", write)?;

        // ratterm.fs.exists(path) -> bool
        let exists = lua.create_function(|_, path: String| Ok(Path::new(&path).exists()))?;
        fs_table.set("exists", exists)?;

        // ratterm.fs.is_dir(path) -> bool
        let is_dir = lua.create_function(|_, path: String| Ok(Path::new(&path).is_dir()))?;
        fs_table.set("is_dir", is_dir)?;

        // ratterm.fs.is_file(path) -> bool
        let is_file = lua.create_function(|_, path: String| Ok(Path::new(&path).is_file()))?;
        fs_table.set("is_file", is_file)?;

        // ratterm.fs.list_dir(path) -> table of entries or nil
        let list_dir = lua.create_function(|lua, path: String| {
            match fs::read_dir(&path) {
                Ok(entries) => {
                    let table = lua.create_table()?;
                    let mut idx = 1;
                    for entry in entries.filter_map(Result::ok) {
                        if let Some(name) = entry.file_name().to_str() {
                            table.set(idx, name.to_string())?;
                            idx += 1;
                        }
                    }
                    Ok(Some(table))
                }
                Err(_) => Ok(None),
            }
        })?;
        fs_table.set("list_dir", list_dir)?;

        // ratterm.fs.mkdir(path) -> bool
        let mkdir = lua.create_function(|_, path: String| {
            match fs::create_dir_all(&path) {
                Ok(()) => Ok(true),
                Err(_) => Ok(false),
            }
        })?;
        fs_table.set("mkdir", mkdir)?;

        // ratterm.fs.remove(path) -> bool
        let remove = lua.create_function(|_, path: String| {
            let p = Path::new(&path);
            let result = if p.is_dir() {
                fs::remove_dir_all(p)
            } else {
                fs::remove_file(p)
            };
            match result {
                Ok(()) => Ok(true),
                Err(_) => Ok(false),
            }
        })?;
        fs_table.set("remove", remove)?;

        // ratterm.fs.rename(from, to) -> bool
        let rename = lua.create_function(|_, (from, to): (String, String)| {
            match fs::rename(&from, &to) {
                Ok(()) => Ok(true),
                Err(_) => Ok(false),
            }
        })?;
        fs_table.set("rename", rename)?;

        // ratterm.fs.copy(from, to) -> bool
        let copy = lua.create_function(|_, (from, to): (String, String)| {
            match fs::copy(&from, &to) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        })?;
        fs_table.set("copy", copy)?;

        // ratterm.fs.append(path, content) -> bool
        let append = lua.create_function(|_, (path, content): (String, String)| {
            use std::fs::OpenOptions;
            use std::io::Write;

            let result = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut f| f.write_all(content.as_bytes()));

            match result {
                Ok(()) => Ok(true),
                Err(_) => Ok(false),
            }
        })?;
        fs_table.set("append", append)?;

        Ok(fs_table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_fs_read_write() {
        let lua = Lua::new();
        let fs_table = LuaFs::create_table(&lua).expect("create table");
        lua.globals().set("fs", fs_table).expect("set global");

        let dir = TempDir::new().expect("temp dir");
        let file_path = dir.path().join("test.txt");
        let path_str = file_path.to_str().expect("path str");

        // Write file
        let script = format!(r#"return fs.write("{}", "Hello, Lua!")"#, path_str.replace('\\', "\\\\"));
        let result: bool = lua.load(&script).eval().expect("eval write");
        assert!(result);

        // Read file
        let script = format!(r#"return fs.read("{}")"#, path_str.replace('\\', "\\\\"));
        let result: Option<String> = lua.load(&script).eval().expect("eval read");
        assert_eq!(result, Some("Hello, Lua!".to_string()));
    }

    #[test]
    fn test_fs_exists() {
        let lua = Lua::new();
        let fs_table = LuaFs::create_table(&lua).expect("create table");
        lua.globals().set("fs", fs_table).expect("set global");

        let dir = TempDir::new().expect("temp dir");
        let file_path = dir.path().join("exists.txt");
        let path_str = file_path.to_str().expect("path str").replace('\\', "\\\\");

        // File doesn't exist yet
        let script = format!(r#"return fs.exists("{}")"#, path_str);
        let result: bool = lua.load(&script).eval().expect("eval");
        assert!(!result);

        // Create file
        fs::write(&file_path, "test").expect("write");

        // Now it exists
        let result: bool = lua.load(&script).eval().expect("eval");
        assert!(result);
    }

    #[test]
    fn test_fs_list_dir() {
        let lua = Lua::new();
        let fs_table = LuaFs::create_table(&lua).expect("create table");
        lua.globals().set("fs", fs_table).expect("set global");

        let dir = TempDir::new().expect("temp dir");
        let dir_str = dir.path().to_str().expect("path str").replace('\\', "\\\\");

        // Create some files
        fs::write(dir.path().join("a.txt"), "a").expect("write");
        fs::write(dir.path().join("b.txt"), "b").expect("write");
        fs::create_dir(dir.path().join("subdir")).expect("mkdir");

        let script = format!(r#"return fs.list_dir("{}")"#, dir_str);
        let result: Table = lua.load(&script).eval().expect("eval");

        // Should have 3 entries
        let mut count = 0;
        for pair in result.pairs::<i64, String>() {
            let (_, _name) = pair.expect("pair");
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn test_fs_mkdir_remove() {
        let lua = Lua::new();
        let fs_table = LuaFs::create_table(&lua).expect("create table");
        lua.globals().set("fs", fs_table).expect("set global");

        let dir = TempDir::new().expect("temp dir");
        let new_dir = dir.path().join("new_folder");
        let path_str = new_dir.to_str().expect("path str").replace('\\', "\\\\");

        // Create directory
        let script = format!(r#"return fs.mkdir("{}")"#, path_str);
        let result: bool = lua.load(&script).eval().expect("eval");
        assert!(result);
        assert!(new_dir.exists());

        // Remove directory
        let script = format!(r#"return fs.remove("{}")"#, path_str);
        let result: bool = lua.load(&script).eval().expect("eval");
        assert!(result);
        assert!(!new_dir.exists());
    }

    #[test]
    fn test_fs_append() {
        let lua = Lua::new();
        let fs_table = LuaFs::create_table(&lua).expect("create table");
        lua.globals().set("fs", fs_table).expect("set global");

        let dir = TempDir::new().expect("temp dir");
        let file_path = dir.path().join("append.txt");
        let path_str = file_path.to_str().expect("path str").replace('\\', "\\\\");

        // Write initial content
        fs::write(&file_path, "Hello").expect("write");

        // Append
        let script = format!(r#"return fs.append("{}", ", World!")"#, path_str);
        let result: bool = lua.load(&script).eval().expect("eval");
        assert!(result);

        // Read back
        let content = fs::read_to_string(&file_path).expect("read");
        assert_eq!(content, "Hello, World!");
    }
}
