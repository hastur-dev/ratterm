# Ratterm Major Upgrades — Phased Build Prompt

## How to Use This Document

Each phase below is a **self-contained, executable build unit** for Claude Code. Phases are independent — you can execute them in any order. Within each phase, every step must **compile and pass all tests before proceeding**. No stubs, no `todo!()`, no skipped steps.

**Execution rules (apply to every phase):**
- Read all existing relevant source files before writing any code
- Run `cargo build` after every new file or non-trivial change
- Run `cargo test` after every test addition
- Mark each checkpoint ✅ before proceeding
- If a step fails, fix it before moving to the next step
- Final verification must pass completely before the phase is considered done

---

## Phase 1 — Native Git Integration Dashboard

### Goal
Build a full Git integration layer: gutter indicators, a Git Status Dashboard, inline diff viewer, interactive staging, commit UI, blame view, branch operations, and conflict detection.

### Dependencies to add (`Cargo.toml`)
```toml
git2 = "0.19"
```

### Steps

**Step 1.1 — Scaffold the `src/git/` module**
- Create `src/git/mod.rs` exporting all submodules
- Create `src/git/api.rs` with pure functions wrapping `git2`:
  - `fn git_status(repo_path: &Path) -> Result<Vec<StatusEntry>>`
  - `fn git_diff(repo_path: &Path, file: Option<&Path>) -> Result<DiffResult>`
  - `fn git_log(repo_path: &Path, limit: usize) -> Result<Vec<CommitEntry>>`
  - `fn git_blame(repo_path: &Path, file: &Path) -> Result<Vec<BlameLine>>`
  - `fn git_branch_list(repo_path: &Path) -> Result<Vec<BranchEntry>>`
  - `fn git_stash_list(repo_path: &Path) -> Result<Vec<StashEntry>>`
  - `fn git_commit(repo_path: &Path, message: &str, amend: bool) -> Result<()>`
  - `fn git_stage_file(repo_path: &Path, file: &Path) -> Result<()>`
  - `fn git_unstage_file(repo_path: &Path, file: &Path) -> Result<()>`
  - `fn git_checkout_branch(repo_path: &Path, branch: &str) -> Result<()>`
  - `fn git_stash_op(repo_path: &Path, op: StashOp) -> Result<()>`
- Define all supporting structs: `StatusEntry`, `DiffResult`, `DiffHunk`, `CommitEntry`, `BlameLine`, `BranchEntry`, `StashEntry`, `StashOp`
- Run `cargo build` ✅

**Step 1.2 — Unit tests for all API functions**
- Create `src/git/tests.rs`
- Use `tempfile` crate to create a real git repo fixture: `init`, `add`, `commit` a file, modify it
- Write one test per API function asserting correct output structure
- Run `cargo test git::` — all tests pass ✅

**Step 1.3 — Gutter indicator model**
- Create `src/git/gutter.rs`
- `fn compute_gutter_indicators(diff: &DiffResult) -> HashMap<usize, GutterMark>`
- `enum GutterMark { Added, Modified, Deleted }`
- Unit test: given a known diff, assert correct line→mark mapping ✅
- Run `cargo build` ✅

**Step 1.4 — Git state in `App`**
- Add to `App` struct in `src/app/mod.rs`:
  ```rust
  pub git_dashboard: Option<GitDashboard>,
  pub git_gutter: HashMap<usize, GutterMark>,
  pub git_blame_active: bool,
  pub git_blame_data: Vec<BlameLine>,
  ```
- Define `GitDashboard` struct in `src/git/dashboard.rs` with fields: `view`, `staged_files`, `unstaged_files`, `untracked_files`, `commit_log`, `branch_list`, `stash_list`, `selected_index`
- Run `cargo build` ✅

**Step 1.5 — Input handler `src/app/input_git.rs`**
- Follow the pattern of existing input handlers
- Handle: `Ctrl+Shift+G` → open/close Git Dashboard, arrow navigation, `s` → stage, `u` → unstage, `c` → open commit popup, `b` → branch view, `d` → diff view, `Ctrl+B` → toggle blame, `p` → stash pop
- Wire into main input dispatch in `src/app/input.rs`
- Run `cargo build` ✅

**Step 1.6 — Render integration**
- Add git dashboard render function to `src/app/render.rs`
- Render staged/unstaged/untracked lists using `ListSelectable` trait and `apply_dashboard_navigation()`
- Render gutter marks in editor widget (green `+`, yellow `~`, red `-` in line number column)
- Render blame gutter panel (commit hash + author + date) when `git_blame_active`
- Render inline unified diff widget using ratatui `Paragraph` with colored lines
- Run `cargo build` ✅

**Step 1.7 — Commit popup UI**
- Create `src/git/commit_ui.rs` — popup form with: message input, amend toggle, sign-off toggle, confirmation
- Wire popup state into `App`
- Render popup in render pass
- Run `cargo build` ✅

**Step 1.8 — Conflict detection**
- Add `fn detect_conflict_markers(content: &str) -> Vec<usize>` to `src/git/api.rs` — returns line numbers containing `<<<<<<<`
- When opening a file with conflict markers, highlight those regions in the editor
- Run `cargo build` ✅

**Step 1.9 — `.ratrc` config keys**
- Add `git-gutter = true` and `git-blame = true` to config parser
- Load and apply at startup
- Run `cargo build` ✅

**Step 1.10 — Integration tests**
- Test: open dashboard, navigate staged list, stage a file, verify it moves to staged section
- Test: toggle blame view, verify blame data renders
- Test: open diff view for modified file, verify hunk count matches
- Run `cargo test` — all pass ✅

### Final Verification — Phase 1
Execute each of the following manually or via automated test and confirm ✅:
- [ ] `Ctrl+Shift+G` opens the Git Dashboard with staged/unstaged/untracked sections populated
- [ ] Arrow keys navigate the list; `s` stages a file; `u` unstages it
- [ ] Modified file shows green/yellow/red gutter marks in the editor
- [ ] `Ctrl+B` on a tracked file shows blame data per line in the gutter panel
- [ ] `c` opens the commit popup; filling the message and confirming creates a commit (verify with `git log`)
- [ ] All `cargo test git::` tests pass
- [ ] `cargo build` produces zero warnings related to this phase

---

## Phase 2 — Integrated Debugger (DAP)

### Goal
Implement a Debug Adapter Protocol client: breakpoint management, debug toolbar, variable inspector, call stack panel, debug console, launch configurations, adapter auto-detection, and inline value display.

### Dependencies to add
```toml
dap = "0.5"           # or implement minimal DAP client directly via tokio process stdio
serde_json = "1"      # likely already present
```

### Steps

**Step 2.1 — Scaffold `src/debugger/` module**
- Create submodules: `mod.rs`, `client.rs`, `session.rs`, `breakpoints.rs`, `variables.rs`, `callstack.rs`, `console.rs`, `launch.rs`
- Define core types: `DebugSession`, `Breakpoint`, `StackFrame`, `Variable`, `DebugEvent`, `DapMessage`
- Run `cargo build` ✅

**Step 2.2 — DAP client (`src/debugger/client.rs`)**
- Implement `DapClient` that spawns a debug adapter process and communicates via stdin/stdout using DAP JSON-RPC framing (Content-Length headers)
- Methods: `initialize()`, `launch(config)`, `set_breakpoints(file, lines)`, `continue_()`, `step_over()`, `step_in()`, `step_out()`, `pause()`, `terminate()`, `evaluate(expr, frame_id)`
- Client runs in a `tokio::spawn` task; sends `DebugEvent`s back to App via `tokio::sync::mpsc`
- Run `cargo build` ✅

**Step 2.3 — Breakpoint persistence (`src/debugger/breakpoints.rs`)**
- `BreakpointStore` — HashMap of `file_path → Vec<line_number>`
- Save to `.ratterm/breakpoints.json` on change; load at startup
- Unit tests: add, remove, persist, reload ✅
- Run `cargo build` ✅

**Step 2.4 — Launch configuration (`src/debugger/launch.rs`)**
- Parse `.ratterm/launch.json` (VS Code-compatible subset): `program`, `args`, `env`, `cwd`, `adapter`
- `fn detect_adapter(file_ext: &str) -> Option<AdapterKind>` — maps `.rs` → `codelldb`, `.py` → `debugpy`, `.js/.ts` → `node-debug`
- Unit test: parse a sample launch.json, assert fields ✅
- Run `cargo build` ✅

**Step 2.5 — Debug state in `App`**
- Add: `debug_session: Option<DebugSession>`, `debug_state: DebugState`, `debug_panel_visible: bool`
- `DebugState` enum: `Idle`, `Running`, `Paused { frame: StackFrame }`, `Stopped`
- Run `cargo build` ✅

**Step 2.6 — Input handler `src/app/input_debugger.rs`**
- `F9` → toggle breakpoint on current editor line
- `F5` → continue / start debugging
- `F10` → step over, `F11` → step in, `Shift+F11` → step out
- `Shift+F5` → stop, `Ctrl+Shift+F5` → restart
- Wire into main input dispatch
- Run `cargo build` ✅

**Step 2.7 — Render: debug panel**
- Render debug panel below/beside editor when `debug_panel_visible`
- Left column: call stack frames (navigate with arrows)
- Right column: variable tree (expand with Enter)
- Bottom: debug console with input box for expression evaluation
- Show breakpoint markers (red `●`) in editor gutter
- Run `cargo build` ✅

**Step 2.8 — Inline value display**
- When `DebugState::Paused`, fetch local variables for current frame
- Render variable values as ghost text at end of their declaration lines in the editor
- Run `cargo build` ✅

**Step 2.9 — Status bar integration**
- Show debug state in status bar: `[DEBUG: Paused at main.rs:42]` or `[DEBUG: Running]`
- Run `cargo build` ✅

**Step 2.10 — Tests**
- Mock DAP server (simple TCP echo that responds to `initialize` and `launch` with correct responses)
- Test breakpoint set/remove/persist/reload
- Test variable tree expansion
- Test call stack frame navigation
- Run `cargo test debugger::` — all pass ✅

