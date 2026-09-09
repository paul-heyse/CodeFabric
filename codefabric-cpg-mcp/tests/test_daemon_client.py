"""Focused generated-v2 tests for the private grpc.aio daemon port."""

from __future__ import annotations

import asyncio
import sys
import time
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Literal, cast

import grpc
import pytest
from pydantic import SecretStr

from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum
from codefabric_cpg_mcp.contracts.wire_models import JsonObject, QueryToolInput, ValidateToolInput
from codefabric_cpg_mcp.daemon import (
    AcceptedQuery,
    CpgDaemonClient,
    DaemonProtocolError,
    DaemonRpcError,
    InputChallenge,
    ManifestSelector,
    PageSelector,
    ReferenceSelector,
    StringInputAnswer,
)
from codefabric_cpg_mcp.daemon.client import _processing_summary, _safe_error
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as query_pb
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2_grpc as query_grpc
from codefabric_cpg_mcp.settings import Settings


@pytest.mark.parametrize(
    ("reference", "expected"),
    [
        (query_pb.SAFE_DIAGNOSTIC_REFERENCE_LIFECYCLE_FAILED_CLOSED, "lifecycle.failed_closed"),
        (query_pb.SAFE_DIAGNOSTIC_REFERENCE_QUERY_CHALLENGE_REJECTED, "query.challenge_rejected"),
        (query_pb.SAFE_DIAGNOSTIC_REFERENCE_QUERY_TERMINAL, "query.terminal"),
    ],
)
def test_released_safe_diagnostics_preserve_their_closed_public_name(
    reference: query_pb.SafeDiagnosticReference, expected: str
) -> None:
    value = query_pb.SafeErrorMetadata(
        code=query_pb.SAFE_ERROR_CODE_INTERNAL,
        layer=query_pb.SAFE_ERROR_LAYER_QUERY,
        diagnostic_reference=reference,
    )
    assert _safe_error(value).diagnostic_reference == expected


def test_typed_processing_rejects_inconsistent_scope_and_preserves_unknown_exhaustion() -> None:
    message = query_pb.QueryProcessingSummary(
        query_id="q1",
        source_generation=3,
        scope="selected_targets",
        family="function-declarations",
        languages=["rust"],
        requested_partitions=2,
        completed_partitions=1,
        remaining_partitions=1,
        remainder=[
            query_pb.ProcessingRemainder(
                language="rust",
                scope_kind="cargo_target",
                path_bytes=b"Cargo.toml",
                path="Cargo.toml",
                target="broken",
                state=query_pb.PROCESSING_STATE_UNAVAILABLE,
                reason_code="compiler_target_unavailable",
            )
        ],
    )
    assert _processing_summary(message).additional_rows is None
    message.additional_rows = False
    assert _processing_summary(message).additional_rows is False
    message.completed_partitions = 2
    with pytest.raises(DaemonProtocolError, match="invalid typed processing"):
        _processing_summary(message)
    message.completed_partitions = 1
    # The wire accepts unknown enum numbers even though the generated stub is closed.
    message.remainder[0].MergeFromString(b"\x30\xe7\x07")
    with pytest.raises(DaemonProtocolError, match="invalid typed processing"):
        _processing_summary(message)
    message.remainder[0].state = query_pb.PROCESSING_STATE_UNAVAILABLE
    message.remainder[0].path = "another-file"
    with pytest.raises(DaemonProtocolError, match="invalid typed processing"):
        _processing_summary(message)


def _settings(
    socket_path: Path,
    *,
    launch_grant: bytes = b"g" * 32,
    daemon_generation: int = 7,
) -> Settings:
    return Settings(
        format="codefabric.adapter-launch.v1",
        query_socket=socket_path,
        launch_grant_hex=SecretStr(launch_grant.hex()),
        adapter_program=Path(sys.executable).resolve(),
        adapter_arguments=("-m", "codefabric_cpg_mcp"),
        daemon_generation=daemon_generation,
        supervisor_generation=11,
        session_expires_at_unix_ms=4_000_000_000_000,
        maximum_request_state_ttl_seconds=30,
        query_timeout_seconds=5.0,
        readiness_timeout_seconds=2.0,
        maximum_resource_chunk_bytes=64 * 1024,
    )


