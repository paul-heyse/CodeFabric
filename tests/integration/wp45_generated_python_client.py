"""Installed-wheel generated-client oracle for the production v2 UDS service."""

from __future__ import annotations

import asyncio
import json
import sys
import time
from pathlib import Path

import codefabric_cpg_mcp
import grpc
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum
from codefabric_cpg_mcp.daemon.channel import create_local_channel
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as pb
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2_grpc as rpc
from google.protobuf.duration_pb2 import Duration

SEMANTIC_PROFILE = "codefabric.semantic-query.v2"
SESSION_METADATA_KEY = "codefabric-session-bin"


def context(correlation_id: str) -> pb.RequestContext:
    return pb.RequestContext(
        correlation_id=correlation_id,
        remaining_budget=Duration(seconds=10),
    )


def submission(workspace_id: str, semantic_request_id: str) -> pb.QuerySubmission:
    request = {
        "specification": "composable semantic CPG fact query",
        "version": "2.0",
        "semantic_request_id": semantic_request_id,
        "scope": {"workspace_id": workspace_id},
        "freshness": {"policy": "best_available_snapshot"},
        "queries": [
            {
                "request": "find code entities",
                "query_id": "query-clause:python-interop",
                "looking_for": "syntax nodes",
                "within": [],
                "where": [],
                "return": {"limit": {"maximum_results": 3}},
            }
        ],
    }
    canonical = canonicalize_value(request)
    return pb.QuerySubmission(
        canonical_request_json=canonical,
        request_checksum=checksum(canonical),
        semantic_request_id=semantic_request_id,
        semantic_profile=SEMANTIC_PROFILE,
        result_limits=pb.ResultLimits(
            maximum_result_bytes=1 << 20,
            maximum_result_pages=4,
        ),
    )


async def start_query(
    stub: rpc.CpgQueryServiceStub,
    metadata: tuple[tuple[str, bytes], ...],
    query: pb.QuerySubmission,
    prefix: str,
) -> pb.AcceptedQuery:
    outcome = await stub.StartQuery(
        pb.StartQueryRequest(
            context=context(f"{prefix}-initial"),
            initial=pb.InitialQueryStart(query=query),
        ),
        metadata=metadata,
        timeout=10.0,
    )
    for expected_round in range(1, 4):
        assert outcome.WhichOneof("outcome") == "input_challenge"
        challenge = outcome.input_challenge
        assert challenge.round == expected_round
        assert len(challenge.requirements) == 1
        outcome = await stub.StartQuery(
            pb.StartQueryRequest(
                context=context(f"{prefix}-round-{expected_round}"),
                continuation=pb.QueryChallengeContinuation(
                    daemon_continuation=challenge.daemon_continuation,
                    challenge_id=challenge.challenge_id,
                    round=challenge.round,
                    answers=[
                        pb.InputAnswer(
                            semantic_field_id=f"field:round-{expected_round}",
                            choice_id=f"choice:round-{expected_round}",
                        )
                    ],
                ),
            ),
            metadata=metadata,
            timeout=10.0,
        )
    assert outcome.WhichOneof("outcome") == "accepted"
    assert outcome.accepted.state in (
        pb.QUERY_EXECUTION_STATE_QUEUED,
        pb.QUERY_EXECUTION_STATE_RUNNING,
    )
    return outcome.accepted


async def read_resource(
    stub: rpc.CpgQueryServiceStub,
    metadata: tuple[tuple[str, bytes], ...],
    public_handle: str,
    selector: pb.ResourceSelector,
    correlation_id: str,
) -> bytes:
    content = bytearray()
    expected_offset = 0
    saw_end = False
    stream = stub.ReadResource(
        pb.ReadResourceRequest(
            context=context(correlation_id),
            public_handle=public_handle,
            selector=selector,
            maximum_bytes=7,
        ),
        metadata=metadata,
        timeout=10.0,
    )
    async for chunk in stream:
        assert chunk.public_handle == public_handle
        assert chunk.offset == expected_offset
        expected_offset += len(chunk.content)
        content.extend(chunk.content)
        saw_end = chunk.end_of_resource
    assert saw_end
    return bytes(content)


async def release_resource(
    stub: rpc.CpgQueryServiceStub,
    metadata: tuple[tuple[str, bytes], ...],
    public_handle: str,
    release_id: str,
) -> pb.ReleaseResourceResponse:
    return await stub.ReleaseResource(
        pb.ReleaseResourceRequest(
            context=context(f"release-{release_id}"),
            public_handle=public_handle,
            release_id=release_id,
        ),
        metadata=metadata,
        timeout=10.0,
    )


