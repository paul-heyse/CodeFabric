"""Generated canonical FastMCP protocol fingerprint policy."""

from collections.abc import Iterable, Mapping
from typing import Any, Protocol

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
    def to_mcp_tool(self) -> Any: ...


def normalize_mcp_tool(value: Mapping[str, Any]) -> dict[str, Any]:
    unexpected = set(value) - _FASTMCP_TOOL_KEY_SET
    if unexpected:
        raise ValueError(f"unexpected MCP Tool fields: {sorted(unexpected)}")
    return {key: value[key] for key in FASTMCP_TOOL_KEYS if key in value}


def _manifest(records: list[dict[str, Any]]) -> dict[str, Any]:
    records.sort(key=lambda record: str(record["name"]))
    names = [record["name"] for record in records]
    if len(names) != len(set(names)):
        raise ValueError("duplicate public tool name in fingerprint manifest")
    return {"profile": FASTMCP_TOOL_PROFILE, "tools": records}


def fastmcp_protocol_manifest(tools: Iterable[Any]) -> dict[str, Any]:
    return _manifest(
        [
            normalize_mcp_tool(tool.model_dump(mode="json", by_alias=True, exclude_none=True))
            for tool in tools
        ]
    )


def fastmcp_protocol_fingerprint(tools: Iterable[Any]) -> str:
    return checksum(canonicalize_value(fastmcp_protocol_manifest(tools)))


def fastmcp_tool_manifest(tools: Iterable[FastMCPToolView]) -> dict[str, Any]:
    return _manifest(
        [
            normalize_mcp_tool(
                tool.to_mcp_tool().model_dump(mode="json", by_alias=True, exclude_none=True)
            )
            for tool in tools
        ]
    )


def fastmcp_tool_fingerprint(tools: Iterable[FastMCPToolView]) -> str:
    return checksum(canonicalize_value(fastmcp_tool_manifest(tools)))
