"""In-memory MCP protocol tests for the empty Wave 0 catalog."""

import asyncio

from fastmcp import Client

from codefabric_cpg_mcp.server import mcp


def test_in_memory_protocol_initializes_and_has_empty_catalog(
    monkeypatch,
) -> None:
    monkeypatch.setenv("CODEFABRIC_CPG_DAEMON_TARGET", "unix:///tmp/codefabric.sock")
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "workspace-main")
    monkeypatch.setenv("CODEFABRIC_AGENT_INSTANCE_ID", "pytest-primary")
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "test-secret")

    async def exercise() -> None:
        async with Client(mcp) as client:
            assert client.initialize_result is not None
            assert client.initialize_result.serverInfo.name == "CodeFabric Present-State CPG"
            assert await client.ping()
            assert await client.list_tools() == []
            assert await client.list_resources() == []
            assert await client.list_prompts() == []

    asyncio.run(exercise())
