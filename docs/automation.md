# Driving ratterm from a program

Everything in this document works the same on Windows, Linux and macOS, with
no terminal attached.

## Why this exists

The control API could already drive the PTY and the editor buffer, but it
could not see the interface. Popups, dashboards, the status bar and the key
hint bar were invisible to it, and there was no way to send a key to the
application rather than to the shell. An agent could type into a terminal and
read the bytes back, and nothing else.

Three things close that: a snapshot of the rendered frame, key and mouse
injection, and a headless mode that needs no PTY.

## Headless mode

```sh
rat --headless 120x40 --no-update
rat --headless --api-tcp 47113          # control endpoint on loopback TCP
rat --headless --api-socket /tmp/ratterm-1.sock
```

The application runs its normal event loop with no terminal. Nothing is drawn
until something asks for a snapshot, so an idle instance costs nothing. Useful
under `systemd`, in a container, or over SSH.

`--api-socket` picks the endpoint per instance, so several instances can run at
once. `--api-tcp` is the one that crosses machines: forwarding a remote Unix
socket with `ssh -L /local.sock:/remote.sock` depends on the OpenSSH version at
both ends and is unreliable from a Windows client, while a loopback TCP port
forwards with plain

```sh
ssh -N -L 47113:127.0.0.1:47113 cthulhu-computer
```

The TCP endpoint refuses any address that is not loopback, and refuses to start
without a token.

## Authentication

Every run mints a 32-byte token and writes it to `~/.ratterm/api.token`
(`0600` on Unix). The first message on a connection must be:

```json
{"id":"1","method":"session.authenticate","params":{"token":"<hex from the file>"}}
```

Until that succeeds only `system.ping` is answered; everything else returns
error code `-32001`. The token file is removed when the instance shuts down.

`--api-no-auth` turns the requirement off for a local, trusted run. It does not
apply to `--api-tcp`, which always requires a token because a loopback port is
visible to every process on the machine.

## Seeing the interface

`app.snapshot` renders the current frame into an off-screen buffer and returns
it:

```json
{"id":"2","method":"app.snapshot","params":{"width":120,"height":40,"cells":false}}
```

```json
{
  "width": 120,
  "height": 40,
  "lines": ["┌ No terminal ──…", "…"],
  "cursor": [12, 4],
  "cells": []
}
```

`lines` is the frame as text, one string per row with trailing spaces trimmed —
enough for most assertions. Pass `"cells": true` to also get every cell with
its foreground colour, background colour and modifiers, which is how you check
focus and highlighting. It is off by default because a 120x40 frame is 4,800
cells.

## Driving the interface

| Method | Parameters | Effect |
|---|---|---|
| `app.send_key` | `{"key": "ctrl+shift+u"}` or `{"code": "F2", "modifiers": ["ctrl"]}` | One key into `App::handle_key` |
| `app.send_mouse` | `{"event": "left_down@10,4"}` | One mouse event |
| `app.type` | `{"text": "hello"}` | One key per character |
| `app.state` | none | Mode, size, tab count, focus, status |
| `hosts.list` | none | Hosts with reachability and capabilities |
| `hosts.summary` | none | Fleet counts for a header line |

Key names are the same ones `.ratrc` uses: `ctrl+q`, `alt+shift+right`, `F2`,
`enter`, `esc`, `space`, or a single character.

## Scenarios

A scenario is a list of steps in YAML or JSON. The same file runs unchanged on
every platform, which is the cross-platform check this project needs.

```yaml
name: ssh manager lists hosts
width: 120
height: 40
steps:
  - key: F2
  - expect_text: "SSH Manager"
  - expect_text: "fixture-gpu"
  - snapshot: open
  - key: esc
  - expect_not_text: "fixture-gpu"
  - expect_status: "closed"
```

Run one, or a directory:

```sh
rat --scenario tests/scenarios/02-ssh-manager.yaml --fixtures tests/fixtures/fleet
rat --scenario-dir tests/scenarios --fixtures tests/fixtures/fleet
```

The exit code is 0 when every scenario passes and 1 otherwise, so CI can use it
directly. Snapshots are written to `test-results/` as text and as JSON, along
with `test-results/scenarios.json` summarising the run.

### Steps

| Step | Meaning |
|---|---|
| `key: <description>` | Press a key |
| `type: <text>` | Type a string |
| `mouse: <kind>@<col>,<row>` | Send a mouse event |
| `expect_text: <text>` | The frame contains this |
| `expect_not_text: <text>` | The frame does not contain this |
| `expect_status: <text>` | The status bar contains this |
| `snapshot: <name>` | Write a snapshot |
| `resize: <W>x<H>` | Change the frame size |
| `open_file: <path>` | Open a file in the editor |
| `wait_ms: <n>` | Sleep |
| `tick` | One iteration of the update loop |
| `note: <text>` | A comment |

A misspelled step name fails the run rather than being skipped: a silently
skipped assertion is a test that passes for the wrong reason. A scenario with
no assertions at all is reported with a warning for the same reason.

Scenarios enable the F1–F4 and F6 test keys by default (`test_keys: false` opts
out; F5 is the debugger's, so it is not one of them).
The real shortcut for the command palette differs between Windows 11 and every
other platform, so a scenario written against it would not be the same test
everywhere.

## Fixtures

```sh
rat --fixtures tests/fixtures/fleet
```

A fixture directory stands in for `~/.ratterm`:

| File | Contents |
|---|---|
| `ssh_hosts.toml` | Hosts and credentials, in the real format |
| `docker_items.toml` | Quick-connect slots and the selected Docker host |
| `metrics.json` | One entry per host, seeding the health dashboard |
| `kubeconfig` | Contexts for the Kubernetes screens |

All four are optional. While fixtures are active the application neither reads
nor writes the user's real configuration, and the shared remote executor is
cleared, so a run cannot reach a real machine: a remote call fails immediately
with "unknown host" instead of dialling out. The shipped fleet uses addresses
from the RFC 5737 documentation range, which resolve to nothing.

That extends to Kubernetes: with fixtures active the screens read
`<fixtures>/kubeconfig` or nothing at all, never `~/.kube/config`. A fixture
directory with no kubeconfig produces a screen saying so, which is the correct
result for a run that was not given one.

## Running the scenarios from `cargo test`

`tests/scenario_suite_tests.rs` runs every scenario in `tests/scenarios/`, so
an interface regression fails the ordinary test command rather than only a
separate CI job.

```sh
cargo test --test scenario_suite_tests
```
