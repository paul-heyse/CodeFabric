"""FastMCP 4 presentation-cell behavior over an application-owned daemon port."""

from __future__ import annotations

import asyncio
import base64
import time
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import fastmcp
import pytest
from fastmcp import Client
from fastmcp.exceptions import ToolError
from mcp import MCPError
from mcp.types import (
    BlobResourceContents,
    ElicitResult,
    InputRequiredResult,
    ResourceTemplateReference,
)
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import SimpleSpanProcessor
from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter
from pydantic import SecretStr

import codefabric_cpg_mcp.server as server_module
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum
from codefabric_cpg_mcp.daemon import (
    AcceptedQuery,
    AuthorityGeneration,
    CancellationResult,
    ChallengeAnswer,
    DaemonProtocolError,
    DaemonQueryResult,
    DaemonStatus,
    InputChallenge,
    InputRequirement,
    PublicDaemonStatus,
    QueryPreparation,
    ReferenceCompletion,
    ReferenceCompletionCandidate,
    ReferenceDocument,
    ReleaseResult,
    ResourceHandle,
    ResourceReadLimits,
    StringChallengeConstraints,
)
from codefabric_cpg_mcp.server import (
    MODERN_PROTOCOL_VERSION,
    REFERENCE_RESOURCE_TEMPLATE,
    RESULT_RESOURCE_TEMPLATE,
    create_server,
)
from codefabric_cpg_mcp.settings import Settings


def _settings() -> Settings:
    return Settings(
        format="codefabric.adapter-launch.v1",
        query_socket=Path("/tmp/codefabric-test-query.sock"),
        launch_grant_hex=SecretStr("ab" * 32),
        adapter_program=Path("/usr/bin/python3"),
        adapter_arguments=("-m", "codefabric_cpg_mcp"),
        daemon_generation=7,
        supervisor_generation=11,
        session_expires_at_unix_ms=int(time.time() * 1000) + 120_000,
        maximum_request_state_ttl_seconds=1,
        query_timeout_seconds=5.0,
        readiness_timeout_seconds=1.0,
        maximum_resource_chunk_bytes=64 * 1024,
    )


def _authority() -> AuthorityGeneration:
    return AuthorityGeneration(
        session_id="session:one",
        session_generation=1,
        daemon_generation=7,
        supervisor_generation=11,
        policy_generation=13,
        revocation_generation=17,
    )


