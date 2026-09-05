# Testing

How ratterm is tested, and how to run each part locally.

There are five layers, from cheapest to slowest: unit tests next to the code,
integration tests in `tests/`, scenarios that drive the rendered interface,
a headless instance driven over the control API, and a ConPTY harness that
spawns the real binary. CI runs the first four on Windows, Linux and macOS; the
fifth is Windows-only and ignored by default.

## Quick start

```bash
cargo test --all-features                                   # unit + integration
cargo test --test scenario_suite_tests                      # the scenario suite
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
```

## Unit tests

Every module keeps its tests in a `#[cfg(test)]` block beside the code it
tests, so a private function can be tested without being made public.

```bash
cargo test                       # everything
cargo test editor::buffer        # one module
cargo test -- --nocapture        # with output
```

## Integration tests

`tests/` holds the tests that use the crate through its public API.

| File | What it covers |
|---|---|
| `api_auth_tests.rs` | The control-API token handshake, end to end over a real endpoint |
| `completion_tests.rs` | Editor completion |
| `daemon_tests.rs` | Daemon metric collection and storage |
| `editor_buffer_tests.rs` | Text buffer operations |
| `editor_tab_state_tests.rs` | Per-tab editor state, kept separate across tab switches |
| `health_dashboard_tests.rs` | Opening, closing and populating the health dashboard |
| `lsp_integration_tests.rs` | LSP request and response handling |
| `portability_tests.rs` | Platform gates and the CI matrix, read from the sources and the workflow |
| `scenario_suite_tests.rs` | Runs every scenario in `tests/scenarios/` |
| `terminal_grid_tests.rs` | The terminal cell buffer |
| `terminal_parser_tests.rs` | ANSI and VT100 escape sequence parsing |
| `terminal_pty_lifecycle_tests.rs` | PTY teardown, including the paths that used to leak |
| `terminal_pty_tests.rs` | PTY spawning and management |
| `ui_tests.rs` | Widget rendering |
| `expectrl_*.rs` | The ConPTY harness. Windows only, and ignored by default — see below |

PTY tests spawn a real shell and are slow on Windows. To skip them while
working on something else:

```bash
cargo test -- --skip pty
```

## Scenarios

A scenario is a list of steps in YAML or JSON that drives the rendered
interface and asserts on what is on screen. The same file runs unchanged on
Windows, Linux and macOS, which is the cross-platform check this project needs:
a popup that draws on one platform and not another fails here rather than in
someone's terminal.

```yaml
name: ssh manager lists hosts
steps:
  - key: F2
  - expect_text: "SSH Manager"
  - expect_text: "fixture-gpu"
  - snapshot: open
  - key: esc
  - expect_not_text: "fixture-gpu"
  - expect_status: "closed"
```

The steps are `key`, `type`, `mouse`, `expect_text`, `expect_not_text`,
`expect_status`, `snapshot`, `resize`, `open_file`, `wait_ms`, `tick` and
`note`. `docs/automation.md` documents each one.

Ten scenarios live in `tests/scenarios/`, covering startup, the SSH manager,
the Docker manager, the health dashboard, the command palette, editor tabs,
opening a file, resizing, the status and hint bars, and the unsaved-work quit
guard.

Run them from the command line:

```bash
cargo build
./target/debug/rat --scenario tests/scenarios/02-ssh-manager.yaml \
                   --fixtures tests/fixtures/fleet
./target/debug/rat --scenario-dir tests/scenarios \
                   --fixtures tests/fixtures/fleet \
                   --results-dir test-results --no-update
```

The exit code is 0 when every scenario passes and 1 otherwise, so CI uses it
directly with no extra assertion. Snapshots are written to `test-results/` as
text and as JSON, alongside `test-results/scenarios.json` summarising the run.
`--results-dir` moves that elsewhere; the default is `test-results`.

Two rules keep a scenario from passing for the wrong reason: a misspelled step
name fails the run rather than being skipped, and a scenario with no assertions
at all is reported with a warning.

Scenarios enable the F1–F4 test keys by default (`test_keys: false` opts out).
The real command-palette shortcut differs between Windows 11 and every other
platform, so a scenario written against it would not be the same test
everywhere.

`tests/scenario_suite_tests.rs` runs the whole directory from `cargo test`, so
an interface regression fails the ordinary test command and not only a separate
CI job:

```bash
cargo test --test scenario_suite_tests
```

## Fixtures

```bash
rat --fixtures tests/fixtures/fleet
```

A fixture directory stands in for `~/.ratterm`, so a run sees the same fleet on
every machine.

| File | Contents |
|---|---|
| `ssh_hosts.toml` | Hosts and credentials, in the real format |
| `docker_items.toml` | Quick-connect slots and the selected Docker host |
| `metrics.json` | One entry per host, seeding the health dashboard |

