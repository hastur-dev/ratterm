# Ratterm Improvement Recommendations

Survey date: 2026-09-04. Branch `fix/release-verify-version` at commit `5f90fc3`.
Scope: multi-machine management, editor quality, Docker and Kubernetes management,
resource logging, consistency, and AI-driven verification on Windows and Linux.

Every finding below cites the file that demonstrates it. Nothing in this document
has been changed in the codebase.

---

## 1. Current state at a glance

| Area | What exists | Main gap |
|---|---|---|
| SSH hosts | `src/ssh/` (host list, scanner, status checker, TOML storage) | Every remote action spawns a fresh `ssh`/`plink`/`sshpass` process; no persistent session |
| Health metrics | `src/ssh/collector.rs` (SSH polling) and `src/daemon/` (push daemon over reverse tunnel) | Two competing collectors; only the latest sample is kept, nothing is persisted |
| Docker | `src/docker/` (discovery, container ops, create form) and `src/docker_logs/` (bollard streaming, jsonl.gz storage) | Remote hosts use shelled `ssh docker ...` strings; bollard is local-only; one host visible at a time |
| Kubernetes | none | Zero references to kube, kubectl, or helm anywhere in `src/` or `docs/` |
| Editor | `src/editor/` (~2.2k lines), `src/lsp/`, `src/completion/` | Single shared buffer for all tabs; no syntax highlighting; minimal Vim |
| AI control | `src/api/` IPC over named pipe / Unix socket, 45 methods | Can drive the PTY and editor but cannot see or drive the TUI itself |
| Tests | 20 integration files in `tests/`, expectrl harness | expectrl harness is Windows-only (`tests/helpers/mod.rs` gates it on `cfg(windows)`) |

---

## 2. Multi-machine management

### 2.1 Replace process-per-command SSH with one persistent session per host

Today each Docker or metrics call builds a shell string and spawns a new client:

- `src/docker/discovery.rs:634` `build_remote_docker_command` produces
  `sshpass -p '<password>' ssh -o StrictHostKeyChecking=no ...` (line 715) or a `plink -pw` command.
- `src/docker/api.rs:363` runs that string through `cmd /C` or `sh -c`.
- `src/ssh/collector.rs:352` spawns `ssh` per host per refresh; the Windows path prefers WSL `sshpass`, then `plink`.
- `docs/fixes.md` documents that spawning `plink.exe` corrupted console keyboard input; the fix was process-creation flags, not removing the spawn.

Consequences: password visible in the process list on Linux, host-key checking disabled, a TCP + auth handshake for every action, and three external tools (`ssh`, `sshpass`, `plink`) whose presence is probed at runtime.

Recommendation:

- Introduce a `RemoteSession` type owning one authenticated `ssh2::Session` per host
  (the `ssh2` crate is already a dependency and already used in `src/remote/sftp.rs`).
  Expose `exec(cmd) -> Output`, `sftp()`, and `forward_local_port()` on it.
- Keep sessions in a `SessionPool` keyed by SSH host id, with idle timeout and reconnect.
- Route Docker discovery, container ops, metrics collection, and SFTP through the pool.
  Delete `build_sshpass_command`, `build_plink_command`, and the WSL fallback.
- Keep the `ProxyJump` chain logic from `src/ssh/host.rs:600` but implement it as
  nested `ssh2` channels (direct-tcpip) instead of command-line flags.
- Honour known_hosts. If a host key is unknown, prompt in the TUI once, then pin it.

### 2.2 One host model, one source of truth for "is it up"

`DockerHost::Remote` in `src/docker/container.rs:20` carries `host_id` and also
copies `hostname`, `port`, `username`. Editing a host in the SSH manager leaves stale
copies in `docker_items.toml`. Host reachability is tracked in three places:
`App::host_statuses`, `HealthDashboard::hosts[].metrics.status`, and
`StatusChecker`. They can disagree on screen.

Recommendation:

- `DockerHost::Remote { host_id }` only. Resolve hostname and credentials through the
  SSH host list at call time.
- A single `HostRegistry` (hosts, credentials, last-seen status, capabilities such as
  "has docker", "has kubectl", "has nvidia-smi") owned by `App`. Dashboards borrow from it.
- Capability detection runs once per session and is cached in the registry, so the
  Docker manager can show a fleet view ("6 hosts, 4 with Docker, 23 containers") instead
  of one host at a time.

### 2.3 Credential storage is not safe

`src/ssh/storage.rs`:

- `StorageMode` defaults to `Plaintext` (line 27).
- "Encrypted" mode is XOR with a key from a home-grown mixing loop (`derive_key`, line 262,
  comment says "replace with ring::pbkdf2 in production"; `xor_encrypt`, line 446;
  `encrypt_password`, line 553, comment says "Placeholder").

Recommendation:

- Use the OS keychain via the `keyring` crate for passwords and passphrases
  (Windows Credential Manager, macOS Keychain, Secret Service on Linux).
- For the file-based fallback, use `argon2` for key derivation and `aes-gcm` or
  `chacha20poly1305` for encryption. Store salt and nonce alongside the ciphertext.
- Prefer key-based auth and `ssh-agent` / Pageant forwarding. Passwords should be the
  exception, not the default path.
- Verify crate versions on crates.io before pinning; do not copy versions from memory.

---

## 3. Long-term tracking and resource logging

### 3.1 Metrics are latest-only and in memory

- `src/daemon/receiver.rs:45` caches `HashMap<String, DaemonMetrics>`; each POST overwrites the previous sample.
- `src/ssh/collector.rs:92` holds `HashMap<u32, DeviceMetrics>`, same pattern.
- `src/ui/health_dashboard/mod.rs` renders only the current sample. No history, no sparkline, no "since when".
- No `history`, `VecDeque`, or persistence code exists under `src/daemon`, `src/ssh`, or `src/ui/health_dashboard`.

Docker logs are the one place with durable storage (`src/docker_logs/log_storage.rs`,
`~/.ratterm/docker_logs/{container_id}/{date}.jsonl.gz`, retention in hours).

Recommendation:

- Add a single embedded database at `~/.ratterm/ratterm.db` (SQLite via `rusqlite`
  with the `bundled` feature, or `sled` if you want pure Rust). Tables:
  `hosts`, `metric_samples(host_id, ts, cpu_load1, mem_used, disk_used, gpu_util, ...)`,
  `container_events(host_id, container_id, ts, event)`, `k8s_events(...)`,
  `sessions(started, ended, hosts_touched)`.
- Write every received sample. Downsample in the background: raw for 24 h,
  1-minute averages for 30 days, 1-hour averages beyond. Configurable in `.ratrc`.
- Health dashboard detail view: ratatui `Sparkline` for the last hour, plus
  "min / max / avg over 24 h" and "offline since" derived from the DB.
- Index Docker log files in the same DB (container, date, line count, byte size) so
  search across days does not need to decompress every file.
- Alerts: threshold rules in `.ratrc` (`alert.cpu = 90`, `alert.disk = 85`) evaluated
  on ingest, surfaced in the status bar and recorded in the DB.

### 3.2 Merge the two collectors

The SSH polling collector and the push daemon produce the same `DeviceMetrics` by
different transports and are toggled independently. Pick the daemon as the primary
path (cheaper, one-second resolution) and keep SSH polling only as the fallback when
the daemon cannot be deployed. Both should write to the same ingest function so the
dashboard and the DB see one stream.

The daemon script (`src/daemon/script.rs`) is Bash and relies on `curl` or `wget`.
Consider shipping a static musl build of a tiny Rust agent instead; it removes the
`/proc` parsing from shell, supports Windows hosts, and can report Docker and
Kubernetes state on the same channel.

---

## 4. Docker management across machines

Current architecture, per `src/docker/`:

- Local: `docker` CLI via `Command` for discovery and container ops;
  `bollard` (`Docker::connect_with_local_defaults`, `src/docker_logs/client.rs:23`) for log streaming.
- Remote: shelled `ssh <host> docker ps --format ...` and string parsing (`discovery.rs:573`, `608`).
- Only `DockerHostManager::current_host` is active; no cross-host list.

Recommendation:

- Use `bollard` everywhere. For remote hosts, open a local port forward through the
  `RemoteSession` (section 2.1) to the remote daemon socket
  (`ssh -L` semantics via `ssh2` direct-tcpip channel to `/var/run/docker.sock`,
  or run `docker system dial-stdio` over the exec channel) and give bollard that endpoint.
  This removes all output parsing and gives typed responses, events, and stats streams.
- Subscribe to the Docker events stream per host and write to `container_events`.
  That is what makes "what happened to this container overnight" answerable.
- Fleet view in the Docker manager: hosts as collapsible groups, containers under
  each, aggregate counts in the header. Quick-connect slots already key on
  `"remote:{host_id}"` (`container.rs:847`), so the data model is close.
- Compose support: detect `docker-compose.yml` / `compose.yaml` in the editor's project
  root and offer `up`, `down`, `logs` for the whole stack.
- Windows caveat: keep the existing Docker Desktop start logic (`discovery.rs:844`),
  but detect named-pipe vs. socket automatically.

---

## 5. Kubernetes management (new)

Nothing exists today. Recommended shape, mirroring the Docker module so the UI
patterns (`ListSelectable`, dashboard navigation, key hint bar) carry over:

```
src/k8s/
  mod.rs        # public API: contexts, namespaces, resources, actions
  client.rs     # kube-rs Client per kubeconfig context; optional SSH port-forward to remote API server
  resources.rs  # typed views: Pod, Deployment, Service, Node, Event
  actions.rs    # scale, restart (rollout), delete pod, exec, port-forward
  logs.rs       # pod log streaming into the existing docker_logs buffer/storage types
  storage.rs    # ~/.ratterm/k8s.toml: pinned contexts, favourite namespaces
src/ui/k8s_manager/   # selector + widget, same layout as docker_manager
src/app/input_k8s.rs  # keys: n namespace, c context, l logs, e exec, s scale, r rollout restart
```

Design notes:

- Use `kube` + `k8s-openapi` (check current versions and the matching Kubernetes
  feature flag on crates.io first). They read `~/.kube/config` and handle auth
  plugins, so no `kubectl` shelling.
- For clusters only reachable from a fleet host (k3s on a Pi, for example), reuse the
  `RemoteSession` port forward from section 2.1 to reach the API server, and read the
  remote kubeconfig over SFTP once.
- Pod logs should flow into the same `LogBuffer` / `LogStorage` used by Docker logs
  (`src/docker_logs/log_buffer.rs`, `log_storage.rs`) so search, saved patterns, and
  retention are shared. Rename that module to `logs/` once both feed it.
- `kubectl exec` and `docker exec` should both open a new terminal tab through the
  multiplexer, the way Docker exec does now.
- Watch API for Pods and Events, written to `k8s_events` in the DB (section 3.1).

---

## 6. Editor

### 6.1 Correctness bugs

1. **Switching tabs discards unsaved work and undo history.**
   `src/app/file_ops.rs:351` `next_file` and `prev_file` call `self.editor.open(&file.path)`,
   and `Editor::open` (`src/editor/mod.rs:138`) replaces `self.buffer` with a fresh read from
   disk. `OpenFile` (`src/app/mod.rs`) holds only `path` and `name`. There is one `Editor`
   for every tab. Any edit not yet saved is lost silently on `Alt+Shift+Left/Right`,
   and the error from `open` is discarded with `let _ =`.
   Fix: `OpenFile` should own its `Buffer`, `Cursor`, `View`, and undo stack (or own a
   whole `Editor`), and `App` should switch which one is active.