class FakeDaemonPort:
    """Independent typed presentation double; no generated protobuf crosses this seam."""

    def __init__(
        self,
        *,
        challenge: bool = False,
        block_watch: bool = False,
        failed_reads: frozenset[str] = frozenset(),
        status_error: Exception | None = None,
    ) -> None:
        self.settings = _settings()
        self.authority = _authority()
        self.challenge = challenge
        self.challenge_issued = False
        self.block_watch = block_watch
        self.watch_started = asyncio.Event()
        self.watch_timeouts: list[float | None] = []
        self.connect_calls = 0
        self.close_calls = 0
        self.status_calls = 0
        self.reference_calls = 0
        self.completion_calls = 0
        self.validation_calls = 0
        self.start_calls: list[tuple[str, dict[str, Any]]] = []
        self.continue_calls: list[tuple[str, tuple[ChallengeAnswer, ...]]] = []
        self.read_calls: list[tuple[str, object, int, int, str]] = []
        self.cancel_calls: list[tuple[str, str, str]] = []
        self.release_calls: list[tuple[str, str, str]] = []
        self.manifest = canonicalize_value({"format": "codefabric.result-manifest.v2"})
        self.pages = (
            canonicalize_value({"page": 0, "rows": ["a"]}),
            canonicalize_value({"page": 1, "rows": ["b"]}),
        )
        self.failed_reads = failed_reads
        self.status_error = status_error
        self.reference_content = canonicalize_value({"kind": "guide", "version": "2.3.0"})

    def current_settings(self) -> Settings:
        return self.settings

    def current_resource_limits(self) -> ResourceReadLimits:
        return ResourceReadLimits(
            maximum_chunk_bytes=self.settings.maximum_resource_chunk_bytes,
            maximum_resource_bytes=16 * 1024 * 1024,
        )

    async def connect(self, *, correlation_id: str = "adapter-connect") -> None:
        assert correlation_id == "adapter-connect"
        self.connect_calls += 1

    async def close(self) -> None:
        self.close_calls += 1

    async def status(self, *, correlation_id: str) -> DaemonStatus:
        assert correlation_id
        self.status_calls += 1
        if self.status_error is not None:
            raise self.status_error
        return DaemonStatus(
            authority=self.authority,
            lifecycle="READY",
            lifecycle_sequence=3,
            active_epoch_id="epoch:one",
            running_queries=0,
            queued_queries=0,
            public_status=PublicDaemonStatus(
                lifecycle="READY",
                lifecycle_sequence=3,
                active_epoch_id="epoch:one",
                running_queries=0,
                queued_queries=0,
                accepted_queries=1,
                reserved_result_bytes=1024,
                reserved_result_pages=1,
            ),
        )

    async def reference(
        self,
        kind: str,
        version_value: str | None,
        *,
        correlation_id: str,
    ) -> ReferenceDocument:
        assert kind in {
            "capability",
            "guide",
            "recipe",
            "request_schema",
            "response_schema",
            "snapshot",
        }
        assert correlation_id
        self.reference_calls += 1
        return ReferenceDocument(
            authority=self.authority,
            reference_id=f"reference:{self.reference_calls}",
            resource=ResourceHandle(
                kind="reference",
                public_handle=f"public:reference:{self.reference_calls}",
                media_type="application/json",
                byte_length=len(self.reference_content),
                content_checksum=checksum(self.reference_content),
                expires_at_unix_ms=4_000_000_000_000,
                authority=self.authority,
            ),
        )

    async def complete_reference(
        self,
        *,
        variable: str,
        prefix: str,
        kind: str | None,
        selector: str | None,
        maximum_candidates: int,
        correlation_id: str,
    ) -> ReferenceCompletion:
        assert variable in {"kind", "released_version"}
        assert selector is None
        assert maximum_candidates == 100
        assert correlation_id
        self.completion_calls += 1
        candidate = "request_schema" if variable == "kind" else "2.3.0"
        assert candidate.startswith(prefix.replace("-", "_"))
        return ReferenceCompletion(
            authority=self.authority,
            candidates=(
                ReferenceCompletionCandidate(
                    value=candidate,
                    presentation_key=f"reference.{candidate}",
                ),
            ),
            total=1,
            has_more=False,
        )

    async def validate(self, value: object, *, correlation_id: str) -> QueryPreparation:
        assert correlation_id
        self.validation_calls += 1
        request = value.request  # type: ignore[attr-defined]
        return QueryPreparation(
            authority=self.authority,
            valid=True,
            semantic_request_id="semantic:validated",
            normalized_request=request,
            cost_class="bounded",
            estimated_result_bytes=1024,
            estimated_result_pages=1,
        )

    def _accepted(self) -> AcceptedQuery:
        return AcceptedQuery(
            authority=self.authority,
            daemon_query_id="query:one",
            semantic_request_id="semantic:one",
            operation_fingerprint="operation:one",
            accepted_at_unix_ms=int(time.time() * 1000),
            observation_expires_at_unix_ms=4_000_000_000_000,
            state="QUEUED",
            idempotent_replay=False,
        )

    async def start_query(
        self,
        value: object,
        *,
        correlation_id: str,
        **identity: Any,
    ) -> AcceptedQuery | InputChallenge:
        self.start_calls.append((correlation_id, identity))
        if self.challenge and not self.challenge_issued:
            self.challenge_issued = True
            now_ms = int(time.time() * 1000)
            return InputChallenge(
                authority=self.authority,
                semantic_request_id="semantic:guard",
                challenge_id="challenge:one",
                round=1,
                remaining_rounds=1,
                issued_at_unix_ms=now_ms,
                expires_at_unix_ms=now_ms + 60_000,
                maximum_answer_bytes=4096,
                explanation_code="required_input_missing",
                requirements=(
                    InputRequirement(
                        semantic_field_id="traversal_direction",
                        input_kind="string",
                        presentation_key="query.traversal_direction",
                        description_key="query.traversal_direction.description",
                        required=True,
                        constraints=StringChallengeConstraints(
                            minimum_length=1,
                            maximum_length=16,
                            format="identifier",
                        ),
                    ),
                ),
                daemon_continuation=b"daemon-continuation-one",
            )
        return self._accepted()

    async def continue_query(
        self,
        challenge: InputChallenge,
        answers: tuple[ChallengeAnswer, ...],
        *,
        correlation_id: str,
    ) -> AcceptedQuery:
        assert challenge.challenge_id == "challenge:one"
        self.continue_calls.append((correlation_id, answers))
        return self._accepted()

    async def watch_query(
        self,
        accepted: AcceptedQuery,
        *,
        correlation_id: str,
        progress: Any = None,
        timeout_seconds: float | None = None,
    ) -> DaemonQueryResult:
        assert accepted.daemon_query_id == "query:one"
        assert correlation_id
        assert timeout_seconds == self.settings.query_timeout_seconds
        self.watch_timeouts.append(timeout_seconds)
        self.watch_started.set()
        if self.block_watch:
            await asyncio.Event().wait()
        if progress is not None:
            await progress(1, 1, "complete")
        return DaemonQueryResult(
            authority=self.authority,
            semantic_request_id="semantic:one",
            daemon_query_id="query:one",
            execution_state="SUCCEEDED",
            epoch_id="epoch:one",
            package_id="package:one",
            manifest=ResourceHandle(
                kind="result_manifest",
                public_handle="public:manifest:one",
                package_id="package:one",
                media_type="application/json",
                byte_length=len(self.manifest),
                content_checksum=checksum(self.manifest),
                expires_at_unix_ms=4_000_000_000_000,
                authority=self.authority,
            ),
            pages=tuple(
                ResourceHandle(
                    kind="result_page",
                    public_handle=f"public:page:{ordinal}",
                    package_id="package:one",
                    page_ordinal=ordinal,
                    media_type="application/vnd.apache.arrow.stream",
                    byte_length=len(content),
                    content_checksum=checksum(content),
                    expires_at_unix_ms=4_000_000_000_000,
                    authority=self.authority,
                )
                for ordinal, content in enumerate(self.pages)
            ),
            total_rows=2,
            total_pages=len(self.pages),
            total_bytes=len(self.manifest) + sum(map(len, self.pages)),
            notices=(),
        )

    async def read_resource(
        self,
        public_handle: str,
        selector: object,
        *,
        offset: int,
        maximum_bytes: int,
        correlation_id: str,
    ) -> bytes:
        self.read_calls.append((public_handle, selector, offset, maximum_bytes, correlation_id))
        assert offset == 0
        assert maximum_bytes == self.settings.maximum_resource_chunk_bytes
        if public_handle in self.failed_reads:
            raise DaemonProtocolError("injected incomplete read")
        if public_handle.startswith("public:reference"):
            return self.reference_content
        if public_handle.startswith("public:page:"):
            return self.pages[int(public_handle.rsplit(":", 1)[1])]
        return self.manifest

    async def cancel_query(
        self,
        daemon_query_id: str,
        *,
        cancellation_id: str,
        correlation_id: str,
        timeout_seconds: float = 2.0,
    ) -> CancellationResult:
        assert timeout_seconds == self.settings.cancellation_cleanup_timeout_seconds
        self.cancel_calls.append((daemon_query_id, cancellation_id, correlation_id))
        return CancellationResult(
            authority=self.authority,
            cancellation_id=cancellation_id,
            acknowledgement="accepted",
            idempotent_replay=False,
        )

    async def release_resource(
        self,
        public_handle: str,
        *,
        release_id: str,
        correlation_id: str,
        timeout_seconds: float = 2.0,
    ) -> ReleaseResult:
        assert timeout_seconds == self.settings.cancellation_cleanup_timeout_seconds
        self.release_calls.append((public_handle, release_id, correlation_id))
        return ReleaseResult(
            authority=self.authority,
            release_id=release_id,
            state="released",
            idempotent_replay=False,
        )