async def wait_for(path: Path, label: str) -> None:
    deadline = time.monotonic() + 20.0
    while not path.exists():
        if time.monotonic() >= deadline:
            raise TimeoutError(label)
        await asyncio.sleep(0.01)


async def oracle(
    socket_path: Path,
    launch_grant: bytes,
    workspace_id: str,
    reference_info: Path,
    ready_request: Path,
    ready_ack: Path,
) -> None:
    installed_module = Path(codefabric_cpg_mcp.__file__).resolve()
    assert installed_module.is_relative_to(Path(sys.prefix).resolve())
    assert "codefabric-cpg-mcp/src" not in str(installed_module)
    await wait_for(socket_path, "production UDS was not created")
    await wait_for(
        reference_info, "Rust controller did not publish the reference handle"
    )
    reference = json.loads(reference_info.read_bytes())
    reference_handle = reference["public_handle"]
    reference_content = bytes.fromhex(reference["content_hex"])

    channel = create_local_channel(f"unix://{socket_path}")
    stub = rpc.CpgQueryServiceStub(channel)
    try:
        handshake = await stub.Handshake(
            pb.HandshakeRequest(
                launch_grant=launch_grant,
                adapter_version="python-installed-wheel-generated-production-v2-client",
                minimum_minor=0,
                maximum_minor=0,
                desired_semantic_profiles=[SEMANTIC_PROFILE],
                maximum_resource_chunk_bytes=64 * 1024,
                remaining_budget=Duration(seconds=10),
                correlation_id="python-production-handshake",
            ),
            timeout=10.0,
        )
        assert len(handshake.session_token) == 32
        assert handshake.authority.daemon_generation == 7
        assert handshake.authority.supervisor_generation == 9
        assert handshake.authority.policy_generation == 8
        assert handshake.authority.revocation_generation == 9
        assert handshake.selected_minor == 0
        assert handshake.selected_semantic_profile == SEMANTIC_PROFILE
        assert handshake.effective_limits.maximum_challenge_rounds == 3
        metadata = ((SESSION_METADATA_KEY, bytes(handshake.session_token)),)

        completion = await stub.GetReference(
            pb.GetReferenceRequest(
                context=context("python-completion-deny"),
                completion=pb.ReferenceCompletionRequest(
                    variable=pb.REFERENCE_TEMPLATE_VARIABLE_KIND,
                    selector="authorization-obscured-selector",
                    maximum_candidates=16,
                ),
            ),
            metadata=metadata,
            timeout=10.0,
        )
        assert completion.WhichOneof("result") == "completion"
        assert completion.completion.total == 0
        assert not completion.completion.candidates

        try:
            await stub.GetReference(
                pb.GetReferenceRequest(
                    context=context("python-reference-deny"),
                    read=pb.ReferenceReadRequest(
                        kind=pb.REFERENCE_KIND_GUIDE,
                        version="2.3",
                    ),
                ),
                metadata=metadata,
                timeout=10.0,
            )
        except grpc.aio.AioRpcError as error:
            assert error.code() is grpc.StatusCode.PERMISSION_DENIED
        else:
            raise AssertionError("missing live reference authority was not denied")

        request_context = context("python-forward-only-context")
        assert set(request_context.DESCRIPTOR.fields_by_name) == {
            "correlation_id",
            "remaining_budget",
        }
        try:
            await stub.GetStatus(
                pb.GetStatusRequest(context=request_context),
                metadata=((SESSION_METADATA_KEY, b"x" * 32),),
                timeout=10.0,
            )
        except grpc.aio.AioRpcError as error:
            assert error.code() is grpc.StatusCode.UNAUTHENTICATED
        else:
            raise AssertionError("unregistered metadata session was accepted")

        result_submission = submission(workspace_id, "request:python-production-result")
        validation = await stub.ValidateQuery(
            pb.ValidateQueryRequest(
                context=context("python-pure-validation"),
                query=result_submission,
            ),
            metadata=metadata,
            timeout=10.0,
        )
        assert not validation.preparation.errors
        assert (
            validation.preparation.semantic_request_id
            == "request:python-production-result"
        )
        assert len(validation.preparation.input_requirements) == 1
        assert (
            validation.preparation.input_requirements[0].semantic_field_id
            == "field:round-1"
        )

        ready_request.write_text("validated\n", encoding="utf-8")
        await wait_for(ready_ack, "Rust controller did not open production admission")
        accepted = await start_query(
            stub,
            metadata,
            result_submission,
            "python-production-result",
        )

        saw_snapshot = False
        saw_progress = False
        ready = None
        terminal_state = None
        watch = stub.WatchQuery(
            pb.WatchQueryRequest(
                context=context("python-watch-result"),
                daemon_query_id=accepted.daemon_query_id,
            ),
            metadata=metadata,
            timeout=20.0,
        )
        async for event in watch:
            variant = event.WhichOneof("event")
            if variant == "snapshot_pinned":
                saw_snapshot = True
            elif variant == "progress":
                saw_progress = True
            elif variant == "result_ready":
                ready = event.result_ready
            elif variant == "terminal":
                terminal_state = event.terminal.state
                break
        assert saw_snapshot and saw_progress
        assert terminal_state == pb.QUERY_EXECUTION_STATE_SUCCEEDED
        assert ready is not None
        assert ready.total_rows == 3

        manifest_bytes = await read_resource(
            stub,
            metadata,
            ready.manifest.public_handle,
            pb.ResourceSelector(manifest=pb.ManifestSelector()),
            "python-read-manifest",
        )
        public_manifest = json.loads(manifest_bytes)
        public_manifest_text = json.dumps(public_manifest, sort_keys=True)
        assert "packages/" not in public_manifest_text
        assert "object_store" not in public_manifest_text
        for page in ready.pages:
            page_bytes = await read_resource(
                stub,
                metadata,
                page.public_handle,
                pb.ResourceSelector(
                    page=pb.PageSelector(page_ordinal=page.page_ordinal)
                ),
                "python-read-page",
            )
            assert page_bytes

        observed_reference = await read_resource(
            stub,
            metadata,
            reference_handle,
            pb.ResourceSelector(
                reference=pb.ReferenceReadRequest(
                    kind=pb.REFERENCE_KIND_GUIDE,
                    version="2.3",
                )
            ),
            "python-read-reference",
        )
        assert observed_reference == reference_content
        released_reference = await release_resource(
            stub,
            metadata,
            reference_handle,
            "release:python-reference",
        )
        assert released_reference.state == pb.RELEASE_STATE_RELEASED
        replayed_reference = await release_resource(
            stub,
            metadata,
            reference_handle,
            "release:python-reference",
        )
        assert replayed_reference.state == pb.RELEASE_STATE_ALREADY_RELEASED
        assert replayed_reference.idempotent_replay

        for index, page in enumerate(ready.pages):
            released = await release_resource(
                stub,
                metadata,
                page.public_handle,
                f"release:python-page-{index}",
            )
            assert released.state == pb.RELEASE_STATE_RELEASED
        released_manifest = await release_resource(
            stub,
            metadata,
            ready.manifest.public_handle,
            "release:python-manifest",
        )
        assert released_manifest.state == pb.RELEASE_STATE_RELEASED

        cancelled = await start_query(
            stub,
            metadata,
            submission(workspace_id, "request:python-production-cancel"),
            "python-production-cancel",
        )
        cancellation = pb.CancelQueryRequest(
            context=context("python-cancel"),
            daemon_query_id=cancelled.daemon_query_id,
            cancellation_id="cancel:python-production",
        )
        first_cancel = await stub.CancelQuery(
            cancellation,
            metadata=metadata,
            timeout=10.0,
        )
        assert first_cancel.acknowledgement == pb.CANCELLATION_ACKNOWLEDGEMENT_ACCEPTED
        replay_cancel = await stub.CancelQuery(
            cancellation,
            metadata=metadata,
            timeout=10.0,
        )
        assert replay_cancel.acknowledgement == pb.CANCELLATION_ACKNOWLEDGEMENT_REPLAYED
        assert replay_cancel.idempotent_replay

        cancelled_watch = stub.WatchQuery(
            pb.WatchQueryRequest(
                context=context("python-watch-cancelled"),
                daemon_query_id=cancelled.daemon_query_id,
            ),
            metadata=metadata,
            timeout=20.0,
        )
        async for event in cancelled_watch:
            if event.WhichOneof("event") == "terminal":
                assert event.terminal.state == pb.QUERY_EXECUTION_STATE_CANCELLED
                break
        else:
            raise AssertionError("cancelled query produced no terminal event")
        print("python-installed-wheel-generated-production-client-ok", flush=True)
    finally:
        await channel.close()


def main() -> None:
    asyncio.run(
        oracle(
            socket_path=Path(sys.argv[1]),
            launch_grant=bytes.fromhex(sys.argv[2]),
            workspace_id=sys.argv[3],
            reference_info=Path(sys.argv[4]),
            ready_request=Path(sys.argv[5]),
            ready_ack=Path(sys.argv[6]),
        )
    )


if __name__ == "__main__":
    main()