2. **README claims syntax highlighting via tree-sitter** (`README.md:87`). The
   `tree-sitter*` crates are in `Cargo.toml` but nothing in `src/` references them.
   `src/ui/editor_widget.rs` renders one foreground colour. Either implement it or
   remove the claim and the dependencies (they cost build time on every CI run).
3. **Two LSP implementations.** `src/lsp/` and `src/completion/lsp/` both contain
   `client.rs`, `config.rs`, `manager.rs`. `config.rs` is byte-identical; `client.rs`
   differs by about 1,700 diff lines. `src/completion/mod.rs:39` re-exports from
   `crate::lsp`, so the copy under `completion/lsp/` looks like the stale one.
   Delete it after confirming `detect_language` is the only thing still used from there.

### 6.2 Missing editing features

Measured against what a text editor is expected to do:

| Feature | Status | Where it would go |
|---|---|---|
| Syntax highlighting | absent | `src/editor/highlight.rs` with tree-sitter queries; cache per line, invalidate on edit |
| Auto-indent on Enter | absent (`Enter` inserts `\n` only, `input_editor.rs`) | `Editor::insert_newline_with_indent` |
| Bracket matching / auto-pair | absent | `src/editor/brackets.rs` |
| Multiple cursors | absent | `Cursor` becomes `Vec<Cursor>`; edits fan out |
| Search and replace UI | `Buffer::replace_all` exists, no UI | popup + `:%s/a/b/g` in Vim command mode |
| Vim: counts, operators with motions (`d2w`, `ciw`), text objects, registers, dot-repeat, marks, `:s` | absent; `handle_editor_normal_key` covers roughly 30 single keys | a small operator-pending state machine in `src/editor/vim/` |
| Emacs: kill ring, `C-space` mark, `M-x` | absent; `Tab` inserts four literal spaces | `src/editor/emacs.rs` |
| Code folding | absent | tree-sitter fold queries once highlighting exists |
| Per-tab dirty indicator | absent (see 6.1) | editor tab bar |
| Large-file guard | absent (`read_to_string` unbounded) | size check, read-only mode above a threshold |
| Format on save | present (`lsp_format_on_save`) | keep |
| Diagnostics, hover, references, rename, symbols | present in `src/lsp/` | keep |

### 6.3 Structure

`App` in `src/app/mod.rs` has about 90 fields and 543 methods across 31 `impl App`
blocks in 37 files. Twenty-five of those fields are LSP UI state (`lsp_hover`,
`lsp_references_selected`, ...). This is where the "consistency" complaints come from:
every dashboard reimplements selection, scrolling, and refresh on `App` directly.

Recommendation:

- Extract `LspUiState`, `GitUiState`, `DebugUiState`, `DashboardState<T>` structs.
- Make each manager (SSH, Docker, K8s, Health, Git) implement one `Panel` trait:
  `handle_key`, `tick`, `render`, `hints`. `App` holds a `Vec<Box<dyn Panel>>` and a
  focus index. The duplicate navigation code called out in
  `docs/duplicate_code_report.md` disappears with it.
- Editor logic lives entirely in `src/editor/`; `src/app/input_editor.rs` should map
  keys to `EditorCommand` values and nothing else. That is also what makes the Vim
  state machine testable without `App`.

---

## 7. Consistency and hygiene

- Config is spread across five formats: `~/.ratrc` (hand-parsed INI),
  `ssh_hosts.toml`, `docker_items.toml`, `breakpoints.json`, `saved_searches.json`,
  `launch.json`. Consolidate user settings into one schema-validated TOML and move
  runtime state into the DB from section 3.1.
- `.claude/CLAUDE.md` states MSRV 1.75 and edition 2021; `Cargo.toml` says
  `rust-version = "1.85"`, `edition = "2024"`. `docs/architecture.md` lists modules
  that do not exist (`ssh/hosts.rs`, `docker/remote.rs`, `extensions/`) and omits
  `git`, `debugger`, `daemon`, `theme`, `docker_logs`.
