"""Transport and handshake for the ratterm control API.

The control API speaks newline-delimited JSON over three transports: a named
pipe on Windows, a Unix domain socket elsewhere, and a loopback TCP port on
every platform. This module hides that difference behind one client.

Every connection starts unauthenticated. The first message must be

    {"id": "1", "method": "session.authenticate", "params": {"token": "<hex>"}}

with the token the instance published to its token file. Until that succeeds
the instance answers everything except ``system.ping`` with error code
``-32001``.

Every read here takes a deadline. A read that reaches it raises
:class:`ApiTimeout` rather than parking, so a caller driving this from an agent
tool cannot be left waiting on an instance that died.
"""

from __future__ import annotations

import json
import queue
import socket
import sys
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import IO, Any, Protocol

__all__ = [
    "ApiError",
    "ApiTimeout",
    "AuthenticationError",
    "ConnectionLost",
    "Endpoint",
    "LocalEndpoint",
    "ProtocolError",
    "RattermClient",
    "TcpEndpoint",
    "TokenError",
    "connect",
    "default_endpoint",
    "default_token_path",
    "find_free_port",
    "parse_endpoint",
    "read_token",
    "wait_until_ready",
]

# JSON-RPC error code the instance returns when the token is missing or wrong.
ERROR_UNAUTHENTICATED = -32001

# Default per-request deadline, in seconds. The instance's own request timeout
# is 5s, so this leaves room for the round trip on top of it.
DEFAULT_TIMEOUT = 10.0

# Default loopback port, matching DEFAULT_TCP_PORT in src/api/transport/tcp.rs.
DEFAULT_TCP_PORT = 47113

# A token is 32 bytes, hex encoded.
TOKEN_HEX_LENGTH = 64

# Hostnames the control endpoint will bind. The Rust side refuses anything
# else, so rejecting it here produces a better message than a failed connect.
LOOPBACK_HOSTS = frozenset({"localhost", "127.0.0.1", "::1"})

# Upper bound on responses read while looking for a matching request id. The
# instance does not send unsolicited messages, so anything beyond this is a
# desynchronised stream rather than traffic worth skipping.
MAX_SKIPPED_RESPONSES = 32

_WINDOWS = sys.platform == "win32"


# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------


class ApiError(Exception):
    """An error the instance reported, or one raised talking to it."""


class ApiTimeout(ApiError):
    """A read did not complete before its deadline."""


class ConnectionLost(ApiError):
    """The connection closed, or failed, part way through an exchange."""


class ProtocolError(ApiError):
    """The instance sent something that is not a valid response."""


class TokenError(ApiError):
    """The token file is missing, unreadable, or not a token."""


class AuthenticationError(ApiError):
    """The instance refused the token with ``-32001``."""


class MethodError(ApiError):
    """The instance answered with an error object.

    Carries the JSON-RPC ``code`` so a caller can tell a bad method name from
    a bad argument without matching on the message text.
    """

    def __init__(self, code: int, message: str) -> None:
        super().__init__(f"{message} (code {code})")
        self.code = code
        self.raw_message = message


# ---------------------------------------------------------------------------
# Endpoints
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class TcpEndpoint:
    """A loopback TCP control endpoint.

    This is the transport that crosses machines: ``ssh -N -L
    PORT:127.0.0.1:PORT`` forwards it unchanged, which neither a Unix socket
    nor a named pipe does reliably.
    """

    host: str
    port: int

    def describe(self) -> str:
        """Returns the endpoint as it appears in logs."""
        return f"tcp://{self.host}:{self.port}"

    def cli_args(self) -> list[str]:
        """Returns the ``rat`` flags that make an instance listen here."""
        return ["--api-tcp", f"{self.host}:{self.port}"]


@dataclass(frozen=True)
class LocalEndpoint:
    """A named pipe on Windows, or a Unix domain socket path elsewhere."""

    path: str

    def describe(self) -> str:
        """Returns the endpoint as it appears in logs."""
        return self.path

    def cli_args(self) -> list[str]:
        """Returns the ``rat`` flags that make an instance listen here."""
        return ["--api-socket", self.path]


Endpoint = TcpEndpoint | LocalEndpoint


