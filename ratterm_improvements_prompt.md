# ratterm Improvements: SSH, Docker & IDE Upgrades
## Claude Code Build Prompt — Phased Execution Plan

**Project:** `hastur-dev/ratterm`  
**Target:** Comprehensive upgrades to SSH management, Docker management, and the IDE subsystem  
**Execution model:** Claude Code autonomous agent — each phase must fully compile and pass all tests before proceeding  
**Hard rules:**
- No `todo!()`, no `unimplemented!()`, no stub functions — every function must be complete
- `cargo build` must succeed at every ✅ checkpoint
- `cargo test` must pass at every ✅ checkpoint
- Read every file before modifying it — `cat` before `edit`
- Compile after every individual file change, not just at phase end
- Test count is cumulative and must grow each phase

---

## Pre-Flight: Repository Audit

Before writing any code, execute the following read-only audit to understand current state:

```bash
# Understand full structure
find src -name "*.rs" | sort
cat Cargo.toml
cat src/main.rs
```

Then read each source file in full:

```bash
cat src/app.rs          # or equivalent app state file
cat src/ui.rs           # or equivalent render file
cat src/editor.rs       # editor subsystem
cat src/terminal.rs     # PTY terminal subsystem
cat src/ssh.rs          # SSH subsystem (if exists)
cat src/docker.rs       # Docker subsystem (if exists)
cat src/config.rs       # config/ratrc parsing
```

Use `find src -name "*.rs"` to enumerate all files and read all of them. Do not guess at structure — read first.

After the audit, document your findings in a comment block at the top of this task log before proceeding to Phase 1.

---

## Phase 1: Test Scaffolding & Compile Baseline

**Goal:** Establish that the project compiles cleanly and build the integration test harness that all subsequent phases will extend.

**Minimum test count after this phase: 15**

### Step 1.1 — Verify clean baseline

```bash
cargo build 2>&1
cargo test 2>&1
```

If `cargo build` fails, fix all errors before proceeding. Do not continue with a broken baseline.

✅ **Checkpoint 1.1:** `cargo build` exits 0.

### Step 1.2 — Add test utilities module

Create `src/test_utils.rs`:

```rust
// src/test_utils.rs
// Shared test helpers — only compiled in #[cfg(test)] contexts

#[cfg(test)]
pub mod helpers {
    /// Build a minimal AppConfig with all optional fields at their defaults.
    /// Used so tests never depend on disk state.
    pub fn default_config() -> crate::config::Config {
        // Construct using the public API of Config — do not assume field names;
        // read src/config.rs first and use whatever the actual constructor is.
        todo_replace_with_real_constructor()
    }
}
```

> **IMPORTANT:** After reading `src/config.rs`, replace `todo_replace_with_real_constructor()` with the real construction. No stubs survive this phase.

Add the module to `src/main.rs` or `src/lib.rs` (whichever is the crate root) under `#[cfg(test)]`:

```rust
#[cfg(test)]
mod test_utils;
```

✅ **Checkpoint 1.2:** `cargo test` passes with ≥ 15 tests.

### Step 1.3 — Add integration test file

Create `tests/integration_smoke.rs`:

```rust
//! Smoke-level integration tests: verify that core data structures
//! can be constructed and exercised without a real terminal.

#[test]
fn config_default_constructs() { /* ... */ }

#[test]
fn ssh_profile_roundtrip_toml() { /* Phase 2 will fill this */ }

#[test]
fn docker_panel_state_default() { /* Phase 3 will fill this */ }

#[test]
fn editor_buffer_open_empty() { /* Phase 4 will fill this */ }
```

These are placeholder tests that must compile. In each subsequent phase, replace the placeholder body with real assertions. A test with an empty body `{}` is acceptable here as long as it compiles — but do not use `todo!()`.

✅ **Checkpoint 1.3:** `cargo test` passes, all 4 integration placeholders compile and run (even if they assert nothing yet).

---

## Phase 2: SSH Management — Connection Profiles & Session Manager

**Goal:** Replace or extend any existing ad-hoc SSH code with a full profile-based connection manager. The user should be able to manage saved SSH connections from within the TUI.

**Minimum test count after this phase: 35**

### Step 2.1 — Read existing SSH code

```bash
cat src/ssh.rs           # read in full
grep -rn "ssh" src/      # find all SSH references
```

Document every public type and function currently in the SSH subsystem before touching anything.

### Step 2.2 — Create `src/ssh/mod.rs` (or extend `src/ssh.rs`)

If SSH code is currently in a flat `src/ssh.rs`, convert it to a module directory:

```
src/ssh/
    mod.rs          ← re-exports, module-level docs
    profile.rs      ← SshProfile, SshProfileStore
    session.rs      ← SshSession, connection state machine
    sftp.rs         ← SftpPanel, directory listing, file ops
    tunnel.rs       ← PortForwardConfig, active tunnel tracking
    ui.rs           ← TUI widgets for the SSH panel
```

All existing functionality must be preserved and must still compile. Migration is allowed; deletion of working code is not.

### Step 2.3 — `src/ssh/profile.rs`

Implement the following types. Read `src/config.rs` to understand how the existing config parser works, then follow the same pattern.

```rust
// src/ssh/profile.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A saved SSH connection profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshProfile {
    /// Human-readable nickname shown in the UI (e.g. "prod-web-01")
    pub name: String,
    /// Target hostname or IP
    pub host: String,
    /// TCP port (default 22)
    pub port: u16,
    /// Remote username
    pub username: String,
    /// Authentication method
    pub auth: SshAuthMethod,
    /// Optional jump host (bastion)
    pub jump_host: Option<String>,
    /// Optional port forwards to establish automatically on connect
    pub auto_forwards: Vec<PortForwardConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SshAuthMethod {
    /// Use the running ssh-agent
    Agent,
    /// Path to a private key file, with optional passphrase stored in system keyring
    Key { path: PathBuf, passphrase_in_keyring: bool },
    /// Password prompt (never stored on disk)
    Password,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortForwardConfig {
    pub direction: ForwardDirection,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ForwardDirection { Local, Remote, Dynamic }

/// Persisted collection of profiles, saved to ~/.ratterm/ssh_profiles.toml
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SshProfileStore {
    pub profiles: Vec<SshProfile>,
}

impl SshProfileStore {
    /// Load from the canonical path, returning an empty store if the file
    /// does not exist. Returns Err only on parse failures.
    pub fn load() -> anyhow::Result<Self> { todo!() }

    /// Persist to the canonical path, creating parent directories as needed.
    pub fn save(&self) -> anyhow::Result<()> { todo!() }

    /// Returns the canonical profiles path: ~/.ratterm/ssh_profiles.toml
    pub fn profiles_path() -> PathBuf { todo!() }

    pub fn add(&mut self, profile: SshProfile) { self.profiles.push(profile); }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        self.profiles.len() < before
    }

    pub fn find(&self, name: &str) -> Option<&SshProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }
}
```

Replace every `todo!()` with a real implementation. No stubs survive.

Unit tests to add in `src/ssh/profile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn profile_serializes_to_toml() { /* ... */ }

    #[test]
    fn profile_store_roundtrip() { /* serialize → parse → compare */ }

    #[test]
    fn profile_store_add_remove() { /* add two, remove one, verify one remains */ }

    #[test]
    fn port_forward_config_roundtrip() { /* ... */ }

    #[test]
    fn auth_method_key_roundtrip() { /* ... */ }
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `Cargo.toml` if not present.

✅ **Checkpoint 2.3:** `cargo test ssh::profile` passes all 5 new tests.

### Step 2.4 — `src/ssh/session.rs`

Implement a connection state machine that wraps `ssh2::Session`. Read the existing SSH code first to understand what is already wired up.

```rust
// src/ssh/session.rs

use super::profile::{SshProfile, SshAuthMethod};

#[derive(Debug, Clone, PartialEq)]
pub enum SshConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    Connected { username: String, host: String },
    Error(String),
}

pub struct SshSession {
    pub state: SshConnectionState,
    // inner ssh2::Session — only present when Connected
    inner: Option<ssh2::Session>,
    pub profile: Option<SshProfile>,
}

impl SshSession {
    pub fn new() -> Self { /* ... */ }

    /// Begin connecting to the given profile. Updates state to Connecting.
    /// Does NOT block — callers should poll `state` or await a channel.
    pub fn connect(&mut self, profile: SshProfile) -> anyhow::Result<()> { /* ... */ }

    pub fn disconnect(&mut self) { /* ... */ }

    pub fn is_connected(&self) -> bool {
        matches!(self.state, SshConnectionState::Connected { .. })
    }

    /// Execute a command over the session, returning stdout as a String.
    pub fn exec(&self, cmd: &str) -> anyhow::Result<String> { /* ... */ }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_starts_disconnected() { /* ... */ }

    #[test]
    fn session_transitions_to_error_on_bad_host() { /* ... */ }

