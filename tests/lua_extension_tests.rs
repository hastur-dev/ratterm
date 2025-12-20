//! Integration tests for the Lua extension system.
//!
//! These tests verify the complete Lua plugin loading, API functionality,
//! event handling, timers, and error recovery across different scenarios.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use ratterm::extension::lua::{LuaPlugin, LuaPluginManager};
use ratterm::extension::lua_api::events::EventType;
use ratterm::extension::lua_api::LuaContext;
use ratterm::extension::manifest::load_manifest;

/// Returns the path to test fixtures.
fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("lua")
}

/// Helper to load a fixture extension.
fn load_fixture(name: &str) -> (LuaPlugin, PathBuf) {
    let dir = fixtures_path().join(name);
    let manifest_path = dir.join("extension.toml");

    assert!(
        manifest_path.exists(),
        "Fixture manifest not found: {:?}",
        manifest_path
    );

    let manifest = load_manifest(&manifest_path).expect("Failed to load manifest");
    let plugin = LuaPlugin::new(&manifest, &dir).expect("Failed to create plugin");

    (plugin, dir)
}

// ============================================================================
// Basic Extension Tests
// ============================================================================

mod basic_extension {
    use super::*;

    #[test]
    fn test_load_and_lifecycle() {
        let (plugin, _dir) = load_fixture("basic-extension");

        // Call on_load
        plugin.call_on_load().expect("on_load should succeed");

        // Check notification was sent
        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("basic-test loaded")),
            "Expected load notification, got: {:?}",
            notifications
        );

        // Verify plugin metadata
        assert_eq!(plugin.name(), "basic-test");
        assert_eq!(plugin.version(), "1.0.0");
    }

    #[test]
    fn test_command_registration() {
        let (plugin, _dir) = load_fixture("basic-extension");
        plugin.call_on_load().expect("on_load");

        // Drain the load notification
        plugin.take_notifications();

        // Check commands were registered
        let commands = plugin.get_commands();
        assert!(commands.len() >= 2, "Expected at least 2 commands");

        let cmd_ids: Vec<_> = commands.iter().map(|(id, _, _)| id.as_str()).collect();
        assert!(cmd_ids.contains(&"basic.hello"), "Missing basic.hello command");
        assert!(
            cmd_ids.contains(&"basic.cursor"),
            "Missing basic.cursor command"
        );
    }

    #[test]
    fn test_command_execution() {
        let (plugin, _dir) = load_fixture("basic-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications(); // drain

        // Execute hello command with argument
        plugin
            .execute_command("basic.hello", &["Tester".to_string()])
            .expect("execute command");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("Hello, Tester!")),
            "Expected greeting, got: {:?}",
            notifications
        );
    }

    #[test]
    fn test_command_execution_default_arg() {
        let (plugin, _dir) = load_fixture("basic-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Execute hello command without argument
        plugin.execute_command("basic.hello", &[]).expect("execute");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("Hello, World!")),
            "Expected default greeting, got: {:?}",
            notifications
        );
    }
}

// ============================================================================
// Complex Extension Tests
// ============================================================================

mod complex_extension {
    use super::*;

