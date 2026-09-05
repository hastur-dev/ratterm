"""Tests for the transport, the handshake and the failure paths.

Every test here runs against the fake instance in ``conftest.py``, so the
suite passes on a machine with no ratterm binary and no fleet.
"""

from __future__ import annotations

import time
from pathlib import Path

import pytest

from conftest import GOOD_TOKEN, WRONG_TOKEN, FakeInstance
from ratterm_mcp.client import (
    ApiTimeout,
    AuthenticationError,
    ConnectionLost,
    MethodError,
    ProtocolError,
    RattermClient,
    TokenError,
    connect,
    read_token,
    wait_until_ready,
)

# ---------------------------------------------------------------------------
# Handshake
# ---------------------------------------------------------------------------


def test_the_handshake_is_the_first_message_on_the_connection(instance, token_file):
    with connect(instance.endpoint, token_path=token_file) as client:
        assert client.connected

    assert instance.received, "the instance received nothing"
    first = instance.received[0]
    assert first["method"] == "session.authenticate"
    assert first["params"]["token"] == GOOD_TOKEN


def test_a_call_after_the_handshake_returns_its_result(instance, token_file):
    with connect(instance.endpoint, token_path=token_file) as client:
        snapshot = client.snapshot()

    assert snapshot["lines"] == ["fixture-gpu", "ready"]
    assert snapshot["width"] == 4
    methods = [entry["method"] for entry in instance.received]
    assert methods == ["session.authenticate", "app.snapshot"]


def test_a_call_before_connecting_is_an_error(instance, token_file):
    client = RattermClient(instance.endpoint, GOOD_TOKEN, token_path=token_file)
    with pytest.raises(ConnectionLost, match="not connected"):
        client.call("app.state")


def test_the_request_id_advances_so_replies_can_be_matched(instance, token_file):
    with connect(instance.endpoint, token_path=token_file) as client:
        client.state()
        client.state()

    ids = [entry["id"] for entry in instance.received]
    assert ids == ["1", "2", "3"]


# ---------------------------------------------------------------------------
# Authentication refusal
# ---------------------------------------------------------------------------


def test_a_rejected_token_raises_rather_than_retrying(make_instance, tmp_path: Path):
    server = make_instance("refuse")
    path = tmp_path / "api.token"
    path.write_text(WRONG_TOKEN, encoding="utf-8")

    started = time.monotonic()
    with pytest.raises(AuthenticationError, match="wrong or stale"):
        connect(server.endpoint, token_path=path)

    assert time.monotonic() - started < 5.0, "a refusal must not be retried"


def test_the_refusal_message_names_the_token_file(make_instance, tmp_path: Path):
    server = make_instance("refuse")
    path = tmp_path / "api.token"
    path.write_text(WRONG_TOKEN, encoding="utf-8")

    with pytest.raises(AuthenticationError) as caught:
        connect(server.endpoint, token_path=path)

    assert str(path) in str(caught.value)


def test_waiting_for_readiness_does_not_retry_a_refusal(make_instance, tmp_path: Path):
    server = make_instance("refuse")
    path = tmp_path / "api.token"
    path.write_text(WRONG_TOKEN, encoding="utf-8")

    started = time.monotonic()
    with pytest.raises(AuthenticationError):
        wait_until_ready(server.endpoint, token_path=path, timeout=30.0)

    assert time.monotonic() - started < 5.0


# ---------------------------------------------------------------------------
# Timeouts
# ---------------------------------------------------------------------------


def test_a_silent_instance_times_out_instead_of_hanging(make_instance, token_file):
    server = make_instance("silent")
    with connect(server.endpoint, token_path=token_file) as client:
        started = time.monotonic()
        with pytest.raises(ApiTimeout, match="within"):
            client.call("app.state", timeout=0.4)
        elapsed = time.monotonic() - started

    assert 0.3 < elapsed < 3.0, f"the deadline was not honoured: {elapsed:.2f}s"


def test_an_endpoint_nothing_listens_on_fails_quickly(tmp_path: Path):
    from ratterm_mcp.client import TcpEndpoint, find_free_port

    path = tmp_path / "api.token"
    path.write_text(GOOD_TOKEN, encoding="utf-8")
    dead = TcpEndpoint("127.0.0.1", find_free_port())

    started = time.monotonic()
    with pytest.raises(ConnectionLost):
        connect(dead, token_path=path, timeout=1.0)
    assert time.monotonic() - started < 5.0


