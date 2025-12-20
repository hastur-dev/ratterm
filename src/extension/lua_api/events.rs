//! Lua API for event hooks.
//!
//! Provides `ratterm.events.*` functions for subscribing to application events.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mlua::{Function, Lua, RegistryKey, Result as LuaResult, Table};

use super::LuaState;

/// Event types that can be subscribed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// File opened in editor.
    FileOpen,
    /// File saved.
    FileSave,
    /// File closed.
    FileClose,
    /// Key pressed.
    KeyPress,
    /// Terminal output received.
    TerminalOutput,
    /// Focus changed between panes.
    FocusChanged,
    /// Theme changed.
    ThemeChanged,
}

impl EventType {
    /// Parse event type from string.
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "file_open" | "fileopen" => Some(EventType::FileOpen),
            "file_save" | "filesave" => Some(EventType::FileSave),
            "file_close" | "fileclose" => Some(EventType::FileClose),
            "key_press" | "keypress" => Some(EventType::KeyPress),
            "terminal_output" | "terminaloutput" => Some(EventType::TerminalOutput),
            "focus_changed" | "focuschanged" => Some(EventType::FocusChanged),
            "theme_changed" | "themechanged" => Some(EventType::ThemeChanged),
            _ => None,
        }
    }

    /// Convert to string.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::FileOpen => "file_open",
            EventType::FileSave => "file_save",
            EventType::FileClose => "file_close",
            EventType::KeyPress => "key_press",
            EventType::TerminalOutput => "terminal_output",
            EventType::FocusChanged => "focus_changed",
            EventType::ThemeChanged => "theme_changed",
        }
    }
}

/// An event subscription.
pub struct EventSubscription {
    /// Unique subscription ID.
    pub id: u64,
    /// Event type.
    pub event_type: EventType,
    /// Registry key for the callback function.
    pub callback_key: RegistryKey,
}

/// Manages event subscriptions.
pub struct LuaEvents {
    /// Subscriptions by event type.
    subscriptions: HashMap<EventType, Vec<EventSubscription>>,
    /// Next subscription ID.
    next_id: u64,
}

impl Default for LuaEvents {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaEvents {
    /// Creates a new event manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            next_id: 1,
        }
    }

    /// Subscribes to an event.
    pub fn subscribe(&mut self, event_type: EventType, callback_key: RegistryKey) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let sub = EventSubscription {
            id,
            event_type,
            callback_key,
        };

        self.subscriptions.entry(event_type).or_default().push(sub);
        id
    }

    /// Unsubscribes from an event by subscription ID.
    pub fn unsubscribe(&mut self, id: u64) -> Option<RegistryKey> {
        for subs in self.subscriptions.values_mut() {
            if let Some(pos) = subs.iter().position(|s| s.id == id) {
                let sub = subs.remove(pos);
                return Some(sub.callback_key);
            }
        }
        None
    }

    /// Gets all subscriptions for an event type.
    #[must_use]
    pub fn get_subscriptions(&self, event_type: EventType) -> &[EventSubscription] {
        self.subscriptions
            .get(&event_type)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Clears all subscriptions.
    pub fn clear(&mut self) {
        self.subscriptions.clear();
    }
}

/// Creates the events API table.
pub fn create_table(lua: &Lua, state: Arc<Mutex<LuaState>>) -> LuaResult<Table> {
    let events = lua.create_table()?;

    // ratterm.events.on(event_name, callback) -> subscription_id
    let state_clone = state.clone();
    let on = lua.create_function(move |lua, (event_name, callback): (String, Function)| {
        let event_type = EventType::from_str(&event_name).ok_or_else(|| {
            mlua::Error::RuntimeError(format!("Unknown event type: {}", event_name))
        })?;

        let callback_key = lua.create_registry_value(callback)?;

        let id = if let Ok(mut s) = state_clone.lock() {
            s.events.subscribe(event_type, callback_key)
        } else {
            return Err(mlua::Error::RuntimeError("Failed to lock state".to_string()));
        };

        Ok(id)
    })?;
    events.set("on", on)?;

    // ratterm.events.off(subscription_id)
    let state_clone = state.clone();
    let off = lua.create_function(move |lua, id: u64| {
        if let Ok(mut s) = state_clone.lock() {
            if let Some(key) = s.events.unsubscribe(id) {
                // Remove from registry
                lua.remove_registry_value(key)?;
            }
        }
        Ok(())
    })?;
    events.set("off", off)?;

    Ok(events)
}

/// Dispatches an event to all subscribers.
pub fn dispatch_event(
    lua: &Lua,
    state: &Arc<Mutex<LuaState>>,
    event_type: EventType,
    args: impl mlua::IntoLuaMulti + Clone,
) -> LuaResult<()> {
    let callbacks: Vec<Function> = {
        let s = state
            .lock()
            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to lock state: {}", e)))?;

        s.events
            .get_subscriptions(event_type)
            .iter()
            .filter_map(|sub| lua.registry_value::<Function>(&sub.callback_key).ok())
            .collect()
    };

    for callback in callbacks {
        if let Err(e) = callback.call::<()>(args.clone()) {
            tracing::warn!("Event callback error for {:?}: {}", event_type, e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_from_str() {
        assert_eq!(EventType::from_str("file_open"), Some(EventType::FileOpen));
        assert_eq!(EventType::from_str("FileSave"), Some(EventType::FileSave));
        assert_eq!(EventType::from_str("KEY_PRESS"), Some(EventType::KeyPress));
        assert_eq!(EventType::from_str("invalid"), None);
    }

    #[test]
    fn test_events_subscribe() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let events = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("events", events).expect("set global");

        // Subscribe to event
        let id: u64 = lua
            .load(
                r#"
            return events.on("file_open", function(path)
                -- callback
            end)
            "#,
            )
            .eval()
            .expect("eval");

        assert!(id > 0);

        let s = state.lock().expect("lock");
        let subs = s.events.get_subscriptions(EventType::FileOpen);
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].id, id);
    }

    #[test]
    fn test_events_unsubscribe() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let events = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("events", events).expect("set global");

        // Subscribe and unsubscribe
        lua.load(
            r#"
            local id = events.on("file_save", function() end)
            events.off(id)
            "#,
        )
        .exec()
        .expect("exec");

        let s = state.lock().expect("lock");
        let subs = s.events.get_subscriptions(EventType::FileSave);
        assert!(subs.is_empty());
    }

    #[test]
    fn test_dispatch_event() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let events = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("events", events).expect("set global");

        // Subscribe to event
        lua.load(
            r#"
            received_path = nil
            events.on("file_open", function(path)
                received_path = path
            end)
            "#,
        )
        .exec()
        .expect("exec");

        // Dispatch event
        dispatch_event(&lua, &state, EventType::FileOpen, "/path/to/file.rs").expect("dispatch");

        // Check callback was called
        let result: String = lua.globals().get("received_path").expect("get result");
        assert_eq!(result, "/path/to/file.rs");
    }
}