@asynccontextmanager
async def _client(
    port: FakeDaemonPort,
    *,
    elicitation_handler: Any = None,
) -> AsyncIterator[Client[Any]]:
    server = create_server(_settings(), lambda _settings: port)
    async with Client(
        server,
        mode=MODERN_PROTOCOL_VERSION,
        elicitation_handler=elicitation_handler,
        cache=False,
    ) as client:
        yield client


def test_fastmcp_registers_exact_modern_target_surface() -> None:
    port = FakeDaemonPort()

    async def exercise() -> None:
        server = create_server(_settings(), lambda _settings: port)
        assert server._extensions == {}  # application extension registry, not discovery
        async with Client(server, mode="auto", cache=False) as client:
            tools = {tool.name: tool for tool in await client.list_tools()}
            assert set(tools) == {
                "query_code_graph",
                "validate_code_graph_query",
                "get_code_graph_status",
                "get_code_graph_reference",
            }
            assert set(tools["query_code_graph"].input_schema["properties"]) == {
                "request",
                "delivery",
            }
            assert set(tools["validate_code_graph_query"].input_schema["properties"]) == {"request"}
            assert tools["get_code_graph_status"].input_schema["properties"] == {}
            assert set(tools["get_code_graph_reference"].input_schema["properties"]) == {
                "kind",
                "version",
            }
            templates = await client.list_resource_templates()
            assert {(template.name, str(template.uri_template)) for template in templates} == {
                ("codefabric-result", RESULT_RESOURCE_TEMPLATE),
                ("codefabric-reference", REFERENCE_RESOURCE_TEMPLATE),
            }
            assert await client.list_resources() == []
            assert await client.list_prompts() == []
            discovery = client.session.discover_result
            assert discovery is not None
            assert discovery.capabilities.extensions == {"io.modelcontextprotocol/ui": {}}
            assert discovery.capabilities.tasks is None
            assert discovery.capabilities.completions is not None
            assert fastmcp.settings.telemetry_mode == "propagation_only"
            assert fastmcp.settings.mcp_camelcase_compat is False
            assert fastmcp.settings.telemetry_mode == "propagation_only"

    asyncio.run(exercise())