- Five empty directories at the repo root: `srceditor`, `srclsp`, `srcterminal`,
  `srcui`, `srcutils`. Remove them.
- `PLAN.md` and `ratterm_upgrade_phases.md` describe work that has since landed
  (theme system, git, debugger). Archive or delete.
- `test_api.py`, `test_background_api.py`, `test_output.txt` at the root belong under
  `scripts/` or `tests/`, and the `.txt` should not be committed.
- The IPC API (`src/api/`) has no authentication. Any local process can open
  `\\.\pipe\ratterm-api` or `/tmp/ratterm-api.sock` and read the editor or send keys
  to the PTY. Add a per-session token written to `~/.ratterm/api.token` with `0600`
  permissions and require it on the first message.

---

## 8. AI-driven verification on Windows and Linux

Goal: an agent (Claude Code or another) launches Ratterm, performs a scenario, and
verifies what is on screen, on this Windows box and on a native Linux machine.

### 8.1 What already exists

- **IPC API** (`src/api/`): named pipe on Windows, Unix socket elsewhere. 45 methods
  covering `terminal.send_keys`, `terminal.read_buffer`, `editor.open_file`,
  `editor.read_content`, `editor.write_content`, `docker.*`, `background.*`,
  `layout.*`, `tabs.*`, `theme.*`, `system.*`. Missing: any way to observe the rendered
  TUI (popups, dashboards, status bar), any way to send a key to the App rather than
  the PTY, and all of SSH, health, Git, LSP, and debugger.
- **`--test-keys`** (`src/main.rs:185`): F1/F2/F3 open palette, SSH manager, Docker manager.
- **`--test`** (`src/main.rs:128`): a fixed scripted run that logs redraw behaviour.
- **expectrl harness** (`tests/helpers/tui_harness.rs`): spawn, `expect_text`,
  `read_screen`, key senders. Compiled only on Windows.
- **Docker CI images** (`docker/Dockerfile.test*`): run `cargo test`, not the TUI.
- **`test_api.py`**: Windows-only pipe client using `pywin32`.

### 8.2 Recommended additions to the application

1. **`app.snapshot`** API method. Render the current frame into a
   `ratatui::backend::TestBackend` buffer and return it as JSON:
   `{ "width", "height", "lines": [...], "cursor": [x, y], "cells": [{x, y, ch, fg, bg, mods}] }`.
   Plain text lines are enough for most assertions; the styled cell list lets an
   agent verify colours and focus. This is the single most valuable addition; it
   turns the whole UI into something an agent can read.
2. **`app.send_key`** API method taking `{ "code": "F2", "modifiers": ["ctrl"] }` and
   feeding a `crossterm::event::KeyEvent` into `App::handle_key`. Together with
   `app.snapshot`, every popup and dashboard becomes drivable. Add `app.send_mouse` for
   the terminal selection features.
3. **`--headless WxH`** flag. Run the event loop against `TestBackend` with no real
   terminal, so the app can run under `systemd`, in a Docker container, or over SSH
   without a PTY. Combine with `--api-socket <path>` to pick the IPC endpoint per
   instance so several agents can run in parallel.
4. **`--fixtures <dir>`** flag. Load SSH hosts, Docker items, and metrics from fixture
   files and route remote transport through a mock `RemoteSession`, so agent runs
   never touch real machines unless explicitly asked.
5. **Scenario files.** A YAML or JSON list of steps (`key`, `type`, `api`,
   `expect_text`, `expect_not_text`, `snapshot name`) executed by `rat --scenario f.yaml`,
   exit code reflects pass/fail, snapshots written to `test-results/`. The same file
   runs unchanged on Windows and Linux, which is the cross-platform consistency check
   this project needs.
6. **Auth token** for the API (section 7) so the headless instance is not an open door.

### 8.3 Recommended agent-side tooling

