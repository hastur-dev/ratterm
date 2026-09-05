"""Tests for the tool layer.

The tools are driven against the fake instance from ``conftest.py``, so this
suite also runs on a machine with no ratterm binary. What it does not cover is
`launch` against a real instance; that needs the binary and is covered by the
headless-smoke job in CI.
"""

from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

pytest.importorskip("mcp", reason="the tool layer needs the mcp package")

from conftest import GOOD_TOKEN
from ratterm_mcp import server
from ratterm_mcp.client import connect
from ratterm_mcp.server import SESSION, ToolError, scenario_argv

EXPECTED_TOOLS = {
    "launch",
    "snapshot",
    "send_key",
    "type_text",
    "send_mouse",
    "state",
    "api",
    "run_scenario",
    "stop",
}


@pytest.fixture(autouse=True)
def clean_session():
    """Leaves no instance behind between tests."""
    SESSION.reset()
    SESSION.process = None
    SESSION.log_path = None
    SESSION.binary = "rat"
    SESSION.fixtures = None
    yield
    SESSION.reset()
    SESSION.process = None


@pytest.fixture
def attached(instance, token_file, monkeypatch):
    """Points the session at the fake instance without launching anything."""
    monkeypatch.delenv("RATTERM_MCP_ENDPOINT", raising=False)
    SESSION.endpoint = instance.endpoint
    SESSION.token_path = token_file
    return instance


# ---------------------------------------------------------------------------
# Registration
# ---------------------------------------------------------------------------


def test_every_documented_tool_is_registered():
    names = {tool.name for tool in asyncio.run(server.mcp.list_tools())}
    assert EXPECTED_TOOLS <= names, f"missing: {EXPECTED_TOOLS - names}"


def test_every_tool_has_a_description():
    for tool in asyncio.run(server.mcp.list_tools()):
        assert tool.description, f"{tool.name} has no description"


# ---------------------------------------------------------------------------
# Tools against a fake instance
# ---------------------------------------------------------------------------


def test_snapshot_returns_the_frame_as_text(attached):
    result = server.snapshot()
    assert result["text"] == "fixture-gpu\nready"
    assert result["width"] == 4
    assert "cells" not in result


def test_snapshot_asks_for_cells_only_when_told_to(attached):
    server.snapshot()
    assert attached.received[-1]["params"] == {"cells": False}

    result = server.snapshot(cells=True)
    assert attached.received[-1]["params"] == {"cells": True}
    assert "cells" in result


def test_a_slow_first_frame_is_retried(make_instance, token_file, monkeypatch):
    monkeypatch.delenv("RATTERM_MCP_ENDPOINT", raising=False)
    server_instance = make_instance("slow_first_frame")
    SESSION.endpoint = server_instance.endpoint
    SESSION.token_path = token_file

    result = server.snapshot()
    assert result["text"] == "fixture-gpu\nready"
    assert server_instance.frames_requested == 2


def test_the_warm_up_waits_out_a_slow_first_frame(make_instance, token_file):
    server_instance = make_instance("slow_first_frame")
    client = connect(server_instance.endpoint, token_path=token_file)
    try:
        elapsed = server._warm_up(client, 10.0)
    finally:
        client.close()

    assert elapsed is not None
    assert server_instance.frames_requested == 2


def test_the_warm_up_gives_up_on_an_error_that_is_not_a_timeout(instance, token_file):
    client = connect(instance.endpoint, token_path=token_file)
    try:
        # The fake instance answers app.snapshot, so drive the warm-up into a
        # different failure by closing the connection under it.
        client.close()
        assert server._warm_up(client, 2.0) is None
    finally:
        client.close()


def test_an_error_that_is_not_a_timeout_is_not_retried(attached):
    with pytest.raises(ToolError):
        server.api("nonsense.method")
    calls = [e for e in attached.received if e["method"] == "nonsense.method"]
    assert len(calls) == 1