def test_legacy_initialize_is_rejected_before_business_dispatch() -> None:
    port = FakeDaemonPort()

    async def exercise() -> None:
        server = create_server(_settings(), lambda _settings: port)
        with pytest.raises(MCPError, match="Unsupported protocol era"):
            async with Client(server, mode="legacy", cache=False):
                pytest.fail("legacy server must not become usable")
        assert port.start_calls == []
        assert port.validation_calls == 0
        assert port.status_calls == 0
        assert port.reference_calls == 0

    asyncio.run(exercise())


def test_atomic_query_publishes_scoped_resources_and_releases_out_of_order_reads() -> None:
    port = FakeDaemonPort()

    async def exercise() -> None:
        async with _client(port) as client:
            result = await client.call_tool(
                "query_code_graph",
                {
                    "request": {
                        "semantic_request_id": "semantic:one",
                        "queries": [{"query_id": "q"}],
                    },
                    "delivery": "resource",
                },
            )
            assert result.structured_content is not None
            output = result.structured_content
            assert output["outcome"] == "accepted"
            assert output["daemon_query_id"] == "query:one"
            assert "mcp_call_id" not in output
            assert "rpc_attempt_id" not in output
            assert [page["page_ordinal"] for page in output["pages"]] == [0, 1]
            assert await client.read_resource(output["pages"][1]["uri"])
            assert await client.read_resource(output["manifest"]["uri"])
        assert len(port.start_calls) == 1
        assert port.validation_calls == 0
        assert [call[0] for call in port.read_calls] == [
            "public:page:1",
            "public:manifest:one",
        ]
        assert [call[0] for call in port.release_calls] == [
            "public:page:1",
            "public:manifest:one",
        ]
        assert [call[1] for call in port.release_calls] == [
            "release:public:page:1",
            "release:public:manifest:one",
        ]
        assert [call[4] for call in port.read_calls] == [call[2] for call in port.release_calls]
        assert all("public:page:0" not in call for call in port.release_calls)

    asyncio.run(exercise())


