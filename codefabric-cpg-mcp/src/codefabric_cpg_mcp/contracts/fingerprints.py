"""Canonical schema and FastMCP protocol fingerprint policies."""

from collections.abc import Iterable, Mapping
from typing import Any, Protocol

from mcp.types import Tool as MCPTool

from .json import canonicalize_value, checksum

FASTMCP_TOOL_PROFILE = "codefabric-fastmcp-tool-manifest-v1"
FASTMCP_TOOL_KEYS = (
    "name",
    "title",
    "description",
    "inputSchema",
    "outputSchema",
    "icons",
    "annotations",
    "_meta",
    "execution",
)
_FASTMCP_TOOL_KEY_SET = frozenset(FASTMCP_TOOL_KEYS)


class FastMCPToolView(Protocol):
    """Stable structural seam for objects that expose an MCP Tool view."""

    def to_mcp_tool(self) -> MCPTool:
        """Return the protocol-owned representation."""
        ...


def normalize_mcp_tool(value: Mapping[str, Any]) -> dict[str, Any]:
    """Select the frozen protocol-facing MCP Tool v1 fields."""

    unexpected = set(value) - _FASTMCP_TOOL_KEY_SET
    if unexpected:
        raise ValueError(f"unexpected MCP Tool fields: {sorted(unexpected)}")
    return {key: value[key] for key in FASTMCP_TOOL_KEYS if key in value}


def fastmcp_tool_manifest(tools: Iterable[FastMCPToolView]) -> dict[str, Any]:
    """Build the sorted canonical-value input to the FastMCP profile."""

    records = [
        normalize_mcp_tool(
            tool.to_mcp_tool().model_dump(mode="json", by_alias=True, exclude_none=True)
        )
        for tool in tools
    ]
    records.sort(key=lambda record: str(record["name"]))
    names = [record["name"] for record in records]
    if len(names) != len(set(names)):
        raise ValueError("duplicate public tool name in fingerprint manifest")
    return {"profile": FASTMCP_TOOL_PROFILE, "tools": records}


def fastmcp_tool_fingerprint(tools: Iterable[FastMCPToolView]) -> str:
    """Fingerprint protocol-facing tool data with RFC 8785 and BLAKE3."""

    return checksum(canonicalize_value(fastmcp_tool_manifest(tools)))