def test_waiting_for_readiness_gives_up_at_its_deadline(tmp_path: Path):
    from ratterm_mcp.client import TcpEndpoint, find_free_port

    path = tmp_path / "api.token"
    path.write_text(GOOD_TOKEN, encoding="utf-8")
    dead = TcpEndpoint("127.0.0.1", find_free_port())

    started = time.monotonic()
    with pytest.raises(ConnectionLost, match="did not answer"):
        wait_until_ready(dead, token_path=path, timeout=0.5, poll_interval=0.05)
    assert time.monotonic() - started < 5.0


def test_waiting_stops_as_soon_as_the_process_is_gone(tmp_path: Path):
    from ratterm_mcp.client import TcpEndpoint, find_free_port

    path = tmp_path / "api.token"
    path.write_text(GOOD_TOKEN, encoding="utf-8")
    dead = TcpEndpoint("127.0.0.1", find_free_port())

    started = time.monotonic()
    with pytest.raises(ConnectionLost, match="exited"):
        wait_until_ready(dead, token_path=path, timeout=30.0, is_alive=lambda: False)
    assert time.monotonic() - started < 2.0


# ---------------------------------------------------------------------------
# Malformed replies and connection loss
# ---------------------------------------------------------------------------


def test_a_reply_that_is_not_json_is_reported_as_a_protocol_error(
    make_instance, token_file
):
    server = make_instance("malformed")
    with connect(server.endpoint, token_path=token_file) as client:
        with pytest.raises(ProtocolError, match="not JSON"):
            client.call("app.state", timeout=2.0)


def test_a_connection_dropped_mid_call_is_reported(make_instance, token_file):
    server = make_instance("drop")
    with connect(server.endpoint, token_path=token_file) as client:
        with pytest.raises(ConnectionLost, match="closed the connection"):
            client.call("app.state", timeout=2.0)


def test_an_unknown_method_reports_the_instance_error_code(instance, token_file):
    with connect(instance.endpoint, token_path=token_file) as client:
        with pytest.raises(MethodError) as caught:
            client.call("nonsense.method")

    assert caught.value.code == -32601


def test_calling_after_the_connection_closed_is_an_error(instance, token_file):
    client = connect(instance.endpoint, token_path=token_file)
    client.close()
    assert not client.connected
    with pytest.raises(ConnectionLost):
        client.call("app.state")


def test_closing_twice_is_not_an_error(instance, token_file):
    client = connect(instance.endpoint, token_path=token_file)
    client.close()
    client.close()


# ---------------------------------------------------------------------------
# Token file
# ---------------------------------------------------------------------------


def test_a_valid_token_file_reads_back_lowercased(tmp_path: Path):
    path = tmp_path / "api.token"
    path.write_text(("AB" * 32) + "\n", encoding="utf-8")
    assert read_token(path) == "ab" * 32


def test_a_missing_token_file_names_the_path_and_the_way_out(tmp_path: Path):
    path = tmp_path / "absent.token"
    with pytest.raises(TokenError) as caught:
        read_token(path)
    assert str(path) in str(caught.value)
    assert "--api-no-auth" in str(caught.value)


def test_a_token_file_holding_something_else_is_rejected(tmp_path: Path):
    path = tmp_path / "api.token"
    path.write_text("not a token", encoding="utf-8")
    with pytest.raises(TokenError, match="hex"):
        read_token(path)


def test_the_token_is_read_when_connecting_not_when_constructing(
    instance: FakeInstance, tmp_path: Path
):
    # An instance publishes its token and then binds its listener, so a token
    # read before the connect can be the previous run's. Constructing a client
    # against a file that does not exist yet, and having it work once the file
    # appears, is what shows the read is deferred.
    path = tmp_path / "api.token"
    client = RattermClient(instance.endpoint, token_path=path)
    assert not path.exists()

    path.write_text(GOOD_TOKEN, encoding="utf-8")
    with client:
        assert client.state()["mode"] == "Normal"


def test_a_missing_token_is_tolerated_when_the_token_is_not_required(
    instance: FakeInstance, tmp_path: Path
):
    # An instance started with --api-no-auth accepts any token, so a client
    # with no token file must still be able to complete the handshake. The
    # fake instance rejects the empty token, which is what proves the request
    # was sent rather than skipped.
    with pytest.raises(AuthenticationError):
        connect(
            instance.endpoint,
            token_path=tmp_path / "absent.token",
            require_token=False,
        )
    assert instance.received[0]["params"]["token"] == ""
