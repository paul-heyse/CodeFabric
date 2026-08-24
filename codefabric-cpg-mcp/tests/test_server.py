"""In-memory MCP protocol tests for the daemon-backed public catalog."""

import asyncio
from types import SimpleNamespace
from typing import Any

from fastmcp import Client

from codefabric_cpg_mcp.daemon import CpgDaemonClient, DaemonQueryResult
from codefabric_cpg_mcp.server import mcp


def _environment(monkeypatch) -> None:
    monkeypatch.setenv("CODEFABRIC_CPG_DAEMON_TARGET", "unix:///tmp/codefabric.sock")
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "workspace-main")
    monkeypatch.setenv("CODEFABRIC_AGENT_INSTANCE_ID", "pytest-primary")
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "test-secret")


def _handshake() -> SimpleNamespace:
    return SimpleNamespace(
        daemon_version="0.1.0",
        negotiated_rpc_version="1.0",
        negotiated_semantic_query_version="1.3",
        readiness=SimpleNamespace(
            supported_query_forms=[
                "find code entities",
                "retrieve facts",
                "follow relationships",
            ]
        ),
        effective_limits=SimpleNamespace(
            maximum_control_message_bytes=4 * 1024 * 1024,
            maximum_payload_chunk_bytes=1024 * 1024,
        ),
    )


def _snapshot() -> dict[str, Any]:
    return {
        "snapshot_id": "snapshot:00000000000000000000000000000000",
        "workspace_id": "workspace-main",
        "repository_id": None,
        "worktree_id": None,
        "source_generation": 1,
        "source_inventory_digest": "b3:" + "1" * 64,
        "durable_base_publication": "publication:00000000000000000000000000000000",
        "base_table_version_digest": "b3:" + "2" * 64,
        "overlay_generation": 0,
        "overlay_checksum": "b3:" + "3" * 64,
        "analysis_context_set_id": "context-set:00000000000000000000000000000000",
        "analysis_context_ids": ["context:source"],
        "freshness_state": "CURRENT",
        "source_trust_state": "CURRENT_BYTES_VERIFIED",
        "event_stream_health": "HEALTHY",
        "git_acceleration_status": "NOT_REQUIRED",
        "git_operation_summary": None,
        "pending_update_count": 0,
        "ontology_version": "1.3",
        "schema_bundle_version": "1.0",
        "provider_bundle_version": "1.0",
        "derivation_bundle_version": "1.0",
        "query_language_version": "1.3",
        "capability_summaries": [],
        "diagnostic_references": [],
    }


def test_in_memory_protocol_publishes_exact_four_tool_catalog(monkeypatch) -> None:
    _environment(monkeypatch)

    async def connect(client: CpgDaemonClient) -> None:
        client.handshake_response = _handshake()

    async def close(_client: CpgDaemonClient) -> None:
        return None

    monkeypatch.setattr(CpgDaemonClient, "connect", connect)
    monkeypatch.setattr(CpgDaemonClient, "close", close)

    async def exercise() -> None:
        async with Client(mcp) as client:
            assert client.initialize_result is not None
            assert client.initialize_result.serverInfo.name == "CodeFabric Present-State CPG"
            assert await client.ping()
            tools = await client.list_tools()
            assert {tool.name for tool in tools} == {
                "get_code_graph_reference",
                "get_code_graph_status",
                "query_code_graph",
                "validate_code_graph_query",
            }
            assert await client.list_resources() == []
            assert await client.list_prompts() == []
            reference = await client.call_tool(
                "get_code_graph_reference",
                {"reference": "query_tool_output_schema"},
            )
            assert reference.structured_content is not None
            assert reference.structured_content["mode"] == "inline"

    asyncio.run(exercise())


def test_public_tools_shape_daemon_results_through_generated_models(monkeypatch) -> None:
    _environment(monkeypatch)

    async def connect(client: CpgDaemonClient) -> None:
        client.handshake_response = _handshake()

    async def close(_client: CpgDaemonClient) -> None:
        return None

    async def execute(
        _client: CpgDaemonClient, _request: dict, _delivery: str
    ) -> DaemonQueryResult:
        return DaemonQueryResult(
            semantic_request_id="gate-b",
            daemon_query_id="query:gate-b",
            canonical_bytes=b"{}",
            response={
                "snapshot": _snapshot(),
                "query_results": [{"query_id": "q1"}],
            },
            snapshot=_snapshot(),
            checksum="b3:" + "0" * 64,
            artifact_id="artifact:gate-b",
            lease_expires_at_unix_ms=1_800_000,
            result_row_count=1,
            result_byte_count=2,
            freshness_state="CURRENT",
            availability_state="AVAILABLE",
            limit_state="NOT_APPLIED",
        )

    async def validate(_client: CpgDaemonClient, request: dict):
        return (
            SimpleNamespace(
                valid=True,
                effective_semantic_request_id="gate-b",
                provisional_snapshot_checks=["workspace-authorized"],
                canonical_error_records_json=[],
                cost_class="bounded-wave5",
            ),
            request,
        )

    async def status(_client: CpgDaemonClient):
        return (
            SimpleNamespace(readiness=2),
            {
                "freshness_state": "CURRENT",
                "workspace_id": "workspace-main",
                "snapshot": _snapshot(),
            },
        )

    monkeypatch.setattr(CpgDaemonClient, "connect", connect)
    monkeypatch.setattr(CpgDaemonClient, "close", close)
    monkeypatch.setattr(CpgDaemonClient, "execute", execute)
    monkeypatch.setattr(CpgDaemonClient, "validate", validate)
    monkeypatch.setattr(CpgDaemonClient, "status", status)

    async def exercise() -> None:
        async with Client(mcp) as client:
            query = await client.call_tool(
                "query_code_graph", {"request": {"semantic_request_id": "gate-b"}}
            )
            assert query.structured_content is not None
            assert query.structured_content["semantic_request_id"] == "gate-b"
            validated = await client.call_tool(
                "validate_code_graph_query",
                {"request": {"semantic_request_id": "gate-b"}},
            )
            assert validated.structured_content is not None
            assert validated.structured_content["valid"] is True
            current = await client.call_tool("get_code_graph_status", {})
            assert current.structured_content is not None
            assert current.structured_content["ready"] is True
            assert current.structured_content["workspace_id"] == "workspace-main"

    asyncio.run(exercise())
