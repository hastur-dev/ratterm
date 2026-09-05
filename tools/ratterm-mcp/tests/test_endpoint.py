"""Tests for endpoint parsing and the token path."""

from __future__ import annotations

from pathlib import Path

import pytest

from ratterm_mcp.client import (
    LocalEndpoint,
    TcpEndpoint,
    default_endpoint,
    default_token_path,
    find_free_port,
    parse_endpoint,
)


def test_a_bare_port_is_a_loopback_tcp_endpoint():
    endpoint = parse_endpoint("47119")
    assert endpoint == TcpEndpoint("127.0.0.1", 47119)
    assert endpoint.cli_args() == ["--api-tcp", "127.0.0.1:47119"]


def test_an_address_with_a_port_is_a_tcp_endpoint():
    assert parse_endpoint("127.0.0.1:9000") == TcpEndpoint("127.0.0.1", 9000)


def test_a_tcp_scheme_is_accepted():
    assert parse_endpoint("tcp://127.0.0.1:9000") == TcpEndpoint("127.0.0.1", 9000)


def test_a_windows_pipe_name_is_a_local_endpoint():
    name = r"\\.\pipe\ratterm-api"
    endpoint = parse_endpoint(name)
    assert endpoint == LocalEndpoint(name)
    assert endpoint.cli_args() == ["--api-socket", name]


def test_a_unix_socket_path_is_a_local_endpoint():
    endpoint = parse_endpoint("/tmp/ratterm-1.sock")
    assert endpoint == LocalEndpoint("/tmp/ratterm-1.sock")


def test_a_non_loopback_address_is_refused():
    with pytest.raises(ValueError, match="loopback"):
        parse_endpoint("0.0.0.0:9000")
    with pytest.raises(ValueError, match="loopback"):
        parse_endpoint("10.0.0.217:47113")


def test_an_impossible_port_is_refused():
    with pytest.raises(ValueError, match="usable TCP port"):
        parse_endpoint("0")
    with pytest.raises(ValueError, match="usable TCP port"):
        parse_endpoint("70000")


def test_an_empty_endpoint_is_refused():
    with pytest.raises(ValueError, match="empty"):
        parse_endpoint("   ")


def test_the_default_endpoint_is_a_free_loopback_port():
    endpoint = default_endpoint()
    assert isinstance(endpoint, TcpEndpoint)
    assert endpoint.host == "127.0.0.1"
    assert 1 <= endpoint.port <= 65535


def test_two_free_ports_in_a_row_are_usable():
    first = find_free_port()
    second = find_free_port()
    assert 1 <= first <= 65535
    assert 1 <= second <= 65535


def test_the_default_token_path_matches_the_rust_side():
    path = default_token_path()
    assert path == Path.home() / ".ratterm" / "api.token"


def test_endpoints_describe_themselves_for_logs():
    assert TcpEndpoint("127.0.0.1", 47113).describe() == "tcp://127.0.0.1:47113"
    assert LocalEndpoint("/tmp/x.sock").describe() == "/tmp/x.sock"
