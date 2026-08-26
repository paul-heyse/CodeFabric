"""Production MCP protocol tests against the generated asynchronous gRPC stubs."""

from __future__ import annotations

import asyncio
import json
import secrets
import time
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

import grpc
import pytest
from fastmcp import Client
from jsonschema import Draft202012Validator
from mcp.types import TextResourceContents

from codefabric_cpg_mcp.contracts.fingerprints import fastmcp_protocol_fingerprint
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum
from codefabric_cpg_mcp.contracts.model_registries import CpgdFeature
from codefabric_cpg_mcp.contracts.schemas import schema_manifest
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as query_pb
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2_grpc as query_grpc
from codefabric_cpg_mcp.server import mcp
from codefabric_cpg_mcp.settings import process_settings

FIXTURE = Path(__file__).parent / "fixtures/production-tool-manifest-v1.json"


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
        "freshness_state": "UNAVAILABLE",
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


class ProductionStubDaemon:
    """Deterministic protocol peer implemented through the generated service base."""

    def __init__(self) -> None:
        self.semantic_execution_state = "NOT_EXECUTED_DEPENDENCY"
        self.availability_state = "NOT_APPLICABLE"
        self.completeness_state = "NOT_APPLICABLE"
        self.freshness_state = "UNAVAILABLE"
        self.limit_state = "HARD_LIMIT_REJECTED"
        self.query_status_state = "NOT_EXECUTED_DEPENDENCY"
        self.truncated = True
        self.notices = ["daemon-authored notice"]
        self.fail_terminal = False
        self.revoked = False
        self.validation_errors: list[bytes] = []
        self.read_result_calls = 0
        self.release_result_calls = 0
        self.start_requests: list[Any] = []
        self.payload = self._payload()

    def _payload(self) -> bytes:
        return canonicalize_value(
            {
                "availability_state": self.availability_state,
                "completeness_state": self.completeness_state,
                "execution_state": self.semantic_execution_state,
                "freshness_state": self.freshness_state,
                "limit_state": self.limit_state,
                "query_results": [{"query_id": "q-dependency"}],
                "snapshot": _snapshot(),
            }
        )

    async def Handshake(self, request: Any, _context: Any) -> Any:  # noqa: N802
        assert request.required_feature_bits == int(CpgdFeature.REQUIRED)
        assert request.host_capabilities.profile_digest
        return query_pb.HandshakeResponse(
            daemon_instance_id="daemon:test",
            daemon_version="0.1.0",
            rust_build="test",
            negotiated_rpc_version="1.0",
            negotiated_semantic_query_version="1.3",
            negotiated_feature_bits=request.required_feature_bits,
            negotiated_compression=query_pb.PAYLOAD_COMPRESSION_IDENTITY,
            effective_limits=query_pb.EffectiveLimitsProfile(
                maximum_control_message_bytes=4 * 1024 * 1024,
                maximum_payload_chunk_bytes=1024 * 1024,
                maximum_inline_response_bytes=1024 * 1024,
                maximum_concurrent_queries=4,
                query_orphan_replay_seconds=60,
                profile_digest="b3:" + "4" * 64,
            ),
            readiness=query_pb.ReadinessSummary(
                readiness=query_pb.WORKSPACE_READINESS_READY,
                active_snapshot_id=_snapshot()["snapshot_id"],
                supported_language_codes=[10, 20],
                supported_query_forms=["find code entities"],
            ),
        )

    async def GetStatus(self, request: Any, _context: Any) -> Any:  # noqa: N802
        status = canonicalize_value(
            {
                "agent_instance_id": request.agent_instance_id,
                "capability_statuses": [],
                "freshness_state": "UNAVAILABLE",
                "notices": ["daemon-status-notice"],
                "ready": True,
                "service_limits": {
                    "maximum_concurrent_queries": 4,
                    "maximum_control_message_bytes": 4 * 1024 * 1024,
                    "maximum_payload_chunk_bytes": 1024 * 1024,
                },
                "snapshot": _snapshot(),
                "supported_languages": ["python", "rust"],
                "supported_request_forms": ["find code entities"],
                "versions": {"daemon": "0.1.0", "rpc": "1.0", "semantic_query": "1.3"},
                "workspace_id": request.workspace_id,
            }
        )
        return query_pb.StatusResponse(
            workspace_id=request.workspace_id,
            readiness=query_pb.WORKSPACE_READINESS_READY,
            canonical_public_status_json=status,
            status_checksum=checksum(status),
            observed_at_unix_ms=int(time.time() * 1000),
        )

    async def ValidateQuery(self, request: Any, _context: Any) -> Any:  # noqa: N802
        return query_pb.ValidateQueryResponse(
            valid=not self.validation_errors,
            canonical_normalized_request_json=request.canonical_request_json,
            normalized_request_checksum=request.request_checksum,
            effective_semantic_request_id="semantic:test",
            provisional_snapshot_checks=["workspace-authorized"],
            canonical_error_records_json=self.validation_errors,
            cost_class="bounded-test",
        )

    async def StartQuery(self, request: Any, _context: Any) -> Any:  # noqa: N802
        self.start_requests.append(request)
        return query_pb.StartQueryResponse(
            daemon_query_id="query:test",
            resume_token=b"resume-token",
            accepted_at_unix_ms=int(time.time() * 1000),
            query_execution_state=query_pb.QUERY_EXECUTION_STATE_ACCEPTED,
            queue_class="interactive",
            negotiated_request_version="1.3",
            negotiated_response_version="1.3",
            effective_semantic_request_id="semantic:test",
            cancel_token=b"cancel-token",
        )

    async def StreamQuery(self, _request: Any, _context: Any) -> AsyncIterator[Any]:  # noqa: N802
        if self.fail_terminal:
            record = canonicalize_value(
                {
                    "code": 410,
                    "detail": "dependency unavailable",
                    "name": "FAILED_DEPENDENCY",
                    "path": ["queries", "q-dependency"],
                    "phase": "EXECUTION",
                }
            )
            yield query_pb.QueryEvent(
                terminal=query_pb.TerminalEvent(
                    execution_state=query_pb.QUERY_EXECUTION_STATE_FAILED,
                    canonical_error_record_json=record,
                    cleanup_state="COMPLETE",
                    semantic_execution_state="FAILED",
                    completeness_state="UNAVAILABLE",
                )
            )
            return

        self.payload = self._payload()
        snapshot = canonicalize_value(_snapshot())
        artifact_checksum = checksum(self.payload)
        yield query_pb.QueryEvent(
            snapshot_pinned=query_pb.SnapshotPinnedEvent(
                canonical_public_snapshot_metadata_json=snapshot,
                metadata_checksum=checksum(snapshot),
            )
        )
        yield query_pb.QueryEvent(
            artifact_ready=query_pb.ArtifactReadyEvent(
                artifact_id="artifact:test",
                artifact_checksum=artifact_checksum,
                content_type="application/json",
                encoding=query_pb.PAYLOAD_COMPRESSION_IDENTITY,
                lease_expires_at_unix_ms=int((time.time() + 60) * 1000),
                lease_token="lease-token",
            )
        )
        yield query_pb.QueryEvent(
            terminal=query_pb.TerminalEvent(
                execution_state=query_pb.QUERY_EXECUTION_STATE_SUCCEEDED,
                availability_state=self.availability_state,
                freshness_state=self.freshness_state,
                limit_state=self.limit_state,
                dependency_state="FAILED_DEPENDENCY",
                canonical_response_checksum=artifact_checksum,
                artifact_id="artifact:test",
                result_row_count=0,
                result_byte_count=len(self.payload),
                cleanup_state="RETAINED_BY_LEASE",
                semantic_execution_state=self.semantic_execution_state,
                completeness_state=self.completeness_state,
                truncated=self.truncated,
                query_statuses=[
                    query_pb.QueryStatusSummary(
                        query_id="q-dependency",
                        execution_state=self.query_status_state,
                        notices=["query-status-notice"],
                    )
                ],
                notices=self.notices,
            )
        )

    async def AttachQuery(self, request: Any, context: Any) -> AsyncIterator[Any]:  # noqa: N802
        del request
        await context.abort(grpc.StatusCode.UNIMPLEMENTED, "attach unused by adapter test")
        if False:  # pragma: no cover - marks this generated RPC as a stream implementation.
            yield query_pb.QueryEvent()

    async def CancelQuery(self, request: Any, _context: Any) -> Any:  # noqa: N802
        return query_pb.CancelQueryResponse(
            daemon_query_id=request.daemon_query_id,
            state=query_pb.CANCELLATION_STATE_CANCELLED,
            acknowledged_at_unix_ms=int(time.time() * 1000),
        )

    async def ReadResult(self, request: Any, context: Any) -> AsyncIterator[Any]:  # noqa: N802
        self.read_result_calls += 1
        if self.revoked:
            await context.abort(grpc.StatusCode.NOT_FOUND, "lease revoked")
        payload = self.payload[request.offset : request.offset + request.maximum_bytes]
        yield query_pb.ResultChunk(
            artifact_id=request.artifact_id,
            offset=request.offset,
            uncompressed_length=len(payload),
            payload=payload,
            payload_checksum=checksum(payload),
            artifact_checksum=checksum(self.payload),
            content_type="application/json",
            encoding=query_pb.PAYLOAD_COMPRESSION_IDENTITY,
            final_chunk=request.offset + len(payload) == len(self.payload),
            lease_expires_at_unix_ms=int((time.time() + 60) * 1000),
        )

    async def ReleaseResult(self, request: Any, _context: Any) -> Any:  # noqa: N802
        self.release_result_calls += 1
        return query_pb.ReleaseResultResponse(artifact_id=request.artifact_id, released=True)


