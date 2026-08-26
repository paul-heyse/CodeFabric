"""Execute the Gate B semantic request through the locked FastMCP STDIO process."""

from __future__ import annotations

import asyncio
import json
import os
import sys
from pathlib import Path

from fastmcp import Client
from fastmcp.client.transports import StdioTransport


async def _run(request_path: Path) -> dict[str, object]:
    repository_root = Path(__file__).resolve().parents[1]
    request = json.loads(request_path.read_text(encoding="utf-8"))
    environment = os.environ.copy()
    transport = StdioTransport(
        command="uv",
        args=[
            "run",
            "--frozen",
            "--project",
            str(repository_root / "codefabric-cpg-mcp"),
            "python",
            "-m",
            "codefabric_cpg_mcp",
        ],
        env=environment,
        cwd=str(repository_root),
        keep_alive=False,
        log_file=repository_root / "target" / "gate-b-adapter.stderr.log",
    )
    async with Client(transport, timeout=120.0) as client:
        tools = await client.list_tools()
        result = await client.call_tool(
            "query_code_graph",
            {"request": request, "delivery": "inline"},
        )
    if result.structured_content is None:
        raise RuntimeError("Gate B FastMCP call returned no structured content")
    return {
        "structured_content": result.structured_content,
        "tool_names": sorted(tool.name for tool in tools),
        "transport": "stdio",
    }


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: gate_b_adapter_probe.py <request.json>", file=sys.stderr)
        return 2
    result = asyncio.run(_run(Path(sys.argv[1])))
    sys.stdout.write(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