class V2DaemonPortStub(query_grpc.CpgQueryServiceServicer):
    """Independent transport double for the frozen generated contract."""

    def __init__(self) -> None:
        self.session_token = b"s" * 32
        self.query_id = "query:one"
        self.semantic_id = "semantic:one"
        self.public_handle = "handle:manifest:one"
        self.manifest = canonicalize_value({"format": "codefabric.result-manifest.v2"})
        self.page_handle = "handle:page:zero"
        self.page_content = b"arrow-page"
        self.reference_id = "reference:one"
        self.reference_handle = "handle:reference:one"
        self.reference_content = b"reference"
        self.start_legs: list[str] = []
        self.validation_calls = 0
        self.read_calls = 0
        self.cancel_calls = 0
        self.release_calls = 0
        self.session_expires_at_unix_ms = 4_000_000_000_000
        self.empty_start_outcome = False
        self.fail_status = False
        self.status_extra: dict[str, object] = {}
        self.source_observations: list[query_pb.WorkspaceSourceObservation] = []
        self.unsafe_diagnostic = False
        self.progress_stage: int | None = None
        self.challenge_fault: str | None = None
        self.handshake_calls = 0
        self.watch_calls = 0
        self.watch_cursors: list[bytes | None] = []
        self.snapshot_freshness: int | None = query_pb.SNAPSHOT_FRESHNESS_CURRENT
        self.disconnect_after: Literal["snapshot_pinned", "result_ready"] | None = None
        self.change_replayed_result = False
        self.replacement_daemon_generation = 7
        self.active_session_id = "session:one"
        self.active_session_generation = 3
        self.active_daemon_generation = 7
        self.issued_manifest_handles = [self.public_handle]

    def authority(self) -> query_pb.AuthorityGeneration:
        return query_pb.AuthorityGeneration(
            session_id=self.active_session_id,
            session_generation=self.active_session_generation,
            daemon_generation=self.active_daemon_generation,
            supervisor_generation=11,
            policy_generation=5,
            revocation_generation=2,
        )

    def assert_session(
        self,
        request_context: query_pb.RequestContext,
        context: grpc.aio.ServicerContext,
    ) -> None:
        assert ("codefabric-session-bin", self.session_token) in tuple(
            context.invocation_metadata() or ()
        )
        assert request_context.correlation_id == "mcp-request:one"
        assert set(request_context.DESCRIPTOR.fields_by_name) == {
            "correlation_id",
            "remaining_budget",
        }
        assert request_context.remaining_budget.ToTimedelta().total_seconds() > 0

    # pyrefly: ignore [bad-override]
    async def Handshake(
        self,
        request: query_pb.HandshakeRequest,
        context: grpc.aio.ServicerContext,
    ) -> query_pb.HandshakeResponse:
        del context
        expected_grant = b"g" * 32 if self.handshake_calls == 0 else b"h" * 32
        assert request.launch_grant == expected_grant
        assert request.remaining_budget.ToTimedelta().total_seconds() > 0
        self.handshake_calls += 1
        if self.handshake_calls > 1:
            self.active_daemon_generation = self.replacement_daemon_generation
            self.active_session_generation = (
                self.active_session_generation + 1 if self.active_daemon_generation == 7 else 1
            )
            self.active_session_id = f"session:replacement:{self.handshake_calls}"
            self.session_token = b"t" * 32
            self.public_handle = f"handle:manifest:reissued:{self.handshake_calls}"
            self.page_handle = f"handle:page:reissued:{self.handshake_calls}"
            self.issued_manifest_handles.append(self.public_handle)
        return query_pb.HandshakeResponse(
            session_token=self.session_token,
            authority=self.authority(),
            selected_minor=0,
            selected_semantic_profile="codefabric.semantic-query.v2",
            lifecycle=query_pb.LIFECYCLE_STATE_READY,
            effective_limits=query_pb.EffectiveLimits(
                maximum_control_message_bytes=4 * 1024 * 1024,
                maximum_resource_chunk_bytes=64 * 1024,
                maximum_result_bytes=16 * 1024 * 1024,
                maximum_result_pages=32,
                maximum_concurrent_queries=4,
                maximum_watch_events=64,
                maximum_challenge_fields=4,
                maximum_choices_per_field=8,
                maximum_challenge_rounds=3,
                maximum_reference_completion_candidates=10,
                maximum_validation_issues=8,
            ),
            session_expires_at_unix_ms=self.session_expires_at_unix_ms,
            reference_index_revision="reference-index:one",
            reserved_control=query_pb.ReservedControlContract(
                reserved_capacity=4,
                operations=[
                    query_pb.RESERVED_CONTROL_OPERATION_HANDSHAKE,
                    query_pb.RESERVED_CONTROL_OPERATION_GET_STATUS,
                    query_pb.RESERVED_CONTROL_OPERATION_CANCEL_QUERY,
                    query_pb.RESERVED_CONTROL_OPERATION_RELEASE_RESOURCE,
                ],
            ),
        )

    # pyrefly: ignore [bad-override]
    async def GetStatus(
        self,
        request: query_pb.GetStatusRequest,
        context: grpc.aio.ServicerContext,
    ) -> query_pb.GetStatusResponse:
        self.assert_session(request.context, context)
        if self.fail_status:
            error = query_pb.SafeErrorMetadata(
                code=query_pb.SAFE_ERROR_CODE_NOT_AUTHORIZED,
                layer=query_pb.SAFE_ERROR_LAYER_AUTHORIZATION,
                retryable=False,
                correlation_id=request.context.correlation_id,
            )
            if self.unsafe_diagnostic:
                error.diagnostic_reference = cast(query_pb.SafeDiagnosticReference, 99)
            context.set_trailing_metadata(
                (("codefabric-safe-error-bin", error.SerializeToString()),)
            )
            await context.abort(grpc.StatusCode.PERMISSION_DENIED, "secret daemon prose")
        public_value: dict[str, object] = {
            "semantic_release": "codefabric-relational-data-fabric@2.3.0",
            "accepted_queries": 1,
            "active_epoch_id": "epoch:one",
            "lifecycle": "READY",
            "lifecycle_sequence": 4,
            "queued_queries": 0,
            "reserved_result_bytes": 1024,
            "reserved_result_pages": 1,
            "running_queries": 0,
        }
        public_value.update(self.status_extra)
        public = canonicalize_value(cast(JsonObject, public_value))
        return query_pb.GetStatusResponse(
            authority=self.authority(),
            lifecycle=query_pb.LIFECYCLE_STATE_READY,
            lifecycle_sequence=4,
            active_epoch_id="epoch:one",
            running_queries=0,
            queued_queries=0,
            canonical_public_status_json=public,
            source_observations=self.source_observations,
        )

    # pyrefly: ignore [bad-override]
    async def GetReference(
        self,
        request: query_pb.GetReferenceRequest,
        context: grpc.aio.ServicerContext,
    ) -> query_pb.GetReferenceResponse:
        self.assert_session(request.context, context)
        if request.WhichOneof("operation") == "completion":
            return query_pb.GetReferenceResponse(
                authority=self.authority(),
                completion=query_pb.ReferenceCompletion(
                    candidates=[
                        query_pb.ReferenceCompletionCandidate(
                            value="guide",
                            presentation_key="reference.kind.guide",
                        )
                    ],
                    total=1,
                    has_more=False,
                ),
            )
        return query_pb.GetReferenceResponse(
            authority=self.authority(),
            reference=query_pb.ReferenceDocument(
                reference_id=self.reference_id,
                resource=query_pb.ResourceDescriptor(
                    kind=query_pb.RESOURCE_KIND_REFERENCE,
                    public_handle=self.reference_handle,
                    media_type="text/plain",
                    byte_length=len(self.reference_content),
                    content_checksum=checksum(self.reference_content),
                    expires_at_unix_ms=1_900_000_000_000,
                    authority=self.authority(),
                ),
            ),
        )

    # pyrefly: ignore [bad-override]
    async def ValidateQuery(
        self,
        request: query_pb.ValidateQueryRequest,
        context: grpc.aio.ServicerContext,
    ) -> query_pb.ValidateQueryResponse:
        self.assert_session(request.context, context)
        self.validation_calls += 1
        assert request.query.request_checksum == checksum(request.query.canonical_request_json)
        return query_pb.ValidateQueryResponse(
            authority=self.authority(),
            preparation=query_pb.QueryPreparation(
                canonical_normalized_request_json=request.query.canonical_request_json,
                semantic_request_id=self.semantic_id,
                cost_class="bounded",
                estimated_result_bytes=1024,
                estimated_result_pages=1,
            ),
        )

    # pyrefly: ignore [bad-override]
    async def StartQuery(
        self,
        request: query_pb.StartQueryRequest,
        context: grpc.aio.ServicerContext,
    ) -> query_pb.StartQueryResponse:
        self.assert_session(request.context, context)
        leg = request.WhichOneof("leg")
        assert leg is not None
        self.start_legs.append(leg)
        if self.empty_start_outcome:
            return query_pb.StartQueryResponse()
        if leg == "initial":
            minimum_length = 64 if self.challenge_fault == "reversed_bounds" else 1
            maximum_length = 1 if self.challenge_fault == "reversed_bounds" else 64
            input_kind = (
                query_pb.CHALLENGE_INPUT_KIND_BOOLEAN
                if self.challenge_fault == "kind_mismatch"
                else query_pb.CHALLENGE_INPUT_KIND_STRING
            )
            requirement = query_pb.InputRequirement(
                semantic_field_id="query.symbol",
                input_kind=input_kind,
                presentation_key="query.symbol",
                required=True,
                constraints=query_pb.ChallengeConstraints(
                    string_constraints=query_pb.ChallengeStringConstraints(
                        minimum_length=minimum_length,
                        maximum_length=maximum_length,
                        format=query_pb.CHALLENGE_STRING_FORMAT_IDENTIFIER,
                    )
                ),
            )
            requirements = [requirement]
            if self.challenge_fault == "duplicate_field":
                requirements.append(requirement)
            return query_pb.StartQueryResponse(
                input_challenge=query_pb.InputChallenge(
                    authority=self.authority(),
                    semantic_request_id=self.semantic_id,
                    challenge_id="challenge:one",
                    round=1,
                    remaining_rounds=2,
                    issued_at_unix_ms=1_800_000_000_000,
                    expires_at_unix_ms=1_800_000_030_000,
                    maximum_answer_bytes=1024,
                    explanation_code=query_pb.CHALLENGE_EXPLANATION_CODE_REQUIRED_INPUT_MISSING,
                    requirements=requirements,
                    daemon_continuation=b"opaque-daemon-continuation",
                )
            )
        assert request.continuation.answers[0].WhichOneof("value") == "string_value"
        return query_pb.StartQueryResponse(
            accepted=query_pb.AcceptedQuery(
                authority=self.authority(),
                daemon_query_id=self.query_id,
                semantic_request_id=self.semantic_id,
                operation_fingerprint="operation:one",
                accepted_at_unix_ms=1_800_000_000_000,
                observation_expires_at_unix_ms=1_800_000_060_000,
                state=query_pb.QUERY_EXECUTION_STATE_QUEUED,
                idempotent_replay=False,
            )
        )

    # pyrefly: ignore [bad-override]
    async def WatchQuery(
        self,
        request: query_pb.WatchQueryRequest,
        context: grpc.aio.ServicerContext,
    ) -> AsyncIterator[query_pb.QueryEvent]:
        self.assert_session(request.context, context)
        self.watch_calls += 1
        cursor = bytes(request.cursor) if request.HasField("cursor") else None
        self.watch_cursors.append(cursor)
        after_sequence = 0 if cursor is None else int(cursor.decode().removeprefix("cursor:"))

        def header(sequence: int) -> query_pb.QueryEventHeader:
            return query_pb.QueryEventHeader(
                authority=self.authority(),
                daemon_query_id=self.query_id,
                sequence=sequence,
                emitted_at_unix_ms=1_800_000_000_000 + sequence,
                cursor=f"cursor:{sequence}".encode(),
            )

        events = [
            query_pb.QueryEvent(
                snapshot_pinned=query_pb.SnapshotPinnedEvent(
                    header=header(1),
                    epoch_id="epoch:one",
                    source_generation=2,
                    activation_head=3,
                    lifecycle_watermark=4,
                    freshness=cast(query_pb.SnapshotFreshness, self.snapshot_freshness),
                    analysis_context_set_id="context-set:one",
                )
            )
        ]
        next_sequence = 2
        if self.progress_stage is not None:
            events.append(
                query_pb.QueryEvent(
                    progress=query_pb.ProgressEvent(
                        header=header(next_sequence),
                        stage=cast(query_pb.ProgressStage, self.progress_stage),
                        completed=1,
                        total=2,
                    )
                )
            )
            next_sequence += 1
        events.append(
            query_pb.QueryEvent(
                result_ready=query_pb.ResultReadyEvent(
                    header=header(next_sequence),
                    package_id="package:one",
                    manifest=query_pb.ResourceDescriptor(
                        kind=query_pb.RESOURCE_KIND_RESULT_MANIFEST,
                        public_handle=self.public_handle,
                        package_id="package:one",
                        media_type="application/vnd.codefabric.result-manifest+json",
                        byte_length=len(self.manifest),
                        content_checksum=checksum(self.manifest),
                        expires_at_unix_ms=1_800_000_060_000,
                        authority=self.authority(),
                    ),
                    total_rows=(3 if self.change_replayed_result and self.watch_calls > 1 else 2),
                    total_pages=1,
                    total_bytes=len(self.page_content),
                    pages=[
                        query_pb.ResourceDescriptor(
                            kind=query_pb.RESOURCE_KIND_RESULT_PAGE,
                            public_handle=self.page_handle,
                            package_id="package:one",
                            page_ordinal=0,
                            media_type="application/vnd.apache.arrow.stream",
                            byte_length=len(self.page_content),
                            content_checksum=checksum(self.page_content),
                            expires_at_unix_ms=1_800_000_060_000,
                            authority=self.authority(),
                        )
                    ],
                )
            )
        )
        next_sequence += 1
        events.append(
            query_pb.QueryEvent(
                terminal=query_pb.TerminalEvent(
                    header=header(next_sequence),
                    state=query_pb.QUERY_EXECUTION_STATE_SUCCEEDED,
                )
            )
        )
        for event in events:
            variant = event.WhichOneof("event")
            assert variant is not None
            payload = getattr(event, variant)
            if payload.header.sequence <= after_sequence:
                continue
            yield event
            if self.watch_calls == 1 and variant == self.disconnect_after:
                await context.abort(grpc.StatusCode.UNAVAILABLE, "injected watch transport loss")

    # pyrefly: ignore [bad-override]
    async def CancelQuery(
        self,
        request: query_pb.CancelQueryRequest,
        context: grpc.aio.ServicerContext,
    ) -> query_pb.CancelQueryResponse:
        self.assert_session(request.context, context)
        self.cancel_calls += 1
        return query_pb.CancelQueryResponse(
            authority=self.authority(),
            cancellation_id=request.cancellation_id,
            acknowledgement=query_pb.CANCELLATION_ACKNOWLEDGEMENT_ALREADY_TERMINAL,
            terminal=query_pb.TerminalObservation(
                state=query_pb.QUERY_EXECUTION_STATE_SUCCEEDED,
                observed_at_unix_ms=1_800_000_000_003,
            ),
            idempotent_replay=False,
        )

    # pyrefly: ignore [bad-override]
    async def ReadResource(
        self,
        request: query_pb.ReadResourceRequest,
        context: grpc.aio.ServicerContext,
    ) -> AsyncIterator[query_pb.ResourceChunk]:
        self.assert_session(request.context, context)
        self.read_calls += 1
        if request.public_handle == self.public_handle:
            assert request.selector.WhichOneof("selector") == "manifest"
            complete_content = self.manifest
        elif request.public_handle == self.page_handle:
            assert request.selector.WhichOneof("selector") == "page"
            assert request.selector.page.page_ordinal == 0
            complete_content = self.page_content
        else:
            assert request.public_handle == self.reference_handle
            assert request.public_handle != self.reference_id
            assert request.selector.WhichOneof("selector") == "reference"
            assert request.selector.reference.kind == query_pb.REFERENCE_KIND_GUIDE
            assert not request.selector.reference.HasField("version")
            complete_content = self.reference_content
        offset = request.offset
        while offset < len(complete_content):
            content = complete_content[offset : offset + request.maximum_bytes]
            offset += len(content)
            yield query_pb.ResourceChunk(
                authority=self.authority(),
                public_handle=request.public_handle,
                offset=offset - len(content),
                content=content,
                content_checksum=checksum(complete_content),
                end_of_resource=offset == len(complete_content),
            )

    # pyrefly: ignore [bad-override]
    async def ReleaseResource(
        self,
        request: query_pb.ReleaseResourceRequest,
        context: grpc.aio.ServicerContext,
    ) -> query_pb.ReleaseResourceResponse:
        self.assert_session(request.context, context)
        self.release_calls += 1
        return query_pb.ReleaseResourceResponse(
            authority=self.authority(),
            release_id=request.release_id,
            state=query_pb.RELEASE_STATE_RELEASED,
            idempotent_replay=False,
        )


