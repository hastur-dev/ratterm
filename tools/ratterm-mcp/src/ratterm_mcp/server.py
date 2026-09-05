"""An MCP server that launches, sees and drives ratterm.

The tools here map onto the control API described in ``docs/automation.md``:
``launch`` starts ``rat --headless`` and waits for it to answer, ``snapshot``
renders the frame, ``send_key`` / ``type_text`` / ``send_mouse`` drive it,
``state`` reports what it thinks it is showing, ``api`` reaches anything the
typed tools do not cover, ``run_scenario`` runs the scenario runner, and
``stop`` shuts the instance down.

One instance at a time. The process, the endpoint and the authenticated
connection live in :data:`SESSION`, so the tools after ``launch`` do not have
to be told where to look.

Two environment variables let the tools reach an instance this server did not
start, which is what an endpoint forwarded with ``ssh -L`` needs:

``RATTERM_MCP_ENDPOINT``
    A port, address, pipe name or socket path to connect to.
``RATTERM_MCP_TOKEN_FILE``
    Where that instance's token was copied to. Defaults to
    ``~/.ratterm/api.token``.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from mcp.server.mcpserver import MCPServer

from ratterm_mcp.client import (
    ApiError,
    ConnectionLost,
    Endpoint,
    MethodError,
    RattermClient,
    default_endpoint,
    default_token_path,
    parse_endpoint,
    wait_until_ready,
)

__all__ = ["SESSION", "Session", "ToolError", "main", "mcp", "scenario_argv"]

# How long a launch waits for the endpoint to answer.
LAUNCH_TIMEOUT_SECONDS = 30.0

# How long a scenario run may take before it is killed.
SCENARIO_TIMEOUT_SECONDS = 600.0

# How long to wait for a stopped instance to exit before killing it.
STOP_GRACE_SECONDS = 5.0

# How much of the instance log to quote when something goes wrong.
LOG_TAIL_BYTES = 4000

# The instance's own dispatch timeout, from RESPONSE_TIMEOUT_MS in
# src/api/server.rs: it gives up on its event loop after five seconds and
# answers with this code.
INSTANCE_TIMEOUT_CODE = -32000

# How many times to ask for a frame, and how long to wait between attempts.
# For the first seconds after startup the instance's event loop is attaching
# its terminal, and a frame requested then can pass the five-second dispatch
# timeout above. A later attempt gets a warm loop.
SNAPSHOT_ATTEMPTS = 3
SNAPSHOT_RETRY_SECONDS = 1.0

# `mcp` 2.x renamed FastMCP to MCPServer; the decorator still returns the
# function unchanged, so every tool below is also an ordinary callable.
mcp = MCPServer("ratterm")


class ToolError(RuntimeError):
    """A tool could not do what it was asked. The message reaches the agent.

    Carries the instance's JSON-RPC error code when there was one, so a caller
    can tell a slow render from a bad method name.
    """

    def __init__(self, message: str, code: int | None = None) -> None:
        super().__init__(message)
        self.code = code


@dataclass
class Session:
    """The instance this server is driving, if any."""

    process: subprocess.Popen[bytes] | None = None
    client: RattermClient | None = None
    endpoint: Endpoint | None = None
    token_path: Path | None = None
    log_path: Path | None = None
    binary: str = "rat"
    fixtures: str | None = None

    def alive(self) -> bool:
        """Returns true while the launched process is still running."""
        return self.process is not None and self.process.poll() is None

    def reset(self) -> None:
        """Forgets the instance, closing the connection if one is open."""
        if self.client is not None:
            self.client.close()
        self.client = None
        self.process = None
        self.endpoint = None


SESSION = Session()


# ---------------------------------------------------------------------------
# Connection handling
# ---------------------------------------------------------------------------


def _environment_endpoint() -> tuple[Endpoint, Path] | None:
    """Returns the endpoint named by the environment, if there is one."""
    raw = os.environ.get("RATTERM_MCP_ENDPOINT")
    if not raw:
        return None
    token_file = os.environ.get("RATTERM_MCP_TOKEN_FILE")
    token_path = Path(token_file) if token_file else default_token_path()
    return parse_endpoint(raw), token_path


def _client() -> RattermClient:
    """Returns a connected client, reconnecting if the connection dropped.

    Raises:
        ToolError: if there is no instance to talk to.
    """
    if SESSION.client is not None and SESSION.client.connected:
        return SESSION.client

    if SESSION.endpoint is None:
        from_env = _environment_endpoint()
        if from_env is None:
            raise ToolError(
                "no ratterm instance. Call launch first, or set "
                "RATTERM_MCP_ENDPOINT to an instance that is already running."
            )
        SESSION.endpoint, SESSION.token_path = from_env

    if SESSION.process is not None and not SESSION.alive():
        raise ToolError(
            "the ratterm instance exited. "
            + _log_tail_message(SESSION.log_path)
            + " Call launch to start a new one."
        )

    try:
        client = wait_until_ready(
            SESSION.endpoint,
            token_path=SESSION.token_path,
            timeout=5.0,
            is_alive=SESSION.alive if SESSION.process is not None else None,
        )
    except ApiError as exc:
        raise ToolError(str(exc)) from exc

    SESSION.client = client
    return client


def _call(method: str, params: dict[str, Any] | None = None) -> Any:
    """Calls a control-API method, turning any failure into a tool error.

    A dropped connection is retried once: a stale handle is not a reason to
    make the agent launch a new instance. An error the instance *answered*
    with is not retried, because the instance already acted on the request.
    """
    client = _client()
    try:
        return client.call(method, params)
    except MethodError as exc:
        raise ToolError(f"{method} failed: {exc}", code=exc.code) from exc
    except ConnectionLost as exc:
        client.close()
        SESSION.client = None
        try:
            return _client().call(method, params)
        except MethodError as second:
            raise ToolError(f"{method} failed: {second}", code=second.code) from exc
        except ApiError as second:
            raise ToolError(f"{method} failed: {second}") from exc
    except ApiError as exc:
        raise ToolError(f"{method} failed: {exc}") from exc


def _log_tail_message(path: Path | None) -> str:
    """Returns the end of the instance log, for an error message."""
    if path is None or not path.exists():
        return "No log was captured."
    try:
        data = path.read_bytes()
    except OSError as exc:
        return f"The log at {path} could not be read: {exc}."
    tail = data[-LOG_TAIL_BYTES:].decode("utf-8", errors="replace").strip()
    if not tail:
        return f"The log at {path} is empty."
    return f"Last output:\n{tail}"


# ---------------------------------------------------------------------------
# Tools
# ---------------------------------------------------------------------------


@mcp.tool()
def launch(
    binary: str = "rat",
    args: list[str] | None = None,
    fixtures: str | None = None,
    endpoint: str | None = None,
) -> dict[str, Any]:
    """Start ratterm headless and wait until its control API answers.

    Args:
        binary: the ratterm executable. A bare name is looked up on PATH.
        args: extra flags for the instance, such as ["--test-keys"].
        fixtures: a fixture directory to load instead of the real
            configuration, such as "tests/fixtures/fleet". A fixture run
            cannot reach a real machine.
        endpoint: where the instance should listen: a port, a loopback
            address, a Windows pipe name, or a Unix socket path. Defaults to
            loopback TCP on a free port, which is the transport that forwards
            over ssh -L.

    Returns:
        The endpoint, the token path, the process id, the log path, the
        command run, and how long the first frame took. A frame is rendered
        before this returns so the first snapshot an agent takes is not cold.
    """
    if SESSION.alive():
        raise ToolError(
            "a ratterm instance is already running on "
            f"{SESSION.endpoint.describe() if SESSION.endpoint else 'an endpoint'}"
            "; call stop first."
        )

    try:
        target = parse_endpoint(endpoint) if endpoint else default_endpoint()
    except ValueError as exc:
        raise ToolError(f"bad endpoint {endpoint!r}: {exc}") from exc

    argv = [binary, "--headless", *target.cli_args(), "--no-update"]
    if fixtures:
        argv += ["--fixtures", fixtures]
    argv += list(args or [])

    log_handle = tempfile.NamedTemporaryFile(
        prefix="ratterm-mcp-", suffix=".log", delete=False
    )
    log_path = Path(log_handle.name)

    try:
        process = subprocess.Popen(
            argv,
            stdout=log_handle,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
        )
    except OSError as exc:
        log_handle.close()
        raise ToolError(f"could not start {binary!r}: {exc}") from exc
    finally:
        log_handle.close()

    SESSION.reset()
    SESSION.process = process
    SESSION.endpoint = target
    SESSION.token_path = default_token_path()
    SESSION.log_path = log_path
    SESSION.binary = binary
    SESSION.fixtures = fixtures

    try:
        SESSION.client = wait_until_ready(
            target,
            token_path=SESSION.token_path,
            timeout=LAUNCH_TIMEOUT_SECONDS,
            is_alive=SESSION.alive,
        )
    except ApiError as exc:
        _terminate(process)
        SESSION.reset()
        raise ToolError(
            f"ratterm started but {target.describe()} never answered: {exc}. "
            + _log_tail_message(log_path)
        ) from exc

    first_frame = _warm_up(SESSION.client, LAUNCH_TIMEOUT_SECONDS)

    return {
        "endpoint": target.describe(),
        "token_path": str(SESSION.token_path),
        "pid": process.pid,
        "log": str(log_path),
        "command": argv,
        # None means the instance answered but never rendered in time. It is
        # still usable for state and for the host registry; snapshots may be
        # slow.
        "first_frame_seconds": None if first_frame is None else round(first_frame, 2),
    }


@mcp.tool()
def snapshot(cells: bool = False) -> dict[str, Any]:
    """Render the current frame and return it as text.

    Args:
        cells: also return every cell with its colours and modifiers. Off by
            default because a 120x40 frame is 4,800 cells.

    Returns:
        The frame size, the cursor position, the frame as one string, the
        lines, and the cells when asked for.
    """
    result = _render(cells)
    lines = result.get("lines") or []
    payload: dict[str, Any] = {
        "width": result.get("width"),
        "height": result.get("height"),
        "cursor": result.get("cursor"),
        "text": "\n".join(str(line) for line in lines),
        "lines": lines,
    }
    if cells:
        payload["cells"] = result.get("cells") or []
    return payload


@mcp.tool()
def send_key(key: str) -> dict[str, Any]:
    """Send one key to the application.

    Args:
        key: a key description in the spelling .ratrc and the scenario files
            use: "ctrl+q", "alt+shift+right", "F2", "enter", "esc", "space",
            or a single character.

    Returns:
        The key the instance says it delivered, under "sent".
    """
    return _as_dict("app.send_key", _call("app.send_key", {"key": key}))


@mcp.tool()
def type_text(text: str) -> dict[str, Any]:
    """Type a string into the application, one key per character.

    Returns:
        The number of characters delivered, under "typed".
    """
    return _as_dict("app.type", _call("app.type", {"text": text}))


@mcp.tool()
def send_mouse(event: str) -> dict[str, Any]:
    """Send one mouse event.

    Args:
        event: "<kind>@<col>,<row>", where kind is one of left_down, left_up,
            left_drag, right_down, right_up, middle_down, middle_up, moved,
            scroll_up, scroll_down. For example "left_down@10,4".

    Returns:
        The event the instance says it delivered, under "sent".
    """
    return _as_dict("app.send_mouse", _call("app.send_mouse", {"event": event}))


@mcp.tool()
def state() -> dict[str, Any]:
    """Return the mode, frame size, focus, tab count and status bar text."""
    result = _call("app.state")
    if not isinstance(result, dict):
        raise ToolError(f"app.state returned {type(result).__name__}")
    return result


@mcp.tool()
def api(method: str, params: dict[str, Any] | None = None) -> Any:
    """Call any control-API method, for what the typed tools do not cover.

    Args:
        method: a method name such as "hosts.list", "editor.open_file",
            "terminal.read_buffer" or "theme.set".
        params: the method's parameters.
    """
    return _call(method, params or {})


@mcp.tool()
def run_scenario(path: str, results_dir: str | None = None) -> dict[str, Any]:
    """Run a scenario file or a directory of them, and return the report.

    Runs in its own process, separate from any launched instance. The binary
    and the fixture directory come from the last launch, so a scenario reads
    the same fleet the running instance does.

    Args:
        path: a scenario file, or a directory of them.
        results_dir: where snapshots and scenarios.json are written. Defaults
            to the runner's own default, test-results.

    Returns:
        The exit code, the report the runner printed, and the command run. The
        exit code is 0 when every scenario passed and 1 otherwise.
    """
    argv = scenario_argv(
        binary=SESSION.binary,
        path=path,
        results_dir=results_dir,
        fixtures=SESSION.fixtures,
    )

    try:
        completed = subprocess.run(
            argv,
            capture_output=True,
            timeout=SCENARIO_TIMEOUT_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise ToolError(
            f"the scenario run did not finish within "
            f"{SCENARIO_TIMEOUT_SECONDS:.0f}s and was killed"
        ) from exc
    except OSError as exc:
        raise ToolError(f"could not run {SESSION.binary!r}: {exc}") from exc

    report = completed.stdout.decode("utf-8", errors="replace")
    errors = completed.stderr.decode("utf-8", errors="replace")

    return {
        "exit_code": completed.returncode,
        "passed": completed.returncode == 0,
        "report": report,
        "stderr": errors,
        "command": argv,
    }


@mcp.tool()
def stop() -> dict[str, Any]:
    """Shut the launched instance down.

    Asks it to quit over the control API first, so it removes its token file,
    then terminates the process if it is still there.
    """
    if SESSION.process is None:
        SESSION.reset()
        return {"stopped": False, "reason": "no instance was launched"}

    process = SESSION.process
    quit_sent = False
    if SESSION.client is not None and SESSION.client.connected:
        try:
            SESSION.client.call("system.quit", {"force": True}, timeout=2.0)
            quit_sent = True
        except ApiError:
            # The instance may close the connection while answering, which
            # looks like a failure and is in fact the shutdown working.
            quit_sent = True

    if quit_sent:
        # Give it a moment to leave its event loop and remove its token file.
        # Terminating first would leave the token behind for the next run to
        # read and be refused with.
        _wait_for_exit(process, STOP_GRACE_SECONDS)

    code = _terminate(process)
    log_path = SESSION.log_path
    SESSION.reset()

    return {
        "stopped": True,
        "quit_sent": quit_sent,
        "exit_code": code,
        "log": str(log_path) if log_path else None,
    }


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _warm_up(client: RattermClient, seconds: float) -> float | None:
    """Renders one frame so the first snapshot an agent takes is not cold.

    For the first seconds after startup the instance's event loop is busy
    attaching its terminal, and a frame requested then can pass the instance's
    own five-second dispatch timeout. Paying that here, once, keeps it out of
    every later call.

    Returns:
        How long the first frame took, or None if none arrived in time or the
        instance failed for another reason. Neither is fatal: state and the
        host registry work regardless.
    """
    started = time.monotonic()
    deadline = started + seconds
    while time.monotonic() < deadline:
        try:
            client.call("app.snapshot", {"cells": False}, timeout=seconds)
        except MethodError as exc:
            if exc.code == INSTANCE_TIMEOUT_CODE:
                continue
            return None
        except ApiError:
            return None
        return time.monotonic() - started
    return None


def _as_dict(method: str, result: Any) -> dict[str, Any]:
    """Returns the instance's result, insisting it is an object."""
    if not isinstance(result, dict):
        raise ToolError(
            f"{method} returned {type(result).__name__}, expected an object"
        )
    return result