def parse_endpoint(text: str) -> Endpoint:
    """Parses an endpoint written as a port, an address, a pipe or a path.

    Accepted forms::

        47119                       loopback TCP on that port
        127.0.0.1:47119             loopback TCP
        tcp://127.0.0.1:47119       loopback TCP
        \\\\.\\pipe\\ratterm-api    a Windows named pipe
        /tmp/ratterm-1.sock         a Unix domain socket

    Raises:
        ValueError: if the text is empty, names a non-loopback address, or
            carries a port that is not a number in range.
    """
    stripped = text.strip()
    if not stripped:
        raise ValueError("endpoint is empty")

    if stripped.startswith("\\\\") or stripped.startswith("//./pipe/"):
        return LocalEndpoint(stripped)

    body = stripped[len("tcp://") :] if stripped.startswith("tcp://") else stripped

    if body.isdigit():
        return TcpEndpoint("127.0.0.1", _port(body))

    if ":" in body:
        host, _, port_text = body.rpartition(":")
        host = host.strip("[]")
        if port_text.isdigit() and host and "/" not in host and "\\" not in host:
            if host not in LOOPBACK_HOSTS:
                raise ValueError(
                    f"{host!r} is not a loopback address; the control endpoint "
                    "is never exposed to the network"
                )
            return TcpEndpoint(host, _port(port_text))

    return LocalEndpoint(stripped)


def _port(text: str) -> int:
    """Parses a TCP port, rejecting 0 and anything out of range."""
    value = int(text)
    if not 1 <= value <= 65535:
        raise ValueError(f"{value} is not a usable TCP port")
    return value