    #[test]
    fn is_connected_false_when_disconnected() { /* ... */ }
}
```

✅ **Checkpoint 2.4:** `cargo test ssh::session` passes. `cargo build` succeeds.

### Step 2.5 — `src/ssh/sftp.rs` — SFTP file browser state

```rust
// src/ssh/sftp.rs

#[derive(Debug, Clone)]
pub struct SftpEntry {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub permissions: u32,
}

#[derive(Debug)]
pub struct SftpPanel {
    pub current_path: std::path::PathBuf,
    pub entries: Vec<SftpEntry>,
    pub selected_index: usize,
    pub is_active: bool,
}

impl SftpPanel {
    pub fn new() -> Self { /* ... */ }

    /// Populate entries from a live SSH session's SFTP subsystem.
    pub fn list_directory(&mut self, session: &ssh2::Session, path: &std::path::Path)
        -> anyhow::Result<()> { /* ... */ }

    pub fn selected_entry(&self) -> Option<&SftpEntry> {
        self.entries.get(self.selected_index)
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.entries.len() {
            self.selected_index += 1;
        }
    }

    pub fn navigate_into_selected(&mut self) -> Option<std::path::PathBuf> {
        self.selected_entry()
            .filter(|e| e.is_dir)
            .map(|e| self.current_path.join(&e.name))
    }

    pub fn navigate_up(&mut self) -> std::path::PathBuf {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
        }
        self.current_path.clone()
    }
}
```

Unit tests (no live SSH required — test pure state logic):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sftp_panel_starts_at_root() { /* ... */ }

    #[test]
    fn selected_entry_none_when_empty() { /* ... */ }

    #[test]
    fn move_up_clamps_at_zero() { /* ... */ }

    #[test]
    fn navigate_up_from_nested_path() { /* ... */ }

    #[test]
    fn navigate_into_dir_returns_new_path() { /* ... */ }
}
```

✅ **Checkpoint 2.5:** `cargo test ssh::sftp` passes all 5 new tests.

### Step 2.6 — `src/ssh/ui.rs` — TUI widgets

Implement ratatui `Widget` rendering for the SSH panel. Read the existing UI render code first to understand the layout system in use.

Create the following render functions (no `todo!()` — implement each one fully):

```rust
// src/ssh/ui.rs

use ratatui::{prelude::*, widgets::*};
use super::profile::SshProfile;
use super::session::SshConnectionState;
use super::sftp::SftpPanel;

/// Render the SSH profile list in the given area.
pub fn render_ssh_profile_list(
    f: &mut Frame,
    area: Rect,
    profiles: &[SshProfile],
    selected: usize,
    is_focused: bool,
) { /* ... */ }

/// Render the SSH connection status bar.
pub fn render_ssh_status(
    f: &mut Frame,
    area: Rect,
    state: &SshConnectionState,
) { /* ... */ }

/// Render the SFTP file browser panel.
pub fn render_sftp_panel(
    f: &mut Frame,
    area: Rect,
    panel: &SftpPanel,
) { /* ... */ }
```

Unit tests — use ratatui's `TestBackend` to verify rendering does not panic:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn make_terminal(w: u16, h: u16) -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(w, h)).unwrap()
    }

    #[test]
    fn render_empty_profile_list_does_not_panic() {
        let mut t = make_terminal(80, 24);
        t.draw(|f| {
            let area = f.area();
            render_ssh_profile_list(f, area, &[], 0, false);
        }).unwrap();
    }

    #[test]
    fn render_ssh_status_disconnected_does_not_panic() { /* ... */ }

    #[test]
    fn render_sftp_panel_empty_does_not_panic() { /* ... */ }
}
```

✅ **Checkpoint 2.6:** `cargo test ssh::ui` passes. Total test count ≥ 35.

### Step 2.7 — Wire SSH panel into app state

Read `src/app.rs` (or equivalent) in full, then:

1. Add `ssh_profiles: SshProfileStore` field to the app state struct
2. Add `active_ssh_session: Option<SshSession>` field
3. Add `sftp_panel: Option<SftpPanel>` field
4. Add `show_ssh_panel: bool` toggle field
5. In the app's event handler, add keybinding `Ctrl+Shift+H` → toggle SSH panel
6. In the app's render function, conditionally render the SSH panel when `show_ssh_panel` is true
7. Load `SshProfileStore` on startup in the app constructor

After each of these 7 sub-steps, run `cargo build` and fix any errors before proceeding to the next.

✅ **Checkpoint 2.7:** `cargo build` succeeds. `cargo test` passes all prior tests plus any new ones.

---

## Phase 3: Docker Management — Full Container Lifecycle Panel

**Goal:** Upgrade the Docker subsystem from basic container listing to a full management panel with real-time log streaming, image management, stats, and Docker Compose support.

**Minimum test count after this phase: 60**

### Step 3.1 — Read existing Docker code

```bash
cat src/docker.rs        # read in full
grep -rn "bollard\|docker" src/
grep -rn "bollard" Cargo.toml
```

Document every existing Docker type and function before modifying anything.

### Step 3.2 — Restructure into `src/docker/` module

If Docker code is in a flat `src/docker.rs`, migrate to:

```
src/docker/
    mod.rs          ← re-exports
    container.rs    ← ContainerState, ContainerAction, lifecycle ops
    image.rs        ← ImageInfo, image management
    logs.rs         ← LogStreamer, ring-buffer log storage
    stats.rs        ← ContainerStats, live CPU/memory/network
    compose.rs      ← DockerComposeProject detection and ops
    ui.rs           ← ratatui widgets for all Docker panels
```

Preserve all existing functionality during migration.

### Step 3.3 — `src/docker/container.rs`

```rust
// src/docker/container.rs

#[derive(Debug, Clone, PartialEq)]
pub enum ContainerStatus {
    Running,
    Exited(i64),  // exit code
    Paused,
    Restarting,
    Dead,
    Created,
    Unknown(String),
}

impl ContainerStatus {
    /// Parse from the string returned by the Docker daemon.
    pub fn from_docker_str(s: &str) -> Self { /* ... */ }

    pub fn is_running(&self) -> bool {
        matches!(self, ContainerStatus::Running)
    }

    pub fn display_color(&self) -> ratatui::style::Color {
        match self {
            ContainerStatus::Running => ratatui::style::Color::Green,
            ContainerStatus::Paused  => ratatui::style::Color::Yellow,
            ContainerStatus::Exited(_) | ContainerStatus::Dead => ratatui::style::Color::Red,
            _ => ratatui::style::Color::Gray,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: String,           // full 64-char ID
    pub short_id: String,     // first 12 chars
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub ports: Vec<PortMapping>,
    pub created_at: i64,      // Unix timestamp
}

#[derive(Debug, Clone)]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: Option<u16>,
    pub protocol: String,     // "tcp" | "udp"
}

/// Actions the user can invoke on a container.
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Remove { force: bool },
    OpenShell,
    ViewLogs,
    Inspect,
}

/// Pure state for the container list panel (no async — driven by app update loop).
#[derive(Debug)]
pub struct ContainerListPanel {
    pub containers: Vec<ContainerInfo>,
    pub selected_index: usize,
    pub filter: String,
    pub show_all: bool,   // include stopped containers
    pub is_loading: bool,
    pub last_error: Option<String>,
}

impl ContainerListPanel {
    pub fn new() -> Self { /* ... */ }

    pub fn filtered_containers(&self) -> Vec<&ContainerInfo> {
        self.containers.iter()
            .filter(|c| {
                (self.show_all || c.status.is_running()) &&
                (self.filter.is_empty() ||
                    c.name.contains(&self.filter) ||
                    c.image.contains(&self.filter))
            })
            .collect()
    }

    pub fn selected_container(&self) -> Option<&ContainerInfo> {
        let filtered = self.filtered_containers();
        filtered.get(self.selected_index).copied()
    }

    pub fn move_up(&mut self) { if self.selected_index > 0 { self.selected_index -= 1; } }

    pub fn move_down(&mut self) {
        let max = self.filtered_containers().len().saturating_sub(1);
        if self.selected_index < max { self.selected_index += 1; }
    }

    pub fn update_containers(&mut self, new_list: Vec<ContainerInfo>) {
        // Preserve selected container by ID across refreshes
        let selected_id = self.selected_container().map(|c| c.id.clone());
        self.containers = new_list;
        if let Some(id) = selected_id {
            let filtered = self.filtered_containers();
            if let Some(pos) = filtered.iter().position(|c| c.id == id) {
                self.selected_index = pos;
            }
        }
    }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_status_from_running_str() {
        assert_eq!(ContainerStatus::from_docker_str("running"), ContainerStatus::Running);
    }

    #[test]
    fn container_status_from_exited_str() {
        matches!(ContainerStatus::from_docker_str("exited"), ContainerStatus::Exited(_));
    }