    #[test]
    fn test_event_subscriptions() {
        let (plugin, _dir) = load_fixture("complex-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Dispatch various events
        plugin.dispatch_event(EventType::FileOpen, "/test/file1.rs");
        plugin.dispatch_event(EventType::FileSave, "/test/file1.rs");
        plugin.dispatch_event(EventType::FileClose, "/test/file1.rs");

        // Execute the event log command to see captured events
        plugin.execute_command("complex.event_log", &[]).expect("cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.len() >= 3,
            "Expected 3+ notifications for events"
        );
    }

    #[test]
    fn test_multiple_event_dispatches() {
        let (plugin, _dir) = load_fixture("complex-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Dispatch many events
        for i in 0..10 {
            plugin.dispatch_event(EventType::FileOpen, &format!("/file{}.rs", i));
        }

        plugin.execute_command("complex.event_log", &[]).expect("cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.len() >= 10,
            "Expected 10+ event notifications"
        );
    }
}

// ============================================================================
// Preload Extension Tests
// ============================================================================

mod preload_extension {
    use super::*;

    #[test]
    fn test_preload_files_loaded() {
        let (plugin, _dir) = load_fixture("preload-extension");
        plugin.call_on_load().expect("on_load");

        let notifications = plugin.take_notifications();

        // Should not have any ERROR notifications
        let errors: Vec<_> = notifications.iter().filter(|n| n.contains("ERROR")).collect();
        assert!(
            errors.is_empty(),
            "Preload errors detected: {:?}",
            errors
        );

        // Should have success notification
        assert!(
            notifications
                .iter()
                .any(|n| n.contains("preload files loaded successfully")),
            "Missing success notification"
        );
    }

    #[test]
    fn test_preload_functions_available() {
        let (plugin, _dir) = load_fixture("preload-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Execute command that uses preloaded functions
        plugin.execute_command("preload.test", &[]).expect("cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications
                .iter()
                .any(|n| n.contains("apple, banana, cherry")),
            "Preload function Utils.join not working"
        );
    }
}

// ============================================================================
// Error Extension Tests
// ============================================================================

mod error_extension {
    use super::*;

    #[test]
    fn test_error_handling_in_command() {
        let (plugin, _dir) = load_fixture("error-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Execute command that throws error - should not panic
        let result = plugin.execute_command("error.throw", &[]);
        assert!(result.is_err(), "Should return error for throwing command");
    }

    #[test]
    fn test_safe_error_handling() {
        let (plugin, _dir) = load_fixture("error-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Execute safe error command
        plugin.execute_command("error.safe", &[]).expect("cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("Caught error")),
            "Error was not caught properly"
        );
    }

    #[test]
    fn test_invalid_api_usage() {
        let (plugin, _dir) = load_fixture("error-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Test invalid API usage
        plugin.execute_command("error.invalid_api", &[]).expect("cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications
                .iter()
                .any(|n| n.contains("Correctly returned nil")),
            "API should return nil for invalid inputs"
        );
    }
}

// ============================================================================
// Plugin Manager Tests
// ============================================================================

mod plugin_manager {
    use super::*;

    #[test]
    fn test_load_multiple_plugins() {
        let mut manager = LuaPluginManager::new();

        // Load multiple fixtures
        for name in ["basic-extension", "complex-extension", "error-extension"] {
            let dir = fixtures_path().join(name);
            let manifest = load_manifest(&dir.join("extension.toml")).expect("manifest");
            manager.load(&manifest, &dir).expect("load");
        }

        assert_eq!(manager.count(), 3);
        assert!(manager.get("basic-test").is_some());
        assert!(manager.get("complex-test").is_some());
        assert!(manager.get("error-test").is_some());
    }

    #[test]
    fn test_unload_plugin() {
        let mut manager = LuaPluginManager::new();

        let dir = fixtures_path().join("basic-extension");
        let manifest = load_manifest(&dir.join("extension.toml")).expect("manifest");
        manager.load(&manifest, &dir).expect("load");

        assert_eq!(manager.count(), 1);

        // Unload
        let unloaded = manager.unload("basic-test");
        assert!(unloaded);
        assert_eq!(manager.count(), 0);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_dispatch_event_to_all() {
        let mut manager = LuaPluginManager::new();

        // Load plugins that handle events
        let dir = fixtures_path().join("complex-extension");
        let manifest = load_manifest(&dir.join("extension.toml")).expect("manifest");
        manager.load(&manifest, &dir).expect("load");

        // Drain initial notifications
        manager.take_all_notifications();

        // Dispatch event to all plugins
        manager.dispatch_event(EventType::FileOpen, "/shared/file.rs");

        // The event should be logged in complex-extension
        if let Some(plugin) = manager.get("complex-test") {
            plugin.execute_command("complex.event_log", &[]).unwrap();
        }

        let notifications = manager.take_all_notifications();
        assert!(!notifications.is_empty(), "Event should have triggered notification");
    }

    #[test]
    fn test_collect_all_commands() {
        let mut manager = LuaPluginManager::new();

        // Load multiple plugins
        for name in ["basic-extension", "complex-extension"] {
            let dir = fixtures_path().join(name);
            let manifest = load_manifest(&dir.join("extension.toml")).expect("manifest");
            manager.load(&manifest, &dir).expect("load");
        }

        let commands = manager.get_all_commands();

        // Should have commands from both plugins
        let cmd_ids: Vec<_> = commands.iter().map(|(id, _, _)| id.as_str()).collect();

        assert!(
            cmd_ids.iter().any(|id| id.starts_with("basic.")),
            "Missing basic commands"
        );
        assert!(
            cmd_ids.iter().any(|id| id.starts_with("complex.")),
            "Missing complex commands"
        );
    }

    #[test]
    fn test_execute_command_across_plugins() {
        let mut manager = LuaPluginManager::new();

        // Load basic extension
        let dir = fixtures_path().join("basic-extension");
        let manifest = load_manifest(&dir.join("extension.toml")).expect("manifest");
        manager.load(&manifest, &dir).expect("load");

        manager.take_all_notifications();

        // Execute command by ID
        manager
            .execute_command("basic.hello", &["Manager".to_string()])
            .expect("execute");

        let notifications = manager.take_all_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("Hello, Manager!")),
            "Command execution failed"
        );
    }

    #[test]
    fn test_update_all_contexts() {
        let mut manager = LuaPluginManager::new();

        let dir = fixtures_path().join("basic-extension");
        let manifest = load_manifest(&dir.join("extension.toml")).expect("manifest");
        manager.load(&manifest, &dir).expect("load");

        // Create a context
        let ctx = LuaContext {
            editor_content: Some("test content".to_string()),
            cursor_pos: (5, 10),
            current_file: Some("/test/file.rs".to_string()),
            terminal_lines: vec!["line1".to_string(), "line2".to_string()],
            terminal_size: (80, 24),
            theme_name: "dracula".to_string(),
            config: std::collections::HashMap::new(),
        };

        // Update all contexts - should not panic
        manager.update_all_contexts(ctx);
    }
}

// ============================================================================
// Timer Tests
// ============================================================================

mod timer_tests {
    use super::*;

    #[test]
    fn test_timer_scheduling() {
        let (plugin, _dir) = load_fixture("complex-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Start the timer
        plugin.execute_command("complex.start_timer", &[]).expect("start");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("Started timer")),
            "Timer start notification missing"
        );

        // Check there's a pending timeout
        let timeout = plugin.next_timer_timeout();
        assert!(timeout.is_some(), "Should have pending timer");
        assert!(
            timeout.expect("timeout") <= Duration::from_millis(100),
            "Timeout should be ~100ms"
        );
    }

    #[test]
    fn test_timer_fires() {
        let (plugin, _dir) = load_fixture("complex-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Start timer
        plugin.execute_command("complex.start_timer", &[]).expect("start");
        plugin.take_notifications();

        // Wait for timer to be ready
        thread::sleep(Duration::from_millis(150));

        // Process timers
        plugin.process_timers();

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("Timer fired")),
            "Timer should have fired"
        );

        // Stop timer
        plugin.execute_command("complex.stop_timer", &[]).expect("stop");
    }

    #[test]
    fn test_timer_cancel() {
        let (plugin, _dir) = load_fixture("complex-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Start and immediately stop
        plugin.execute_command("complex.start_timer", &[]).expect("start");
        plugin.take_notifications();

        plugin.execute_command("complex.stop_timer", &[]).expect("stop");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("Stopped timer")),
            "Timer stop notification missing"
        );

        // Should have no pending timeout
        let timeout = plugin.next_timer_timeout();
        assert!(timeout.is_none(), "Should have no pending timer after cancel");
    }
}

// ============================================================================
// Context and State Tests
// ============================================================================

mod context_tests {
    use super::*;

    #[test]
    fn test_context_update() {
        let (plugin, _dir) = load_fixture("basic-extension");
        plugin.call_on_load().expect("on_load");

        // Update context
        let ctx = LuaContext {
            editor_content: Some("Hello, World!".to_string()),
            cursor_pos: (1, 5),
            current_file: Some("/project/main.rs".to_string()),
            terminal_lines: vec!["$ cargo build".to_string()],
            terminal_size: (120, 40),
            theme_name: "matrix".to_string(),
            config: std::collections::HashMap::new(),
        };

        plugin.update_context(ctx);

        // Execute cursor command to verify context is accessible
        plugin.take_notifications();
        plugin.execute_command("basic.cursor", &[]).expect("cursor cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("line 1")),
            "Cursor position not updated"
        );
    }
}

// ============================================================================
// Edge Cases and Stress Tests
// ============================================================================

mod stress_tests {
    use super::*;

    #[test]
    fn test_many_commands() {
        let (plugin, _dir) = load_fixture("basic-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Execute command many times
        for i in 0..100 {
            plugin
                .execute_command("basic.hello", &[format!("User{}", i)])
                .expect("execute");
        }

        let notifications = plugin.take_notifications();
        assert_eq!(notifications.len(), 100, "Should have 100 notifications");
    }

    #[test]
    fn test_many_events() {
        let (plugin, _dir) = load_fixture("complex-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Dispatch many events
        for i in 0..100 {
            plugin.dispatch_event(EventType::FileOpen, &format!("/file{}.rs", i));
            plugin.dispatch_event(EventType::FileSave, &format!("/file{}.rs", i));
        }

        // Event log command should work
        plugin.execute_command("complex.event_log", &[]).expect("cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.len() >= 200,
            "Should have many event notifications"
        );
    }

    #[test]
    fn test_rapid_context_updates() {
        let (plugin, _dir) = load_fixture("basic-extension");
        plugin.call_on_load().expect("on_load");

        // Rapid context updates
        for i in 0..100 {
            let ctx = LuaContext {
                editor_content: Some(format!("Content {}", i)),
                cursor_pos: (i as usize, i as usize),
                current_file: Some(format!("/file{}.rs", i)),
                terminal_lines: vec![],
                terminal_size: (80, 24),
                theme_name: "test".to_string(),
                config: std::collections::HashMap::new(),
            };
            plugin.update_context(ctx);
        }

        // Should not panic or corrupt state
        plugin.execute_command("basic.cursor", &[]).expect("cmd");
    }
}

// ============================================================================
// Cross-Platform Specific Tests
// ============================================================================

mod cross_platform {
    use super::*;
    use std::fs;

    #[test]
    fn test_fs_operations_cross_platform() {
        let (plugin, _dir) = load_fixture("complex-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Create a temp file path that works on all platforms
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("ratterm_lua_test.txt");
        let test_path = test_file.to_string_lossy().to_string();

        // Clean up any existing file
        let _ = fs::remove_file(&test_file);

        // Execute fs test command with our temp path
        plugin
            .execute_command("complex.fs_test", &[test_path.clone(), "Cross-platform test".to_string()])
            .expect("fs test");

        let notifications = plugin.take_notifications();

        // Should have written successfully
        assert!(
            notifications.iter().any(|n| n.contains("Wrote file")),
            "Write should succeed on all platforms"
        );

        // Should have read back correctly
        assert!(
            notifications.iter().any(|n| n.contains("Cross-platform test")),
            "Read should match written content"
        );

        // Clean up
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_path_handling() {
        let fixtures = fixtures_path();

        // Verify fixtures path exists and is accessible
        assert!(fixtures.exists(), "Fixtures path should exist");
        assert!(fixtures.is_dir(), "Fixtures should be a directory");

        // List fixture directories
        let entries: Vec<_> = fs::read_dir(&fixtures)
            .expect("read fixtures dir")
            .filter_map(|e| e.ok())
            .collect();

        assert!(entries.len() >= 4, "Should have at least 4 fixture extensions");
    }

    #[test]
    fn test_unicode_content() {
        let (plugin, _dir) = load_fixture("basic-extension");
        plugin.call_on_load().expect("on_load");
        plugin.take_notifications();

        // Test with unicode argument
        plugin
            .execute_command("basic.hello", &["世界".to_string()])
            .expect("unicode cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("世界")),
            "Unicode should be preserved"
        );

        // Test with emoji
        plugin
            .execute_command("basic.hello", &["🦀 Rust".to_string()])
            .expect("emoji cmd");

        let notifications = plugin.take_notifications();
        assert!(
            notifications.iter().any(|n| n.contains("🦀")),
            "Emoji should be preserved"
        );
    }
}
