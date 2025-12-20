//! Lua API for timers and scheduled callbacks.
//!
//! Provides `ratterm.timer.*` functions for scheduling delayed and repeating callbacks.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mlua::{Function, Lua, RegistryKey, Result as LuaResult, Table};

use super::LuaState;

/// A scheduled timer.
pub struct Timer {
    /// Unique timer ID.
    pub id: u64,
    /// When the timer should next fire.
    pub next_fire: Instant,
    /// Interval for repeating timers (None for one-shot).
    pub interval: Option<Duration>,
    /// Registry key for the callback function.
    pub callback_key: RegistryKey,
    /// Whether the timer is active.
    pub active: bool,
}

/// Manages timers.
pub struct LuaTimers {
    /// Active timers by ID.
    pub(crate) timers: HashMap<u64, Timer>,
    /// Next timer ID.
    next_id: u64,
}

impl Default for LuaTimers {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaTimers {
    /// Creates a new timer manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timers: HashMap::new(),
            next_id: 1,
        }
    }

    /// Schedules a one-shot timer.
    pub fn after(&mut self, delay_ms: u64, callback_key: RegistryKey) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let timer = Timer {
            id,
            next_fire: Instant::now() + Duration::from_millis(delay_ms),
            interval: None,
            callback_key,
            active: true,
        };

        self.timers.insert(id, timer);
        id
    }

    /// Schedules a repeating timer.
    pub fn every(&mut self, interval_ms: u64, callback_key: RegistryKey) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let interval = Duration::from_millis(interval_ms);
        let timer = Timer {
            id,
            next_fire: Instant::now() + interval,
            interval: Some(interval),
            callback_key,
            active: true,
        };

        self.timers.insert(id, timer);
        id
    }

    /// Cancels a timer.
    pub fn cancel(&mut self, id: u64) -> Option<RegistryKey> {
        self.timers.remove(&id).map(|t| t.callback_key)
    }

    /// Gets IDs of all timers that are ready to fire.
    pub fn get_ready_timer_ids(&mut self) -> Vec<u64> {
        let now = Instant::now();
        let mut ready = Vec::new();

        for timer in self.timers.values_mut() {
            if timer.active && timer.next_fire <= now {
                ready.push(timer.id);

                if let Some(interval) = timer.interval {
                    // Reschedule repeating timer
                    timer.next_fire = now + interval;
                } else {
                    // Mark one-shot timer as inactive
                    timer.active = false;
                }
            }
        }

        // Remove inactive one-shot timers
        self.timers.retain(|_, t| t.active);

        ready
    }

    /// Gets the callback for a timer by ID.
    pub fn get_callback(&self, id: u64) -> Option<&RegistryKey> {
        self.timers.get(&id).map(|t| &t.callback_key)
    }

    /// Returns the time until the next timer fires (for event loop timeout).
    #[must_use]
    pub fn next_timeout(&self) -> Option<Duration> {
        let now = Instant::now();
        self.timers
            .values()
            .filter(|t| t.active)
            .map(|t| t.next_fire.saturating_duration_since(now))
            .min()
    }

    /// Clears all timers.
    pub fn clear(&mut self) {
        self.timers.clear();
    }

    /// Returns the number of active timers.
    #[must_use]
    pub fn count(&self) -> usize {
        self.timers.values().filter(|t| t.active).count()
    }
}

/// Creates the timer API table.
pub fn create_table(lua: &Lua, state: Arc<Mutex<LuaState>>) -> LuaResult<Table> {
    let timer = lua.create_table()?;

    // ratterm.timer.after(ms, callback) -> timer_id
    let state_clone = state.clone();
    let after = lua.create_function(move |lua, (ms, callback): (u64, Function)| {
        let callback_key = lua.create_registry_value(callback)?;

        let id = if let Ok(mut s) = state_clone.lock() {
            s.timers.after(ms, callback_key)
        } else {
            return Err(mlua::Error::RuntimeError(
                "Failed to lock state".to_string(),
            ));
        };

        Ok(id)
    })?;
    timer.set("after", after)?;

    // ratterm.timer.every(ms, callback) -> timer_id
    let state_clone = state.clone();
    let every = lua.create_function(move |lua, (ms, callback): (u64, Function)| {
        let callback_key = lua.create_registry_value(callback)?;

        let id = if let Ok(mut s) = state_clone.lock() {
            s.timers.every(ms, callback_key)
        } else {
            return Err(mlua::Error::RuntimeError(
                "Failed to lock state".to_string(),
            ));
        };

        Ok(id)
    })?;
    timer.set("every", every)?;

    // ratterm.timer.cancel(timer_id)
    let state_clone = state.clone();
    let cancel = lua.create_function(move |lua, id: u64| {
        if let Ok(mut s) = state_clone.lock() {
            if let Some(key) = s.timers.cancel(id) {
                lua.remove_registry_value(key)?;
            }
        }
        Ok(())
    })?;
    timer.set("cancel", cancel)?;

    Ok(timer)
}