All three are optional; the shipped fleet has the first and the third. It is
four hosts — `fixture-gpu`, `fixture-rock5c`, `fixture-pi` and
`fixture-behind-jump`, the last behind a jump host.

**A fixture run cannot reach a real machine.** Three things enforce that:

- While fixtures are active the application neither reads nor writes the user's
  real configuration.
- Loading fixtures clears the shared remote executor's target table
  (`Fixtures::isolate_remote_access` in `src/fixtures.rs`), so every host id
  becomes unknown to the executor and a remote call fails immediately rather
  than dialling out.
- The addresses are in the RFC 5737 documentation range (192.0.2.0/24), which
  resolves to nothing, and the passwords are obvious placeholders.

## Headless mode and the control API

```bash
rat --headless --api-tcp 47119 --fixtures tests/fixtures/fleet --no-update
```

The application runs its normal event loop with no terminal attached. Nothing
is drawn until something asks for a snapshot, so an idle instance costs
nothing. `--headless 120x40` sets the frame size; the default is 120x40.

A client drives it over the control API: newline-delimited JSON on a named pipe
(Windows), a Unix domain socket (elsewhere), or a loopback TCP port (both).
`app.snapshot` returns the frame as text and, when asked, every cell with its
colours and modifiers; `app.send_key`, `app.type` and `app.send_mouse` drive
it; `app.state` reports the mode, size, focus, tab count and status line.

Authentication is on by default. Each run mints a 32-byte token, writes it to
`~/.ratterm/api.token` (`0600` on Unix), and answers everything but
`session.authenticate` and `system.ping` with `-32001` until the token is
presented. `--api-no-auth` turns that off for a local trusted run; it does not
apply to `--api-tcp`, which always requires a token because a loopback port is
visible to every process on the machine.

Two details matter when writing a client. The token file must be read *after*
the endpoint accepts the connection — an instance publishes its token and then
binds its listener, so a token read earlier can be the previous run's. And the
instance answers from its own event loop and gives up after five seconds with
error `-32000`; for the first seconds after startup that loop is attaching its
terminal, and the first frame can pass five seconds in a debug build, so a
client should render one frame and tolerate a timeout before it starts
asserting.

`docs/automation.md` is the full contract.

## The MCP server

`tools/ratterm-mcp/` is a Python MCP server that wraps the control API as
agent tools: `launch`, `snapshot`, `send_key`, `type_text`, `send_mouse`,
`state`, `api`, `run_scenario` and `stop`. It speaks all three transports and
defaults to loopback TCP on a free port, which is the one that forwards over
`ssh -L`.

```bash
cd tools/ratterm-mcp
python -m venv .venv
.venv/bin/pip install -e ".[dev]"      # Windows: .venv\Scripts\pip
.venv/bin/python -m pytest             # Windows: .venv\Scripts\python -m pytest
```

Its own suite needs no ratterm binary: a fake newline-JSON server on a loopback
socket covers the handshake, a successful call, a refused token, a timeout, a
reply that is not JSON, a connection dropped mid-call, and a first frame that
times out. `tools/ratterm-mcp/README.md` has the rest, including how to point
the tools at an instance on another machine.

## The ConPTY harness

`tests/expectrl_*.rs` spawn the compiled binary in a real pseudo-console and
assert on the bytes it writes. This is the only layer that exercises the actual
terminal output path, escape sequences included.

It is **Windows only and ignored by default**. Two gates do that:

`tests/helpers/mod.rs`:

```rust
#[cfg(windows)]
pub mod tui_harness;
```

and the first line of every one of the test files, for example
`tests/expectrl_smoke_tests.rs`:

```rust
#![cfg(windows)]
```

`Cargo.toml` matches, with the ConPTY crate as a Windows-only dev dependency:

```toml
[target.'cfg(windows)'.dev-dependencies]
conpty = "0.5"
```

The harness itself uses `conpty` rather than `expectrl`, despite the file
names. ConPTY needs its output pipe drained continuously on a background
thread; `expectrl`'s single-shot reads on the main thread fill the pipe buffer
and deadlock the child. `tests/helpers/tui_harness.rs` runs a reader thread
into a shared buffer for exactly that reason.

Any test that spawns the binary is also marked `#[ignore]`, because it needs
`cargo build --release` first. To run them:

```bash
cargo build --release
cargo test --test expectrl_smoke_tests -- --ignored
```

`expectrl_status_check_tests.rs` is the exception worth knowing about: most of
it is pure assertions on screen-scraping helpers and runs unignored, while
three tests that need network access and saved hosts are ignored.

Everything the harness checks on Windows, the scenario runner checks on all
three platforms, without a PTY. Prefer a scenario for new interface tests.

## What CI runs

