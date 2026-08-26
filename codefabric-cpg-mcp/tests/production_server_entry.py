"""FastMCP CLI loader that exposes the production server object unchanged."""

from codefabric_cpg_mcp.server import mcp

__all__ = ["mcp"]