    #[test]
    fn panel_filtered_containers_respects_show_all() { /* ... */ }

    #[test]
    fn panel_filter_by_name() { /* ... */ }

    #[test]
    fn panel_move_down_clamps_at_end() { /* ... */ }

    #[test]
    fn panel_update_preserves_selection_by_id() { /* ... */ }

    #[test]
    fn selected_container_none_when_empty() { /* ... */ }
}
```

✅ **Checkpoint 3.3:** `cargo test docker::container` passes all 7 tests.

### Step 3.4 — `src/docker/logs.rs` — Ring-buffer log streaming

```rust
// src/docker/logs.rs

/// A fixed-capacity ring buffer for container log lines.
/// Oldest lines are dropped when capacity is exceeded.
#[derive(Debug)]
pub struct LogBuffer {
    lines: std::collections::VecDeque<LogLine>,
    capacity: usize,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub timestamp: Option<String>,
    pub stream: LogStream,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogStream { Stdout, Stderr }

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self { lines: std::collections::VecDeque::with_capacity(capacity), capacity }
    }

    pub fn push(&mut self, line: LogLine) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn lines(&self) -> impl Iterator<Item = &LogLine> {
        self.lines.iter()
    }

    pub fn len(&self) -> usize { self.lines.len() }

    pub fn is_empty(&self) -> bool { self.lines.is_empty() }

    pub fn clear(&mut self) { self.lines.clear(); }

    /// Return the last `n` lines.
    pub fn tail(&self, n: usize) -> impl Iterator<Item = &LogLine> {
        let skip = self.lines.len().saturating_sub(n);
        self.lines.iter().skip(skip)
    }
}

/// State for the log viewer panel.
#[derive(Debug)]
pub struct LogViewerPanel {
    pub container_id: Option<String>,
    pub buffer: LogBuffer,
    pub scroll_offset: usize,
    pub follow: bool,  // auto-scroll to bottom on new lines
    pub filter: String,
}

impl LogViewerPanel {
    pub fn new() -> Self {
        Self {
            container_id: None,
            buffer: LogBuffer::new(10_000),
            scroll_offset: 0,
            follow: true,
            filter: String::new(),
        }
    }

    pub fn attach_to(&mut self, container_id: String) {
        self.container_id = Some(container_id);
        self.buffer.clear();
        self.scroll_offset = 0;
    }

    pub fn append_line(&mut self, line: LogLine) {
        self.buffer.push(line);
        if self.follow {
            self.scroll_offset = self.buffer.len().saturating_sub(1);
        }
    }

    pub fn toggle_follow(&mut self) { self.follow = !self.follow; }

    pub fn scroll_up(&mut self, n: usize) {
        self.follow = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        let max = self.buffer.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + n).min(max);
    }

    pub fn visible_lines(&self, height: usize) -> Vec<&LogLine> {
        self.buffer.lines()
            .filter(|l| self.filter.is_empty() || l.content.contains(&self.filter))
            .skip(self.scroll_offset)
            .take(height)
            .collect()
    }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(content: &str) -> LogLine {
        LogLine { timestamp: None, stream: LogStream::Stdout, content: content.to_string() }
    }

    #[test]
    fn log_buffer_respects_capacity() {
        let mut b = LogBuffer::new(3);
        for i in 0..5 { b.push(make_line(&i.to_string())); }
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn log_buffer_tail_returns_last_n() { /* ... */ }

    #[test]
    fn log_viewer_follow_auto_scrolls() { /* ... */ }

    #[test]
    fn log_viewer_toggle_follow_disables() { /* ... */ }

    #[test]
    fn log_viewer_scroll_up_disables_follow() { /* ... */ }

    #[test]
    fn log_viewer_visible_lines_respects_filter() { /* ... */ }
}
```

✅ **Checkpoint 3.4:** `cargo test docker::logs` passes all 6 tests.

### Step 3.5 — `src/docker/stats.rs` — Live container stats

```rust
// src/docker/stats.rs

/// A single stats snapshot for one container.
#[derive(Debug, Clone, Default)]
pub struct ContainerStats {
    pub container_id: String,
    pub cpu_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub pids: u64,
}

impl ContainerStats {
    pub fn memory_percent(&self) -> f64 {
        if self.memory_limit_bytes == 0 { return 0.0; }
        (self.memory_usage_bytes as f64 / self.memory_limit_bytes as f64) * 100.0
    }

    pub fn memory_usage_mb(&self) -> f64 {
        self.memory_usage_bytes as f64 / 1_048_576.0
    }

    pub fn memory_limit_mb(&self) -> f64 {
        self.memory_limit_bytes as f64 / 1_048_576.0
    }
}

/// Rolling history of stats for sparkline rendering.
#[derive(Debug)]
pub struct StatsHistory {
    pub container_id: String,
    pub cpu_history: std::collections::VecDeque<f64>,
    pub mem_history: std::collections::VecDeque<f64>,
    capacity: usize,
}

impl StatsHistory {
    pub fn new(container_id: String, capacity: usize) -> Self {
        Self {
            container_id,
            cpu_history: std::collections::VecDeque::with_capacity(capacity),
            mem_history: std::collections::VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, stats: &ContainerStats) {
        if self.cpu_history.len() == self.capacity { self.cpu_history.pop_front(); }
        if self.mem_history.len() == self.capacity { self.mem_history.pop_front(); }
        self.cpu_history.push_back(stats.cpu_percent);
        self.mem_history.push_back(stats.memory_percent());
    }

    pub fn cpu_sparkline_data(&self) -> Vec<u64> {
        self.cpu_history.iter().map(|v| *v as u64).collect()
    }

    pub fn mem_sparkline_data(&self) -> Vec<u64> {
        self.mem_history.iter().map(|v| *v as u64).collect()
    }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_percent_zero_when_limit_zero() {
        let s = ContainerStats { memory_limit_bytes: 0, ..Default::default() };
        assert_eq!(s.memory_percent(), 0.0);
    }

    #[test]
    fn memory_percent_correct() { /* ... */ }

    #[test]
    fn stats_history_respects_capacity() { /* ... */ }

    #[test]
    fn sparkline_data_matches_history() { /* ... */ }
}
```

✅ **Checkpoint 3.5:** `cargo test docker::stats` passes.

### Step 3.6 — `src/docker/image.rs`

```rust
// src/docker/image.rs

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub id: String,
    pub short_id: String,
    pub tags: Vec<String>,
    pub size_bytes: u64,
    pub created_at: i64,
}

impl ImageInfo {
    pub fn display_tag(&self) -> String {
        self.tags.first().cloned().unwrap_or_else(|| format!("<none>:{}", &self.short_id))
    }

    pub fn size_mb(&self) -> f64 {
        self.size_bytes as f64 / 1_048_576.0
    }
}

#[derive(Debug)]
pub struct ImageListPanel {
    pub images: Vec<ImageInfo>,
    pub selected_index: usize,
    pub filter: String,
}

impl ImageListPanel {
    pub fn new() -> Self { /* ... */ }

    pub fn filtered_images(&self) -> Vec<&ImageInfo> {
        self.images.iter()
            .filter(|i| self.filter.is_empty() || i.display_tag().contains(&self.filter))
            .collect()
    }

    pub fn selected_image(&self) -> Option<&ImageInfo> {
        self.filtered_images().get(self.selected_index).copied()
    }

    pub fn move_up(&mut self) { if self.selected_index > 0 { self.selected_index -= 1; } }

    pub fn move_down(&mut self) {
        let max = self.filtered_images().len().saturating_sub(1);
        if self.selected_index < max { self.selected_index += 1; }
    }
}
```

Tests: add 4 unit tests covering `filtered_images`, `selected_image`, cursor movement, and `size_mb`.

### Step 3.7 — `src/docker/compose.rs`

```rust
// src/docker/compose.rs
// Docker Compose project detection and state — does NOT execute compose commands directly;
// it discovers projects and builds the command strings that the terminal subsystem executes.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ComposeProject {
    pub name: String,
    pub directory: PathBuf,
    pub compose_file: PathBuf,
    pub services: Vec<String>,
}

impl ComposeProject {
    /// Scan a directory for compose files (docker-compose.yml, docker-compose.yaml,
    /// compose.yml, compose.yaml).
    pub fn discover_in(dir: &Path) -> Vec<ComposeProject> { /* ... */ }

    /// Returns the shell command string to run `docker compose up -d` in this project.
    pub fn up_command(&self) -> String {
        format!("cd {} && docker compose up -d", self.directory.display())
    }

    pub fn down_command(&self) -> String {
        format!("cd {} && docker compose down", self.directory.display())
    }

