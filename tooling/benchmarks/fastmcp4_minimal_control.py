"""Minimal installed FastMCP 4 modern-only STDIO benchmark control.

This process contains no CodeFabric adapter, daemon channel, provider,
transform, extension, task, session, resource, prompt, or completion handler.
It isolates the pinned framework's process, modern discovery, framing, schema,
and one asynchronous no-op tool cost.  STDOUT is reserved for MCP bytes while
the server is running.
"""

from __future__ import annotations

import json
import logging
import sys
from typing import Any, cast

import fastmcp
from fastmcp import FastMCP
from fastmcp.server.middleware import CallNext, Middleware, MiddlewareContext
from mcp import MCPError

CONTROL_ID = "minimal-fastmcp4-stdio-control-v1"
CONTROL_VERSION = "1.0.0"
MODERN_PROTOCOL_VERSION = "2026-07-28"


def _protocol_version(context: MiddlewareContext[Any]) -> str | None:
    fastmcp_context = context.fastmcp_context
    if fastmcp_context is None or fastmcp_context.request_context is None:
        return None
    return cast(str | None, fastmcp_context.request_context.protocol_version)


def _unsupported_protocol() -> MCPError:
    return MCPError(
        code=-32600,
        message="Unsupported protocol era",
        data={"code": "unsupported_protocol_era"},
    )


class ModernOnlyMiddleware(Middleware):
    """Admit discovery and operation only for the modern product era."""

    async def on_initialize(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        del context, call_next
        raise _unsupported_protocol()

    async def on_discover(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        if _protocol_version(context) not in {None, MODERN_PROTOCOL_VERSION}:
            raise _unsupported_protocol()
        return await call_next(context)

    async def on_request(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        if context.method != "server/discover" and (
            _protocol_version(context) != MODERN_PROTOCOL_VERSION
        ):
            raise _unsupported_protocol()
        return await call_next(context)


def build_control() -> FastMCP[Any]:
    """Construct the exact minimal control without import-time serving."""

    fastmcp.settings.telemetry_mode = "propagation_only"
    fastmcp.settings.mcp_camelcase_compat = False
    logging.getLogger("fastmcp").setLevel(logging.CRITICAL + 1)
    logging.getLogger("mcp").setLevel(logging.CRITICAL + 1)
    server = FastMCP(
        name="CodeFabric WP50 FastMCP 4 minimal control",
        version=CONTROL_VERSION,
        instructions="Benchmark control only.",
        middleware=[ModernOnlyMiddleware()],
        on_duplicate="error",
        strict_input_validation=True,
        mask_error_details=True,
        list_page_size=50,
        tasks=False,
    )

    @server.tool(
        name="no_op", description="Return the supplied benchmark token unchanged."
    )
    async def no_op(value: str = "ready") -> str:
        return value

    return server


def description() -> dict[str, object]:
    """Return static control identity; this is not a benchmark result."""

    return {
        "control_id": CONTROL_ID,
        "control_version": CONTROL_VERSION,
        "protocol_version": MODERN_PROTOCOL_VERSION,
        "transport": "direct-stdio",
        "tasks": False,
        "tools": ["no_op"],
        "resources": [],
        "prompts": [],
        "completion_handlers": [],
        "application_extensions": [],
        "daemon_channel": False,
    }


def main() -> int:
    if sys.argv[1:] == ["--describe"]:
        print(json.dumps(description(), separators=(",", ":"), sort_keys=True))
        return 0
    if sys.argv[1:]:
        print("minimal control accepts only --describe", file=sys.stderr)
        return 2
    build_control().run(transport="stdio", show_banner=False)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