def test_state_returns_what_the_instance_reports(attached):
    assert server.state()["mode"] == "Normal"


def test_send_key_passes_the_description_through(attached):
    with pytest.raises(ToolError):
        # The fake instance does not implement app.send_key, which is enough
        # to show the request was formed and sent.
        server.send_key("ctrl+q")
    assert attached.received[-1]["method"] == "app.send_key"
    assert attached.received[-1]["params"] == {"key": "ctrl+q"}


def test_type_text_and_send_mouse_use_the_documented_parameters(attached):
    with pytest.raises(ToolError):
        server.type_text("hello")
    assert attached.received[-1]["params"] == {"text": "hello"}

    with pytest.raises(ToolError):
        server.send_mouse("left_down@10,4")
    assert attached.received[-1]["params"] == {"event": "left_down@10,4"}


def test_the_api_tool_reaches_a_method_the_typed_tools_do_not_cover(attached):
    assert server.api("system.get_version") == {"version": "0.0.0-fake"}


def test_a_method_the_instance_refuses_becomes_a_tool_error(attached):
    with pytest.raises(ToolError, match=r"nonsense\.method"):
        server.api("nonsense.method")


def test_the_handshake_is_sent_once_per_connection(attached):
    server.state()
    server.state()
    methods = [entry["method"] for entry in attached.received]
    assert methods.count("session.authenticate") == 1


def test_a_tool_reconnects_after_the_connection_is_closed(attached):
    server.state()
    assert SESSION.client is not None
    SESSION.client.close()
    assert server.state()["mode"] == "Normal"


# ---------------------------------------------------------------------------
# No instance
# ---------------------------------------------------------------------------


def test_a_tool_with_no_instance_says_to_launch_first(monkeypatch):
    monkeypatch.delenv("RATTERM_MCP_ENDPOINT", raising=False)
    with pytest.raises(ToolError, match="launch"):
        server.state()


def test_the_endpoint_can_come_from_the_environment(instance, token_file, monkeypatch):
    monkeypatch.setenv("RATTERM_MCP_ENDPOINT", str(instance.endpoint.port))
    monkeypatch.setenv("RATTERM_MCP_TOKEN_FILE", str(token_file))
    assert server.state()["mode"] == "Normal"


def test_stop_without_a_launch_says_so():
    result = server.stop()
    assert result["stopped"] is False


def test_launch_reports_a_binary_that_does_not_exist(tmp_path: Path):
    with pytest.raises(ToolError, match="could not start"):
        server.launch(binary=str(tmp_path / "no-such-rat"))


def test_launch_rejects_a_non_loopback_endpoint():
    with pytest.raises(ToolError, match="loopback"):
        server.launch(binary="rat", endpoint="0.0.0.0:9000")


# ---------------------------------------------------------------------------
# Scenario command line
# ---------------------------------------------------------------------------


def test_a_scenario_file_uses_the_single_file_flag(tmp_path: Path):
    path = tmp_path / "01-startup.yaml"
    path.write_text("name: x\nsteps: []\n", encoding="utf-8")
    argv = scenario_argv("rat", str(path))
    assert argv[:3] == ["rat", "--scenario", str(path)]
    assert "--no-update" in argv


def test_a_scenario_directory_uses_the_directory_flag(tmp_path: Path):
    argv = scenario_argv("rat", str(tmp_path))
    assert argv[:3] == ["rat", "--scenario-dir", str(tmp_path)]


def test_fixtures_and_results_directory_are_passed_through(tmp_path: Path):
    argv = scenario_argv(
        "rat", str(tmp_path), results_dir="out", fixtures="tests/fixtures/fleet"
    )
    assert "--fixtures" in argv
    assert argv[argv.index("--fixtures") + 1] == "tests/fixtures/fleet"
    assert argv[argv.index("--results-dir") + 1] == "out"


def test_the_token_used_is_the_one_in_the_token_file(attached):
    server.state()
    assert attached.received[0]["params"]["token"] == GOOD_TOKEN