    pub fn logs_command(&self, service: Option<&str>, follow: bool) -> String {
        let svc = service.unwrap_or("");
        let f = if follow { " -f" } else { "" };
        format!("cd {} && docker compose logs{} {}", self.directory.display(), f, svc)
    }
}
```

Tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn discover_finds_docker_compose_yml() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("docker-compose.yml"), "version: '3'\nservices:\n  web:\n    image: nginx\n").unwrap();
        let projects = ComposeProject::discover_in(dir.path());
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn discover_finds_compose_yaml_variant() { /* ... */ }

    #[test]
    fn up_command_contains_directory() { /* ... */ }

    #[test]
    fn logs_command_with_follow_flag() { /* ... */ }

    #[test]
    fn discover_empty_dir_returns_empty() { /* ... */ }
}
```

✅ **Checkpoint 3.7:** `cargo test docker` passes all docker module tests.

### Step 3.8 — `src/docker/ui.rs` — Docker TUI widgets

Implement `render_container_list`, `render_log_viewer`, `render_stats_panel`, `render_image_list`, and `render_compose_panel` using ratatui. Each widget must use `TestBackend` in its unit test and must not panic on empty state.

Add 5 render-does-not-panic tests (one per widget).

✅ **Checkpoint 3.8:** `cargo test docker::ui` passes.

### Step 3.9 — Wire Docker panel into app state

Read `src/app.rs` in full, then:

1. Add `docker_panel: ContainerListPanel` to app state
2. Add `docker_log_viewer: LogViewerPanel` to app state
3. Add `docker_image_panel: ImageListPanel` to app state
4. Add `docker_stats: HashMap<String, StatsHistory>` to app state
5. Add `show_docker_panel: bool` toggle
6. Wire `Ctrl+Shift+D` → toggle Docker panel
7. Add tab navigation within Docker panel: `1` = Containers, `2` = Images, `3` = Compose
8. Wire `s` on selected container → OpenShell (runs `docker exec -it <id> /bin/sh` in the active terminal pane)
9. Wire `l` on selected container → ViewLogs (attaches log viewer to that container)
10. Add a 5-second background refresh via `tokio::time::interval` that re-fetches container list

After each numbered sub-step above, run `cargo build` and fix errors.

✅ **Checkpoint 3.9:** `cargo build` succeeds. `cargo test` passes ≥ 60 total tests.

---

## Phase 4: IDE Syntax Highlighting Engine

**Goal:** Add language-aware syntax highlighting to the editor using tree-sitter. The editor currently renders lines as plain text; this phase makes it render colored tokens.

**Minimum test count after this phase: 80**

### Step 4.1 — Read editor code

```bash
cat src/editor.rs        # read entirely
grep -n "render\|draw\|line" src/editor.rs | head -60
cat src/ui.rs            # read how editor is currently rendered
```

Understand exactly how the editor currently converts buffer lines to ratatui `Line`/`Span` objects before touching anything.

### Step 4.2 — Add tree-sitter dependencies

Add to `Cargo.toml` under `[dependencies]`:

```toml
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-typescript = { version = "0.21", features = [] }
tree-sitter-toml = "0.21"
tree-sitter-json = "0.21"
tree-sitter-bash = "0.21"
tree-sitter-markdown = "0.2"
```

Run `cargo build` immediately after adding these. Fix any version conflicts before proceeding.

✅ **Checkpoint 4.2:** `cargo build` succeeds with new dependencies.

### Step 4.3 — Create `src/editor/highlight.rs`

```
src/editor/
    mod.rs            ← existing editor code (or re-export from flat editor.rs)
    highlight.rs      ← NEW: syntax highlighting engine
    language.rs       ← NEW: language detection
```

If `src/editor.rs` is currently a flat file, convert to module:
1. Create `src/editor/` directory
2. Move `src/editor.rs` content to `src/editor/mod.rs`
3. Add `pub mod highlight;` and `pub mod language;`
4. Update `src/main.rs` — no import path changes should be needed if pub re-exports are preserved

```rust
// src/editor/language.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Toml,
    Json,
    Bash,
    Markdown,
    PlainText,
}

impl Language {
    /// Detect from file extension. Returns PlainText for unknown extensions.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs"                         => Language::Rust,
            "py" | "pyw"                 => Language::Python,
            "js" | "mjs" | "cjs"         => Language::JavaScript,
            "ts" | "tsx"                 => Language::TypeScript,
            "toml"                       => Language::Toml,
            "json" | "jsonc"             => Language::Json,
            "sh" | "bash" | "zsh"        => Language::Bash,
            "md" | "markdown"            => Language::Markdown,
            _                            => Language::PlainText,
        }
    }

    /// Detect from a shebang line (e.g. "#!/usr/bin/env python3").
    pub fn from_shebang(first_line: &str) -> Option<Self> {
        if first_line.contains("python") { return Some(Language::Python); }
        if first_line.contains("bash") || first_line.contains("sh") {
            return Some(Language::Bash);
        }
        if first_line.contains("node") { return Some(Language::JavaScript); }
        None
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Rust       => "Rust",
            Language::Python     => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Toml       => "TOML",
            Language::Json       => "JSON",
            Language::Bash       => "Bash",
            Language::Markdown   => "Markdown",
            Language::PlainText  => "Text",
        }
    }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_extension_detected() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
    }

    #[test]
    fn ts_extension_detected() { /* ... */ }

    #[test]
    fn unknown_extension_returns_plain_text() {
        assert_eq!(Language::from_extension("xyz"), Language::PlainText);
    }

    #[test]
    fn python_shebang_detected() {
        assert_eq!(Language::from_shebang("#!/usr/bin/env python3"), Some(Language::Python));
    }

    #[test]
    fn non_shebang_returns_none() {
        assert_eq!(Language::from_shebang("fn main() {}"), None);
    }

    #[test]
    fn display_name_non_empty() {
        let langs = [Language::Rust, Language::Python, Language::PlainText];
        for l in langs { assert!(!l.display_name().is_empty()); }
    }
}
```

✅ **Checkpoint 4.3a:** `cargo test editor::language` passes all 6 tests.

### Step 4.4 — `src/editor/highlight.rs`

```rust
// src/editor/highlight.rs

use tree_sitter::{Parser, Language as TsLanguage, Tree};
use ratatui::style::{Color, Style};
use super::language::Language;

/// A single highlighted token span within a source line.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    /// Byte offset within the line (not char offset)
    pub start_byte: usize,
    pub end_byte: usize,
    pub style: Style,
}

/// Maps tree-sitter node kinds to ratatui styles.
pub struct ThemeMap {
    pub keyword:    Style,
    pub string:     Style,
    pub number:     Style,
    pub comment:    Style,
    pub function:   Style,
    pub type_name:  Style,
    pub operator:   Style,
    pub variable:   Style,
    pub default:    Style,
}

impl Default for ThemeMap {
    fn default() -> Self {
        Self {
            keyword:   Style::default().fg(Color::Magenta),
            string:    Style::default().fg(Color::Green),
            number:    Style::default().fg(Color::Cyan),
            comment:   Style::default().fg(Color::DarkGray),
            function:  Style::default().fg(Color::Blue),
            type_name: Style::default().fg(Color::Yellow),
            operator:  Style::default().fg(Color::White),
            variable:  Style::default().fg(Color::LightBlue),
            default:   Style::default(),
        }
    }
}

/// The syntax highlighting engine. One instance per open editor buffer.
pub struct SyntaxHighlighter {
    parser: Parser,
    language: Language,
    tree: Option<Tree>,
    theme: ThemeMap,
}

impl SyntaxHighlighter {
    /// Create a new highlighter for the given language.
    /// Returns None if tree-sitter does not support the language.
    pub fn new(language: Language) -> Option<Self> {
        let ts_lang = get_ts_language(language)?;
        let mut parser = Parser::new();
        parser.set_language(&ts_lang).ok()?;
        Some(Self { parser, language, tree: None, theme: ThemeMap::default() })
    }

    /// Re-parse the full source text. Call this when the buffer changes.
    pub fn parse(&mut self, source: &str) {
        self.tree = self.parser.parse(source, self.tree.as_ref());
    }

    /// Compute highlight spans for a single line.
    /// `line_start_byte` is the byte offset of the line within the full source.
    pub fn highlight_line(
        &self,
        source: &str,
        line_start_byte: usize,
        line_end_byte: usize,
    ) -> Vec<HighlightSpan> {
        let Some(tree) = &self.tree else { return vec![]; };
        let root = tree.root_node();
        let mut spans = Vec::new();
        collect_spans(root, source.as_bytes(), line_start_byte, line_end_byte, &self.theme, &mut spans);
        spans.sort_by_key(|s| s.start_byte);
        spans
    }
}

/// Recursively walk the syntax tree and collect styled spans.
fn collect_spans(
    node: tree_sitter::Node,
    source: &[u8],
    line_start: usize,
    line_end: usize,
    theme: &ThemeMap,
    out: &mut Vec<HighlightSpan>,
) {
    let node_start = node.start_byte();
    let node_end = node.end_byte();

    // Skip nodes entirely outside the current line
    if node_end <= line_start || node_start >= line_end { return; }

    // Only emit leaf nodes (no children means terminal token)
    if node.child_count() == 0 {
        let style = style_for_node_kind(node.kind(), theme);
        out.push(HighlightSpan {
            start_byte: node_start.max(line_start) - line_start,
            end_byte:   node_end.min(line_end)   - line_start,
            style,
        });
        return;
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_spans(child, source, line_start, line_end, theme, out);
        }
    }
}

fn style_for_node_kind(kind: &str, theme: &ThemeMap) -> Style {
    match kind {
        "identifier"                   => theme.variable,
        "string_literal" | "string"    => theme.string,
        "integer_literal"| "float_literal" | "number" => theme.number,
        "line_comment" | "block_comment" | "comment"  => theme.comment,
        k if is_keyword(k)             => theme.keyword,
        _                              => theme.default,
    }
}

fn is_keyword(kind: &str) -> bool {
    matches!(kind,
        "fn" | "let" | "mut" | "pub" | "use" | "mod" | "struct" | "enum" |
        "impl" | "trait" | "if" | "else" | "match" | "for" | "while" |
        "return" | "true" | "false" | "self" | "Self" | "super" | "crate" |
        "async" | "await" | "move" | "type" | "where" | "const" | "static" |
        "def" | "class" | "import" | "from" | "None" | "True" | "False" |
        "function" | "const" | "var" | "let" | "export" | "default"
    )
}

fn get_ts_language(lang: Language) -> Option<TsLanguage> {
    match lang {
        Language::Rust       => Some(tree_sitter_rust::LANGUAGE.into()),
        Language::Python     => Some(tree_sitter_python::LANGUAGE.into()),
        Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        Language::TypeScript => Some(tree_sitter_typescript::language_typescript()),
        Language::Toml       => Some(tree_sitter_toml::LANGUAGE.into()),
        Language::Json       => Some(tree_sitter_json::LANGUAGE.into()),
        Language::Bash       => Some(tree_sitter_bash::LANGUAGE.into()),
        _                    => None,
    }
}
```