def test_incomplete_resource_read_never_releases_its_handle() -> None:
    port = FakeDaemonPort(failed_reads=frozenset({"public:page:0"}))

    async def exercise() -> None:
        async with _client(port) as client:
            result = await client.call_tool(
                "query_code_graph",
                {
                    "request": {
                        "semantic_request_id": "semantic:one",
                        "queries": [{"query_id": "q"}],
                    },
                    "delivery": "resource",
                },
            )
            assert result.structured_content is not None
            with pytest.raises(MCPError, match="DAEMON_PROTOCOL_ERROR"):
                await client.read_resource(result.structured_content["pages"][0]["uri"])
        assert [call[0] for call in port.read_calls] == ["public:page:0"]
        assert port.release_calls == []

    asyncio.run(exercise())


def test_guard_roundtrip_reenters_with_new_leg_correlation_and_daemon_state() -> None:
    port = FakeDaemonPort(challenge=True)

    async def answer(message: str, response_type: Any, params: Any, context: Any) -> dict[str, str]:
        del response_type, params, context
        assert message == "query.traversal_direction.description"
        return {"value": "outgoing"}

    async def exercise() -> None:
        async with _client(port, elicitation_handler=answer) as client:
            result = await client.call_tool(
                "query_code_graph",
                {
                    "request": {
                        "semantic_request_id": "semantic:guard",
                        "queries": [{"query_id": "q"}],
                    }
                },
            )
            assert result.structured_content is not None
            assert result.structured_content["outcome"] == "accepted"
        assert len(port.start_calls) == 1
        assert port.start_calls[0][1] == {}
        assert len(port.continue_calls) == 1
        assert port.start_calls[0][0] != port.continue_calls[0][0]
        assert port.continue_calls[0][1][0].semantic_field_id == "traversal_direction"
        assert port.continue_calls[0][1][0].value == "outgoing"  # type: ignore[union-attr]

    asyncio.run(exercise())


def test_tampered_guard_state_fails_before_daemon_continuation() -> None:
    port = FakeDaemonPort(challenge=True)

    async def exercise() -> None:
        arguments = {
            "request": {"semantic_request_id": "semantic:guard", "queries": [{"query_id": "q"}]}
        }
        async with _client(port) as client:
            first = await client.session.call_tool(
                "query_code_graph", arguments, allow_input_required=True
            )
            assert isinstance(first, InputRequiredResult)
            assert first.request_state is not None
            assert first.request_state.startswith("v1.")
            tampered = first.request_state[:-1] + ("A" if first.request_state[-1] != "A" else "B")
            with pytest.raises(MCPError, match="Invalid or expired requestState"):
                await client.session.call_tool(
                    "query_code_graph",
                    arguments,
                    input_responses={
                        "traversal_direction": ElicitResult(
                            action="accept", content={"value": "outgoing"}
                        )
                    },
                    request_state=tampered,
                    allow_input_required=True,
                )
        assert port.continue_calls == []

    asyncio.run(exercise())