def find_free_port(host: str = "127.0.0.1") -> int:
    """Returns a loopback port nothing is listening on right now.

    The port is released before it is returned, so a process racing for the
    same port can still win it. That is unavoidable without holding the socket
    open, and the caller finds out immediately because the instance fails to
    bind.
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind((host, 0))
        return int(sock.getsockname()[1])


def default_endpoint() -> Endpoint:
    """Returns the endpoint a launch uses when the caller names none.

    Loopback TCP on a free port: it works on all three platforms, several
    instances can run at once, and it forwards over ``ssh -L``.
    """
    return TcpEndpoint("127.0.0.1", find_free_port())


# ---------------------------------------------------------------------------
# Token file
# ---------------------------------------------------------------------------


def default_token_path() -> Path:
    """Returns where an instance publishes its token by default.

    Mirrors ``SessionToken::default_path`` in ``src/api/auth.rs``.
    """
    return Path.home() / ".ratterm" / "api.token"


def read_token(path: Path) -> str:
    """Reads and validates the hex token an instance published.

    Raises:
        TokenError: if the file is missing, unreadable, or does not hold a
            64-character hex token.
    """
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise TokenError(
            f"no API token at {path}. Start ratterm first, or pass "
            "--api-no-auth for a local run with no token."
        ) from exc
    except OSError as exc:
        raise TokenError(f"could not read the API token at {path}: {exc}") from exc

    token = text.strip()
    if len(token) != TOKEN_HEX_LENGTH or not all(
        c in "0123456789abcdefABCDEF" for c in token
    ):
        raise TokenError(
            f"{path} does not hold an API token: expected "
            f"{TOKEN_HEX_LENGTH} hex characters, found {len(token)}"
        )
    return token.lower()


# ---------------------------------------------------------------------------
# Transports
# ---------------------------------------------------------------------------


class Transport(Protocol):
    """A newline-delimited byte channel with deadlines on every read."""

    def send_line(self, line: str) -> None:
        """Writes one line, appending the newline."""

    def read_line(self, timeout: float) -> str:
        """Returns the next non-empty line, or raises before `timeout`."""

    def close(self) -> None:
        """Closes the channel. Safe to call more than once."""


class _LineBuffer:
    """Splits a byte stream into lines, keeping any partial trailing line."""

    def __init__(self) -> None:
        self._pending = bytearray()

    def feed(self, chunk: bytes) -> None:
        """Adds bytes read from the channel."""
        self._pending.extend(chunk)

    def take(self) -> str | None:
        """Removes and returns the next complete non-empty line, if any."""
        while True:
            index = self._pending.find(b"\n")
            if index < 0:
                return None
            raw = bytes(self._pending[:index])
            del self._pending[: index + 1]
            line = raw.decode("utf-8", errors="replace").strip()
            if line:
                return line


class SocketTransport:
    """Newline-delimited JSON over a TCP or Unix domain socket."""

    def __init__(self, sock: socket.socket) -> None:
        self._sock = sock
        self._lines = _LineBuffer()
        self._open = True

    def send_line(self, line: str) -> None:
        """Writes one line, appending the newline."""
        if not self._open:
            raise ConnectionLost("the connection is closed")
        try:
            self._sock.sendall(line.encode("utf-8") + b"\n")
        except OSError as exc:
            self._open = False
            raise ConnectionLost(f"could not send to the instance: {exc}") from exc

    def read_line(self, timeout: float) -> str:
        """Returns the next non-empty line, or raises before `timeout`."""
        deadline = time.monotonic() + timeout
        while True:
            line = self._lines.take()
            if line is not None:
                return line

            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ApiTimeout(f"no reply from the instance within {timeout:.1f}s")

            if not self._open:
                raise ConnectionLost("the instance closed the connection")

            self._sock.settimeout(remaining)
            try:
                chunk = self._sock.recv(65536)
            except TimeoutError as exc:
                raise ApiTimeout(
                    f"no reply from the instance within {timeout:.1f}s"
                ) from exc
            except OSError as exc:
                self._open = False
                raise ConnectionLost(f"the connection failed: {exc}") from exc

            if not chunk:
                self._open = False
                raise ConnectionLost("the instance closed the connection")
            self._lines.feed(chunk)

    def close(self) -> None:
        """Closes the socket. Safe to call more than once."""
        self._open = False
        try:
            self._sock.close()
        except OSError:
            # Already closed, or closed underneath us. Nothing left to do.
            pass


class PipeTransport:
    """Newline-delimited JSON over a Windows named pipe.

    A pipe handle opened as a file has no read timeout, so the read runs on a
    daemon thread that posts chunks to a queue and :meth:`read_line` waits on
    the queue with a deadline. That keeps a stuck instance from stalling a tool
    call; the thread itself dies with the process.
    """

    def __init__(self, handle: IO[bytes]) -> None:
        self._handle = handle
        self._lines = _LineBuffer()
        self._chunks: queue.Queue[bytes | None] = queue.Queue()
        self._open = True
        self._thread = threading.Thread(
            target=self._pump, name="ratterm-pipe-reader", daemon=True
        )
        self._thread.start()

    def _pump(self) -> None:
        """Moves bytes off the pipe until it closes."""
        while True:
            try:
                chunk = self._handle.read(65536)
            except OSError:
                # A broken pipe reads as end of stream for our purposes.
                self._chunks.put(None)
                return
            if not chunk:
                self._chunks.put(None)
                return
            self._chunks.put(chunk)

    def send_line(self, line: str) -> None:
        """Writes one line, appending the newline."""
        if not self._open:
            raise ConnectionLost("the connection is closed")
        try:
            self._handle.write(line.encode("utf-8") + b"\n")
            self._handle.flush()
        except OSError as exc:
            self._open = False
            raise ConnectionLost(f"could not send to the instance: {exc}") from exc

    def read_line(self, timeout: float) -> str:
        """Returns the next non-empty line, or raises before `timeout`."""
        deadline = time.monotonic() + timeout
        while True:
            line = self._lines.take()
            if line is not None:
                return line

            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ApiTimeout(f"no reply from the instance within {timeout:.1f}s")

            try:
                chunk = self._chunks.get(timeout=remaining)
            except queue.Empty as exc:
                raise ApiTimeout(
                    f"no reply from the instance within {timeout:.1f}s"
                ) from exc

            if chunk is None:
                self._open = False
                raise ConnectionLost("the instance closed the pipe")
            self._lines.feed(chunk)

    def close(self) -> None:
        """Closes the pipe handle. Safe to call more than once."""
        self._open = False
        try:
            self._handle.close()
        except OSError:
            pass


def open_transport(endpoint: Endpoint, timeout: float = DEFAULT_TIMEOUT) -> Transport:
    """Opens a transport to `endpoint`.

    Raises:
        ConnectionLost: if nothing is listening there.
    """
    if isinstance(endpoint, TcpEndpoint):
        try:
            sock = socket.create_connection((endpoint.host, endpoint.port), timeout)
        except OSError as exc:
            raise ConnectionLost(
                f"could not reach {endpoint.describe()}: {exc}"
            ) from exc
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        return SocketTransport(sock)

    if _WINDOWS:
        try:
            handle = open(endpoint.path, "r+b", buffering=0)
        except OSError as exc:
            raise ConnectionLost(
                f"could not open the named pipe {endpoint.path}: {exc}"
            ) from exc
        return PipeTransport(handle)

    unix_family = getattr(socket, "AF_UNIX", None)
    if unix_family is None:
        raise ConnectionLost(
            f"this platform has no Unix domain sockets; use a TCP endpoint "
            f"instead of {endpoint.path}"
        )
    sock = socket.socket(unix_family, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    try:
        sock.connect(endpoint.path)
    except OSError as exc:
        sock.close()
        raise ConnectionLost(f"could not reach {endpoint.path}: {exc}") from exc
    return SocketTransport(sock)


# ---------------------------------------------------------------------------
# Client
# ---------------------------------------------------------------------------


class RattermClient:
    """One authenticated connection to a ratterm control endpoint.

    Pass `token` to use a token you already hold, or `token_path` to read one
    from the file an instance published. The file is read after the endpoint
    accepts the connection, never before: an instance writes its token and
    then binds its listener, so a token read before the connect can be the
    previous run's, and presenting that gets a -32001 that looks like a real
    authentication failure.
    """

    def __init__(
        self,
        endpoint: Endpoint,
        token: str | None = None,
        timeout: float = DEFAULT_TIMEOUT,
        token_path: Path | None = None,
        require_token: bool = True,
    ) -> None:
        self.endpoint = endpoint
        self.timeout = timeout
        self.token_path = token_path
        self.require_token = require_token
        self._token = token
        self._transport: Transport | None = None
        self._next_id = 0

    # -- lifecycle ----------------------------------------------------------

    def connect(self) -> None:
        """Opens the transport, reads the token, and presents it.

        Closes any connection this client already holds, so a reconnect does
        not leak the previous socket.

        Raises:
            ConnectionLost: if nothing is listening.
            TokenError: if the token file is required and unusable.
            AuthenticationError: if the instance refuses the token.
        """
        self.close()
        transport = open_transport(self.endpoint, self.timeout)
        self._transport = transport
        try:
            self._authenticate()
        except ApiError:
            self.close()
            raise

    def _resolve_token(self) -> str | None:
        """Returns the token to present, reading the token file if needed."""
        if self._token is not None:
            return self._token
        if self.token_path is None:
            return None
        try:
            return read_token(self.token_path)
        except TokenError:
            if self.require_token:
                raise
            return None

    def close(self) -> None:
        """Closes the connection. Safe to call more than once."""
        if self._transport is not None:
            self._transport.close()
            self._transport = None

    def __enter__(self) -> RattermClient:
        # `connect()` returns a client that has already handshaken, and using
        # it as a context manager must not repeat that.
        if self._transport is None:
            self.connect()
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()

    @property
    def connected(self) -> bool:
        """Returns true while the transport is open."""
        return self._transport is not None

    # -- requests -----------------------------------------------------------

    def call(
        self,
        method: str,
        params: dict[str, Any] | None = None,
        timeout: float | None = None,
    ) -> Any:
        """Calls one control-API method and returns its result.

        Raises:
            ConnectionLost: if the connection is not open, or drops mid-call.
            ApiTimeout: if no reply arrives in time.
            ProtocolError: if the reply is not a valid response.
            AuthenticationError: on error code -32001.
            MethodError: on any other error the instance reports.
        """
        if self._transport is None:
            raise ConnectionLost("not connected; call connect() first")

        deadline = timeout if timeout is not None else self.timeout
        self._next_id += 1
        request_id = str(self._next_id)
        payload = json.dumps(
            {"id": request_id, "method": method, "params": params or {}}
        )

        self._transport.send_line(payload)
        response = self._read_response(request_id, deadline)

        error = response.get("error")
        if error is not None:
            self._raise_for_error(error)

        return response.get("result")

    def _read_response(self, request_id: str, timeout: float) -> dict[str, Any]:
        """Reads until the reply with `request_id` arrives."""
        transport = self._transport
        if transport is None:
            raise ConnectionLost("not connected")

        deadline = time.monotonic() + timeout
        for _ in range(MAX_SKIPPED_RESPONSES):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ApiTimeout(f"no reply to {request_id} within {timeout:.1f}s")

            line = transport.read_line(remaining)
            try:
                decoded = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ProtocolError(
                    f"the instance sent something that is not JSON: {line[:200]!r}"
                ) from exc

            if not isinstance(decoded, dict):
                raise ProtocolError(
                    f"expected a JSON object, got {type(decoded).__name__}"
                )
            if decoded.get("id") == request_id:
                return decoded

        raise ProtocolError(
            f"no reply to {request_id} after {MAX_SKIPPED_RESPONSES} messages; "
            "the stream is out of step"
        )

    def _raise_for_error(self, error: Any) -> None:
        """Turns a JSON-RPC error object into the matching exception."""
        if not isinstance(error, dict):
            raise ProtocolError(f"malformed error object: {error!r}")

        code = error.get("code")
        message = str(error.get("message", "no message"))

        if code == ERROR_UNAUTHENTICATED:
            where = f" in {self.token_path}" if self.token_path else ""
            raise AuthenticationError(
                f"the instance refused the API token{where}: {message}. "
                "The token is wrong or stale — every run mints a new one, so "
                "re-read the token file this instance published."
            )

        raise MethodError(int(code) if isinstance(code, int) else -1, message)

    def _authenticate(self) -> None:
        """Sends the handshake as the first message on the connection."""
        # An instance started with --api-no-auth accepts any token, including
        # the empty one, so this is the same message either way.
        token = self._resolve_token()
        self.call("session.authenticate", {"token": token or ""})

    # -- convenience --------------------------------------------------------

    def snapshot(self, cells: bool = False) -> dict[str, Any]:
        """Renders the current frame and returns it."""
        result = self.call("app.snapshot", {"cells": cells})
        if not isinstance(result, dict):
            raise ProtocolError(f"app.snapshot returned {type(result).__name__}")
        return result

    def state(self) -> dict[str, Any]:
        """Returns mode, size, focus, tab count and status."""
        result = self.call("app.state")
        if not isinstance(result, dict):
            raise ProtocolError(f"app.state returned {type(result).__name__}")
        return result


def connect(
    endpoint: Endpoint,
    token_path: Path | None = None,
    timeout: float = DEFAULT_TIMEOUT,
    require_token: bool = True,
) -> RattermClient:
    """Opens an authenticated client to `endpoint`.

    Args:
        endpoint: where the instance is listening.
        token_path: the token file; defaults to ``~/.ratterm/api.token``.
        timeout: per-request deadline in seconds.
        require_token: when false, a missing token file is not an error, which
            is what an instance started with ``--api-no-auth`` needs.

    Raises:
        TokenError: if the token file is required and unusable.
        ConnectionLost: if nothing is listening.
        AuthenticationError: if the instance refuses the token.
    """
    client = RattermClient(
        endpoint,
        timeout=timeout,
        token_path=token_path or default_token_path(),
        require_token=require_token,
    )
    client.connect()
    return client


def wait_until_ready(
    endpoint: Endpoint,
    token_path: Path | None = None,
    timeout: float = 20.0,
    poll_interval: float = 0.1,
    require_token: bool = True,
    is_alive: Callable[[], bool] | None = None,
) -> RattermClient:
    """Polls `endpoint` until it answers, then returns a connected client.

    Args:
        endpoint: where the instance is listening.
        token_path: the token file to read.
        timeout: how long to keep trying, in seconds.
        poll_interval: how long to wait between attempts.
        require_token: passed through to :func:`connect`.
        is_alive: an optional zero-argument callable. When it returns false the
            wait stops at once rather than running out the clock, which is how
            a launch reports a process that died during startup.

    Raises:
        ConnectionLost: if the endpoint never answered.
        AuthenticationError: if it answered and refused the token. This is not
            retried: a wrong token does not become right by waiting.
    """
    deadline = time.monotonic() + timeout
    last: ApiError = ConnectionLost(f"{endpoint.describe()} never answered")

    while time.monotonic() < deadline:
        if is_alive is not None and not is_alive():
            raise ConnectionLost(
                f"the instance exited before {endpoint.describe()} answered"
            )
        try:
            return connect(
                endpoint,
                token_path=token_path,
                require_token=require_token,
            )
        except AuthenticationError:
            raise
        except (ConnectionLost, TokenError, ApiTimeout) as exc:
            # The token file appears slightly after the process starts, and
            # the listener slightly after that. Both are worth retrying.
            last = exc
        time.sleep(poll_interval)

    raise ConnectionLost(
        f"{endpoint.describe()} did not answer within {timeout:.0f}s: {last}"
    )
