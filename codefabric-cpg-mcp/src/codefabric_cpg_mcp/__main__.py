"""STDIO entrypoint and stderr-only runtime identity surface."""

import json
import sys
from importlib.metadata import version
from platform import python_version

from . import __version__
from .server import create_server
from .settings import process_settings


def _identity() -> dict[str, str]:
    return {
        "adapter": __version__,
        "fastmcp": version("fastmcp"),
        "grpcio": version("grpcio"),
        "protobuf": version("protobuf"),
        "pydantic": version("pydantic"),
        "python": python_version(),
    }


def main() -> int:
    """Run STDIO MCP, or emit machine-readable identity to stderr."""

    if sys.argv[1:] == ["--identity"]:
        print(json.dumps(_identity(), sort_keys=True), file=sys.stderr)
        return 0
    if sys.argv[1:]:
        print("usage: python -m codefabric_cpg_mcp [--identity]", file=sys.stderr)
        return 2

    settings = process_settings()
    server = create_server(settings)
    server.run(transport="stdio", show_banner=False)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
