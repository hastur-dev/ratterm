# Recent Fixes

## 2026-01-26: Windows Keyboard Input Corruption Fix

### Problem
After opening the SSH Health Dashboard and then closing it, special keys (Escape, Ctrl+key, Shift+key, Alt+key) would stop working. The keys would only generate `Release` events, with no `Press` events being sent by crossterm.

### Root Cause
Spawning `plink.exe` (PuTTY's command-line SSH client) from background threads corrupted the Windows console input mode. Even with `CREATE_NO_WINDOW` and `DETACHED_PROCESS` flags, the console's keyboard handling was affected.

The corruption specifically affected:
- Escape key
- Any key combination with Ctrl modifier
- Any key combination with Shift modifier
- Any key combination with Alt modifier

Regular letter keys and arrow keys continued to work normally.

### Solution
Two-part fix implemented:

#### 1. Process Creation Flags (`src/ssh/collector.rs`)

Added Windows-specific flags to prevent child processes from affecting the parent console:

```rust
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x00000008;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn execute_ssh_command(mut cmd: Command, host_id: u32) -> DeviceMetrics {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // CRITICAL: On Windows, use DETACHED_PROCESS to completely detach from
    // the parent console. This prevents plink.exe from corrupting the console's
    // input mode.
    #[cfg(windows)]
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);

    // ... rest of function
}
```

#### 2. Release Event Workaround (`src/app/input.rs`)

Accept Release events for keys that are affected by the console corruption:

```rust
pub(super) fn handle_key(&mut self, key: KeyEvent) {
    // Normally we only handle Press events. However, on Windows, spawning
    // child processes (like plink.exe) can corrupt the console input mode,
    // causing certain keys to only generate Release events (no Press).
    // As a workaround, we also accept Release events for keys that are
    // commonly affected: Escape, and any key with Ctrl/Alt/Shift modifiers.
    let dominated_key = key.code == KeyCode::Esc
        || key.modifiers.contains(KeyModifiers::CONTROL)
        || key.modifiers.contains(KeyModifiers::ALT)
        || key.modifiers.contains(KeyModifiers::SHIFT);

    if key.kind != KeyEventKind::Press {
        if key.kind == KeyEventKind::Release && dominated_key {
            // Fall through to handle this Release event as if it were a Press
        } else {
            return;
        }
    }

    // ... rest of function
}
```

#### 3. Terminal Mode Reset (`src/app/health_ops.rs`)

Reset terminal raw mode when closing the dashboard as an additional safety measure:

```rust
pub fn close_health_dashboard(&mut self) {
    // ... cleanup code ...

    // CRITICAL: Reset terminal raw mode to fix keyboard input
    if let Err(e) = disable_raw_mode() {
        warn!("Failed to disable raw mode: {}", e);
    }
    if let Err(e) = enable_raw_mode() {
        warn!("Failed to re-enable raw mode: {}", e);
    }

    self.mode = AppMode::Normal;
}
```

### Files Changed
- `src/ssh/collector.rs` - Added Windows process creation flags
- `src/app/input.rs` - Added Release event workaround for dominated keys
- `src/app/health_ops.rs` - Added terminal raw mode reset on dashboard close

### Testing
1. Open SSH Manager (Ctrl+H)
2. Press 'h' to open Health Dashboard
3. Wait for SSH connections to complete
4. Press Escape or Ctrl+Q - should work
5. Close dashboard with 'q'
6. Verify Escape, Ctrl+Q, and other special keys still work in normal mode