/// Processes ready timers and calls their callbacks.
pub fn process_timers(lua: &Lua, state: &Arc<Mutex<LuaState>>) -> LuaResult<()> {
    // Get ready timer IDs and their callbacks in one lock
    let ready_callbacks: Vec<(u64, Function)> = {
        let mut s = state
            .lock()
            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to lock state: {}", e)))?;

        // Find ready timers and collect their callbacks before removing them
        let now = std::time::Instant::now();
        let mut ready = Vec::new();

        for timer in s.timers.timers.values_mut() {
            if timer.active && timer.next_fire <= now {
                // Get the callback function from registry
                if let Ok(func) = lua.registry_value::<Function>(&timer.callback_key) {
                    ready.push((timer.id, func));
                }

                if let Some(interval) = timer.interval {
                    // Reschedule repeating timer
                    timer.next_fire = now + interval;
                } else {
                    // Mark one-shot timer as inactive
                    timer.active = false;
                }
            }
        }

        // Remove inactive one-shot timers
        s.timers.timers.retain(|_, t| t.active);

        ready
    };

    // Execute callbacks (outside of lock)
    for (id, callback) in ready_callbacks {
        if let Err(e) = callback.call::<()>(()) {
            tracing::warn!("Timer {} callback error: {}", id, e);
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_timer_after() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let timer = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("timer", timer).expect("set global");

        // Schedule a timer
        let id: u64 = lua
            .load(
                r#"
            return timer.after(10, function()
                -- callback
            end)
            "#,
            )
            .eval()
            .expect("eval");

        assert!(id > 0);
        assert_eq!(state.lock().expect("lock").timers.count(), 1);
    }

    #[test]
    fn test_timer_every() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let timer = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("timer", timer).expect("set global");

        // Schedule a repeating timer
        let id: u64 = lua
            .load(
                r#"
            return timer.every(100, function()
                -- callback
            end)
            "#,
            )
            .eval()
            .expect("eval");

        assert!(id > 0);
        assert_eq!(state.lock().expect("lock").timers.count(), 1);
    }

    #[test]
    fn test_timer_cancel() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let timer = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("timer", timer).expect("set global");

        // Schedule and cancel a timer
        lua.load(
            r#"
            local id = timer.after(1000, function() end)
            timer.cancel(id)
            "#,
        )
        .exec()
        .expect("exec");

        assert_eq!(state.lock().expect("lock").timers.count(), 0);
    }

    #[test]
    fn test_timer_fires() {
        let lua = Lua::new();
        let state = Arc::new(Mutex::new(LuaState::default()));

        let timer = create_table(&lua, state.clone()).expect("create table");
        lua.globals().set("timer", timer).expect("set global");

        // Schedule a timer that fires quickly
        lua.load(
            r#"
            fired = false
            timer.after(1, function()
                fired = true
            end)
            "#,
        )
        .exec()
        .expect("exec");

        // Wait a bit
        sleep(Duration::from_millis(10));

        // Process timers
        process_timers(&lua, &state).expect("process");

        // Check callback was called
        let fired: bool = lua.globals().get("fired").expect("get fired");
        assert!(fired);
    }

    #[test]
    fn test_next_timeout() {
        let mut timers = LuaTimers::new();
        let lua = Lua::new();
        let callback = lua.create_function(|_, ()| Ok(())).expect("create fn");
        let key = lua.create_registry_value(callback).expect("registry");

        timers.after(100, key);

        let timeout = timers.next_timeout();
        assert!(timeout.is_some());
        assert!(timeout.expect("timeout") <= Duration::from_millis(100));
    }
}