> **NOTE:** After reading the actual tree-sitter crate APIs for the version you've added (check their docs/source), adjust the `LANGUAGE` field access pattern if needed. The pattern above matches tree-sitter 0.22.x — verify it compiles.

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighter_creates_for_rust() {
        assert!(SyntaxHighlighter::new(Language::Rust).is_some());
    }

    #[test]
    fn highlighter_returns_none_for_plain_text() {
        assert!(SyntaxHighlighter::new(Language::PlainText).is_none());
    }

    #[test]
    fn parse_and_highlight_rust_line_returns_spans() {
        let mut h = SyntaxHighlighter::new(Language::Rust).unwrap();
        let src = "fn main() {}";
        h.parse(src);
        let spans = h.highlight_line(src, 0, src.len());
        assert!(!spans.is_empty());
    }

    #[test]
    fn keyword_recognized() {
        assert!(is_keyword("fn"));
        assert!(!is_keyword("blorp"));
    }

    #[test]
    fn highlight_spans_sorted_by_start() {
        let mut h = SyntaxHighlighter::new(Language::Rust).unwrap();
        let src = "let x = 42;";
        h.parse(src);
        let spans = h.highlight_line(src, 0, src.len());
        for w in spans.windows(2) {
            assert!(w[0].start_byte <= w[1].start_byte);
        }
    }
}
```

✅ **Checkpoint 4.4:** `cargo test editor::highlight` passes all 5 tests.

### Step 4.5 — Integrate highlighting into editor render

Read `src/editor/mod.rs` and `src/ui.rs` in full. Find the exact function that renders editor lines to ratatui `Line`/`Span` structs. Then:

1. Add `highlighter: Option<SyntaxHighlighter>` to the editor buffer/state struct
2. In the file-open path, detect language from extension and initialize `SyntaxHighlighter::new(lang)`
3. Call `highlighter.parse(&full_text)` whenever the buffer is modified (on every keystroke in insert mode)
4. In the line-render function, replace plain-text span generation with highlighted spans:
   - Compute `line_start_byte` and `line_end_byte` for each visible line
   - Call `highlight_line(...)` to get `Vec<HighlightSpan>`
   - Convert byte-indexed spans to character-indexed ratatui `Span` objects
   - Fall back to plain rendering if no highlighter is present

After the integration, run `cargo build`. Fix all errors. Do not break any existing editor tests.

✅ **Checkpoint 4.5:** `cargo build` succeeds. `cargo test` passes ≥ 80 total tests.

---

## Phase 5: IDE LSP Client

**Goal:** Implement a Language Server Protocol client that launches language servers in background processes and communicates via JSON-RPC over stdin/stdout. Surface hover documentation, go-to-definition, and inline diagnostics in the TUI.

**Minimum test count after this phase: 105**

### Step 5.1 — Add LSP dependencies

```toml
# Cargo.toml [dependencies]
lsp-types = "0.95"
serde_json = "1"
```

Run `cargo build` after adding. Fix any conflicts.

### Step 5.2 — Create `src/lsp/` module

```
src/lsp/
    mod.rs         ← re-exports, LspManager
    client.rs      ← LspClient: process management, JSON-RPC I/O
    protocol.rs    ← Request/Response/Notification types
    diagnostics.rs ← DiagnosticStore, InlineDiagnostic
    hover.rs       ← HoverResult, popup rendering
```

### Step 5.3 — `src/lsp/diagnostics.rs`

```rust
// src/lsp/diagnostics.rs

use lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StoredDiagnostic {
    pub line: u32,          // 0-indexed
    pub col_start: u32,
    pub col_end: u32,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
}

impl StoredDiagnostic {
    pub fn from_lsp(d: &Diagnostic) -> Self {
        let start = d.range.start;
        let end   = d.range.end;
        Self {
            line:      start.line,
            col_start: start.character,
            col_end:   end.character,
            severity:  d.severity.unwrap_or(DiagnosticSeverity::INFORMATION),
            message:   d.message.clone(),
            source:    d.source.clone(),
        }
    }

    pub fn indicator_char(&self) -> &'static str {
        match self.severity {
            DiagnosticSeverity::ERROR       => "✗",
            DiagnosticSeverity::WARNING     => "⚠",
            DiagnosticSeverity::INFORMATION => "ℹ",
            DiagnosticSeverity::HINT        => "·",
            _                               => "?",
        }
    }

    pub fn indicator_color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self.severity {
            DiagnosticSeverity::ERROR       => Color::Red,
            DiagnosticSeverity::WARNING     => Color::Yellow,
            DiagnosticSeverity::INFORMATION => Color::Blue,
            DiagnosticSeverity::HINT        => Color::DarkGray,
            _                               => Color::Gray,
        }
    }
}

/// Stores diagnostics per file URI.
#[derive(Debug, Default)]
pub struct DiagnosticStore {
    /// Map from file URI string → list of diagnostics
    data: HashMap<String, Vec<StoredDiagnostic>>,
}

impl DiagnosticStore {
    pub fn new() -> Self { Self::default() }

    pub fn update(&mut self, uri: String, diagnostics: Vec<StoredDiagnostic>) {
        if diagnostics.is_empty() {
            self.data.remove(&uri);
        } else {
            self.data.insert(uri, diagnostics);
        }
    }

