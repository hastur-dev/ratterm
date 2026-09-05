"""A fake control endpoint, so the tests never need ratterm itself.

The real instance is a Rust binary with a PTY and a fleet behind it. What the
client actually depends on is much smaller: a loopback socket that reads
newline-delimited JSON and writes newline-delimited JSON, and answers
``session.authenticate`` before anything else. That is what this provides, with
a behaviour switch for each failure the client has to handle.
"""

from __future__ import annotations

import json
import socket
import threading
from collections.abc import Iterator
from pathlib import Path

import pytest

from ratterm_mcp.client import Endpoint, TcpEndpoint

# The token the fake instance accepts. Length and alphabet match the real one.
GOOD_TOKEN = "a1" * 32
WRONG_TOKEN = "b2" * 32


class FakeInstance:
    """A loopback server that speaks the control protocol badly on request.

    Behaviours:

    ``ok``
        Authenticates, then answers ``app.snapshot``, ``app.state`` and
        ``system.get_version``. Anything else gets a method-not-found error.
    ``refuse``
        Answers every request, the handshake included, with ``-32001``.
    ``silent``
        Authenticates, then never answers again.
    ``malformed``
        Authenticates, then answers with something that is not JSON.
    ``drop``
        Authenticates, then closes the connection instead of answering.
    ``slow_first_frame``
        Answers the first ``app.snapshot`` with -32000, the instance's own
        dispatch timeout, and every later one normally. This is what a debug
        build does on the first render after startup.
    """

    def __init__(self, behaviour: str = "ok") -> None:
        self.behaviour = behaviour
        self.received: list[dict[str, object]] = []
        self.frames_requested = 0
        self._listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._listener.bind(("127.0.0.1", 0))
        self._listener.listen(4)
        # Short, so shutting the fake instance down between tests is quick.
        self._listener.settimeout(0.05)
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._serve, daemon=True)
        self._thread.start()

    @property
    def endpoint(self) -> Endpoint:
        """Returns the endpoint a client should connect to."""
        host, port = self._listener.getsockname()[:2]
        return TcpEndpoint(host, int(port))

    def close(self) -> None:
        """Stops accepting and closes the listener."""
        self._stop.set()
        self._thread.join(timeout=2.0)
        self._listener.close()

    # -- internals ----------------------------------------------------------

    def _serve(self) -> None:
        while not self._stop.is_set():
            try:
                conn, _peer = self._listener.accept()
            except TimeoutError:
                continue
            except OSError:
                return
            threading.Thread(target=self._handle, args=(conn,), daemon=True).start()

    def _handle(self, conn: socket.socket) -> None:
        conn.settimeout(0.5)
        pending = bytearray()
        authenticated = False

        while not self._stop.is_set():
            index = pending.find(b"\n")
            if index < 0:
                try:
                    chunk = conn.recv(65536)
                except TimeoutError:
                    continue
                except OSError:
                    conn.close()
                    return
                if not chunk:
                    conn.close()
                    return
                pending.extend(chunk)
                continue

            line = bytes(pending[:index])
            del pending[: index + 1]
            if not line.strip():
                continue

            request = json.loads(line.decode("utf-8"))
            self.received.append(request)
            reply = self._reply_to(request, authenticated)
            if reply is None:
                # "silent" and "drop" both stop answering; drop also hangs up.
                if self.behaviour == "drop":
                    conn.close()
                    return
                continue

            if request.get("method") == "session.authenticate" and "error" not in reply:
                authenticated = True

            conn.sendall(reply.encode("utf-8") + b"\n")

    def _reply_to(self, request: dict[str, object], authenticated: bool) -> str | None:
        request_id = str(request.get("id", ""))
        method = str(request.get("method", ""))
        params = request.get("params")
        params = params if isinstance(params, dict) else {}

        if self.behaviour == "refuse":
            return _error(request_id, -32001, "Invalid API token")

        if method == "session.authenticate":
            if params.get("token") != GOOD_TOKEN:
                return _error(request_id, -32001, "Invalid API token")
            return _result(request_id, {"authenticated": True})

        if not authenticated:
            return _error(
                request_id,
                -32001,
                "Authentication required: call session.authenticate first",
            )

        if self.behaviour == "silent":
            return None
        if self.behaviour == "drop":
            return None
        if self.behaviour == "malformed":
            return "this line is not JSON at all"

        if method == "app.snapshot":
            self.frames_requested += 1
            if self.behaviour == "slow_first_frame" and self.frames_requested == 1:
                return _error(request_id, -32000, "Request timed out")
            payload = {
                "width": 4,
                "height": 2,
                "lines": ["fixture-gpu", "ready"],
                "cursor": [1, 1],
                "cells": [] if not params.get("cells") else [{"x": 0, "y": 0}],
            }
            return _result(request_id, payload)
        if method == "app.state":
            return _result(request_id, {"mode": "Normal", "status": "ok"})
        if method == "system.get_version":
            return _result(request_id, {"version": "0.0.0-fake"})

        return _error(request_id, -32601, f"Method not found: {method}")


def _result(request_id: str, result: object) -> str:
    """Encodes a success response."""
    return json.dumps({"id": request_id, "result": result})


def _error(request_id: str, code: int, message: str) -> str:
    """Encodes an error response."""
    return json.dumps({"id": request_id, "error": {"code": code, "message": message}})


@pytest.fixture
def token_file(tmp_path: Path) -> Path:
    """Writes a valid token file and returns its path."""
    path = tmp_path / "api.token"
    path.write_text(GOOD_TOKEN, encoding="utf-8")
    return path


@pytest.fixture
def instance() -> Iterator[FakeInstance]:
    """A fake instance that behaves."""
    server = FakeInstance("ok")
    yield server
    server.close()


@pytest.fixture
def make_instance() -> Iterator[object]:
    """A factory for a fake instance with a chosen behaviour."""
    created: list[FakeInstance] = []

    def factory(behaviour: str) -> FakeInstance:
        server = FakeInstance(behaviour)
        created.append(server)
        return server

    yield factory
    for server in created:
        server.close()
