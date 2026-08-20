"""FastMCP server declaration for the Wave 0 adapter shell."""

from collections.abc import AsyncIterator
from typing import Any

from fastmcp import FastMCP
from fastmcp.server.lifespan import lifespan

from .settings import Settings

SERVER_INSTRUCTIONS = """\
Use CodeFabric to inspect a registered workspace's immutable present-state code graph.
This Wave 0 shell intentionally publishes no tools, resources, or prompts.
"""


@lifespan
async def server_lifespan(_server: FastMCP[Any]) -> AsyncIterator[dict[str, Settings]]:
    """Load one immutable settings snapshot for this process lifetime."""

    settings = Settings()
    try:
        yield {"settings": settings}
    finally:
        # Wave 0 owns no daemon client or other closeable process resource.
        pass


mcp = FastMCP(
    name="CodeFabric Present-State CPG",
    version="1.3.0",
    instructions=SERVER_INSTRUCTIONS,
    lifespan=server_lifespan,
    on_duplicate="error",
    strict_input_validation=True,
    mask_error_details=True,
    list_page_size=50,
    tasks=False,
)

__all__ = ["mcp"]