    pub fn get(&self, uri: &str) -> &[StoredDiagnostic] {
        self.data.get(uri).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn for_line(&self, uri: &str, line: u32) -> Vec<&StoredDiagnostic> {
        self.get(uri).iter().filter(|d| d.line == line).collect()
    }

    pub fn error_count(&self, uri: &str) -> usize {
        self.get(uri).iter()
            .filter(|d| d.severity == DiagnosticSeverity::ERROR)
            .count()
    }

    pub fn warning_count(&self, uri: &str) -> usize {
        self.get(uri).iter()
            .filter(|d| d.severity == DiagnosticSeverity::WARNING)
            .count()
    }

    pub fn total_file_count(&self) -> usize { self.data.len() }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_diag(line: u32, sev: DiagnosticSeverity, msg: &str) -> StoredDiagnostic {
        StoredDiagnostic { line, col_start: 0, col_end: 5, severity: sev, message: msg.to_string(), source: None }
    }

    #[test]
    fn error_count_correct() {
        let mut s = DiagnosticStore::new();
        s.update("file:///a.rs".into(), vec![
            make_diag(0, DiagnosticSeverity::ERROR, "e1"),
            make_diag(1, DiagnosticSeverity::WARNING, "w1"),
        ]);
        assert_eq!(s.error_count("file:///a.rs"), 1);
    }

    #[test]
    fn for_line_filters_correctly() { /* ... */ }

    #[test]
    fn update_with_empty_removes_uri() { /* ... */ }

    #[test]
    fn get_nonexistent_uri_returns_empty() { /* ... */ }

    #[test]
    fn indicator_char_for_each_severity() { /* ... */ }

    #[test]
    fn total_file_count_tracks_insertions() { /* ... */ }
}
```

✅ **Checkpoint 5.3:** `cargo test lsp::diagnostics` passes all 6 tests.

### Step 5.4 — `src/lsp/client.rs` — LSP process management

```rust
// src/lsp/client.rs
// Manages a single language server child process and the JSON-RPC channel.
// The actual async I/O is delegated to a background tokio task spawned in `start()`.
// All public methods are synchronous and communicate via channels.

use std::process::Child;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::mpsc;
use lsp_types::{InitializeParams, ServerCapabilities};

pub type RequestId = i64;

#[derive(Debug)]
pub enum LspClientEvent {
    Initialized(ServerCapabilities),
    DiagnosticsPublished { uri: String, diagnostics: Vec<lsp_types::Diagnostic> },
    HoverResponse { id: RequestId, contents: Option<String> },
    DefinitionResponse { id: RequestId, locations: Vec<lsp_types::Location> },
    Error(String),
    ServerExited(Option<i32>),
}

#[derive(Debug)]
pub struct LspClient {
    pub language_id: String,
    pub server_command: Vec<String>,
    pub state: LspClientState,
    pub capabilities: Option<ServerCapabilities>,
    request_id: AtomicI64,
    /// Send requests/notifications to the background I/O task
    outbox: Option<mpsc::UnboundedSender<serde_json::Value>>,
    /// Receive events back from the background I/O task
    pub inbox: Option<mpsc::UnboundedReceiver<LspClientEvent>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LspClientState {
    Stopped,
    Starting,
    Ready,
    ShuttingDown,
}

impl LspClient {
    pub fn new(language_id: &str, server_command: Vec<String>) -> Self {
        Self {
            language_id: language_id.to_string(),
            server_command,
            state: LspClientState::Stopped,
            capabilities: None,
            request_id: AtomicI64::new(1),
            outbox: None,
            inbox: None,
        }
    }

    /// Returns the well-known server command for a language, if any.
    pub fn default_command_for(language_id: &str) -> Option<Vec<String>> {
        match language_id {
            "rust"       => Some(vec!["rust-analyzer".to_string()]),
            "python"     => Some(vec!["pylsp".to_string()]),
            "typescript" => Some(vec!["typescript-language-server".to_string(), "--stdio".to_string()]),
            "javascript" => Some(vec!["typescript-language-server".to_string(), "--stdio".to_string()]),
            _            => None,
        }
    }

    pub fn next_request_id(&self) -> RequestId {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn is_ready(&self) -> bool {
        self.state == LspClientState::Ready
    }

    // start(), send_hover_request(), send_definition_request(), etc. are async
    // and implemented as async fn — see below.
}
```

For the async communication loop, add an `impl LspClient` block with:

- `pub async fn start(&mut self, workspace_root: &std::path::Path) -> anyhow::Result<()>`
  — spawns the server process, creates channels, starts the read/write tasks
- `pub fn send_did_open(&self, uri: &str, text: &str)` — sends `textDocument/didOpen`
- `pub fn send_did_change(&self, uri: &str, version: i32, text: &str)` — sends `textDocument/didChange`
- `pub fn request_hover(&self, uri: &str, line: u32, col: u32) -> RequestId`
- `pub fn request_definition(&self, uri: &str, line: u32, col: u32) -> RequestId`

All notification senders are fire-and-forget (no return value). All request senders return `RequestId`; responses arrive asynchronously via `inbox`.

Unit tests (no live server required — test pure logic):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_rust_returns_rust_analyzer() {
        assert_eq!(
            LspClient::default_command_for("rust"),
            Some(vec!["rust-analyzer".to_string()])
        );
    }

    #[test]
    fn default_command_unknown_returns_none() {
        assert!(LspClient::default_command_for("cobol").is_none());
    }

    #[test]
    fn client_starts_in_stopped_state() {
        let c = LspClient::new("rust", vec![]);
        assert_eq!(c.state, LspClientState::Stopped);
    }

    #[test]
    fn request_ids_increment() {
        let c = LspClient::new("rust", vec![]);
        let id1 = c.next_request_id();
        let id2 = c.next_request_id();
        assert_eq!(id2, id1 + 1);
    }
}
```

### Step 5.5 — `src/lsp/hover.rs` — Hover popup state

```rust
// src/lsp/hover.rs

#[derive(Debug, Clone)]
pub struct HoverResult {
    pub contents: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Default)]
pub struct HoverPopup {
    pub result: Option<HoverResult>,
    pub visible: bool,
}

impl HoverPopup {
    pub fn show(&mut self, result: HoverResult) {
        self.result = Some(result);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn dismiss(&mut self) {
        self.result = None;
        self.visible = false;
    }

    pub fn contents(&self) -> Option<&str> {
        self.result.as_ref().map(|r| r.contents.as_str())
    }
}
```

Add 4 unit tests covering show/hide/dismiss/contents.

### Step 5.6 — Wire LSP into app state

Read `src/app.rs` in full, then:

1. Add `lsp_clients: HashMap<Language, LspClient>` to app state
2. Add `diagnostic_store: DiagnosticStore` to app state
3. Add `hover_popup: HoverPopup` to app state
4. On file open: call `LspClient::default_command_for(language_id)`, if Some → start a client and send `didOpen`
5. On buffer change (every keystroke): send `didChange` to the active client
6. Wire `K` in normal mode (or `Ctrl+Shift+K` in non-vim modes) → `request_hover`; display popup on response
7. Wire `Ctrl+]` (vim: `gd`) → `request_definition`; if single result, open the file and jump to line
8. In the editor line render, read `diagnostic_store.for_line(uri, line_idx)` and append indicator icons after the last visible character
9. In the status bar, show `E:{n} W:{n}` diagnostic counts for the open file

After each of these numbered sub-steps, run `cargo build` and fix errors.

✅ **Checkpoint 5.6:** `cargo build` succeeds. `cargo test` passes ≥ 105 total tests.

---

## Phase 6: IDE Git Integration Panel

**Goal:** Add a Git status panel that shows working tree state, a diff viewer, and allows staging/unstaging files and committing — all via `git` subprocess calls (no libgit2).

**Minimum test count after this phase: 130**

### Step 6.1 — Create `src/git/` module

```
src/git/
    mod.rs        ← re-exports, GitManager
    status.rs     ← GitFileStatus, StatusPanel
    diff.rs       ← DiffHunk, DiffLine, DiffViewer
    commit.rs     ← CommitPanel, commit form state
    ui.rs         ← ratatui widgets
```

### Step 6.2 — `src/git/status.rs`

```rust
// src/git/status.rs

#[derive(Debug, Clone, PartialEq)]
pub enum GitFileState {
    Modified,
    Added,
    Deleted,
    Renamed { from: String },
    Untracked,
    Ignored,
    Conflicted,
}

impl GitFileState {
    /// Parse from the two-character porcelain v1 status code (e.g. " M", "A ", "??")
    pub fn from_porcelain_xy(xy: &str) -> Option<Self> { /* ... */ }

    pub fn symbol(&self) -> &'static str {
        match self {
            GitFileState::Modified    => "M",
            GitFileState::Added       => "A",
            GitFileState::Deleted     => "D",
            GitFileState::Renamed{..} => "R",
            GitFileState::Untracked   => "?",
            GitFileState::Ignored     => "!",
            GitFileState::Conflicted  => "C",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            GitFileState::Modified    => Color::Yellow,
            GitFileState::Added       => Color::Green,
            GitFileState::Deleted     => Color::Red,
            GitFileState::Renamed{..} => Color::Cyan,
            GitFileState::Untracked   => Color::Gray,
            GitFileState::Ignored     => Color::DarkGray,
            GitFileState::Conflicted  => Color::Magenta,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitFileEntry {
    pub path: String,
    pub state: GitFileState,
    pub staged: bool,
}

#[derive(Debug)]
pub struct GitStatusPanel {
    pub entries: Vec<GitFileEntry>,
    pub selected_index: usize,
    pub repo_root: Option<std::path::PathBuf>,
    pub current_branch: String,
    pub is_loading: bool,
}

impl GitStatusPanel {
    pub fn new() -> Self { /* ... */ }

    /// Parse `git status --porcelain` output into file entries.
    pub fn parse_porcelain_output(output: &str) -> Vec<GitFileEntry> { /* ... */ }

    /// Parse `git rev-parse --abbrev-ref HEAD` output.
    pub fn parse_branch_name(output: &str) -> String {
        output.trim().to_string()
    }

    pub fn staged_entries(&self) -> Vec<&GitFileEntry> {
        self.entries.iter().filter(|e| e.staged).collect()
    }

    pub fn unstaged_entries(&self) -> Vec<&GitFileEntry> {
        self.entries.iter().filter(|e| !e.staged).collect()
    }

    pub fn selected_entry(&self) -> Option<&GitFileEntry> {
        self.entries.get(self.selected_index)
    }

    pub fn move_up(&mut self) { if self.selected_index > 0 { self.selected_index -= 1; } }

    pub fn move_down(&mut self) {
        let max = self.entries.len().saturating_sub(1);
        if self.selected_index < max { self.selected_index += 1; }
    }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PORCELAIN: &str = " M src/main.rs\nA  src/new.rs\n?? scratch.txt\n";

    #[test]
    fn parse_porcelain_modified() {
        let entries = GitStatusPanel::parse_porcelain_output(SAMPLE_PORCELAIN);
        assert!(entries.iter().any(|e| e.path == "src/main.rs" &&
            e.state == GitFileState::Modified));
    }

    #[test]
    fn parse_porcelain_added_staged() { /* ... */ }

    #[test]
    fn parse_porcelain_untracked() { /* ... */ }

    #[test]
    fn staged_entries_filters_correctly() { /* ... */ }

    #[test]
    fn parse_branch_name_trims_whitespace() {
        assert_eq!(GitStatusPanel::parse_branch_name("  main\n"), "main");
    }

    #[test]
    fn symbol_chars_non_empty() {
        let states = [GitFileState::Modified, GitFileState::Added, GitFileState::Deleted];
        for s in states { assert!(!s.symbol().is_empty()); }
    }
}
```

✅ **Checkpoint 6.2:** `cargo test git::status` passes all 6 tests.

### Step 6.3 — `src/git/diff.rs`

```rust
// src/git/diff.rs

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineKind { Context, Added, Removed, Header }

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

impl DiffLine {
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self.kind {
            DiffLineKind::Added   => Color::Green,
            DiffLineKind::Removed => Color::Red,
            DiffLineKind::Header  => Color::Cyan,
            DiffLineKind::Context => Color::Reset,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Parses unified diff output (from `git diff` or `git diff --cached`).
pub fn parse_unified_diff(diff_output: &str) -> Vec<DiffHunk> { /* ... */ }

#[derive(Debug)]
pub struct DiffViewer {
    pub hunks: Vec<DiffHunk>,
    pub file_path: Option<String>,
    pub scroll_offset: usize,
}

impl DiffViewer {
    pub fn new() -> Self { /* ... */ }

    pub fn load(&mut self, file_path: String, diff_output: &str) {
        self.file_path = Some(file_path);
        self.hunks = parse_unified_diff(diff_output);
        self.scroll_offset = 0;
    }

    pub fn total_lines(&self) -> usize {
        self.hunks.iter().map(|h| h.lines.len() + 1).sum()
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = (self.scroll_offset + n).min(self.total_lines().saturating_sub(1));
    }
}
```

Unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DIFF: &str =
        "diff --git a/src/main.rs b/src/main.rs\n\
         index abc..def 100644\n\
         --- a/src/main.rs\n\
         +++ b/src/main.rs\n\
         @@ -1,3 +1,4 @@\n\
          fn main() {\n\
         +    println!(\"hello\");\n\
          }\n";