def test_valid_guard_state_is_bound_to_the_original_tool_arguments() -> None:
    port = FakeDaemonPort(challenge=True)

    async def exercise() -> None:
        arguments = {
            "request": {"semantic_request_id": "semantic:guard", "queries": [{"query_id": "q"}]}
        }
        async with _client(port) as client:
            first = await client.session.call_tool(
                "query_code_graph", arguments, allow_input_required=True
            )
            assert isinstance(first, InputRequiredResult)
            assert first.request_state is not None
            altered_arguments = {
                "request": {
                    "semantic_request_id": "semantic:guard",
                    "queries": [{"query_id": "altered"}],
                }
            }
            with pytest.raises(MCPError, match="Invalid or expired requestState"):
                await client.session.call_tool(
                    "query_code_graph",
                    altered_arguments,
                    input_responses={
                        "traversal_direction": ElicitResult(
                            action="accept", content={"value": "outgoing"}
                        )
                    },
                    request_state=first.request_state,
                    allow_input_required=True,
                )
        assert port.continue_calls == []

    asyncio.run(exercise())


def test_status_reference_resource_and_completion_delegate_to_daemon() -> None:
    port = FakeDaemonPort()

    async def exercise() -> None:
        async with _client(port) as client:
            status = await client.call_tool("get_code_graph_status", {})
            assert status.structured_content is not None
            assert status.structured_content["lifecycle"] == "READY"
            validation = await client.call_tool(
                "validate_code_graph_query",
                {"request": {"semantic_request_id": "semantic:validated", "queries": []}},
            )
            assert validation.structured_content is not None
            assert validation.structured_content["estimated_result_pages"] == 1
            reference = await client.call_tool("get_code_graph_reference", {"kind": "guide"})
            assert reference.structured_content is not None
            uri = reference.structured_content["resource"]["uri"]
            assert uri.startswith("cpg://reference/")
            assert await client.read_resource(uri)
            completion = await client.complete(
                ResourceTemplateReference(uri=REFERENCE_RESOURCE_TEMPLATE),
                {"name": "kind", "value": "req"},
            )
            assert completion.values == ["request-schema"]
            assert completion.total == 1
            assert completion.has_more is False
        assert port.status_calls == 1
        assert port.validation_calls == 1
        assert port.reference_calls == 1
        assert port.completion_calls == 1
        assert len(port.read_calls) == 1
        assert port.release_calls == [
            (
                "public:reference:1",
                "release:public:reference:1",
                port.read_calls[0][4],
            )
        ]

    asyncio.run(exercise())


def test_host_cancellation_uses_daemon_query_identity_and_reraises() -> None:
    port = FakeDaemonPort(block_watch=True)

    async def exercise() -> None:
        async with _client(port) as client:
            call = asyncio.create_task(
                client.call_tool(
                    "query_code_graph",
                    {
                        "request": {
                            "semantic_request_id": "semantic:one",
                            "queries": [{"query_id": "q"}],
                        }
                    },
                )
            )
            await asyncio.wait_for(port.watch_started.wait(), timeout=2)
            call.cancel()
            with pytest.raises(asyncio.CancelledError):
                await call
        assert len(port.cancel_calls) == 1
        query_id, cancellation_id, correlation_id = port.cancel_calls[0]
        assert query_id == "query:one"
        assert cancellation_id == "cancel:query:one"
        assert correlation_id

    asyncio.run(exercise())


def test_replacement_generation_supplies_live_timeouts_and_resource_bounds() -> None:
    port = FakeDaemonPort()
    initial = _settings()
    port.settings = initial.model_copy(
        update={
            "query_timeout_seconds": 1.25,
            "cancellation_cleanup_timeout_seconds": 0.75,
            "maximum_resource_chunk_bytes": 16 * 1024,
        }
    )

    async def exercise() -> None:
        server = create_server(initial, lambda _settings: port)
        async with Client(server, mode=MODERN_PROTOCOL_VERSION, cache=False) as client:
            result = await client.call_tool(
                "query_code_graph",
                {
                    "request": {
                        "semantic_request_id": "semantic:replacement",
                        "queries": [{"query_id": "q"}],
                    },
                    "delivery": "resource",
                },
            )
            assert result.structured_content is not None
            await client.read_resource(result.structured_content["manifest"]["uri"])

    asyncio.run(exercise())
    assert port.watch_timeouts == [1.25]
    assert port.read_calls[0][3] == 16 * 1024