- **MCP server `ratterm-mcp`** (Python with `mcp`, or Rust) exposing tools:
  `launch(platform, args)`, `snapshot()`, `send_key(key, mods)`, `type(text)`,
  `api(method, params)`, `run_scenario(path)`, `stop()`. Claude Code connects to it and
  can then "look" at the screen and act. On Windows it speaks the named pipe; on
  Linux the Unix socket. This is the direct answer to "have an AI use the application".
- **Fallback without app changes**: run Ratterm inside `tmux` on Linux and use
  `tmux send-keys` plus `tmux capture-pane -p` from the MCP server. It works today but
  loses colour and cursor information and does not exist on Windows.

### 8.4 Running on both platforms

**Windows (this machine)**
- Build once, launch `rat --headless 120x40 --api-socket \\.\pipe\ratterm-ai-1 --fixtures tests\fixtures\fleet`.
- The MCP server connects over the pipe. For real-terminal tests keep using the
  existing expectrl + ConPTY harness.

**Linux (native, secondary machine)**
- Per the fleet notes, `cthulhu-computer` (10.0.0.217, 16 cores, Docker installed) is
  the reachable Linux node; `rock-5c` (10.0.0.20, aarch64) covers ARM.
- Sync the repo with `git` or `rsync`, `cargo build --release` there, run
  `rat --headless 120x40 --api-socket /tmp/ratterm-ai-1.sock --fixtures ...` under
  `nohup` or `tmux`.
- Reaching a remote Unix socket from a Windows OpenSSH client with
  `ssh -L /local.sock:/remote.sock` is version-dependent and unreliable. Forward a TCP
  port instead: add `--api-tcp 127.0.0.1:PORT` as a third transport (loopback only,
  token required) and use `ssh -N -L PORT:127.0.0.1:PORT cthulhu-computer`.
  Alternatively run the MCP server on the Linux box and connect Claude Code to it over
  SSH stdio.
- The same host has a real Docker daemon, so Docker scenarios can run against genuine
  containers there; Windows runs the same scenarios against fixtures.

**CI**
- Add a `scenario` job to `.github/workflows/ci.yml` with a matrix of
  `windows-latest` and `ubuntu-latest` running `rat --headless --scenario tests/scenarios/*.yaml`.
  Upload `test-results/` as artifacts. Enable the expectrl tests on Linux by removing the
  `cfg(windows)` gate in `tests/helpers/mod.rs` (expectrl supports Unix natively;
  only the ConPTY spawn path is Windows-specific).

### 8.5 Order of work

1. `app.snapshot` and `app.send_key` (small, unlocks everything else).
2. `--headless` and `--api-socket`.
3. MCP server.
4. Scenario runner and first ten scenarios (open editor, switch tab, SSH manager
   open/close, Docker manager open/close, health dashboard, theme switch, palette).
5. `--fixtures` and the mock transport.
6. Linux run on `cthulhu-computer`, then the CI matrix.

---

## 9. Suggested priority

1. Editor tab switch data loss (6.1.1). Users lose work today.
2. Credential storage (2.3) and API authentication (7). Security.
3. Persistent `RemoteSession` pool (2.1). Every remote feature gets faster and simpler.
4. Snapshot and key-injection API plus headless mode (8.2). Makes every later change
   verifiable by an agent on both platforms.
5. Metrics database and history (3.1).
6. Bollard-over-forward for remote Docker (4).
7. Kubernetes module (5).
8. Editor features (6.2) in the order: per-tab buffers, highlighting, auto-indent,
   Vim operators, search/replace UI.
9. Structural cleanup (6.3, 7).

---

## 10. Verification status of this survey

- Repository indexed with codebase-memory (7,211 nodes, 19,566 edges) and read
  directly with grep and file reads.
- `cargo test --no-fail-fast` was started at the beginning of the survey; the build
  had not finished when this document was written, so no test results are reported
  here. Run it before acting on section 6 and record the output.
- No code was modified.