@asynccontextmanager
async def _client(
    tmp_path: Path,
    daemon: V2DaemonPortStub,
) -> AsyncIterator[CpgDaemonClient]:
    server = grpc.aio.server()
    query_grpc.add_CpgQueryServiceServicer_to_server(daemon, server)
    socket_path = tmp_path / "daemon-client.sock"
    assert server.add_insecure_port(f"unix://{socket_path}") == 1
    await server.start()
    client = CpgDaemonClient(_settings(socket_path))
    try:
        yield client
    finally:
        await client.close()
        await server.stop(grace=None)
        socket_path.unlink(missing_ok=True)


async def _accepted_query(client: CpgDaemonClient) -> AcceptedQuery:
    challenge = await client.start_query(
        QueryToolInput(request={"workspace_id": "workspace:one"}),
        correlation_id="mcp-request:one",
    )
    assert isinstance(challenge, InputChallenge)
    accepted = await client.continue_query(
        challenge,
        (
            StringInputAnswer(
                semantic_field_id="query.symbol",
                value="target_symbol",
            ),
        ),
        correlation_id="mcp-request:one",
    )
    assert isinstance(accepted, AcceptedQuery)
    return accepted


def test_daemon_port_maps_closed_v2_contract_without_python_authority(tmp_path: Path) -> None:
    daemon = V2DaemonPortStub()

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            status = await client.status(correlation_id="mcp-request:one")
            assert status.lifecycle == "READY"
            preparation = await client.validate(
                ValidateToolInput(request={"workspace_id": "workspace:one"}),
                correlation_id="mcp-request:one",
            )
            assert preparation.valid
            challenge = await client.start_query(
                QueryToolInput(request={"workspace_id": "workspace:one"}),
                correlation_id="mcp-request:one",
            )
            assert isinstance(challenge, InputChallenge)
            accepted = await client.continue_query(
                challenge,
                (
                    StringInputAnswer(
                        semantic_field_id="query.symbol",
                        value="target_symbol",
                    ),
                ),
                correlation_id="mcp-request:one",
            )
            assert isinstance(accepted, AcceptedQuery)
            result = await client.watch_query(
                accepted,
                correlation_id="mcp-request:one",
            )
            assert result.manifest is not None
            assert result.manifest.public_handle == daemon.public_handle
            assert [page.public_handle for page in result.pages] == [daemon.page_handle]
            content = await client.read_resource(
                result.manifest.public_handle,
                ManifestSelector(),
                offset=0,
                maximum_bytes=64 * 1024,
                correlation_id="mcp-request:one",
            )
            assert content == daemon.manifest
            page_content = await client.read_resource(
                result.pages[0].public_handle,
                PageSelector(page_ordinal=0),
                offset=0,
                maximum_bytes=4,
                correlation_id="mcp-request:one",
            )
            assert page_content == daemon.page_content
            reference = await client.reference(
                "guide",
                None,
                correlation_id="mcp-request:one",
            )
            assert reference.reference_id == daemon.reference_id
            assert reference.resource.public_handle == daemon.reference_handle
            assert reference.resource.public_handle != reference.reference_id
            reference_content = await client.read_resource(
                reference.resource.public_handle,
                ReferenceSelector(reference_kind="guide"),
                offset=0,
                maximum_bytes=64 * 1024,
                correlation_id="mcp-request:one",
            )
            assert reference_content == daemon.reference_content
            completion = await client.complete_reference(
                variable="kind",
                prefix="g",
                kind=None,
                selector=None,
                maximum_candidates=100,
                correlation_id="mcp-request:one",
            )
            assert [candidate.value for candidate in completion.candidates] == ["guide"]
            cancelled = await client.cancel_query(
                accepted.daemon_query_id,
                cancellation_id="cancel:mcp-request:one",
                correlation_id="mcp-request:one",
            )
            assert cancelled.acknowledgement == "already_terminal"
            released = await client.release_resource(
                result.manifest.public_handle,
                release_id="release:mcp-request:one",
                correlation_id="mcp-request:one",
            )
            assert released.state == "released"
            page_released = await client.release_resource(
                result.pages[0].public_handle,
                release_id="release-page:mcp-request:one",
                correlation_id="mcp-request:one",
            )
            assert page_released.state == "released"
            reference_released = await client.release_resource(
                reference.resource.public_handle,
                release_id="release-reference:mcp-request:one",
                correlation_id="mcp-request:one",
            )
            assert reference_released.state == "released"

    asyncio.run(exercise())
    assert daemon.start_legs == ["initial", "continuation"]
    assert daemon.validation_calls == 1
    assert (daemon.read_calls, daemon.cancel_calls, daemon.release_calls) == (3, 1, 3)


