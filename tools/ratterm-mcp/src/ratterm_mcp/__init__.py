"""An MCP server for driving ratterm through its control API.

:mod:`ratterm_mcp.client` holds the transport and the token handshake and has
no dependency on MCP, so it is usable on its own from a script or a test.
:mod:`ratterm_mcp.server` wraps it in the tools an agent calls.
"""

from __future__ import annotations

__version__ = "0.1.0"

__all__ = ["__version__"]