def test_resource_unit_may_span_multiple_negotiated_chunks() -> None:
    port = FakeDaemonPort()
    port.settings = port.settings.model_copy(update={"maximum_resource_chunk_bytes": 16 * 1024})
    port.manifest = b"m" * (32 * 1024)

    async def exercise() -> None:
        async with _client(port) as client:
            result = await client.call_tool(
                "query_code_graph",
                {
                    "request": {
                        "semantic_request_id": "semantic:multi-chunk",
                        "queries": [{"query_id": "q"}],
                    },
                    "delivery": "resource",
                },
            )
            assert result.structured_content is not None
            manifest = result.structured_content["manifest"]
            assert manifest["total_bytes"] == 32 * 1024
            content = await client.read_resource(manifest["uri"])
            assert isinstance(content[0], BlobResourceContents)
            assert base64.b64decode(content[0].blob, validate=True) == port.manifest

    asyncio.run(exercise())
    assert port.read_calls[0][3] == 16 * 1024


def test_replacement_generation_invalidates_a_sealed_old_generation_guard() -> None:
    port = FakeDaemonPort(challenge=True)

    async def exercise() -> None:
        arguments = {
            "request": {"semantic_request_id": "semantic:guard", "queries": [{"query_id": "q"}]}
        }
        async with _client(port) as client:
            first = await client.session.call_tool(
                "query_code_graph", arguments, allow_input_required=True
            )
            assert isinstance(first, InputRequiredResult)
            assert first.request_state is not None
            port.settings = port.settings.model_copy(update={"daemon_generation": 8})
            rejected = await client.session.call_tool(
                "query_code_graph",
                arguments,
                input_responses={
                    "traversal_direction": ElicitResult(
                        action="accept", content={"value": "outgoing"}
                    )
                },
                request_state=first.request_state,
                allow_input_required=True,
            )
            assert not isinstance(rejected, InputRequiredResult)
            assert rejected.is_error
            assert "INVALID_REQUEST_STATE" in str(rejected.content)

    asyncio.run(exercise())
    assert port.continue_calls == []