def test_daemon_port_fails_closed_and_never_exposes_server_prose(tmp_path: Path) -> None:
    daemon = V2DaemonPortStub()

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            daemon.empty_start_outcome = True
            with pytest.raises(DaemonProtocolError, match="closed outcome"):
                await client.start_query(
                    QueryToolInput(request={"workspace_id": "workspace:one"}),
                    correlation_id="mcp-request:one",
                )
            daemon.fail_status = True
            with pytest.raises(DaemonRpcError) as captured:
                await client.status(correlation_id="mcp-request:one")
            assert captured.value.status is grpc.StatusCode.PERMISSION_DENIED
            assert captured.value.error.code == "NOT_AUTHORIZED"
            assert "secret daemon prose" not in str(captured.value)

    asyncio.run(exercise())


def test_daemon_port_rejects_handshake_session_expiry_drift(tmp_path: Path) -> None:
    daemon = V2DaemonPortStub()
    daemon.session_expires_at_unix_ms += 1

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            with pytest.raises(DaemonProtocolError, match="authority or control reservation"):
                await client.connect(correlation_id="mcp-request:one")

    asyncio.run(exercise())


def test_daemon_port_rejects_additive_status_and_unsafe_diagnostic_reference(
    tmp_path: Path,
) -> None:
    daemon = V2DaemonPortStub()

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            daemon.status_extra = {
                "secret_path": "/private/workspace/secret.py",
                "future_internal": {"token": "opaque-secret"},
            }
            with pytest.raises(DaemonProtocolError, match="closed and public"):
                await client.status(correlation_id="mcp-request:one")

            daemon.status_extra = {}
            daemon.fail_status = True
            daemon.unsafe_diagnostic = True
            with pytest.raises(DaemonProtocolError, match="safe error metadata"):
                await client.status(correlation_id="mcp-request:one")

    asyncio.run(exercise())