### Final Verification — Phase 2
- [ ] `F9` on an editor line places a red `●` in the gutter; line persists after restart
- [ ] For a Rust project with `codelldb` installed: `F5` starts a debug session, execution pauses at breakpoint
- [ ] `F10`/`F11`/`Shift+F11` step through code; call stack panel updates
- [ ] Variables panel shows locals with correct values when paused
- [ ] Debug console evaluates an expression and shows the result
- [ ] All `cargo test debugger::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 3 — Full LSP Feature Set

### Goal
Expand the existing LSP client to support: hover, go-to-definition, find references, rename, code actions, diagnostics, signature help, document symbols, workspace symbols, and formatting.

### Steps

**Step 3.1 — Refactor `src/completion/` → `src/lsp/`**
- Move existing completion code to `src/lsp/client.rs` and `src/lsp/completion.rs`
- Create new submodules: `hover.rs`, `definition.rs`, `references.rs`, `rename.rs`, `diagnostics.rs`, `actions.rs`, `symbols.rs`, `formatting.rs`, `signature.rs`
- Update all imports; `cargo build` ✅

**Step 3.2 — Hover (`src/lsp/hover.rs`)**
- Send `textDocument/hover` request when cursor idle 500ms or on `Ctrl+K`
- Parse response markdown content
- Implement simple markdown→styled-text converter (bold, code spans, paragraphs)
- Render hover popup as floating ratatui widget over editor
- Unit test: parse hover response JSON, assert styled output ✅
- Run `cargo build` ✅

**Step 3.3 — Go-to-definition (`src/lsp/definition.rs`)**
- Handle `textDocument/definition`, `typeDefinition`, `implementation`
- On `F12` or `gd`: open file at returned location (new tab if different file)
- Unit test: parse location response, assert file+line ✅
- Run `cargo build` ✅

**Step 3.4 — Find references (`src/lsp/references.rs`)**
- Send `textDocument/references` on `Shift+F12` or `gr`
- Show results in a panel: grouped by file, with line preview and match highlight
- Navigate with arrow keys; Enter opens file at location
- Unit test: parse references response, assert grouping ✅
- Run `cargo build` ✅

**Step 3.5 — Rename (`src/lsp/rename.rs`)**
- `F2`: call `textDocument/prepareRename` first (validate), then show input popup
- On confirm: call `textDocument/rename`, collect `WorkspaceEdit`
- Show diff preview of all changes; on second confirm apply all edits across files
- Unit test: parse workspace edit, assert file changes ✅
- Run `cargo build` ✅

**Step 3.6 — Diagnostics (`src/lsp/diagnostics.rs`)**
- Store diagnostics per file from `textDocument/publishDiagnostics` notifications
- Render in editor: colored underlines (red=error, yellow=warning, blue=info)
- Render gutter icons: `E`/`W`/`I`
- Render inline error at end of line (truncated to fit)
- Diagnostics panel: list all issues, navigate with arrows, Enter jumps to location
- Unit test: parse diagnostics notification, assert underline ranges ✅
- Run `cargo build` ✅

**Step 3.7 — Code actions (`src/lsp/actions.rs`)**
- `Ctrl+.`: call `textDocument/codeAction` at cursor, show popup menu of actions
- Apply selected action via `workspace/executeCommand` or `workspace/applyEdit`
- Unit test: parse code action list, assert titles ✅
- Run `cargo build` ✅

**Step 3.8 — Signature help (`src/lsp/signature.rs`)**
- Trigger on `(` and `,` keypress: call `textDocument/signatureHelp`
- Render signature popup with current parameter highlighted in bold
- Unit test: parse signature response, assert active parameter ✅
- Run `cargo build` ✅

**Step 3.9 — Symbols (`src/lsp/symbols.rs`)**
- `Ctrl+Shift+O`: document symbols — show outline with functions/structs/enums; navigate to on Enter
- `Ctrl+T`: workspace symbols — fuzzy-searchable across all files
- Unit test: parse symbol list, assert hierarchy ✅
- Run `cargo build` ✅

**Step 3.10 — Formatting (`src/lsp/formatting.rs`)**
- `textDocument/formatting` on save if `lsp-format-on-save = true` in `.ratrc`
- `textDocument/rangeFormatting` for visual selection
- Apply text edits from response to editor buffer
- Run `cargo build` ✅

**Step 3.11 — `.ratrc` config keys**
- `lsp-rust = rust-analyzer`, `lsp-python = pyright`, `lsp-format-on-save = true`
- Multi-server lifecycle: auto-start per file type, restart on crash
- Run `cargo build` ✅

**Step 3.12 — Integration tests**
- Mock LSP server responding to each request type with valid JSON responses
- Test: hover popup appears and disappears
- Test: go-to-def opens correct file and line
- Test: rename applies changes across two mock files
- Test: diagnostics underlines appear at correct character ranges
- Run `cargo test lsp::` — all pass ✅

### Final Verification — Phase 3
- [ ] Hovering on a Rust symbol shows type info popup after 500ms
- [ ] `F12` on a function call jumps to its definition (opens new tab if in another file)
- [ ] `Shift+F12` shows all references in a navigable panel
- [ ] `F2` renames a symbol; diff preview shows all affected files; confirm applies changes
- [ ] `Ctrl+.` shows code actions; selecting one applies the edit
- [ ] Compiler errors from rust-analyzer appear as red underlines and in the diagnostics panel
- [ ] `Ctrl+Shift+O` shows document outline; Enter navigates to selected symbol
- [ ] Save with `lsp-format-on-save = true` formats the file
- [ ] All `cargo test lsp::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 4 — Workspace & Multi-Folder Project Support

### Goal
Multiple project folders open simultaneously, workspace file format, task runner, scoped operations, terminal context inheritance, and recent workspaces.

### Steps

**Step 4.1 — Scaffold `src/workspace/` module**
- Create: `mod.rs`, `model.rs`, `loader.rs`, `task.rs`, `watcher.rs`
- Define: `Workspace`, `ProjectFolder`, `WorkspaceTask`, `WorkspaceSettings`
- Run `cargo build` ✅

**Step 4.2 — Workspace file format (`src/workspace/loader.rs`)**
- Parse `.ratterm-workspace.toml`:
  ```toml
  [[folders]]
  path = "./backend"
  name = "API Server"
  [[tasks]]
  name = "Build All"
  command = "cargo build"
  keybinding = "ctrl+shift+b"
  [settings]
  theme = "dracula"
  ```
- `fn load_workspace(path: &Path) -> Result<Workspace>`
- `fn save_workspace(ws: &Workspace, path: &Path) -> Result<()>`
- Unit tests: round-trip parse/save, assert all fields ✅
- Run `cargo build` ✅

**Step 4.3 — Workspace state in `App`**
- Add `workspace: Option<Workspace>` to `App`
- On launch, detect `.ratterm-workspace.toml` in working directory and load it
- Run `cargo build` ✅

**Step 4.4 — File browser: multi-root tree**
- Update file browser widget in `src/ui/file_browser.rs` to render multiple root headers
- Each root is collapsible; color-prefix entries by root index for visual separation
- Unit test: build tree from two roots, assert correct node structure ✅
- Run `cargo build` ✅

**Step 4.5 — Scoped operations**
- Find-in-files: iterate all workspace folder roots
- Git: scope operations to the folder containing the current file
- LSP: track which server serves which folder root
- Run `cargo build` ✅

**Step 4.6 — Task runner (`src/workspace/task.rs`)**
- `fn run_task(task: &WorkspaceTask) -> Result<TaskHandle>` — spawn shell command via tokio process, stream stdout/stderr via channel
- `TaskHandle` contains output receiver and abort handle
- Store active tasks in `App`: `active_tasks: Vec<TaskHandle>`
- Run `cargo build` ✅

**Step 4.7 — Task UI**
- Task picker popup: fuzzy-searchable list of workspace tasks
- Task output panel: scrollable log of stdout/stderr with color support
- Task status in status bar: `[Build All: running...]` / `[Build All: ✓]` / `[Build All: ✗]`
- Run `cargo build` ✅

**Step 4.8 — Terminal context inheritance**
- New terminal tabs open with `cwd` set to the root of the focused project folder
- Run `cargo build` ✅

**Step 4.9 — Recent workspaces**
- Append opened workspace paths to `~/.ratterm/recent_workspaces.toml` (max 20 entries)
- Show in a "Recent Workspaces" picker from the command palette
- Run `cargo build` ✅

**Step 4.10 — Tests**
- Test workspace load/save round-trip
- Test multi-root file tree construction
- Test task spawn: mock command, verify output streaming
- Test recent workspaces list management (dedup, max size)
- Run `cargo test workspace::` — all pass ✅

### Final Verification — Phase 4
- [ ] Create a `.ratterm-workspace.toml` with two `[[folders]]` entries; file browser shows both roots with their names
- [ ] Collapsing a root hides its children; expanding restores them
- [ ] Find-in-files (`Ctrl+Shift+F`) searches across both folders
- [ ] Define a task in the workspace file; trigger its keybinding; task output panel streams output
- [ ] Status bar shows task status (running → success/failure)
- [ ] New terminal tab opens with `cwd` matching the focused folder root
- [ ] All `cargo test workspace::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 5 — Integrated Test Runner Dashboard

### Goal
Test discovery, a Test Dashboard, inline test gutter indicators, test output panel, watch mode, and coverage overlay.

### Dependencies to add
```toml
# No new crates required; use tokio process spawning already present
```

### Steps

**Step 5.1 — Scaffold `src/testrunner/` module**
- Create: `mod.rs`, `adapter.rs`, `cargo_adapter.rs`, `pytest_adapter.rs`, `jest_adapter.rs`, `output_parser.rs`, `state.rs`
- Define: `TestResult`, `TestStatus` (Pass/Fail/Skip/Running), `TestItem`, `TestRunnerState`
- Run `cargo build` ✅

**Step 5.2 — Cargo test adapter (`src/testrunner/cargo_adapter.rs`)**
- `fn discover(project_root: &Path) -> Result<Vec<TestItem>>` — run `cargo test -- --list` and parse output
- `fn run(tests: &[&TestItem], project_root: &Path) -> TestRunHandle` — spawn `cargo test` with JSON output (`CARGO_TERM_COLOR=never`), stream results via channel
- `fn parse_output_line(line: &str) -> Option<TestEvent>` — parse `test name ... ok/FAILED` lines
- Unit tests with sample `cargo test` output strings ✅
- Run `cargo build` ✅

**Step 5.3 — pytest adapter (`src/testrunner/pytest_adapter.rs`)**
- Discover via `pytest --collect-only -q`
- Run via `pytest --tb=short -v`
- Parse pass/fail lines
- Unit tests with sample pytest output strings ✅
- Run `cargo build` ✅

**Step 5.4 — Jest adapter (`src/testrunner/jest_adapter.rs`)**
- Run via `jest --json` and parse JSON output
- Unit tests with sample jest JSON output ✅
- Run `cargo build` ✅

**Step 5.5 — Test state in `App`**
- Add `test_state: TestRunnerState` to `App`
- `TestRunnerState`: discovered tests, last results, active run handle, watch_mode flag
- Run `cargo build` ✅

**Step 5.6 — Test Dashboard render + input**
- `Ctrl+Shift+T` opens Test Dashboard using `apply_dashboard_navigation()` pattern
- Display tests grouped by file/module with status icons: `✓` green, `✗` red, `·` skip, `⟳` running
- Search/filter bar at top
- `r` → run selected test, `R` → run all, `w` → toggle watch mode, `Enter` → jump to test source
- Run `cargo build` ✅

**Step 5.7 — Inline gutter indicators**
- After a test run, mark test function lines in the editor with `✓`/`✗` in the gutter
- On cursor-on-test-function, show keybinding hint to run single test
- Run `cargo build` ✅

**Step 5.8 — Test output panel**
- When a test fails, show failure output below editor (assertion message, backtrace)
- File:line references in output are clickable (navigate to location)
- Run `cargo build` ✅

**Step 5.9 — Watch mode**
- When `watch_mode = true`, re-run affected tests on file save
- Detect which tests relate to the saved file (same module path heuristic)
- Run `cargo build` ✅

**Step 5.10 — Coverage overlay (Rust)**
- Parse `llvm-cov` JSON output: `cargo llvm-cov --json`
- Highlight covered lines with faint green background, uncovered with faint red
- Toggle coverage overlay with `Ctrl+Shift+C`
- Unit test: parse coverage JSON, assert line coverage map ✅
- Run `cargo build` ✅

**Step 5.11 — Tests**
- Test output parsing for each adapter with sample output strings
- Test discovery result structure
- Test watch mode trigger logic
- Test coverage line map generation
- Run `cargo test testrunner::` — all pass ✅

### Final Verification — Phase 5
- [ ] `Ctrl+Shift+T` opens dashboard; tests from a Rust project are discovered and listed
- [ ] `R` runs all tests; results update with `✓`/`✗` per test
- [ ] Editor gutter shows `✓` next to a passing `#[test]` function, `✗` next to a failing one
- [ ] Selecting a failing test shows its output panel with clickable file:line references
- [ ] `w` enables watch mode; saving a file re-runs relevant tests automatically
- [ ] `Ctrl+Shift+C` toggles coverage overlay with colored line backgrounds
- [ ] All `cargo test testrunner::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 6 — AI Assistant Integration

### Goal
LLM chat panel, context-aware prompts, inline edit suggestions, slash commands, streaming responses, API key management, and local LLM support.

### Dependencies to add
```toml
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio-stream = "0.1"
```

### Steps

**Step 6.1 — Scaffold `src/ai/` module**
- Create: `mod.rs`, `client.rs`, `conversation.rs`, `context.rs`, `streaming.rs`, `commands.rs`, `keys.rs`
- Define: `Message`, `Conversation`, `AiProvider`, `AiConfig`, `SlashCommand`
- Run `cargo build` ✅

**Step 6.2 — HTTP client (`src/ai/client.rs`)**
- `AiClient` supporting: Anthropic API, OpenAI-compatible API, Ollama
- `async fn send_message(conv: &Conversation, config: &AiConfig) -> Result<impl Stream<Item = String>>`
- Handle streaming: Server-Sent Events for Anthropic/OpenAI, chunked JSON for Ollama
- Unit test with mock HTTP server: assert correct request format, assert stream yields tokens ✅
- Run `cargo build` ✅

**Step 6.3 — API key management (`src/ai/keys.rs`)**
- Reuse SSH credential storage (master password + encrypted file) to store API keys
- `fn store_api_key(provider: &str, key: &str) -> Result<()>`
- `fn load_api_key(provider: &str) -> Result<String>`
- Unit test: store and retrieve a key ✅
- Run `cargo build` ✅

**Step 6.4 — Context builder (`src/ai/context.rs`)**
- `fn build_context(app: &App) -> ContextPayload` — assembles: current file content or selection, file path, language, cursor position, LSP diagnostics, git diff (if available)
- User-toggleable context flags per conversation
- Unit test: build context from a mock app state, assert fields ✅
- Run `cargo build` ✅

**Step 6.5 — Conversation management (`src/ai/conversation.rs`)**
- `Conversation`: vec of messages, token count tracker, system prompt
- Save/load conversations to `~/.ratterm/ai_conversations/<project_hash>/`
- Unit test: round-trip save/load ✅
- Run `cargo build` ✅

**Step 6.6 — Slash commands (`src/ai/commands.rs`)**
- Register: `/explain`, `/fix`, `/refactor`, `/test`, `/doc`, `/commit`
- Each command builds an appropriate system + user prompt from current context
- Unit test: each slash command produces a non-empty prompt ✅
- Run `cargo build` ✅

**Step 6.7 — AI state in `App`**
- Add: `ai_panel_visible: bool`, `ai_conversation: Conversation`, `ai_streaming: bool`, `ai_stream_buffer: String`
- Run `cargo build` ✅

**Step 6.8 — Chat panel render + input**
- `Ctrl+Shift+A` toggles AI panel (right sidebar or bottom panel)
- Scrollable message history; each message labeled `You` / `Claude`
- Input box at bottom; Enter sends; `Esc` closes
- Slash command autocomplete popup when input starts with `/`
- Token count displayed in panel header
- Run `cargo build` ✅

**Step 6.9 — Streaming render**
- As tokens arrive via channel from the streaming client, append to `ai_stream_buffer` and trigger re-render
- Show cursor/spinner at end of incomplete response
- Run `cargo build` ✅

**Step 6.10 — Inline edit suggestions**
- When AI response contains a code block matching the current file's language, offer it as a diff overlay
- `Tab` → accept, `Esc` → reject, edit keys → modify suggestion before accepting
- Run `cargo build` ✅

**Step 6.11 — `.ratrc` config keys**
```
ai-provider = anthropic
ai-model = claude-sonnet-4-20250514
ai-endpoint = https://api.anthropic.com
```
- Run `cargo build` ✅

**Step 6.12 — Tests**
- Mock LLM server: test full request/response cycle
- Test context building
- Test streaming token accumulation
- Test slash command dispatch
- Test inline diff suggestion accept/reject
- Run `cargo test ai::` — all pass ✅

### Final Verification — Phase 6
- [ ] `Ctrl+Shift+A` opens AI panel; typing a message and pressing Enter sends it to the configured API
- [ ] Response streams token-by-token into the panel
- [ ] `/explain` with code selected sends an explanation request; response appears in panel
- [ ] AI response containing a code block shows as a diff overlay in the editor; `Tab` applies it
- [ ] API key stored via key manager; removed key prompts for re-entry
- [ ] Pointing `ai-endpoint` at a local Ollama instance works
- [ ] All `cargo test ai::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 7 — Tmux-Style Session & Window Management

### Goal
Named sessions, multiple windows per session, arbitrary pane splits, layout presets, pane zoom, session picker, session persistence, and status bar integration.

### Steps

**Step 7.1 — Scaffold `src/session_manager/` module**
- Create: `mod.rs`, `session.rs`, `window.rs`, `pane.rs`, `layout.rs`, `picker.rs`
- Define: `SessionManager`, `Session`, `Window`, `PaneNode { Split { direction, ratio, left, right }, Leaf { content: PaneContent } }`, `PaneContent { Terminal(id), Editor(id) }`, `Direction { Horizontal, Vertical }`
- Run `cargo build` ✅

**Step 7.2 — Pane tree operations (`src/session_manager/pane.rs`)**
- `fn split(tree: PaneNode, target_leaf_id: usize, direction: Direction) -> PaneNode`
- `fn close(tree: PaneNode, leaf_id: usize) -> PaneNode`
- `fn find_neighbor(tree: &PaneNode, leaf_id: usize, direction: Direction) -> Option<usize>`
- `fn zoom(tree: &mut PaneNode, leaf_id: usize)`
- `fn resize(tree: &mut PaneNode, leaf_id: usize, direction: Direction, delta: f32)`
- `fn even_distribute(tree: &mut PaneNode)`
- Unit tests for all operations — split, close, find neighbor, zoom toggle ✅
- Run `cargo build` ✅

**Step 7.3 — Session/window CRUD (`src/session_manager/session.rs`)**
- `fn create_session(name: &str) -> Session`
- `fn rename_session(id: usize, name: &str)`
- `fn delete_session(id: usize)`
- `fn add_window(session_id: usize) -> usize`
- `fn remove_window(session_id: usize, window_id: usize)`
- Unit tests: create, rename, delete, switch ✅
- Run `cargo build` ✅

**Step 7.4 — Layout presets (`src/session_manager/layout.rs`)**
- Define preset `enum Layout { Ide, TerminalGrid, Triple, Focus }`
- `fn apply_preset(layout: Layout) -> PaneNode` — returns a pane tree for each preset
- Unit tests: each preset produces expected tree shape ✅
- Run `cargo build` ✅

**Step 7.5 — Serialize/deserialize pane tree**
- Derive `serde::Serialize` / `Deserialize` for all pane tree types
- `fn save_sessions(manager: &SessionManager, path: &Path) -> Result<()>`
- `fn load_sessions(path: &Path) -> Result<SessionManager>`
- Unit test: round-trip arbitrary tree ✅
- Run `cargo build` ✅

**Step 7.6 — Session manager state in `App`**
- Replace current layout state with `session_manager: SessionManager`
- Active session + window determines what is rendered
- Run `cargo build` ✅

**Step 7.7 — Input handler: session/window/pane keys**
- `Ctrl+Shift+H` → split active pane horizontal
- `Ctrl+Shift+V` → split active pane vertical
- `Ctrl+Shift+W` → close active pane
- `Ctrl+Shift+Z` → zoom/unzoom active pane
- `Alt+1`–`9` → switch windows in current session
- `Ctrl+Shift+S` → open session picker
- `Alt+Arrow` → navigate to neighbor pane
- `Alt+Shift+Arrow` → resize active pane
- Run `cargo build` ✅

**Step 7.8 — Session picker (`src/session_manager/picker.rs`)**
- Popup listing all sessions with window count
- Create (`n`), rename (`r`), delete (`d`), switch (Enter)
- Run `cargo build` ✅

**Step 7.9 — Recursive pane tree renderer**
- `fn render_pane_tree(frame: &mut Frame, tree: &PaneNode, area: Rect, active_leaf: usize)`
- Recursively split the `Rect` by ratio for `Split` nodes; render appropriate content for `Leaf` nodes
- Render pane title bar in each leaf (cwd / process name)
- Run `cargo build` ✅

**Step 7.10 — Status bar integration**
- Show `[session-name] W2/5` (window index / total) in status bar
- Run `cargo build` ✅

**Step 7.11 — Session persistence on exit/restore on launch**
- Save session state to `~/.ratterm/sessions.json` on exit
- Restore on next launch
- Run `cargo build` ✅

**Step 7.12 — Tests**
- Test session CRUD
- Test pane splitting and directional navigation
- Test zoom toggle (tree structure before/after)
- Test layout serialization round-trip
- Test preset layouts produce correct Rect proportions
- Run `cargo test session_manager::` — all pass ✅

### Final Verification — Phase 7
- [ ] `Ctrl+Shift+V` splits the active pane vertically; a new terminal appears in the right half
- [ ] `Alt+Arrow` moves focus between panes directionally
- [ ] `Alt+Shift+Arrow` resizes the active pane; both panes adjust proportionally
- [ ] `Ctrl+Shift+Z` maximizes the active pane; pressing again restores the split
- [ ] `Ctrl+Shift+S` opens the session picker; creating a new session switches to an empty workspace
- [ ] `Alt+1` / `Alt+2` switches between windows in the current session
- [ ] Closing and reopening Ratterm restores the previous session layout exactly
- [ ] Status bar shows session name and window index
- [ ] All `cargo test session_manager::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 8 — Full Tiling Terminal Multiplexer

### Goal
Replace the fixed 2×2 grid with a recursive binary-tree tiling system, visual resize handles, directional pane navigation, pane title bars, broadcast input mode, and pane linking.

### Steps

**Step 8.1 — Refactor `src/terminal/multiplexer.rs`**
- Replace fixed-grid state with `PaneTree` (binary tree identical in structure to Phase 7's pane tree, but terminal-only)
- Preserve existing terminal PTY management; only the layout layer changes
- `cargo build` — existing terminal functionality still works ✅

**Step 8.2 — Recursive renderer for `src/terminal/grid.rs`**
- `fn render_tree(frame: &mut Frame, tree: &PaneTree, area: Rect, active: usize)`
- Each split node divides the `Rect` by its ratio along its direction
- Each leaf renders the terminal content plus a title bar line at the top
- Unit test: given a specific tree and Rect, assert child Rects are correct ✅
- Run `cargo build` ✅

**Step 8.3 — Pane title bar**
- Show in each pane: shell name, `cwd` (last 2 path components), running process name, pane index
- Title bar is 1 row; styled to differentiate active vs inactive
- Run `cargo build` ✅

**Step 8.4 — Directional navigation**
- `fn find_neighbor_in_direction(tree: &PaneTree, active: usize, dir: Direction) -> Option<usize>`
- Algorithm: traverse tree, find active leaf's bounding Rect, find closest leaf in specified direction
- `Ctrl+W, Arrow` or `Alt+Arrow` to navigate
- Unit tests: 2×2 equivalent tree → all four directional lookups return correct neighbors ✅
- Run `cargo build` ✅

**Step 8.5 — Resize with keyboard**
- `Alt+Shift+Arrow`: find the split node above the active leaf in the given direction, adjust its ratio by 0.05, clamp to [0.1, 0.9]
- Re-render immediately
- Run `cargo build` ✅

**Step 8.6 — Pane operations input**
- `Ctrl+Shift+H` → split active terminal pane horizontal
- `Ctrl+Shift+V` → split active terminal pane vertical
- `Ctrl+Shift+W` → close active terminal pane (confirm if last)
- `Ctrl+Shift+Z` → zoom/unzoom
- `Ctrl+Shift+R` → rotate (swap H/V direction of the immediate split parent)
- `Ctrl+Shift+E` → even-distribute all ratios to 0.5 recursively
- `Ctrl+Shift+Arrow` → swap pane content with neighbor in direction
- Run `cargo build` ✅

**Step 8.7 — Broadcast input mode**
- `Ctrl+Shift+B` → toggle broadcast mode
- When active, all key events are sent to every visible terminal pane simultaneously
- Visual indicator in status bar: `[BROADCAST]`
- Run `cargo build` ✅

**Step 8.8 — Pane linking**
- `fn link_pane(terminal_id: usize, editor_id: usize, command: &str)`
- When a linked editor's file is saved, run `command` in the linked terminal
- Configure via command palette: "Link this pane to editor for auto-run"
- Run `cargo build` ✅

**Step 8.9 — Layout serialization**
- Serialize pane tree as part of session save (reuse Phase 7 serde impls)
- Restore on session load
- Run `cargo build` ✅

**Step 8.10 — Tests**
- Test split/close tree operations
- Test directional navigation algorithm on various tree shapes
- Test resize ratio clamping
- Test even-distribute
- Test broadcast mode sends events to all panes
- Run `cargo test terminal::` — all pass ✅

### Final Verification — Phase 8
- [ ] Create a split more than 2 levels deep (split, then split one of the children); all three panes show distinct terminals
- [ ] `Alt+Arrow` navigates between all panes in the correct direction
- [ ] `Alt+Shift+Right` grows the active pane; the adjacent pane shrinks
- [ ] `Ctrl+Shift+Z` zooms the active pane to full screen; pressing again restores all panes
- [ ] `Ctrl+Shift+B` enters broadcast mode; typing appears in all terminal panes simultaneously
- [ ] `Ctrl+Shift+R` rotates a split from horizontal to vertical
- [ ] Pane title bar shows the correct CWD and process name
- [ ] All `cargo test terminal::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 9 — Plugin Marketplace & Enhanced Extension System

### Goal
Marketplace registry browser, one-click install/update/uninstall, Extension API v2 (events, custom panels, commands, keybindings, status bar items, editor decorations), extension sandboxing, hot reload, templates.

### Steps

**Step 9.1 — Marketplace registry client (`src/extension/registry.rs`)**
- `fn fetch_registry(url: &str) -> Result<Vec<ExtensionManifest>>` — fetch and parse JSON index
- Define `ExtensionManifest`: name, description, author, version, downloads, tags, repo_url, compatibility
- Cache registry to `~/.ratterm/extension_registry_cache.json` (TTL 1 hour)
- Unit test with mock JSON: parse, assert fields ✅
- Run `cargo build` ✅

**Step 9.2 — Extension Browser Dashboard**
- `Ctrl+Shift+E` opens Extension Browser using `apply_dashboard_navigation()` pattern
- Views: Installed (with update badges), Popular, Recent, By Category
- Search input filters list
- `i` → install, `u` → update, `d` → uninstall, `Enter` → view details
- Run `cargo build` ✅

**Step 9.3 — Install/update/uninstall flow**
- Download extension archive from `repo_url`, extract to `~/.ratterm/extensions/<name>/`
- Show progress indicator during download
- Auto-start/restart extension process after install/update
- Confirm prompt before uninstall
- Run `cargo build` ✅

**Step 9.4 — Extension API v2: event subscriptions**
- Extend the REST API server to support `POST /subscribe { event: "file_save" }`
- Events: `file_save`, `file_open`, `focus_change`, `terminal_output`, `key_press`
- On event, POST to extension's registered callback URL
- Unit test: subscribe, trigger event, assert callback was invoked ✅
- Run `cargo build` ✅

**Step 9.5 — Extension API v2: custom commands**
- Extensions `POST /register_command { id, name, keybinding }`
- Commands appear in the command palette
- On activation, POST to extension's command handler URL
- Run `cargo build` ✅

**Step 9.6 — Extension API v2: status bar items**
- `POST /statusbar { id, text, color, tooltip }`
- Render extension-provided items in the status bar
- `DELETE /statusbar/:id` removes the item
- Run `cargo build` ✅

**Step 9.7 — Extension API v2: editor decorations**
- `POST /decorate { file, line, gutter_icon, inline_text, bg_color }`
- Apply decorations in the editor render pass
- Run `cargo build` ✅

**Step 9.8 — Extension sandboxing**
- `manifest.toml` declares permissions: `filesystem = ["read:./src"]`, `network = false`, `terminal = false`
- On install, prompt user to approve permissions
- Enforce at API layer: reject requests outside declared scope
- Run `cargo build` ✅

**Step 9.9 — Hot reload**
- Watch extension source directory with `notify` crate (already a dependency or add it)
- On change, restart extension process without restarting Ratterm
- Run `cargo build` ✅

**Step 9.10 — Extension log viewer**
- Extension stdout/stderr captured and shown in a dedicated log panel per extension (accessible from the Extension Browser dashboard)
- Run `cargo build` ✅

**Step 9.11 — Extension templates (`rat ext create`)**
- CLI subcommand scaffolds a new extension directory: `manifest.toml`, `main.py`/`main.js`, `Makefile`, `README.md`
- Run `cargo build` ✅

**Step 9.12 — Tests**
- Test registry fetch and parse with mock server
- Test install/uninstall file operations
- Test event subscription delivery
- Test permission enforcement (request outside scope → HTTP 403)
- Test hot reload detection
- Run `cargo test extension::` — all pass ✅

### Final Verification — Phase 9
- [ ] `Ctrl+Shift+E` opens the Extension Browser; the registry list populates from cache or network
- [ ] Installing an extension downloads it, starts its process, and it appears in the Installed view
- [ ] An extension subscribing to `file_save` receives a POST callback when a file is saved
- [ ] A command registered by an extension appears in the command palette and fires the callback when invoked
- [ ] A status bar item registered by an extension appears in the status bar
- [ ] An extension requesting filesystem access outside its declared scope is rejected
- [ ] Modifying an extension's source triggers hot reload without restarting Ratterm
- [ ] All `cargo test extension::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 10 — Advanced Search & Replace Engine

### Goal
Dedicated search panel, regex mode with validation, live results, replace preview, preserve-case, search history, multi-line search, structural (tree-sitter) search, bookmark results, and `F3` navigation.

### Steps

**Step 10.1 — Scaffold `src/search/` module**
- Create: `mod.rs`, `engine.rs`, `replace.rs`, `history.rs`, `structural.rs`, `state.rs`
- Define: `SearchQuery`, `SearchResult`, `FileMatches`, `ReplacePreview`, `SearchState`
- Run `cargo build` ✅

**Step 10.2 — Search engine (`src/search/engine.rs`)**
- Background thread (via `std::thread::spawn` + `mpsc`) that receives `SearchQuery` and streams `FileMatches` results
- Support: literal, regex (via `regex` crate), case-sensitive/insensitive, whole-word, multi-line
- Glob-based file include/exclude filters
- Visual regex validation: `fn validate_regex(pattern: &str) -> Option<String>` (returns error message if invalid)
- Unit tests: literal search, regex search, multi-line match, glob filter ✅
- Run `cargo build` ✅

**Step 10.3 — Replace engine (`src/search/replace.rs`)**
- `fn compute_replacements(matches: &[FileMatches], replacement: &str, preserve_case: bool) -> Vec<FileEdit>`
- `fn apply_replacements(edits: &[FileEdit]) -> Result<()>`
- Preserve-case: if match is ALL_CAPS → replacement ALL_CAPS; Title_Case → Title_Case; lower → lower
- Unit tests: preserve-case for all three patterns, multi-file edits ✅
- Run `cargo build` ✅

**Step 10.4 — Search history (`src/search/history.rs`)**
- Persist last 50 search/replace patterns to `~/.ratterm/search_history.toml`
- Cycle with up/down arrows in search input
- Unit test: add, cycle, persist, reload ✅
- Run `cargo build` ✅

**Step 10.5 — Structural search (`src/search/structural.rs`)**
- Use tree-sitter queries to search for code patterns: `fn find_pattern(source: &str, lang: Language, query: &str) -> Vec<Range>`
- Example query: `(if_expression consequence: (block (return_expression)))` to find if-blocks containing return
- Unit test: find a pattern in a Rust source string ✅
- Run `cargo build` ✅

**Step 10.6 — Search state in `App`**
- Add `search_state: SearchState` with: query, results, active_result_index, replace_preview, panel_visible
- Run `cargo build` ✅

**Step 10.7 — Search panel render + input**
- `Ctrl+Shift+F` opens search panel (bottom or right panel)
- Inputs: search text, replace text, file glob, regex toggle, case toggle, whole-word toggle
- Results area: file tree grouped results with line preview and highlighted match
- Replace preview button: show diff of all replacements before applying
- `F3` / `Shift+F3` navigate between matches
- Match counter: `3 of 47`
- Run `cargo build` ✅

**Step 10.8 — Replace preview diff view**
- Before applying replace-all, render a scrollable diff showing each change
- Checkboxes per replacement (deselect to skip individual ones)
- Confirm button applies only selected replacements
- Run `cargo build` ✅

**Step 10.9 — Search in selection**
- When editor has a visual selection, `Ctrl+Shift+F` pre-fills the scope to "selection only"
- Search engine limits results to the selection range
- Run `cargo build` ✅

**Step 10.10 — Bookmark results**
- `b` in the search panel bookmarks all current results
- Bookmarks shown in a sidebar; `F3`/`Shift+F3` also cycles through bookmarks when search is closed
- Run `cargo build` ✅

**Step 10.11 — Tests**
- Test regex validation messages
- Test replace-all with preserve-case
- Test file glob filtering
- Test multi-line pattern matching
- Test structural search on sample Rust code
- Test replace preview deselection (skip individual replacement)
- Run `cargo test search::` — all pass ✅

### Final Verification — Phase 10
- [ ] `Ctrl+Shift+F` opens search panel; typing updates results live as you type
- [ ] Enabling regex mode with an invalid pattern shows a validation error inline
- [ ] A search with `*.rs` glob filter returns only Rust files
- [ ] Replace with preserve-case: replacing `foo` → `bar` also replaces `Foo` → `Bar` and `FOO` → `BAR`
- [ ] Replace preview shows a diff; deselecting one replacement and confirming skips it
- [ ] `F3` / `Shift+F3` cycles through matches; match counter updates
- [ ] Structural search finds all `if` blocks containing `return` in a Rust file
- [ ] All `cargo test search::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 11 — Notebook / REPL Integration

### Goal
`.ratbook` notebook format, cell model, kernel management, rich output rendering, cell operations, variable inspector, standalone REPL panel, and `.ipynb` import.

### Steps

**Step 11.1 — Scaffold `src/notebook/` module**
- Create: `mod.rs`, `model.rs`, `kernel.rs`, `renderer.rs`, `ipynb.rs`
- Define: `Notebook`, `Cell { CellType, source, outputs }`, `CellType { Code(lang), Markdown, Output }`, `CellOutput { Text, Table, Error }`
- Run `cargo build` ✅

**Step 11.2 — Notebook format: `.ratbook` (`src/notebook/model.rs`)**
- Serialize/deserialize `.ratbook` (TOML): notebook metadata + cell list
- `fn load_notebook(path: &Path) -> Result<Notebook>`
- `fn save_notebook(nb: &Notebook, path: &Path) -> Result<()>`
- Unit test: round-trip with code + markdown cells ✅
- Run `cargo build` ✅

**Step 11.3 — `.ipynb` import (`src/notebook/ipynb.rs`)**
- Parse Jupyter `.ipynb` JSON format into `Notebook` (read-only import)
- `fn import_ipynb(path: &Path) -> Result<Notebook>`
- Unit test with a sample `.ipynb` fixture ✅
- Run `cargo build` ✅

**Step 11.4 — Kernel management (`src/notebook/kernel.rs`)**
- `Kernel` trait with: `fn start() -> Result<KernelHandle>`, `fn execute(code: &str) -> Result<CellOutput>`, `fn interrupt()`, `fn stop()`
- Implementations: `PythonKernel` (subprocess REPL via stdin/stdout), `NodeKernel`, `RustKernel` (evcxr)
- `fn detect_kernel(lang: &str) -> Option<Box<dyn Kernel>>`
- Unit test with a mock kernel that echoes input ✅
- Run `cargo build` ✅

**Step 11.5 — Output rendering (`src/notebook/renderer.rs`)**
- Plain text output → styled `Paragraph`
- Tabular output (CSV-like) → ratatui `Table`
- Error output → red-styled traceback with syntax-highlighted file:line references
- Render each cell's output below its code block
- Unit test: render tabular output, assert table row count ✅
- Run `cargo build` ✅

**Step 11.6 — Notebook state in `App`**
- Notebooks open as editor tabs: `TabContent::Notebook(Notebook)`
- `active_cell: usize`, `executing_cell: Option<usize>`
- Run `cargo build` ✅

**Step 11.7 — Notebook render + input**
- `Ctrl+Enter` → execute active cell; show spinner while running; replace with output
- `Ctrl+Up/Down` → navigate between cells
- `a` → add cell above, `b` → add cell below, `d d` → delete cell
- `m` → change to Markdown, `y` → change to Code
- `Ctrl+C` → interrupt executing cell
- Run `cargo build` ✅

**Step 11.8 — Markdown cell rendering**
- Render Markdown cells (not editable, just display) using the same markdown renderer from Phase 3 (LSP hover)
- Run `cargo build` ✅

**Step 11.9 — Variable inspector**
- After each execution, query kernel for current variables (language-specific command: `dir()` for Python, etc.)
- Show in a side panel within the notebook view
- Run `cargo build` ✅

**Step 11.10 — Standalone REPL panel**
- `Ctrl+Shift+R` opens a REPL panel (not a full notebook)
- Language auto-detected from current file type
- Input line at bottom; results shown above; history with up/down arrows
- Run `cargo build` ✅

**Step 11.11 — Tests**
- Test notebook round-trip save/load
- Test `.ipynb` import cell extraction
- Test mock kernel execution and output parsing
- Test cell navigation and CRUD operations
- Test tabular output rendering produces correct table dimensions
- Run `cargo test notebook::` — all pass ✅

### Final Verification — Phase 11
- [ ] Open a `.ratbook` file; it displays as a vertical list of cells in an editor tab
- [ ] `Ctrl+Enter` on a Python code cell executes it via the Python kernel; output appears below the cell
- [ ] A cell producing tabular output renders as a formatted table
- [ ] A cell that errors shows a red traceback
- [ ] `Ctrl+C` interrupts a long-running cell
- [ ] `m` converts a code cell to a rendered Markdown cell
- [ ] `Ctrl+Shift+R` opens the REPL panel; evaluating an expression shows the result
- [ ] All `cargo test notebook::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 12 — Collaborative Editing

### Goal
Phase 12A: local multi-cursor editing (`Ctrl+D`, `Alt+Click`, column selection, select all occurrences). Phase 12B: CRDT-based network collaboration (host/join, peer cursors, follow mode, chat, permissions).

### Dependencies to add
```toml
# Phase 12A: no new deps
# Phase 12B:
yrs = "0.18"
tokio-tungstenite = "0.21"
```

### Steps — Phase 12A (Local Multi-Cursor)

**Step 12A.1 — Multi-cursor model (`src/editor/multicursor.rs`)**
- `CursorSet`: `Vec<Cursor>` with dedup/merge logic
- `fn add_cursor(pos: usize)`, `fn remove_cursor(pos: usize)`, `fn merge_overlapping()`
- Unit tests: add, remove, merge overlapping, merge adjacent ✅
- Run `cargo build` ✅

**Step 12A.2 — Multi-cursor operations**
- All editing ops in `src/editor/buffer.rs` apply to every cursor in `CursorSet`
- Insert char: insert at each cursor position (adjusting subsequent positions for offset)
- Delete: delete before/after each cursor
- Undo: undo all cursors' last action together
- Unit tests: multi-cursor insert, delete, paste ✅
- Run `cargo build` ✅

**Step 12A.3 — Input: add/remove cursors**
- `Ctrl+D` → add cursor at next occurrence of current selection (or current word if no selection)
- `Ctrl+Shift+L` → add cursor at every occurrence of current selection
- `Alt+Click` → add cursor at click position (mouse events)
- `Esc` → collapse all cursors to primary
- Run `cargo build` ✅

**Step 12A.4 — Column selection**
- `Alt+Shift+Arrow` → extend a rectangular selection, placing a cursor at each row
- Run `cargo build` ✅

**Step 12A.5 — Render multiple cursors**
- In editor render pass, draw a cursor block/line at each cursor position
- Draw selection highlight at each cursor's selection range
- Run `cargo build` ✅

**Step 12A.6 — Tests (12A)**
- Test `Ctrl+D` selects next occurrence
- Test `Ctrl+Shift+L` selects all 3 occurrences in a sample text
- Test multi-cursor insert produces correct buffer content
- Test cursor merging when two cursors land on same position
- Run `cargo test editor::multicursor` — all pass ✅

### Steps — Phase 12B (Network Collaboration)

**Step 12B.1 — CRDT document model (`src/collab/crdt.rs`)**
- Wrap `yrs::Doc` and `yrs::Text`
- `fn apply_local_insert(doc, pos, text)`, `fn apply_local_delete(doc, pos, len)`
- `fn encode_state(doc) -> Vec<u8>`, `fn apply_update(doc, update: &[u8])`
- Unit test: two docs sync via encoded updates ✅
- Run `cargo build` ✅

**Step 12B.2 — WebSocket transport (`src/collab/transport.rs`)**
- `fn host_session(port: u16) -> Result<CollabServer>` — accepts WebSocket connections
- `fn join_session(addr: &str) -> Result<CollabClient>`
- Both send/receive `CollabMessage` (JSON-encoded): `Update(Vec<u8>)`, `CursorMove { peer_id, pos }`, `Chat { peer_id, text }`, `Presence { peer_id, name, color }`
- Unit test with two in-process clients ✅
- Run `cargo build` ✅

**Step 12B.3 — Collab state in `App`**
- Add: `collab: Option<CollabSession>`, `peer_cursors: HashMap<PeerId, PeerCursor>`
- `CollabSession` holds: role (Host/Guest), connected peers, CRDT doc, transport handle
- Run `cargo build` ✅

**Step 12B.4 — Peer cursor rendering**
- Draw other users' cursors in the editor with a distinct color and name label
- Labels fade after 3 seconds of no movement
- Run `cargo build` ✅

**Step 12B.5 — Follow mode**
- `Ctrl+Shift+F2` → enter follow mode for a peer; scroll/cursor tracks theirs
- Run `cargo build` ✅

**Step 12B.6 — Chat sidebar**
- Simple message list + input box within a collab panel
- Messages from all peers shown with name and color
- Run `cargo build` ✅

**Step 12B.7 — CLI subcommands**
- `rat collab host [--port PORT]` → start server, print session info
- `rat collab join <host:port>` → connect as guest
- Run `cargo build` ✅

**Step 12B.8 — Tests (12B)**
- Test CRDT sync: two docs make concurrent edits; both converge to same content
- Test cursor broadcast: peer cursor move is received and rendered
- Test chat message delivery
- Run `cargo test collab::` — all pass ✅

### Final Verification — Phase 12
- [ ] `Ctrl+D` with "foo" selected adds a cursor at the next "foo"; typing replaces both
- [ ] `Ctrl+Shift+L` with "foo" selected adds cursors at all occurrences; typing replaces all
- [ ] `Esc` collapses multi-cursor to primary cursor
- [ ] Two Ratterm instances: `rat collab host` and `rat collab join`; typing in one appears in the other
- [ ] Peer cursor shown with name label in the joining instance's editor
- [ ] Chat messages appear in both instances' chat sidebar
- [ ] All `cargo test editor::multicursor` and `cargo test collab::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 13 — Snippet Engine & Template System

### Goal
TOML snippet format, tab-stop navigation, mirror placeholders, choice placeholders, variable interpolation, snippet picker, built-in snippets, and project template scaffolding.

### Steps

**Step 13.1 — Scaffold `src/snippets/` module**
- Create: `mod.rs`, `parser.rs`, `engine.rs`, `store.rs`, `variables.rs`, `templates.rs`
- Define: `Snippet`, `TabStop`, `SnippetBody { nodes: Vec<BodyNode> }`, `BodyNode { Text, TabStop(id, default), Mirror(id), Choice(id, options) }`, `SnippetSession`
- Run `cargo build` ✅

**Step 13.2 — Snippet parser (`src/snippets/parser.rs`)**
- Parse snippet body strings with `$1`, `${1:default}`, `${1|a,b,c|}`, `$FILENAME` etc.
- `fn parse_body(body: &str) -> Result<SnippetBody>`
- Unit tests: parse each placeholder type, assert correct node structure ✅
- Run `cargo build` ✅

**Step 13.3 — Snippet store (`src/snippets/store.rs`)**
- Load `.toml` snippet files from `~/.ratterm/snippets/` and built-in defaults
- Index by `(language, prefix)`
- `fn lookup(lang: &str, prefix: &str) -> Option<&Snippet>`
- `fn lookup_all(lang: &str) -> Vec<&Snippet>` for the picker
- Unit test: load a fixture TOML, assert lookup works ✅
- Run `cargo build` ✅

**Step 13.4 — Variable resolver (`src/snippets/variables.rs`)**
- Resolve: `$FILENAME`, `$FILE_BASENAME`, `$DATE` (ISO format), `$CLIPBOARD`, `$SELECTION`, `$LINE_NUMBER`, `$WORKSPACE_NAME`
- `fn resolve_variables(body: &SnippetBody, ctx: &SnippetContext) -> SnippetBody`
- Unit test: resolve each variable ✅
- Run `cargo build` ✅

**Step 13.5 — Tab-stop engine (`src/snippets/engine.rs`)**
- `SnippetSession`: active snippet in editor, current tab-stop index, positions of each stop in the buffer
- `fn insert_snippet(session: &mut SnippetSession, buffer: &mut Buffer, snippet: &Snippet, pos: usize)`
- `fn advance_tab_stop(session: &mut SnippetSession, buffer: &mut Buffer)` → move to next `$N`
- `fn retreat_tab_stop(session: &mut SnippetSession, buffer: &mut Buffer)` → move to previous
- Mirror updates: when `$1` is edited, all `$1` mirrors update to match
- Unit tests: insert snippet, advance through stops, assert cursor positions and buffer state ✅
- Run `cargo build` ✅

**Step 13.6 — Choice placeholder popup**
- When tab-stop advances to a `Choice` node, show a dropdown menu of options
- Arrow keys select; Enter confirms; selection replaces the placeholder
- Run `cargo build` ✅

**Step 13.7 — Input: trigger and picker**
- `Tab` on a known prefix triggers expansion (configurable in `.ratrc`)
- `Ctrl+Shift+;` opens the snippet picker: fuzzy-searchable list filtered by current language
- `Tab` / `Shift+Tab` navigate tab-stops during active snippet session
- `Esc` exits snippet session (leaves text in place)
- Run `cargo build` ✅

**Step 13.8 — Built-in snippets**
- Create default snippet TOML files for: Rust, Python, JavaScript/TypeScript, Go, HTML, CSS, Markdown, Shell
- Each file has at least 5 commonly used snippets
- Run `cargo build` ✅

**Step 13.9 — Project templates (`src/snippets/templates.rs`)**
- `fn list_templates() -> Vec<TemplateManifest>` — reads from `~/.ratterm/templates/`
- `fn scaffold_template(name: &str, dest: &Path, vars: HashMap<String, String>) -> Result<()>`
- CLI: `rat new <template>` — prompts for template variables, scaffolds directory
- Unit test: scaffold a fixture template, assert files created with correct content ✅
- Run `cargo build` ✅

**Step 13.10 — Tests**
- Test tab-stop navigation: 3-stop snippet, assert cursor positions after each Tab
- Test mirror update: editing `$1` updates all `$1` mirrors in buffer
- Test choice dropdown: selecting option 2 replaces placeholder with option 2's text
- Test variable resolution in a complete snippet
- Test template scaffolding
- Run `cargo test snippets::` — all pass ✅

### Final Verification — Phase 13
- [ ] In a Rust file, type `fn` and press `Tab`; a function template expands with cursor on function name
- [ ] `Tab` advances to the parameter placeholder; `Shift+Tab` returns to the name
- [ ] A mirror placeholder: editing the function name updates both occurrences simultaneously
- [ ] A choice placeholder shows a dropdown; selecting an option fills it in
- [ ] `$DATE` variable resolves to today's date
- [ ] `Ctrl+Shift+;` opens snippet picker; fuzzy search finds `fn` snippet
- [ ] `rat new rust-bin` scaffolds a new Rust binary project with correct file structure
- [ ] All `cargo test snippets::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 14 — Persistent Undo & File History

### Goal
Persistent undo history across file closes, undo tree (non-linear), timeline viewer, auto-snapshots, diff between timeline points, and configurable retention.

### Steps

**Step 14.1 — Scaffold `src/history/` module**
- Create: `mod.rs`, `store.rs`, `tree.rs`, `timeline.rs`, `snapshot.rs`, `diff.rs`
- Define: `UndoOp { Insert { pos, text }, Delete { pos, text } }`, `UndoNode { op, timestamp, children, parent }`, `UndoTree`, `TimelineEntry`
- Run `cargo build` ✅

**Step 14.2 — Undo tree model (`src/history/tree.rs`)**
- `UndoTree`: current node pointer + full tree
- `fn push_op(tree: &mut UndoTree, op: UndoOp)` — add child to current node, advance pointer (preserves old redo branch)
- `fn undo(tree: &mut UndoTree) -> Option<UndoOp>` — move to parent
- `fn redo(tree: &mut UndoTree) -> Option<UndoOp>` — move to most-recent child
- `fn redo_to_branch(tree: &mut UndoTree, branch_index: usize) -> Option<UndoOp>` — navigate alternate redo branches
- Unit tests: linear undo/redo, branch preservation after new edit, branch navigation ✅
- Run `cargo build` ✅

**Step 14.3 — Persistent storage (`src/history/store.rs`)**
- Key: `sha256(absolute_file_path)` → `~/.ratterm/history/<hash>.bin`
- Serialize `UndoTree` to a compact binary format (use `bincode` or manual encoding)
- `fn load(file_path: &Path) -> Result<UndoTree>`
- `fn save(file_path: &Path, tree: &UndoTree) -> Result<()>`
- Auto-save on every 10 ops or on file close
- Unit test: save and reload tree, assert structural equality ✅
- Run `cargo build` ✅

**Step 14.4 — Wire into editor buffer**
- Replace `src/editor/buffer.rs` undo stack with `UndoTree` from `src/history/`
- On file open: load persisted tree (or create fresh)
- On undo/redo: apply ops from tree
- Run `cargo build` ✅

**Step 14.5 — Auto-snapshots (`src/history/snapshot.rs`)**
- Create named snapshot nodes in the tree at: file save, paste (if > 20 chars), find-and-replace apply
- Snapshots are named `UndoNode` entries (e.g., `"saved at 14:32"`)
- Unit test: perform paste → assert snapshot node created ✅
- Run `cargo build` ✅

**Step 14.6 — Timeline viewer (`src/history/timeline.rs`)**
- `Ctrl+Shift+H` opens timeline panel
- Scrollable list of undo nodes with timestamps and op summaries
- Arrow keys navigate; Enter previews the file at that point (read-only diff view)
- `r` → restore to this point
- Run `cargo build` ✅

**Step 14.7 — Diff between timeline points**
- Select two timeline entries (mark with `m`); `d` shows a diff between them
- Render as a unified diff in a popup
- Unit test: diff two buffer states, assert correct hunks ✅
- Run `cargo build` ✅

**Step 14.8 — Retention cleanup**
- On startup: prune history entries older than `history-retention` days, over `history-max-size` bytes
- `fn prune(store_dir: &Path, max_age_days: u32, max_size_bytes: u64)`
- Unit test: create over-limit entries, run prune, assert count reduced ✅
- Run `cargo build` ✅

**Step 14.9 — `.ratrc` config keys**
- `history-retention = 30d`, `history-max-size = 50mb`, `history-enabled = true`
- Run `cargo build` ✅

**Step 14.10 — Tests**
- Test undo tree branching (undo → new edit → old branch preserved)
- Test persist/reload with 500-op tree
- Test auto-snapshot on paste
- Test timeline scrubbing: navigate to midpoint, assert buffer state
- Test retention pruning
- Run `cargo test history::` — all pass ✅

### Final Verification — Phase 14
- [ ] Make edits, close the file, reopen it; undo (`u`) restores edits from before the close
- [ ] Undo multiple times, make a new edit, then try to redo — the old redo branch is accessible via the timeline
- [ ] `Ctrl+Shift+H` opens the timeline; navigating to an earlier entry previews the file at that state
- [ ] Pressing `r` on a timeline entry restores the file to that state
- [ ] Pasting a large block of text creates a named snapshot in the timeline
- [ ] All `cargo test history::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 15 — Semantic Syntax Highlighting & Visual Enhancements

### Goal
Semantic tokens from LSP, rainbow brackets, indent guides, bracket matching, color previews, sticky scroll, minimap, code folding, and additional tree-sitter language grammars.

### Steps

**Step 15.1 — Semantic token request (`src/lsp/semantic.rs`)**
- `textDocument/semanticTokens/full` request on file open and change (debounced 300ms)
- Parse `SemanticTokens` response (delta-encoded)
- Map token types to terminal styles: mutable variable → underline, constant → bold, unused → dim, macro → italic+color
- Unit test: decode delta-encoded tokens, assert position+type list ✅
- Run `cargo build` ✅

**Step 15.2 — Apply semantic styles in editor render**
- Semantic styles overlay on top of tree-sitter syntax highlighting
- Semantic style wins where it provides information
- Run `cargo build` ✅

**Step 15.3 — Rainbow brackets (`src/editor/rainbow.rs`)**
- `fn assign_bracket_colors(source: &str) -> HashMap<usize, Color>` — walk bracket pairs, assign cycling colors from palette
- Unmatched brackets → red
- Color palette: 6 distinct colors from current theme
- Unit tests: balanced brackets get matching colors, unmatched → red ✅
- Run `cargo build` ✅

**Step 15.4 — Indent guides**
- Render vertical `│` characters at each indent level in the editor
- Active indent scope (containing cursor) uses brighter color
- Only render in non-whitespace-only lines
- Run `cargo build` ✅

**Step 15.5 — Bracket matching**
- When cursor is on `(`, `)`, `[`, `]`, `{`, `}`, highlight the matching bracket with a distinct background
- `fn find_matching_bracket(source: &str, pos: usize) -> Option<usize>`
- Unit tests: balanced, nested, unmatched ✅
- Run `cargo build` ✅

**Step 15.6 — Color previews**
- `fn detect_color_values(line: &str) -> Vec<(usize, Color)>` — detect `#RRGGBB`, `rgb(r,g,b)` patterns
- Render a 1-char colored block in the gutter for each detected color
- Unit tests: detect hex, rgb variants ✅
- Run `cargo build` ✅

**Step 15.7 — Sticky scroll**
- Top 1–3 rows of the editor show the current scope chain: `fn main > if condition > for loop`
- `fn get_scope_chain(source: &str, cursor_line: usize, lang: Language) -> Vec<String>` — use tree-sitter to walk ancestors
- Unit test: cursor inside nested if → assert scope chain ✅
- Run `cargo build` ✅

**Step 15.8 — Minimap**
- Narrow column (10 chars wide) on right edge showing zoomed-out file
- Each row of minimap represents N lines of source (scaled to file length)
- Syntax colors approximated (use foreground color of first token per line)
- Current viewport highlighted as a block
- Search results and diagnostics shown as colored marks
- Click/scroll on minimap scrolls the editor
- Run `cargo build` ✅

**Step 15.9 — Code folding**
- `fn detect_fold_regions(tree: &Tree) -> Vec<FoldRegion>` — use tree-sitter node types: functions, blocks, imports, comment blocks
- Store fold state per file: `HashSet<usize>` of folded start lines
- `za` (Vim) / `Ctrl+Shift+[` → toggle fold at cursor
- Render folded region as single line with `···` suffix and line count
- Show fold indicator `▶`/`▼` in gutter
- Unit tests: detect fold regions in sample Rust source ✅
- Run `cargo build` ✅

**Step 15.10 — Additional tree-sitter grammars**
- Add grammars for: TypeScript, Go, C, C++, Java, YAML, TOML, JSON, HTML, CSS, SQL, Bash, Dockerfile
- Wire each into the language detector and highlighter
- Run `cargo build` ✅

**Step 15.11 — Tests**
- Test semantic token decode for a sample LSP response
- Test rainbow bracket color assignment (6-cycle, unmatched red)
- Test bracket matching with nested brackets
- Test fold region detection in Rust source
- Test minimap position calculations (line → minimap row)
- Run `cargo test editor::visual` — all pass ✅

### Final Verification — Phase 15
- [ ] Open a Rust file with `rust-analyzer` running; mutable variables appear underlined, constants bold
- [ ] Matching brackets have distinct cycling colors; an unmatched `(` is red
- [ ] Vertical indent guides appear; the guide for the scope containing the cursor is brighter
- [ ] Placing cursor on `{` highlights the matching `}`
- [ ] A line containing `#FF5733` shows a small orange color swatch in the gutter
- [ ] Top of editor shows scope chain (e.g., `impl Foo > fn bar`)
- [ ] Minimap appears on the right edge; scrolling the editor moves the highlighted viewport block
- [ ] `za` collapses a function body; the line shows `fn foo() { ··· } (23 lines)`
- [ ] TypeScript and Go files have correct syntax highlighting
- [ ] All `cargo test editor::visual` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 16 — Kubernetes Dashboard

### Goal
Full Kubernetes cluster management dashboard: resource browser, pod management, deployment operations, log viewer, resource YAML editor, context switching, namespace filtering, metrics, events viewer.

### Dependencies to add
```toml
kube = { version = "0.93", features = ["runtime", "derive"] }
k8s-openapi = { version = "0.22", features = ["v1_30"] }
```

### Steps

**Step 16.1 — Scaffold `src/kubernetes/` module**
- Create: `mod.rs`, `api.rs`, `dashboard.rs`, `logs.rs`, `resources.rs`
- Define: `KubeResource`, `PodInfo`, `DeploymentInfo`, `ServiceInfo`, `ResourceKind`
- Run `cargo build` ✅

**Step 16.2 — Kubernetes API layer (`src/kubernetes/api.rs`)**
- Follow same API-first pattern as `src/docker/api.rs`
- `fn list_namespaces(client: &Client) -> Result<Vec<String>>`
- `fn list_pods(client, namespace) -> Result<Vec<PodInfo>>`
- `fn list_deployments(client, namespace) -> Result<Vec<DeploymentInfo>>`
- `fn list_services(client, namespace) -> Result<Vec<ServiceInfo>>`
- `fn get_pod_logs(client, namespace, pod, container) -> Result<impl Stream<Item=String>>`
- `fn exec_in_pod(client, namespace, pod, container, cmd) -> Result<()>`
- `fn scale_deployment(client, namespace, name, replicas) -> Result<()>`
- `fn rollback_deployment(client, namespace, name) -> Result<()>`
- `fn delete_pod(client, namespace, name) -> Result<()>`
- `fn get_resource_yaml(client, kind, namespace, name) -> Result<String>`
- `fn apply_resource_yaml(client, yaml: &str) -> Result<()>`
- `fn list_contexts() -> Result<Vec<String>>`
- `fn switch_context(name: &str) -> Result<()>`
- `fn list_events(client, namespace) -> Result<Vec<KubeEvent>>`
- Unit tests with mock API responses (use `tower::ServiceBuilder` mock) ✅
- Run `cargo build` ✅

**Step 16.3 — Dashboard state in `App`**
- Add `kube_dashboard: Option<KubeDashboard>` with: active context, namespace filter, selected resource kind, resource list, selected index
- Run `cargo build` ✅

**Step 16.4 — Kubernetes Dashboard render + input**
- `Ctrl+Shift+K` opens dashboard using `apply_dashboard_navigation()` pattern
- Left panel: resource kind selector (Pods, Deployments, Services, etc.)
- Main panel: resource list with status, namespace, age
- `Enter` → describe resource, `l` → logs, `e` → exec, `d` → delete (confirm), `y` → edit YAML, `s` → scale (Deployments), `/` → filter by namespace
- Run `cargo build` ✅

**Step 16.5 — Log viewer**
- Stream pod logs into a scrollable panel (reuse `src/docker_logs/` infrastructure)
- Container selector popup for multi-container pods
- Follow mode toggle, timestamp display, search/filter
- Run `cargo build` ✅

**Step 16.6 — YAML editor integration**
- `y` on a resource opens its YAML in an editor tab
- On save: call `apply_resource_yaml()`; show success/error notification
- YAML syntax highlighting via tree-sitter YAML grammar (added in Phase 15)
- Run `cargo build` ✅

**Step 16.7 — Context and namespace UI**
- Context switcher popup: list all contexts, Enter switches
- Show current context in status bar
- Namespace filter persisted per context
- Run `cargo build` ✅

**Step 16.8 — Events viewer**
- Tab in the dashboard showing cluster events: timestamp, kind, reason, message, namespace
- Filter by severity (Warning / Normal)
- Run `cargo build` ✅

**Step 16.9 — Tests**
- Test resource list parsing from mock API responses
- Test log streaming with a mock stream
- Test YAML round-trip: get → edit → apply
- Test context switching updates the kube client
- Run `cargo test kubernetes::` — all pass ✅

### Final Verification — Phase 16
- [ ] `Ctrl+Shift+K` opens the Kubernetes Dashboard; pod list populates from the current kubeconfig context
- [ ] Arrow keys navigate pods; `l` opens a streaming log view
- [ ] `e` on a running pod opens a terminal tab exec'd into the container
- [ ] `y` opens the pod YAML in an editor tab; saving it applies the change to the cluster
- [ ] Context switcher changes the active cluster; resource list refreshes
- [ ] Namespace filter scopes the resource list
- [ ] Events tab shows recent cluster events with Warning/Normal distinction
- [ ] All `cargo test kubernetes::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 17 — Integrated Documentation Viewer

### Goal
Doc panel, hover-to-doc, man page viewer, `--help` capture, Rust doc integration, cheat sheet panel, search across docs, offline cache.

### Steps

**Step 17.1 — Scaffold `src/docs/` module**
- Create: `mod.rs`, `renderer.rs`, `manpage.rs`, `cache.rs`, `search.rs`, `rustdoc.rs`
- Define: `DocContent { Markdown(String), ManPage(ManDoc) }`, `DocEntry`, `DocCache`
- Run `cargo build` ✅

**Step 17.2 — Markdown renderer (`src/docs/renderer.rs`)**
- `fn render_markdown(md: &str) -> Vec<Line>` — convert Markdown to ratatui `Line` structs
- Support: headers (bold + color), bold, italic, inline code (styled), code blocks (syntax highlighted via tree-sitter), lists (bullet + numbered), tables (ratatui `Table`)
- Unit tests: each element type renders correctly ✅
- Run `cargo build` ✅

**Step 17.3 — Man page viewer (`src/docs/manpage.rs`)**
- `fn fetch_man_page(topic: &str) -> Result<String>` — run `man -P cat <topic>` and capture output
- `fn parse_man_output(raw: &str) -> ManDoc` — parse bold (`**text**`), underline, section headers
- `fn render_man_doc(doc: &ManDoc) -> Vec<Line>`
- Unit test: parse sample man page output, assert section structure ✅
- Run `cargo build` ✅

**Step 17.4 — Offline cache (`src/docs/cache.rs`)**
- Cache doc entries to `~/.ratterm/doc_cache/` keyed by topic hash
- `fn get_cached(topic: &str) -> Option<DocContent>`
- `fn store_cached(topic: &str, content: DocContent)`
- Configurable max cache size (default 100MB); LRU eviction
- Unit test: store, retrieve, evict ✅
- Run `cargo build` ✅

**Step 17.5 — Rust doc integration (`src/docs/rustdoc.rs`)**
- Run `cargo doc --document-private-items --no-deps` in background
- Parse generated HTML from `target/doc/` to extract function/struct docs
- `fn lookup_symbol(symbol: &str) -> Option<DocContent>`
- Unit test with a small fixture crate ✅
- Run `cargo build` ✅

**Step 17.6 — Doc panel state in `App`**
- `doc_panel: Option<DocPanel>` with: content, scroll position, search query
- Run `cargo build` ✅

**Step 17.7 — Doc panel render + input**
- `Ctrl+Q` or double-`K` (Vim) opens doc panel with hover content expanded
- `Ctrl+Shift+M` / `:man <topic>` opens man page
- Scrollable panel rendering `Vec<Line>` from the renderer
- `q` / `Esc` closes panel
- Links in doc content: pressing `Enter` on a link opens it in the system browser
- Run `cargo build` ✅

**Step 17.8 — `--help` capture**
- `:help <command>` runs `<command> --help`, captures output, renders in doc panel
- Run `cargo build` ✅

**Step 17.9 — Cheat sheet panel**
- `Ctrl+Shift+?` opens a context-aware keybinding reference
- Content changes based on current focus: editor mode, terminal, file browser, Git dashboard, etc.
- Run `cargo build` ✅

**Step 17.10 — Search across docs**
- Search input in doc panel searches all cached doc entries
- Results show topic + section + snippet
- Unit test: index 10 entries, search for a keyword, assert correct result ✅
- Run `cargo build` ✅

**Step 17.11 — Tests**
- Test markdown rendering: header, bold, code block, table
- Test man page parsing: section headers, bold spans
- Test cache LRU eviction
- Test rustdoc symbol lookup
- Test `--help` capture and render
- Run `cargo test docs::` — all pass ✅

### Final Verification — Phase 17
- [ ] In a Rust file with LSP hover showing a brief summary, pressing `Ctrl+Q` expands to the full rustdoc
- [ ] `Ctrl+Shift+M` then typing `ls` opens the `ls` man page, rendered with sections and bold text
- [ ] `:help cargo` shows `cargo --help` output in the doc panel
- [ ] `Ctrl+Shift+?` in editor mode shows editor keybindings; switching to terminal mode updates the cheat sheet
- [ ] Searching within the doc panel finds entries across cached topics
- [ ] All `cargo test docs::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 18 — Performance Profiler & System Monitor

### Goal
Real-time system monitor panel, process tree, application-internal performance overlay, slow frame detection, benchmark mode.

### Dependencies to add
```toml
sysinfo = "0.31"
```

### Steps

**Step 18.1 — Scaffold `src/monitor/` module**
- Create: `mod.rs`, `system.rs`, `profiler.rs`, `overlay.rs`, `benchmark.rs`
- Define: `SystemMetrics`, `CpuCore`, `ProcessInfo`, `FrameMetrics`, `BenchmarkResult`
- Run `cargo build` ✅

**Step 18.2 — System metrics collection (`src/monitor/system.rs`)**
- `fn collect_metrics() -> SystemMetrics` using `sysinfo`
- Collect: per-core CPU%, total/used/available RAM, disk I/O (read/write bytes/sec), network I/O, top 20 processes by CPU
- Run in a background task; push to App via channel every 1 second
- Unit test: call once, assert all fields populated ✅
- Run `cargo build` ✅

**Step 18.3 — System monitor panel render**
- CPU sparklines: one per core (last 60 samples)
- RAM bar + percentage
- Disk I/O and network I/O as sparklines
- Process list: PID, name, CPU%, MEM%
- Toggle with `Ctrl+Shift+P` (performance)
- Run `cargo build` ✅

**Step 18.4 — Application profiler (`src/monitor/profiler.rs`)**
- Instrument the render loop: record frame start/end time → `FrameMetrics`
- Instrument event processing: record event receive → dispatch → render time
- Store last 300 frame metrics (ring buffer)
- `fn avg_frame_time(metrics: &[FrameMetrics]) -> Duration`
- `fn p99_frame_time(metrics: &[FrameMetrics]) -> Duration`
- Unit tests: feed 300 fake metrics, assert avg and p99 ✅
- Run `cargo build` ✅

**Step 18.5 — Performance overlay (`src/monitor/overlay.rs`)**
- Toggleable overlay (top-right corner) showing: FPS, frame time (ms), p99 frame time, event queue depth, memory usage
- `Ctrl+Shift+O` (overlay) toggle
- Render as a small semi-transparent panel on top of everything
- Run `cargo build` ✅

**Step 18.6 — Slow frame detection**
- If any frame takes > 16ms (configurable threshold), log a warning to `~/.ratterm/perf.log` with: timestamp, frame time, active component
- Run `cargo build` ✅

**Step 18.7 — Benchmark mode (`src/monitor/benchmark.rs`)**
- `rat --benchmark` runs:
  1. Terminal throughput: `cat /dev/urandom | head -c 10MB`, measure bytes/sec rendered
  2. Startup time: measure from `main()` to first render
  3. Memory baseline: measure RSS after startup with no files open
  4. Large file: open a 100k-line file, measure time to first render and scroll FPS
- Output results as JSON to stdout
- Unit test: run benchmark function with mock data, assert JSON schema ✅
- Run `cargo build` ✅

**Step 18.8 — Tests**
- Test metric collection returns valid ranges (CPU 0-100, RAM > 0)
- Test frame metric ring buffer (oldest evicted at 300+1)
- Test slow frame log written on simulated slow frame
- Test benchmark JSON output schema
- Run `cargo test monitor::` — all pass ✅

### Final Verification — Phase 18
- [ ] `Ctrl+Shift+P` opens the system monitor; CPU sparklines animate in real time
- [ ] Process list shows top processes ordered by CPU usage
- [ ] `Ctrl+Shift+O` toggles the performance overlay showing FPS and frame time
- [ ] Simulating a slow render (add `std::thread::sleep(20ms)` temporarily) causes a warning in `perf.log`
- [ ] `rat --benchmark` completes and outputs a valid JSON report
- [ ] All `cargo test monitor::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 19 — Enhanced Clipboard & Yank Ring

### Goal
50-entry clipboard history, full Vim registers, yank ring navigation, cross-instance clipboard sync, rich paste, paste-as-snippet.

### Steps

**Step 19.1 — Scaffold `src/clipboard/` module**
- Create: `mod.rs`, `history.rs`, `registers.rs`, `ring.rs`, `sync.rs`
- Define: `ClipboardEntry`, `Register`, `YankRing`
- Run `cargo build` ✅

**Step 19.2 — Clipboard history (`src/clipboard/history.rs`)**
- `ClipboardHistory`: deque of up to 50 `ClipboardEntry` items
- Persist to `~/.ratterm/clipboard_history.toml` on change; load at startup
- `fn push(entry)`, `fn get(index)`, `fn search(query) -> Vec<usize>`
- Unit tests: push 51 items → oldest evicted; search finds correct index ✅
- Run `cargo build` ✅

**Step 19.3 — Clipboard history UI**
- `Ctrl+Shift+V` opens clipboard history popup
- Scrollable list with previews (first 80 chars per entry)
- Search bar at top; fuzzy match on content
- Enter → paste selected entry, replacing current clipboard
- Run `cargo build` ✅

**Step 19.4 — Vim registers (`src/clipboard/registers.rs`)**
- `RegisterStore`: HashMap of `char → String` for `"a`–`"z`
- Special registers: `"0` (last yank), `"1`–`"9` (delete ring), `""` (default), `"_` (black hole), `"+` (system), `"*` (selection)
- `fn read(name: char) -> Option<String>`, `fn write(name: char, content: String)`
- Persist named registers (`"a`–`"z`) to `~/.ratterm/registers.toml`
- Unit tests: write/read named, write to delete ring evicts `"9`, black hole discards ✅
- Run `cargo build` ✅

**Step 19.5 — Vim register input**
- `"<char>y` → yank to named register, `"<char>p` → paste from named register
- `"+y` / `"+p` → system clipboard, `"*y` / `"*p` → selection clipboard
- `"_d` → delete without saving to any register
- Run `cargo build` ✅

**Step 19.6 — Yank ring navigation (`src/clipboard/ring.rs`)**
- After pasting with `p`, pressing `Ctrl+Shift+Y` cycles to the previous yank ring entry
- Replaces the just-pasted text with the previous entry
- Ring wraps around
- Unit test: paste, cycle 3 times, assert each produces correct content ✅
- Run `cargo build` ✅

**Step 19.7 — Cross-instance sync (`src/clipboard/sync.rs`)**
- On yank: write to `~/.ratterm/clipboard_sync.json` (atomic write)
- On paste: read latest from `~/.ratterm/clipboard_sync.json` if newer than local
- Use file locking (`fs2` crate) for concurrent access safety
- Unit test: simulate two instances writing/reading ✅
- Run `cargo build` ✅

**Step 19.8 — Rich paste: indentation adjustment**
- On paste, detect the indentation level of the target line
- Adjust pasted content's base indentation to match
- `fn adjust_indentation(content: &str, target_indent: usize, tab_width: usize) -> String`
- Unit test: paste 2-space-indented code into 4-space-indented context ✅
- Run `cargo build` ✅

**Step 19.9 — Tests**
- Test clipboard history eviction at 50 entries
- Test all special register behaviors
- Test yank ring cycling
- Test cross-instance sync file format
- Test indentation adjustment
- Run `cargo test clipboard::` — all pass ✅

### Final Verification — Phase 19
- [ ] `Ctrl+Shift+V` opens clipboard history; the last 10 yanks/copies are listed
- [ ] Searching the history finds entries by content
- [ ] In Vim mode, `"ay` yanks to register `a`; `"ap` pastes from it
- [ ] `"+y` copies to the system clipboard (verify with an external app)
- [ ] After pasting, `Ctrl+Shift+Y` replaces the paste with the previous yank ring entry
- [ ] Yank in one Ratterm instance; open a second instance; paste — the yanked content appears
- [ ] Pasting indented code auto-adjusts to match the target indentation level
- [ ] All `cargo test clipboard::` tests pass
- [ ] `cargo build` zero warnings

---

## Phase 20 — Remote Development Mode

### Goal
Full remote workspace: remote file browser, remote terminal, remote LSP forwarding, remote search, file caching with sync, remote git, connection resilience, port forwarding.

### Steps

**Step 20.1 — `FileSystem` trait abstraction (`src/fs/mod.rs`)**
- Define trait:
  ```rust
  trait FileSystem: Send + Sync {
      fn read_file(&self, path: &Path) -> Result<Vec<u8>>;
      fn write_file(&self, path: &Path, data: &[u8]) -> Result<()>;
      fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
      fn create_dir(&self, path: &Path) -> Result<()>;
      fn delete(&self, path: &Path) -> Result<()>;
      fn rename(&self, from: &Path, to: &Path) -> Result<()>;
      fn exists(&self, path: &Path) -> bool;
      fn metadata(&self, path: &Path) -> Result<FileMeta>;
  }
  ```
- `LocalFileSystem` implementation (wraps `std::fs`)
- Unit tests for `LocalFileSystem` ✅
- Run `cargo build` ✅

**Step 20.2 — `RemoteFileSystem` via SFTP (`src/fs/remote.rs`)**
- Implement `FileSystem` trait using the existing SSH/SFTP infrastructure
- Operations are async internally; use `block_on` at the trait boundary or make the trait async
- `RemoteFileSystem::new(ssh_session: Arc<SshSession>, remote_root: PathBuf)`
- Unit tests with mock SFTP server ✅
- Run `cargo build` ✅

**Step 20.3 — File cache layer (`src/fs/cache.rs`)**
- `CachingFileSystem` wraps any `FileSystem` and caches reads to `~/.ratterm/remote-cache/<host-hash>/`
- On write: write to remote and update cache
- Detect remote modification: compare cached mtime vs remote mtime before open → prompt for conflict resolution
- `fn flush_pending_writes()` → apply any queued writes after reconnect
- Unit tests: cache hit, cache miss fetches remote, stale detection ✅
- Run `cargo build` ✅

**Step 20.4 — Remote workspace model (`src/remote/workspace.rs`)**
- `RemoteWorkspace { host, user, remote_root, fs: Arc<dyn FileSystem>, ssh_session }`
- CLI: `rat remote <user@host>:/path/to/project`
- Wire into `App` as `remote_workspace: Option<RemoteWorkspace>`
- Run `cargo build` ✅

**Step 20.5 — Remote file browser**
- File browser uses `app.remote_workspace.fs` when in remote mode (instead of `LocalFileSystem`)
- All file operations (open, rename, delete) route through `FileSystem` trait
- Status bar shows `[REMOTE: user@host]`
- Run `cargo build` ✅

**Step 20.6 — Remote terminal**
- New terminal tabs in remote mode auto-SSH to the remote host
- `cwd` is synced with remote workspace root
- Run `cargo build` ✅

**Step 20.7 — Remote LSP forwarding**
- In remote mode, start LSP server on the remote host via SSH command
- Forward LSP stdio over an SSH channel (using existing SSH multiplexing)
- Local LSP client connects to the forwarded stdio
- Run `cargo build` ✅

**Step 20.8 — Remote search**
- In remote mode, find-in-files runs `rg --json <pattern>` on the remote host via SSH
- Falls back to iterating remote files via SFTP if `rg` not found
- Run `cargo build` ✅

**Step 20.9 — Connection resilience**
- SSH disconnection → show `[OFFLINE]` indicator in status bar
- Queue all write operations in `pending_writes: Vec<PendingWrite>`
- Retry connection every `remote-reconnect-interval` seconds (config key, default 5s)
- On reconnect: flush pending writes, resume LSP, show `[RECONNECTED]` notification
- Unit test: simulate disconnect → queue 3 writes → reconnect → assert all 3 applied ✅
- Run `cargo build` ✅

**Step 20.10 — Port forwarding**
- Parse `remote-forwards` from `.ratterm-workspace.toml`:
  ```toml
  [[remote.forwards]]
  local = 3000
  remote = 3000
  ```
- Set up `tokio` TCP port forwards on workspace open
- Show active forwards in status bar; `Ctrl+Shift+W` manages them
- Run `cargo build` ✅

**Step 20.11 — Remote git**
- When `remote_workspace` is active, git API functions run their `git2` operations on the cached/synced repo
- Alternative: shell out to `git` on the remote host via SSH exec
- Run `cargo build` ✅

**Step 20.12 — Tests**
- Test `RemoteFileSystem` operations with mock SFTP server
- Test cache layer: hit, miss, stale detection, write-through
- Test disconnection handling and write queue
- Test port forward setup
- Test remote search with mock SSH exec returning `rg --json` output
- Run `cargo test remote::` — all pass ✅

### Final Verification — Phase 20
- [ ] `rat remote user@myhost:/home/user/myproject` opens a workspace where the file browser shows the remote directory
- [ ] Opening a file in the remote workspace reads it from SFTP and displays it in the editor
- [ ] Saving a file writes it back to the remote host
- [ ] New terminal tab auto-SSHs to the remote host
- [ ] `rust-analyzer` starts on the remote host; LSP features (hover, completion) work on remote code
- [ ] Find-in-files searches the remote project (using `rg` if available)
- [ ] Disconnecting the SSH session shows `[OFFLINE]`; saves are queued; reconnect flushes them
- [ ] Port forward `3000:3000` makes the remote web server accessible at `localhost:3000`
- [ ] All `cargo test remote::` tests pass
- [ ] `cargo build` zero warnings

---

## Cross-Phase Notes

- **Every phase must leave `cargo build` and `cargo test` fully green before being considered complete.**
- **No phase may introduce `todo!()`, `unimplemented!()`, or `#[allow(dead_code)]` stubs** — all code must be functional.
- **Test count targets:** Each phase should add a minimum of 15 new tests. The final test suite across all 20 phases should reach ≥ 600 tests.
- **Config keys** introduced by each phase must be documented in a top-level `CONFIG.md` kept up to date throughout.
- **Phases are independent** but the following dependencies are soft: Phase 3 (LSP) should precede Phase 15 (semantic tokens). Phase 7 (session manager) should precede Phase 8 (tiling multiplexer). Phase 1 (git) may be referenced by Phase 20 (remote).