`.github/workflows/ci.yml`, on every push to any branch and on pull requests to
`main` and `master`.

| Job | Platforms | What it does |
|---|---|---|
| `fmt` | ubuntu | `cargo fmt --all -- --check` |
| `clippy` | ubuntu | `cargo clippy --all-targets --all-features -- -D warnings` |
| `test` | ubuntu, windows, macos, ubuntu ARM | `cargo test --all-features`, doc tests, a release build, and `rat --verify` |
| `scenario` | ubuntu, windows, macos | `rat --scenario-dir tests/scenarios --fixtures tests/fixtures/fleet` |
| `headless-smoke` | ubuntu, windows, macos | Starts a headless instance, takes a snapshot over the control API, checks the fixture fleet is on screen |
| `install-script` | ubuntu, windows, macos | Syntax and a dry run of the install scripts |
| `docs` | ubuntu | `cargo doc --no-deps --all-features` with `-D warnings` |
| `audit` | ubuntu | `cargo audit`, advisory only |
| `msrv` | ubuntu | `cargo check --all-features` on Rust 1.89 |

The MSRV is 1.89 because `keyring` 4 needs 1.88 and `kube` 4 needs 1.89.
`rust-version` in `Cargo.toml` and the toolchain in the `msrv` job say the same
thing; keep them together.

The `scenario` job uploads `test-results/` as `scenario-results-<os>` whether it
passed or failed. A failing scenario writes the snapshot showing what was on
screen, which is when it is worth reading.

`scenario` and `headless-smoke` build with `cargo build` rather than
`--release`: the release profile uses fat LTO and a single codegen unit, which
costs far more time than it saves in a run that renders frames and compares
text.

`headless-smoke` is the check that the control API works on each platform
rather than only compiling there. It starts an instance with `--test-keys`,
presses F2 to open the SSH manager, and asserts the frame is not empty and
contains `fixture-gpu`. The startup frame shows the terminal pane and no host
names, which is why it presses a key first.

## Docker

`docker/docker-compose.yml` reproduces the Linux jobs locally.

**Windows:**

```powershell
.\scripts\test-local.ps1            # fmt, clippy, test, docs, audit, msrv
.\scripts\test-local.ps1 clippy     # one service
.\scripts\test-local.ps1 clean
```

**Linux and macOS:**

```bash
./scripts/test-local.sh
./scripts/test-local.sh clippy
./scripts/test-local.sh clean
```

Services: `fmt`, `clippy`, `test`, `docs`, `audit`, `msrv`, `ci-all`,
`install-test`, `lua-test`, plus `test-arm`, `lua-test-arm`, `ci-all-arm` for
ARM64 and `windows-test`. Build artifacts are cached in the `cargo-cache`,
`target-cache` and `target-cache-msrv` volumes.

The Docker images are pinned to `rust:1.85-bookworm`, which is below the
crate's 1.89 minimum. The `msrv` service therefore no longer matches the
`msrv` CI job, and the other services will fail to build until the images are
raised.

## Install scripts

```powershell
.\scripts\test-install.ps1 syntax
.\scripts\test-install.ps1 dry-run
.\scripts\test-install.ps1 full      # installs into a temp directory
```

```bash
./scripts/test-install.sh
./scripts/test-install.sh linux-x64
```

## Writing tests

- Name a test after the behaviour it pins, not the function it calls:
  `an_unauthenticated_request_is_refused_with_the_auth_error_code`, not
  `test_auth_2`.
- Tests must not depend on each other or on the developer's machine. Use
  `tempfile` for paths and `tests/fixtures/fleet` for a fleet.
- Cover the expected path, at least one edge case, and at least one failure.
- Loops in tests, as in the rest of the crate, need a fixed upper bound.
- Reach for a scenario before a ConPTY test. It runs on three platforms
  instead of one, in milliseconds instead of seconds, and it is a data file
  rather than code.

`proptest` is available for properties over generated input:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn a_buffer_round_trips_its_content(text in "\\PC*") {
        let buffer = Buffer::from(&text);
        prop_assert_eq!(buffer.to_string(), text);
    }
}
```

## Troubleshooting

**PTY tests hang on Windows.** Shell initialisation is slow there. Skip them
with `cargo test -- --skip pty`, or run the Docker `test` service instead.

**A scenario fails only in CI.** Download the `scenario-results-<os>` artifact
from the run; it holds the snapshot of the frame at the failing step.

**The control API answers `-32001`.** The token is wrong or stale. Every run
mints a new one, so re-read `~/.ratterm/api.token` from the instance you are
talking to. A previous run that was killed rather than quitting leaves its
token file behind.

**The control API answers `-32000`.** The instance did not answer within its
own five-second window. Take another snapshot; the first one after startup is
the slow one.

**`cargo audit` fails to install.** `cargo install cargo-audit --locked`.
