# ratterm-mcp

An MCP server that launches, sees and drives ratterm.

ratterm already exposes a control API: newline-delimited JSON over a named pipe
on Windows, a Unix domain socket elsewhere, and a loopback TCP port on every
platform. `docs/automation.md` in the repository root is the contract. This
package wraps that API in MCP tools so an agent can start an instance, read the
rendered frame, press keys, and run the scenario suite without a terminal.

## Install

```sh
cd tools/ratterm-mcp
python -m venv .venv
.venv/bin/pip install -e ".[dev]"          # Windows: .venv\Scripts\pip
```

Python 3.11 or later. One runtime dependency, `mcp` 2.x. The 2.x line renamed
`FastMCP` to `MCPServer`; `server.py` uses the new name, so `mcp<2` will not
work.

## Register it

```json
{
  "mcpServers": {
    "ratterm": {
      "command": "/path/to/tools/ratterm-mcp/.venv/bin/ratterm-mcp"
    }
  }
}
```

The entry point runs on stdio.

## Tools

| Tool | Arguments | What it does |
|---|---|---|
| `launch` | `binary`, `args`, `fixtures`, `endpoint` | Starts `rat --headless` on `endpoint`, waits for it to answer, renders one frame so the next call is not cold, and returns the endpoint, the token path, the pid and the log path |
| `snapshot` | `cells` | `app.snapshot`. Returns the frame as one string under `text`, plus `lines`, `width`, `height` and `cursor`; `cells` adds every cell with its colours and modifiers |
| `send_key` | `key` | `app.send_key` |
| `type_text` | `text` | `app.type` |
| `send_mouse` | `event` | `app.send_mouse`, `"<kind>@<col>,<row>"` |
| `state` | none | `app.state`: mode, size, focus, tab count, status |
| `api` | `method`, `params` | Any control-API method, for what the typed tools do not cover |
| `run_scenario` | `path`, `results_dir` | Runs `rat --scenario` (a file) or `--scenario-dir` (a directory) and returns the report and the exit code |
| `stop` | none | Asks the instance to quit, then terminates it if it is still there |

Key names are the ones `.ratrc` and the scenario files use: `ctrl+q`,
`alt+shift+right`, `F2`, `enter`, `esc`, `space`, or a single character.

One instance at a time. `launch` refuses while one is running; call `stop`
first.

`run_scenario` reuses the binary and the fixture directory from the last
`launch`, so a scenario reads the same fleet the running instance does. With no
prior launch it runs `rat` from `PATH` with no fixtures.

## Transports

`launch` defaults to loopback TCP on a free port. That is the transport that
crosses machines: `ssh -N -L 47113:127.0.0.1:47113 host` forwards it unchanged,
while forwarding a Unix socket depends on the OpenSSH version at both ends and
a named pipe does not forward at all.

The other two work too. Pass `endpoint` as a Windows pipe name
(`\\.\pipe\ratterm-1`) or a Unix socket path (`/tmp/ratterm-1.sock`) and the
client speaks that instead. A bare port, `127.0.0.1:47119` and
`tcp://127.0.0.1:47119` all mean loopback TCP; a non-loopback address is
refused before the process starts, because the instance would refuse to bind it
anyway.

## Authentication

Every ratterm run mints a 32-byte token and writes it to `~/.ratterm/api.token`
(`0600` on Unix). The first message on every connection is

```json
{"id":"1","method":"session.authenticate","params":{"token":"<hex>"}}
```

and until it succeeds the instance answers everything else with `-32001`.

The client reads the token file *after* the endpoint accepts the connection,
never before. An instance publishes its token and then binds its listener, so a
token read before connecting can be the previous run's, and presenting that
produces a `-32001` that looks like a real authentication failure. A `-32001`
is reported as a wrong or stale token, naming the file; it is not retried,
because a wrong token does not become right by waiting.

## Driving an instance this server did not start

Set `RATTERM_MCP_ENDPOINT` to a port, address, pipe name or socket path, and
the tools other than `launch` connect there. `RATTERM_MCP_TOKEN_FILE` says
where that instance's token is, defaulting to `~/.ratterm/api.token`. For an
instance on another machine, forward the port and copy the token file:

```sh
ssh -N -L 47113:127.0.0.1:47113 cthulhu-computer &
scp cthulhu-computer:.ratterm/api.token /tmp/remote.token
export RATTERM_MCP_ENDPOINT=47113
export RATTERM_MCP_TOKEN_FILE=/tmp/remote.token
```

## Timeouts

Every read takes a deadline. A dead instance produces an error, never a stuck
tool call. The named-pipe transport has no read timeout of its own, so it reads
on a daemon thread and the deadline is applied to the queue that thread feeds.

The instance answers from its own event loop and gives up after five seconds,
returning error `-32000`. For the first seconds after startup that loop is
attaching its terminal, and a frame requested then can pass the five seconds —
in a debug build the first frame has taken over ten. `launch` absorbs that by
rendering one frame before it returns, and reports how long it took as
`first_frame_seconds`. `snapshot` retries a `-32000` twice more; any other
error is reported as it is, because the instance already acted on the request.

## Tests

```sh
cd tools/ratterm-mcp
.venv/bin/python -m pytest          # Windows: .venv\Scripts\python -m pytest
```

The suite needs no ratterm binary and no fleet. `tests/conftest.py` stands up a
loopback server that speaks newline-delimited JSON, with a behaviour switch for
each failure the client has to handle: a refused token, silence, a reply that
is not JSON, a connection dropped mid-call, and a first frame that times out.

Lint and format:

```sh
.venv/bin/ruff check .
.venv/bin/ruff format --check .
```

## The tmux fallback, and why this exists instead

The other way to drive a terminal program from a script is to run it inside
`tmux` and use `tmux send-keys` to type and `tmux capture-pane` to read the
screen back. It works, and for a program with no control API it is the only
option, but it is worse here in three ways:

- `capture-pane` returns text. Colour and emphasis are gone unless you pass
  `-e` and then parse the escape sequences yourself, so "is this row
  highlighted" and "is the status bar red" are not answerable. `app.snapshot`
  returns every cell with its foreground colour, background colour and
  modifiers.
- The cursor position is not in the captured text at all, and neither is the
  application's own idea of its state. `app.state` reports the mode, the
  focused pane, the tab count and the status line directly.
- There is no tmux on Windows. The scenario suite and this server run the same
  way on Windows, Linux and macOS, which is the property the project needs.

It is also slower: every read is a subprocess. Nothing here needs it.