def test_allowlisted_spans_exclude_handles_payload_answers_and_exception_prose(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    exporter = InMemorySpanExporter()
    provider = TracerProvider()
    provider.add_span_processor(SimpleSpanProcessor(exporter))
    monkeypatch.setattr(
        server_module,
        "_TRACER",
        provider.get_tracer("codefabric.fastmcp.presentation.test"),
    )
    port = FakeDaemonPort(challenge=True)

    async def answer(message: str, response_type: Any, params: Any, context: Any) -> dict[str, str]:
        del message, response_type, params, context
        return {"value": "outgoing-private-answer"}

    async def exercise() -> None:
        async with _client(port, elicitation_handler=answer) as client:
            result = await client.call_tool(
                "query_code_graph",
                {
                    "request": {
                        "semantic_request_id": "semantic:telemetry",
                        "queries": [{"query_id": "q", "private_path": "/private/source/secret.py"}],
                    },
                    "delivery": "resource",
                },
            )
            assert result.structured_content is not None
            await client.read_resource(result.structured_content["manifest"]["uri"])

    asyncio.run(exercise())
    spans = exporter.get_finished_spans()
    assert spans
    span_attributes = [span.attributes or {} for span in spans]
    allowed_attributes = {
        "codefabric.challenge.id",
        "codefabric.daemon.generation",
        "codefabric.mcp.operation",
        "codefabric.mcp.protocol_era",
        "codefabric.mcp.request_id",
        "codefabric.query.id",
        "codefabric.semantic_request.id",
    }
    assert all(span.name == "codefabric.mcp.request" for span in spans)
    assert all(set(attributes) <= allowed_attributes for attributes in span_attributes)
    assert all(not span.events for span in spans)
    assert any(
        attributes.get("codefabric.challenge.id") == "challenge:one"
        for attributes in span_attributes
    )
    assert any(
        attributes.get("codefabric.query.id") == "query:one" for attributes in span_attributes
    )
    rendered = repr(
        [
            (dict(attributes), span.events)
            for attributes, span in zip(span_attributes, spans, strict=True)
        ]
    )
    for forbidden in (
        "public:manifest:one",
        "cpg://result/",
        "/private/source/secret.py",
        "outgoing-private-answer",
        "daemon-continuation-one",
    ):
        assert forbidden not in rendered
    provider.shutdown()


def test_unknown_method_is_collapsed_before_telemetry(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    exporter = InMemorySpanExporter()
    provider = TracerProvider()
    provider.add_span_processor(SimpleSpanProcessor(exporter))
    monkeypatch.setattr(
        server_module,
        "_TRACER",
        provider.get_tracer("codefabric.fastmcp.presentation.unknown-method-test"),
    )
    middleware = server_module.AllowlistedTelemetryMiddleware(_settings)
    context = SimpleNamespace(
        method="token//private/path",
        fastmcp_context=SimpleNamespace(
            request_id="request:one",
            request_context=SimpleNamespace(protocol_version=MODERN_PROTOCOL_VERSION),
        ),
    )

    async def call_next(context: Any) -> str:
        del context
        return "ok"

    assert asyncio.run(middleware.on_request(cast(Any, context), cast(Any, call_next))) == "ok"
    spans = exporter.get_finished_spans()
    assert len(spans) == 1
    attributes = spans[0].attributes or {}
    assert attributes["codefabric.mcp.operation"] == "unsupported"
    assert "token//private/path" not in repr(attributes)
    provider.shutdown()


def test_hostile_request_id_is_not_used_for_spans_or_daemon_correlation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    exporter = InMemorySpanExporter()
    provider = TracerProvider()
    provider.add_span_processor(SimpleSpanProcessor(exporter))
    monkeypatch.setattr(
        server_module,
        "_TRACER",
        provider.get_tracer("codefabric.fastmcp.presentation.hostile-request-id-test"),
    )
    hostile = "secret-token /private/source.py"
    context = SimpleNamespace(
        method="tools/list",
        fastmcp_context=SimpleNamespace(
            request_id=hostile,
            request_context=SimpleNamespace(protocol_version=MODERN_PROTOCOL_VERSION),
        ),
    )
    correlation = server_module.CorrelationMiddleware()
    telemetry = server_module.AllowlistedTelemetryMiddleware(_settings)
    observed: list[str] = []

    async def terminal(context: Any) -> str:
        del context
        observed.append(server_module._correlation_id())
        return "ok"

    async def with_telemetry(context: Any) -> str:
        return await telemetry.on_request(cast(Any, context), cast(Any, terminal))

    async def exercise() -> str:
        return await correlation.on_request(cast(Any, context), cast(Any, with_telemetry))

    assert asyncio.run(exercise()) == "ok"
    assert observed == ["unavailable"]
    assert server_module._correlation_id(cast(Any, SimpleNamespace(request_id=hostile))) == (
        "unavailable"
    )
    spans = exporter.get_finished_spans()
    assert len(spans) == 1
    attributes = spans[0].attributes or {}
    assert attributes["codefabric.mcp.request_id"] == "unavailable"
    assert hostile not in repr(attributes)
    provider.shutdown()


def test_unexpected_daemon_failure_is_safe_on_wire_and_stderr(
    capsys: pytest.CaptureFixture[str],
) -> None:
    port = FakeDaemonPort(
        status_error=RuntimeError("secret-token /private/status/path"),
    )

    async def exercise() -> None:
        async with _client(port) as client:
            with pytest.raises(ToolError, match="INTERNAL"):
                await client.call_tool("get_code_graph_status", {})

    asyncio.run(exercise())
    captured = capsys.readouterr()
    assert "secret-token" not in captured.err
    assert "/private/status/path" not in captured.err