def _environment(monkeypatch: pytest.MonkeyPatch, target: str) -> None:
    monkeypatch.setenv("CODEFABRIC_CPG_DAEMON_TARGET", target)
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "workspace-main")
    monkeypatch.setenv("CODEFABRIC_AGENT_INSTANCE_ID", "pytest-primary")
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "test-secret")
    process_settings.cache_clear()


@asynccontextmanager
async def _production_client(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> AsyncIterator[tuple[Client[Any], ProductionStubDaemon]]:
    daemon = ProductionStubDaemon()
    server = grpc.aio.server()
    query_grpc.add_CpgQueryServiceServicer_to_server(daemon, server)
    socket_path = Path("/tmp") / f"codefabric-wp68-{secrets.token_hex(6)}.sock"
    target = f"unix://{socket_path}"
    assert server.add_insecure_port(target) == 1
    await server.start()
    _environment(monkeypatch, target)
    try:
        async with Client(mcp) as client:
            yield client, daemon
    finally:
        await server.stop(grace=None)
        socket_path.unlink(missing_ok=True)


def test_wp68_behavioral_acceptance(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def exercise() -> None:
        async with _production_client(tmp_path, monkeypatch) as (client, daemon):
            query = await client.call_tool(
                "query_code_graph",
                {"request": {"semantic_request_id": "semantic:test"}, "delivery": "inline"},
            )
            assert query.structured_content is not None
            assert query.structured_content["execution_state"] == "NOT_EXECUTED_DEPENDENCY"
            assert query.structured_content["availability_state"] == "NOT_APPLICABLE"
            assert query.structured_content["completeness_state"] == "NOT_APPLICABLE"
            assert query.structured_content["freshness_state"] == "UNAVAILABLE"
            assert query.structured_content["limit_state"] == "HARD_LIMIT_REJECTED"
            assert query.structured_content["counts"]["truncated"] is True
            assert query.structured_content["query_statuses"] == [
                {"query_id": "q-dependency", "state": "NOT_EXECUTED_DEPENDENCY"}
            ]
            assert query.structured_content["notices"] == daemon.notices
            assert (
                daemon.start_requests[0].freshness_policy == query_pb.FRESHNESS_POLICY_UNSPECIFIED
            )
            assert daemon.start_requests[0].semantic_request_id == ""

            validation_record = canonicalize_value(
                {
                    "code": 400,
                    "detail": "unsupported request form",
                    "name": "INVALID_REQUEST_SCHEMA",
                    "path": ["queries", "0", "form"],
                }
            )
            daemon.validation_errors = [validation_record]
            validated = await client.call_tool(
                "validate_code_graph_query",
                {"request": {"semantic_request_id": "semantic:test"}},
            )
            assert validated.structured_content is not None
            assert validated.structured_content["errors"] == [
                {
                    "code": "INVALID_REQUEST_SCHEMA",
                    "message": "unsupported request form",
                    "path": ["queries", "0", "form"],
                }
            ]

            daemon.fail_terminal = True
            failed = await client.call_tool(
                "query_code_graph",
                {"request": {"semantic_request_id": "semantic:failed"}},
                raise_on_error=False,
            )
            assert failed.is_error is True
            rendered_error = "\n".join(
                block.text for block in failed.content if hasattr(block, "text")
            )
            assert '"name":"FAILED_DEPENDENCY"' in rendered_error
            assert '"path":["queries","q-dependency"]' in rendered_error

    asyncio.run(exercise())


def test_wp68_structural_acceptance(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def exercise() -> None:
        async with _production_client(tmp_path, monkeypatch) as (client, _daemon):
            listed = await client.list_tools()
            accepted = json.loads(FIXTURE.read_text(encoding="utf-8"))
            assert accepted == {
                "fingerprint": fastmcp_protocol_fingerprint(listed),
                "profile": "codefabric-fastmcp-tool-manifest-v1",
                "schema_version": 1,
            }

            query_tool = next(tool for tool in listed if tool.name == "query_code_graph")
            query = await client.call_tool(
                "query_code_graph",
                {"request": {"semantic_request_id": "semantic:test"}, "delivery": "inline"},
            )
            assert query.structured_content is not None
            schemas = schema_manifest()["serialization"]
            assert isinstance(schemas, dict)
            Draft202012Validator(schemas["QueryToolInput"]).validate(
                {"request": {"semantic_request_id": "semantic:test"}, "delivery": "inline"}
            )
            Draft202012Validator(schemas["QueryToolOutput"]).validate(query.structured_content)
            Draft202012Validator.check_schema(query_tool.inputSchema)
            assert query_tool.outputSchema is not None
            Draft202012Validator(query_tool.outputSchema).validate(query.structured_content)

            templates = await client.list_resource_templates()
            assert {str(template.uriTemplate) for template in templates} == {
                "cpg://reference/{reference}/{version}",
                "cpg://result/{artifact_id}",
            }

    asyncio.run(exercise())


def test_wp68_negative_zero_state(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def exercise() -> None:
        async with _production_client(tmp_path, monkeypatch) as (client, daemon):
            query = await client.call_tool(
                "query_code_graph",
                {"request": {"semantic_request_id": "semantic:test"}, "delivery": "resource"},
            )
            assert query.structured_content is not None
            assert query.structured_content["delivery"]["mode"] == "resource"
            assert daemon.read_result_calls == 0

            daemon.revoked = True
            with pytest.raises(Exception, match="lease revoked|resource|NOT_FOUND|result lease"):
                await client.read_resource("cpg://result/artifact:test")
            assert daemon.read_result_calls == 1

    asyncio.run(exercise())


def test_wp68_operational_acceptance(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def exercise() -> None:
        async with _production_client(tmp_path, monkeypatch) as (client, _daemon):
            assert client.initialize_result is not None
            assert client.initialize_result.serverInfo.name == "CodeFabric Present-State CPG"
            assert await client.ping()

            status = await client.call_tool("get_code_graph_status", {})
            assert status.structured_content is not None
            assert status.structured_content["versions"] == {
                "adapter": "1.3",
                "daemon": "0.1.0",
                "rpc": "1.0",
                "semantic_query": "1.3",
            }
            assert status.structured_content["notices"] == ["daemon-status-notice"]

            reference = await client.call_tool(
                "get_code_graph_reference", {"reference": "capabilities"}
            )
            assert reference.structured_content == {
                "media_type": "application/json",
                "mode": "resource",
                "uri": "cpg://reference/capabilities/1.3",
            }
            resources = await client.read_resource("cpg://reference/capabilities/1.3")
            assert len(resources) == 1
            assert isinstance(resources[0], TextResourceContents)
            assert json.loads(resources[0].text)["registry_ids"]

    asyncio.run(exercise())