def test_daemon_port_rejects_unallowlisted_progress_stage(tmp_path: Path) -> None:
    daemon = V2DaemonPortStub()
    daemon.progress_stage = 99

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            challenge = await client.start_query(
                QueryToolInput(request={"workspace_id": "workspace:one"}),
                correlation_id="mcp-request:one",
            )
            assert isinstance(challenge, InputChallenge)
            accepted = await client.continue_query(
                challenge,
                (
                    StringInputAnswer(
                        semantic_field_id="query.symbol",
                        value="target_symbol",
                    ),
                ),
                correlation_id="mcp-request:one",
            )
            assert isinstance(accepted, AcceptedQuery)
            observed: list[str] = []

            async def progress(completed: int, total: int | None, stage: str) -> None:
                del completed, total
                observed.append(stage)

            with pytest.raises(DaemonProtocolError, match="progress stage"):
                await client.watch_query(
                    accepted,
                    correlation_id="mcp-request:one",
                    progress=progress,
                )
            assert observed == []

    asyncio.run(exercise())


@pytest.mark.parametrize(
    ("disconnect_after", "replacement_daemon_generation", "expected_resume_cursor"),
    [
        ("snapshot_pinned", 7, b"cursor:1"),
        ("result_ready", 7, b"cursor:1"),
        ("snapshot_pinned", 8, None),
        ("result_ready", 8, None),
    ],
)
def test_watch_reconnects_with_fresh_session_without_resubmitting_start(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    disconnect_after: Literal["snapshot_pinned", "result_ready"],
    replacement_daemon_generation: int,
    expected_resume_cursor: bytes | None,
) -> None:
    daemon = V2DaemonPortStub()
    daemon.disconnect_after = disconnect_after
    daemon.replacement_daemon_generation = replacement_daemon_generation

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            replacement = _settings(
                client.settings.query_socket,
                launch_grant=b"h" * 32,
                daemon_generation=replacement_daemon_generation,
            )
            monkeypatch.setattr(
                "codefabric_cpg_mcp.daemon.client.next_settings",
                lambda *, timeout_seconds: replacement,
            )
            accepted = await _accepted_query(client)
            original_handle = daemon.public_handle
            result = await client.watch_query(
                accepted,
                correlation_id="mcp-request:one",
            )
            assert result.execution_state == "SUCCEEDED"
            assert result.authority.daemon_generation == replacement_daemon_generation
            assert result.authority.session_id == "session:replacement:2"
            assert result.manifest is not None
            assert result.manifest.public_handle == daemon.public_handle
            assert result.manifest.public_handle != original_handle
            assert [page.public_handle for page in result.pages] == [daemon.page_handle]

    asyncio.run(exercise())
    assert daemon.handshake_calls == 2
    assert daemon.watch_calls == 2
    assert daemon.watch_cursors == [None, expected_resume_cursor]
    assert daemon.start_legs == ["initial", "continuation"]
    assert daemon.issued_manifest_handles == [
        "handle:manifest:one",
        "handle:manifest:reissued:2",
    ]