    #[test]
    fn parse_diff_produces_one_hunk() {
        let hunks = parse_unified_diff(SAMPLE_DIFF);
        assert_eq!(hunks.len(), 1);
    }

    #[test]
    fn parse_diff_hunk_contains_added_line() {
        let hunks = parse_unified_diff(SAMPLE_DIFF);
        let added = hunks[0].lines.iter().any(|l| l.kind == DiffLineKind::Added);
        assert!(added);
    }

    #[test]
    fn diff_viewer_scroll_clamps() { /* ... */ }

    #[test]
    fn added_line_color_is_green() { /* ... */ }
}
```

### Step 6.4 — `src/git/commit.rs`

```rust
// src/git/commit.rs

#[derive(Debug)]
pub struct CommitPanel {
    pub message: String,
    pub cursor_pos: usize,
    pub is_active: bool,
    pub author_override: Option<String>,
}

impl CommitPanel {
    pub fn new() -> Self { /* ... */ }

    pub fn insert_char(&mut self, c: char) {
        self.message.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_before_cursor(&mut self) {
        if self.cursor_pos > 0 {
            let c = self.message[..self.cursor_pos].chars().last().unwrap();
            self.cursor_pos -= c.len_utf8();
            self.message.remove(self.cursor_pos);
        }
    }

    pub fn is_ready(&self) -> bool {
        !self.message.trim().is_empty()
    }

    /// Returns the git commit command args.
    pub fn build_commit_args(&self) -> Vec<String> {
        let mut args = vec!["commit".to_string(), "-m".to_string(), self.message.clone()];
        if let Some(author) = &self.author_override {
            args.push("--author".to_string());
            args.push(author.clone());
        }
        args
    }
}
```

Add 5 unit tests: insert_char, delete_before_cursor, is_ready_false_for_empty, is_ready_true_for_nonempty, build_commit_args_contains_message.

### Step 6.5 — Wire Git panel into app state

1. Add `git_status: GitStatusPanel` to app state
2. Add `diff_viewer: DiffViewer` to app state
3. Add `commit_panel: CommitPanel` to app state
4. Add `show_git_panel: bool` toggle
5. Wire `Ctrl+Shift+G` → toggle Git panel
6. On Git panel open: shell out `git status --porcelain` and `git rev-parse --abbrev-ref HEAD`, parse results
7. Wire `d` on selected file → shell out `git diff <path>`, load into DiffViewer
8. Wire `s` on selected file → shell out `git add <path>`, refresh status
9. Wire `u` on selected file → shell out `git restore --staged <path>`, refresh status
10. Wire `c` → activate CommitPanel; `Enter` when ready → shell out `git commit -m "..."`, refresh status

Git shell-out pattern: use `std::process::Command::new("git").args([...]).current_dir(&repo_root).output()`. Do not use libgit2.

✅ **Checkpoint 6.5:** `cargo build` succeeds. `cargo test` passes ≥ 130 total tests.

---

## Phase 7: IDE Advanced Features — Find-in-Files, Project Tree, Status Bar

**Goal:** Add a find-in-files panel (grep across the project), upgrade the project file tree with git decorations, and improve the status bar with richer info.

**Minimum test count after this phase: 150**

### Step 7.1 — `src/search/mod.rs` — Find-in-Files

```
src/search/
    mod.rs          ← re-exports
    grep.rs         ← GrepQuery, GrepResult, file search logic
    ui.rs           ← FindInFilesPanel widget
```

`GrepResult` fields: `file_path: String`, `line_number: u32`, `line_content: String`, `match_start: usize`, `match_end: usize`.

`GrepQuery` executes via `rg` (ripgrep) if available, falling back to a pure-Rust recursive `std::fs::read_dir` + string search. The rg path constructs `std::process::Command::new("rg")` with `--json` output. The fallback is a synchronous recursive search.

```rust
impl GrepQuery {
    pub fn new(pattern: String, case_sensitive: bool, regex: bool) -> Self { /* ... */ }
    pub fn execute(&self, root: &std::path::Path) -> anyhow::Result<Vec<GrepResult>> { /* ... */ }
}
```

Unit tests (6): empty pattern returns ok, whitespace-only pattern treated as empty, fallback search finds string in tempdir file, result fields populated correctly, case insensitive match, results sorted by file path.

### Step 7.2 — Upgrade project file tree

Read the existing file browser code (likely opened via `Ctrl+O`). Extend the file tree entries with:

```rust
pub struct FileTreeEntry {
    pub name: String,
    pub path: std::path::PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub depth: usize,
    pub git_state: Option<GitFileState>,  // NEW: decoration from git status
}
```

When the Git panel is active and `git_status` contains entries, decorate matching file tree paths with their `GitFileState`. Add a `git_decoration_char(&self) -> Option<&str>` method that returns the symbol only when `git_state` is Some.

Unit tests (4): git_decoration_char returns None when no state, returns symbol when state set, modified state returns "M", untracked returns "?".

### Step 7.3 — Status bar upgrade

Read the current status bar render code. Extend it to show the following segments, left-to-right:

```
[mode] [filename][modified?] | [language] | [line:col] | E:{n} W:{n} | [branch] | [server-status]
```

Where:
- `[modified?]` = `●` if buffer has unsaved changes, empty otherwise
- `E:{n} W:{n}` = diagnostic counts from `DiagnosticStore` (omit if 0/0)
- `[branch]` = current git branch from `GitStatusPanel.current_branch` (omit if not in a git repo)
- `[server-status]` = `LSP:ready` / `LSP:starting` / `LSP:off` based on `LspClientState`

Implement as a pure function `render_status_bar(f, area, state: &AppState)`. Use ratatui `Spans` with distinct styles per segment. Segments separated by `│` with DarkGray style.

Unit tests using `TestBackend` (3): renders without panic when all data is default, renders without panic when lsp is ready, renders without panic with diagnostics.

### Step 7.4 — Wire Find-in-Files into app

1. Wire `Ctrl+Shift+F` → open FindInFilesPanel
2. The panel has a text input for the query; `Enter` executes the search in a background `tokio::spawn`
3. Results populate a scrollable list
4. `Enter` on a result → open the file and jump to the line
5. `Escape` → close panel

After wiring, run `cargo build` and fix errors.

✅ **Checkpoint 7.4:** `cargo build` succeeds. `cargo test` passes ≥ 150 total tests.

---

## Phase 8: Keybinding Help Overlay & Polish

**Goal:** Add a `?` help overlay that dynamically generates the keybinding table, clean up all clippy warnings, and finalize documentation.

**Minimum test count after this phase: 160**

### Step 8.1 — Help overlay

Create `src/help.rs`:

```rust
// src/help.rs

#[derive(Debug, Clone)]
pub struct KeybindingEntry {
    pub category: String,
    pub key: String,
    pub description: String,
}

/// Returns the complete keybinding list for the given editor mode.
pub fn all_keybindings(mode: &crate::config::EditorMode) -> Vec<KeybindingEntry> {
    let mut entries = Vec::new();

    // Global entries (always present)
    entries.push(KeybindingEntry { category: "Global".into(), key: "Ctrl+Q".into(),         description: "Quit".into() });
    entries.push(KeybindingEntry { category: "Global".into(), key: "Ctrl+Shift+P".into(),   description: "Command palette".into() });
    entries.push(KeybindingEntry { category: "Global".into(), key: "Ctrl+Shift+H".into(),   description: "Toggle SSH panel".into() });
    entries.push(KeybindingEntry { category: "Global".into(), key: "Ctrl+Shift+D".into(),   description: "Toggle Docker panel".into() });
    entries.push(KeybindingEntry { category: "Global".into(), key: "Ctrl+Shift+G".into(),   description: "Toggle Git panel".into() });
    entries.push(KeybindingEntry { category: "Global".into(), key: "Ctrl+Shift+F".into(),   description: "Find in files".into() });
    entries.push(KeybindingEntry { category: "Global".into(), key: "?".into(),              description: "Toggle this help".into() });

    // Terminal entries
    entries.push(KeybindingEntry { category: "Terminal".into(), key: "Ctrl+T".into(),        description: "New terminal tab".into() });
    entries.push(KeybindingEntry { category: "Terminal".into(), key: "Ctrl+W".into(),        description: "Close terminal tab".into() });
    entries.push(KeybindingEntry { category: "Terminal".into(), key: "Ctrl+S".into(),        description: "Split horizontal".into() });

    // Mode-specific editor entries
    // ... (add all entries from the keybinding tables in README.md, translated to KeybindingEntry)

    entries
}

#[derive(Debug)]
pub struct HelpOverlay {
    pub visible: bool,
    pub scroll_offset: usize,
    pub search_filter: String,
}

impl HelpOverlay {
    pub fn new() -> Self { Self { visible: false, scroll_offset: 0, search_filter: String::new() } }
    pub fn toggle(&mut self) { self.visible = !self.visible; }
    pub fn scroll_up(&mut self) { self.scroll_offset = self.scroll_offset.saturating_sub(1); }
    pub fn scroll_down(&mut self, max: usize) { if self.scroll_offset + 1 < max { self.scroll_offset += 1; } }
    pub fn filtered_entries<'a>(&self, entries: &'a [KeybindingEntry]) -> Vec<&'a KeybindingEntry> {
        if self.search_filter.is_empty() { return entries.iter().collect(); }
        entries.iter()
            .filter(|e| e.key.to_lowercase().contains(&self.search_filter) ||
                        e.description.to_lowercase().contains(&self.search_filter))
            .collect()
    }
}
```

Unit tests (4): all_keybindings returns non-empty list, filtered_entries respects filter, toggle toggles visibility, scroll_down clamps.

### Step 8.2 — Wire help overlay

1. Add `help_overlay: HelpOverlay` to app state
2. Wire `?` key → `help_overlay.toggle()`
3. In render: when `help_overlay.visible`, render a centered popup over the current view using ratatui's `Clear` widget + a bordered `Paragraph` with the filtered keybinding table
4. `Escape` → close overlay

### Step 8.3 — Clippy clean pass

```bash
cargo clippy -- -D warnings 2>&1
```

Fix every warning. Common items to address:
- Replace `let _ = x;` with proper variable binding or `drop(x)`
- Add `#[must_use]` to pure functions returning values
- Replace `.clone()` on Copy types
- Use `if let` instead of `match` with single arm
- Remove unused imports

Re-run until clean:

```bash
cargo clippy -- -D warnings && echo "CLIPPY CLEAN"
```

✅ **Checkpoint 8.3:** `cargo clippy -- -D warnings` exits 0.

### Step 8.4 — Final test sweep

```bash
cargo test 2>&1
```

Verify:
- Total test count ≥ 160
- Zero test failures
- Zero ignored tests (unless explicitly annotated with `#[ignore]` and a comment explaining why)

Fill in any placeholder test bodies from Phase 1 that still have empty `{}` bodies.

✅ **Checkpoint 8.4:** `cargo test` exits 0 with ≥ 160 passing tests.

### Step 8.5 — Update README

Open `README.md` and add/update the following sections:

**New keybindings table entries** (add to the existing tables):

| Key | Action |
|-----|--------|
| `Ctrl+Shift+H` | Toggle SSH panel |
| `Ctrl+Shift+D` | Toggle Docker panel |
| `Ctrl+Shift+G` | Toggle Git panel |
| `Ctrl+Shift+F` | Find in files |
| `Ctrl+Shift+K` | Show hover documentation (LSP) |
| `Ctrl+]` | Go to definition (LSP) |
| `K` | Hover documentation (Vim normal mode) |
| `gd` | Go to definition (Vim normal mode) |
| `?` | Toggle keybinding help overlay |

**New `~/.ratrc` options**:

```toml
# Enable/disable LSP integration
lsp_enabled = true

# Override language server commands
[lsp_servers]
rust = "rust-analyzer"
python = "pylsp"
typescript = "typescript-language-server --stdio"

# SSH profiles location override (default: ~/.ratterm/ssh_profiles.toml)
ssh_profiles_path = "~/.ratterm/ssh_profiles.toml"
```

**New SSH profiles file format** (`~/.ratterm/ssh_profiles.toml`):

```toml
[[profiles]]
name = "prod-web"
host = "192.168.1.100"
port = 22
username = "deploy"
auth = "Agent"

[[profiles]]
name = "dev-server"
host = "dev.example.com"
port = 22
username = "ian"

[profiles.auth]
type = "Key"
path = "~/.ssh/id_ed25519"
passphrase_in_keyring = false
```

✅ **FINAL CHECKPOINT:** `cargo build --release` exits 0. `cargo test` exits 0. `cargo clippy -- -D warnings` exits 0.

---

## Dependency Summary

Add these to `Cargo.toml` as required by each phase. Verify version compatibility with `cargo build` after each addition:

```toml
[dependencies]
# Phase 2 — SSH
ssh2 = "0.9"                    # (may already be present — check first)

# Phase 3 — Docker  
bollard = "0.17"                # (may already be present — check first)
tokio = { version = "1", features = ["full"] }

# Phase 4 — Syntax highlighting
tree-sitter = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-toml = "0.21"
tree-sitter-json = "0.21"
tree-sitter-bash = "0.21"

# Phase 5 — LSP
lsp-types = "0.95"
serde_json = "1"                # (likely already present)

[dev-dependencies]
tempfile = "3"
```

**Before adding any dependency:** check `Cargo.toml` to see if it already exists at a different version. If it does, use the existing version specifier, or update it if the existing version is incompatible with what is required.

---

## Self-Correction Protocol

If `cargo build` fails at any step:
1. Read the full error output
2. Identify which file and line caused the error
3. Re-read that file with `cat`
4. Fix the error
5. Run `cargo build` again
6. Do not proceed to the next step until the build is clean

If `cargo test` fails:
1. Read the full test failure output
2. Identify the failing test and the assertion that failed
3. Re-read the relevant source file
4. Fix the logic error (do not delete the failing test)
5. Run `cargo test` again

If a dependency version causes a conflict:
1. Run `cargo tree` to understand the dependency graph
2. Adjust version specifiers to resolve the conflict
3. Prefer newer stable versions

---

## Phase Gate Summary

| Phase | Description | Min Tests |
|-------|-------------|-----------|
| 1 | Test scaffolding & baseline | 15 |
| 2 | SSH management | 35 |
| 3 | Docker management | 60 |
| 4 | Syntax highlighting | 80 |
| 5 | LSP client | 105 |
| 6 | Git integration | 130 |
| 7 | Find-in-files, tree, status bar | 150 |
| 8 | Help overlay & polish | 160 |

No phase may begin until the previous phase's checkpoint is confirmed with `cargo test` passing the minimum count.