def _render(cells: bool) -> dict[str, Any]:
    """Asks for a frame, allowing for a slow first render.

    Rendering happens on the instance's own event loop, which gives up after
    five seconds and answers -32000. The first frame after startup builds
    state every later frame reuses and can pass that in a debug build, so a
    timeout is retried; anything else is reported as it is.
    """
    for attempt in range(SNAPSHOT_ATTEMPTS):
        try:
            result = _call("app.snapshot", {"cells": cells})
        except ToolError as exc:
            if exc.code != INSTANCE_TIMEOUT_CODE or attempt + 1 >= SNAPSHOT_ATTEMPTS:
                raise
            time.sleep(SNAPSHOT_RETRY_SECONDS)
            continue

        if not isinstance(result, dict):
            raise ToolError(f"app.snapshot returned {type(result).__name__}")
        return result

    raise ToolError(
        f"the instance did not render a frame in {SNAPSHOT_ATTEMPTS} attempts"
    )


def scenario_argv(
    binary: str,
    path: str,
    results_dir: str | None = None,
    fixtures: str | None = None,
) -> list[str]:
    """Builds the command line for a scenario run.

    Uses ``--scenario-dir`` when `path` is a directory and ``--scenario``
    when it is a file, so a caller does not have to know which flag to pass.
    """
    flag = "--scenario-dir" if Path(path).is_dir() else "--scenario"
    argv = [binary, flag, path, "--no-update"]
    if fixtures:
        argv += ["--fixtures", fixtures]
    if results_dir:
        argv += ["--results-dir", results_dir]
    return argv


def _wait_for_exit(process: subprocess.Popen[bytes], seconds: float) -> int | None:
    """Waits up to `seconds` for a process to exit, returning its code."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        code = process.poll()
        if code is not None:
            return code
        time.sleep(0.05)
    return None


def _terminate(process: subprocess.Popen[bytes]) -> int | None:
    """Ends a process, escalating to a kill if it does not go quietly."""
    if process.poll() is not None:
        return process.returncode

    process.terminate()
    code = _wait_for_exit(process, STOP_GRACE_SECONDS)
    if code is not None:
        return code

    process.kill()
    try:
        process.wait(timeout=STOP_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        return None
    return process.returncode


def main() -> None:
    """Runs the MCP server on stdio."""
    mcp.run(transport="stdio")


if __name__ == "__main__":  # pragma: no cover - entry point
    main()