def test_watch_reconnect_rejects_changed_replayed_result_without_resubmitting_start(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    daemon = V2DaemonPortStub()
    daemon.disconnect_after = "result_ready"
    daemon.change_replayed_result = True

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            replacement = _settings(
                client.settings.query_socket,
                launch_grant=b"h" * 32,
            )
            monkeypatch.setattr(
                "codefabric_cpg_mcp.daemon.client.next_settings",
                lambda *, timeout_seconds: replacement,
            )
            accepted = await _accepted_query(client)
            with pytest.raises(DaemonProtocolError, match="event content changed"):
                await client.watch_query(
                    accepted,
                    correlation_id="mcp-request:one",
                )

    asyncio.run(exercise())
    assert daemon.watch_cursors == [None, b"cursor:1"]
    assert daemon.start_legs == ["initial", "continuation"]


def test_watch_reconnect_settings_renewal_obeys_one_lifetime_budget_without_resubmitting_start(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    daemon = V2DaemonPortStub()
    daemon.disconnect_after = "snapshot_pinned"
    observed_settings_timeouts: list[float] = []

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            replacement = _settings(
                client.settings.query_socket,
                launch_grant=b"h" * 32,
            )

            def delayed_next_settings(*, timeout_seconds: float) -> Settings:
                observed_settings_timeouts.append(timeout_seconds)
                time.sleep(0.2)
                return replacement

            monkeypatch.setattr(
                "codefabric_cpg_mcp.daemon.client.next_settings",
                delayed_next_settings,
            )
            accepted = await _accepted_query(client)
            loop = asyncio.get_running_loop()
            started = loop.time()
            with pytest.raises(DaemonRpcError) as captured:
                await client.watch_query(
                    accepted,
                    correlation_id="mcp-request:one",
                    timeout_seconds=0.05,
                )
            elapsed = loop.time() - started
            assert captured.value.status is grpc.StatusCode.DEADLINE_EXCEEDED
            assert elapsed < 0.15

    asyncio.run(exercise())
    assert len(observed_settings_timeouts) == 1
    assert 0 < observed_settings_timeouts[0] <= 0.05
    assert daemon.handshake_calls == 1
    assert daemon.watch_calls == 1
    assert daemon.start_legs == ["initial", "continuation"]


@pytest.mark.parametrize("fault", ["duplicate_field", "reversed_bounds", "kind_mismatch"])
def test_daemon_port_rejects_malformed_input_requirements(tmp_path: Path, fault: str) -> None:
    daemon = V2DaemonPortStub()
    daemon.challenge_fault = fault

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            with pytest.raises(DaemonProtocolError, match="challenge requirement contract"):
                await client.start_query(
                    QueryToolInput(request={"workspace_id": "workspace:one"}),
                    correlation_id="mcp-request:one",
                )

    asyncio.run(exercise())


def test_daemon_port_source_has_no_displaced_authority_or_duplicate_fields() -> None:
    source = (Path(__file__).parents[1] / "src/codefabric_cpg_mcp/daemon/client.py").read_text(
        encoding="utf-8"
    )
    for forbidden in (
        "_resource_leases",
        "lease_token",
        "freshness_policy",
        "mcp_call_id",
        "rpc_attempt_id",
        "google.protobuf.Any",
        "challenge_json",
        "expected_daemon_generation",
        "expected_session_generation",
        "ProjectionSelector",
    ):
        assert forbidden not in source
    assert set(QueryToolInput.model_fields) == {"request", "delivery"}


@pytest.mark.parametrize(
    ("freshness", "expected"),
    [(None, None), (10, "CURRENT"), (20, "POTENTIALLY_STALE"), (30, "UNAVAILABLE")],
)
def test_snapshot_freshness_presence_and_state_survive_public_watch(
    tmp_path: Path, freshness: int | None, expected: str | None
) -> None:
    daemon = V2DaemonPortStub()
    daemon.snapshot_freshness = freshness

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            accepted = await _accepted_query(client)
            result = await client.watch_query(accepted, correlation_id="mcp-request:one")
            assert result.freshness == expected
            assert result.analysis_context_set_id == "context-set:one"

    asyncio.run(exercise())


def test_explicit_unspecified_snapshot_freshness_is_rejected(tmp_path: Path) -> None:
    daemon = V2DaemonPortStub()
    daemon.snapshot_freshness = query_pb.SNAPSHOT_FRESHNESS_UNSPECIFIED

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            accepted = await _accepted_query(client)
            with pytest.raises(DaemonProtocolError, match="freshness"):
                await client.watch_query(accepted, correlation_id="mcp-request:one")

    asyncio.run(exercise())


@pytest.mark.parametrize("stage", [None, False, True])
def test_source_and_semantic_stage_presence_survives_status(
    tmp_path: Path, stage: bool | None
) -> None:
    daemon = V2DaemonPortStub()
    row = query_pb.WorkspaceSourceObservation(
        workspace_id="workspace:one",
        selected_source_generation=3,
        requested_watermark=8,
        reconciled_watermark=7 if stage else 8,
        freshness=query_pb.SNAPSHOT_FRESHNESS_POTENTIALLY_STALE
        if stage
        else query_pb.SNAPSHOT_FRESHNESS_CURRENT,
        watch_healthy=True,
        rescan_required=False,
        runnable_pending=bool(stage),
    )
    if stage is not None:
        row.source_reconciled_watermark = 8
        row.source_freshness = query_pb.SNAPSHOT_FRESHNESS_CURRENT
        row.semantic_pending = stage
    daemon.source_observations = [row]

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            status = await client.status(correlation_id="mcp-request:one")
            observed = status.source_observations[0]
            assert observed.semantic_pending is stage
            assert observed.source_reconciled_watermark == (None if stage is None else 8)
            assert observed.source_freshness == (None if stage is None else "CURRENT")
            assert observed.runnable_pending is bool(stage)

    asyncio.run(exercise())


@pytest.mark.parametrize("fault", ["unspecified", "missing-stage", "reversed"])
def test_invalid_source_stage_status_is_rejected(tmp_path: Path, fault: str) -> None:
    daemon = V2DaemonPortStub()
    row = query_pb.WorkspaceSourceObservation(
        workspace_id="workspace:one",
        selected_source_generation=3,
        requested_watermark=8,
        reconciled_watermark=7,
        freshness=query_pb.SNAPSHOT_FRESHNESS_POTENTIALLY_STALE,
        watch_healthy=True,
        rescan_required=False,
        runnable_pending=True,
        source_reconciled_watermark=8,
        source_freshness=query_pb.SNAPSHOT_FRESHNESS_CURRENT,
        semantic_pending=True,
    )
    if fault == "unspecified":
        row.source_freshness = query_pb.SNAPSHOT_FRESHNESS_UNSPECIFIED
    elif fault == "missing-stage":
        row.ClearField("semantic_pending")
    else:
        row.source_reconciled_watermark = 6
    daemon.source_observations = [row]

    async def exercise() -> None:
        async with _client(tmp_path, daemon) as client:
            with pytest.raises(DaemonProtocolError):
                await client.status(correlation_id="mcp-request:one")

    asyncio.run(exercise())
