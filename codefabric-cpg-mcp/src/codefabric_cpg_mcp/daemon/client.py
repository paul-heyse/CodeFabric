"""Typed asynchronous client for the private CodeFabric daemon protocol."""

from __future__ import annotations

import asyncio
import platform
import secrets
import time
from contextlib import suppress
from dataclasses import dataclass
from importlib.metadata import version
from typing import Any, cast

import grpc

from ..contracts.json import canonicalize_json, canonicalize_value, checksum
from ..contracts.schemas import schema_fingerprints
from ..contracts.wire_models import JSON_OBJECT_ADAPTER, JsonObject
from ..settings import Settings
from .channel import create_local_channel
from .generated import cpg_query_service_pb2 as query_pb
from .generated import cpg_query_service_pb2_grpc as query_grpc

RPC_VERSION = "1.0"
SEMANTIC_QUERY_VERSION = "1.3"
_HOST_PROFILE_BYTES = canonicalize_value(
    {
        "compression": ["identity"],
        "delivery": ["inline", "resource", "automatic"],
        "resource_links": True,
        "trace_context": True,
    }
)
HOST_PROFILE_DIGEST = checksum(_HOST_PROFILE_BYTES)


class DaemonProtocolError(RuntimeError):
    """The daemon returned an internally inconsistent accepted-protocol result."""


@dataclass(frozen=True, slots=True)
class DaemonQueryResult:
    """Verified canonical result bytes and terminal daemon metadata."""

    semantic_request_id: str
    daemon_query_id: str
    canonical_bytes: bytes
    response: JsonObject | None
    snapshot: JsonObject
    checksum: str
    artifact_id: str
    lease_expires_at_unix_ms: int
    result_row_count: int
    result_byte_count: int
    freshness_state: str
    availability_state: str
    limit_state: str


class CpgDaemonClient:
    """One process-lifetime gRPC channel and its negotiated daemon contract."""

    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.channel: grpc.aio.Channel = create_local_channel(settings.daemon_target)
        self.stub = query_grpc.CpgQueryServiceStub(self.channel)
        self.handshake_response: Any | None = None
        self._connect_lock = asyncio.Lock()
        self._leased_artifacts: dict[str, tuple[str, str, int, int]] = {}

    async def connect(self) -> None:
        """Perform the mandatory version/capability handshake exactly once."""

        if self.handshake_response is not None:
            return
        async with self._connect_lock:
            if self.handshake_response is not None:
                return
            await self._connect()

    async def _connect(self) -> None:
        """Perform the handshake while the caller holds the connection lock."""

        fingerprints = schema_fingerprints()["serialization"]
        request = query_pb.HandshakeRequest(
            adapter_instance_id=self.settings.agent_instance_id,
            adapter_version=version("codefabric-cpg-mcp"),
            fastmcp_version=version("fastmcp"),
            pydantic_version=version("pydantic"),
            python_version=platform.python_version(),
            rpc_versions=query_pb.VersionRange(minimum=RPC_VERSION, maximum=RPC_VERSION),
            semantic_query_versions=query_pb.VersionRange(
                minimum=SEMANTIC_QUERY_VERSION,
                maximum=SEMANTIC_QUERY_VERSION,
            ),
            schema_fingerprints=[
                query_pb.SchemaFingerprint(schema_id=name, version="1.3", digest=digest)
                for name, digest in sorted(cast(dict[str, str], fingerprints).items())
            ],
            desired_workspace_ids=[self.settings.workspace_id],
            host_capabilities=query_pb.HostCapabilityProfile(
                delivery_modes=[
                    query_pb.DELIVERY_PREFERENCE_INLINE,
                    query_pb.DELIVERY_PREFERENCE_RESOURCE,
                    query_pb.DELIVERY_PREFERENCE_AUTO,
                ],
                compression_algorithms=[query_pb.PAYLOAD_COMPRESSION_IDENTITY],
                supports_resource_links=True,
                supports_trace_context=True,
                maximum_frame_bytes=self.settings.max_request_bytes,
                profile_digest=HOST_PROFILE_DIGEST,
            ),
            credential_proof=query_pb.CredentialProof(
                credential_id=self.settings.agent_instance_id,
                capability_token=self.settings.capability_token.get_secret_value().encode(),
            ),
            agent_instance_id=self.settings.agent_instance_id,
        )
        response = await self.stub.Handshake(request, timeout=self.settings.query_timeout_seconds)
        if (
            response.negotiated_rpc_version != RPC_VERSION
            or response.negotiated_semantic_query_version != SEMANTIC_QUERY_VERSION
            or response.negotiated_compression != query_pb.PAYLOAD_COMPRESSION_IDENTITY
        ):
            raise DaemonProtocolError("daemon negotiated an unsupported protocol profile")
        self.handshake_response = response

    async def close(self) -> None:
        """Release the process-lifetime channel."""

        try:
            for artifact_id, (lease_token, _checksum, _expires_at, _byte_count) in tuple(
                self._leased_artifacts.items()
            ):
                try:
                    await self.stub.ReleaseResult(
                        query_pb.ReleaseResultRequest(
                            artifact_id=artifact_id,
                            lease_token=lease_token,
                        ),
                        timeout=self.settings.query_timeout_seconds,
                    )
                except grpc.RpcError:
                    # Daemon expiry/restart already revokes the lease. Channel cleanup
                    # must not be skipped because one best-effort release raced it.
                    pass
                finally:
                    self._leased_artifacts.pop(artifact_id, None)
        finally:
            await self.channel.close()

    async def cancel(self, daemon_query_id: str, handle_token: bytes, reason: str) -> None:
        """Request cancellation for one accepted query handle."""

        response = await self.stub.CancelQuery(
            query_pb.CancelQueryRequest(
                daemon_query_id=daemon_query_id,
                cancel_token=handle_token,
                agent_instance_id=self.settings.agent_instance_id,
                workspace_id=self.settings.workspace_id,
                reason=reason[:256],
            ),
            timeout=self.settings.query_timeout_seconds,
        )
        if response.state not in {
            query_pb.CANCELLATION_STATE_CANCELLED,
            query_pb.CANCELLATION_STATE_CANCELLATION_REQUESTED,
            query_pb.CANCELLATION_STATE_ALREADY_TERMINAL,
        }:
            raise DaemonProtocolError("daemon did not acknowledge query cancellation")

    def canonical_request(self, request: dict[str, Any]) -> bytes:
        """Validate the bounded JSON domain and return RFC 8785 request bytes."""

        value = JSON_OBJECT_ADAPTER.validate_python(request, strict=True)
        canonical = canonicalize_value(value)
        if len(canonical) > self.settings.max_request_bytes:
            raise ValueError("semantic request exceeds the configured byte limit")
        return canonical

    @staticmethod
    def _freshness(request: JsonObject) -> Any:
        value = request.get("freshness_policy")
        if not isinstance(value, str):
            return query_pb.FRESHNESS_POLICY_UNSPECIFIED
        return {
            "best_available_snapshot": query_pb.FRESHNESS_POLICY_BEST_AVAILABLE_SNAPSHOT,
            "wait_for_current": query_pb.FRESHNESS_POLICY_AWAIT_LATEST,
            "current_required": query_pb.FRESHNESS_POLICY_REQUIRE_SOURCE_CURRENT,
        }.get(value, query_pb.FRESHNESS_POLICY_UNSPECIFIED)

    async def validate(self, request: dict[str, Any]) -> tuple[Any, JsonObject]:
        """Validate one canonical semantic request without executing it."""

        canonical = self.canonical_request(request)
        value = cast(JsonObject, JSON_OBJECT_ADAPTER.validate_json(canonical, strict=True))
        response = await self.stub.ValidateQuery(
            query_pb.ValidateQueryRequest(
                agent_instance_id=self.settings.agent_instance_id,
                workspace_id=self.settings.workspace_id,
                semantic_query_version=SEMANTIC_QUERY_VERSION,
                canonical_request_json=canonical,
                request_checksum=checksum(canonical),
                freshness_policy=self._freshness(value),
                host_capability_profile_digest=HOST_PROFILE_DIGEST,
            ),
            timeout=self.settings.query_timeout_seconds,
        )
        normalized_bytes = canonicalize_json(response.canonical_normalized_request_json)
        if (
            normalized_bytes != response.canonical_normalized_request_json
            or checksum(normalized_bytes) != response.normalized_request_checksum
        ):
            raise DaemonProtocolError("normalized request identity differs")
        normalized = cast(
            JsonObject, JSON_OBJECT_ADAPTER.validate_json(normalized_bytes, strict=True)
        )
        return response, normalized

    async def status(self) -> tuple[Any, JsonObject]:
        """Return the verified public status view."""

        response = await self.stub.GetStatus(
            query_pb.StatusRequest(
                agent_instance_id=self.settings.agent_instance_id,
                workspace_id=self.settings.workspace_id,
                include_diagnostics=False,
            ),
            timeout=self.settings.query_timeout_seconds,
        )
        canonical = canonicalize_json(response.canonical_public_status_json)
        if (
            canonical != response.canonical_public_status_json
            or checksum(canonical) != response.status_checksum
        ):
            raise DaemonProtocolError("daemon status identity differs")
        return response, cast(JsonObject, JSON_OBJECT_ADAPTER.validate_json(canonical, strict=True))

    async def execute(self, request: dict[str, Any], delivery: str) -> DaemonQueryResult:
        """Execute, stream, verify, read, and release one immutable result artifact."""

        canonical = self.canonical_request(request)
        value = cast(JsonObject, JSON_OBJECT_ADAPTER.validate_json(canonical, strict=True))
        request_digest = checksum(canonical)
        started = await self.stub.StartQuery(
            query_pb.StartQueryRequest(
                agent_instance_id=self.settings.agent_instance_id,
                workspace_id=self.settings.workspace_id,
                mcp_call_id=f"mcp:{secrets.token_hex(16)}",
                rpc_attempt_id=f"rpc:{secrets.token_hex(16)}",
                semantic_request_id=str(value.get("semantic_request_id", "")),
                semantic_query_version=SEMANTIC_QUERY_VERSION,
                canonical_request_json=canonical,
                request_checksum=request_digest,
                freshness_policy=self._freshness(value),
                delivery_preference={
                    "inline": query_pb.DELIVERY_PREFERENCE_INLINE,
                    "resource": query_pb.DELIVERY_PREFERENCE_RESOURCE,
                    "automatic": query_pb.DELIVERY_PREFERENCE_AUTO,
                }[delivery],
                host_capability_profile_digest=HOST_PROFILE_DIGEST,
                deadline_unix_ms=int((time.time() + self.settings.query_timeout_seconds) * 1000),
                idempotency_key=f"{self.settings.agent_instance_id}:{request_digest}",
                payload_compression=query_pb.PAYLOAD_COMPRESSION_IDENTITY,
            ),
            timeout=self.settings.query_timeout_seconds,
        )
        artifact: Any | None = None
        terminal: Any | None = None
        snapshot: JsonObject | None = None
        try:
            events = self.stub.StreamQuery(
                query_pb.StreamQueryRequest(
                    daemon_query_id=started.daemon_query_id,
                    resume_token=started.resume_token,
                    after_sequence=0,
                ),
                timeout=self.settings.query_timeout_seconds,
            )
            async for event in events:
                variant = event.WhichOneof("event")
                if variant == "snapshot_pinned":
                    snapshot_bytes = canonicalize_json(
                        event.snapshot_pinned.canonical_public_snapshot_metadata_json
                    )
                    if (
                        snapshot_bytes
                        != event.snapshot_pinned.canonical_public_snapshot_metadata_json
                        or checksum(snapshot_bytes) != event.snapshot_pinned.metadata_checksum
                    ):
                        raise DaemonProtocolError("snapshot metadata identity differs")
                    snapshot = cast(
                        JsonObject, JSON_OBJECT_ADAPTER.validate_json(snapshot_bytes, strict=True)
                    )
                elif variant == "artifact_ready":
                    artifact = event.artifact_ready
                elif variant == "terminal":
                    terminal = event.terminal
        except BaseException:
            cancellation = asyncio.create_task(
                self.cancel(started.daemon_query_id, started.resume_token, "adapter interrupted")
            )
            with suppress(BaseException):
                await asyncio.shield(cancellation)
            raise
        if (
            terminal is not None
            and terminal.execution_state != query_pb.QUERY_EXECUTION_STATE_SUCCEEDED
        ):
            raise DaemonProtocolError("daemon query terminated without a successful result")
        if artifact is None or terminal is None or snapshot is None:
            raise DaemonProtocolError(
                "query stream ended without snapshot, artifact, and terminal events"
            )

        use_resource = delivery == "resource" or (
            delivery == "automatic"
            and terminal.result_byte_count > self.settings.inline_result_bytes
        )
        if use_resource:
            self._leased_artifacts[artifact.artifact_id] = (
                artifact.lease_token,
                artifact.artifact_checksum,
                artifact.lease_expires_at_unix_ms,
                terminal.result_byte_count,
            )
            return DaemonQueryResult(
                semantic_request_id=started.effective_semantic_request_id,
                daemon_query_id=started.daemon_query_id,
                canonical_bytes=b"",
                response=None,
                snapshot=snapshot,
                checksum=artifact.artifact_checksum,
                artifact_id=artifact.artifact_id,
                lease_expires_at_unix_ms=artifact.lease_expires_at_unix_ms,
                result_row_count=terminal.result_row_count,
                result_byte_count=terminal.result_byte_count,
                freshness_state=terminal.freshness_state,
                availability_state=terminal.availability_state,
                limit_state=terminal.limit_state,
            )

        payload = await self._read_and_release(
            artifact.artifact_id,
            artifact.lease_token,
            artifact.artifact_checksum,
            terminal.result_byte_count,
        )
        canonical_payload = canonicalize_json(payload)
        if canonical_payload != payload:
            raise DaemonProtocolError("result artifact is not canonical JSON")
        return DaemonQueryResult(
            semantic_request_id=started.effective_semantic_request_id,
            daemon_query_id=started.daemon_query_id,
            canonical_bytes=payload,
            response=cast(JsonObject, JSON_OBJECT_ADAPTER.validate_json(payload, strict=True)),
            snapshot=snapshot,
            checksum=artifact.artifact_checksum,
            artifact_id=artifact.artifact_id,
            lease_expires_at_unix_ms=artifact.lease_expires_at_unix_ms,
            result_row_count=terminal.result_row_count,
            result_byte_count=terminal.result_byte_count,
            freshness_state=terminal.freshness_state,
            availability_state=terminal.availability_state,
            limit_state=terminal.limit_state,
        )

    async def _read_and_release(
        self,
        artifact_id: str,
        lease_token: str,
        artifact_checksum: str,
        result_byte_count: int,
    ) -> bytes:
        """Read, verify, and release one immutable daemon artifact."""

        chunks: list[bytes] = []
        offset = 0
        while True:
            stream = self.stub.ReadResult(
                query_pb.ReadResultRequest(
                    artifact_id=artifact_id,
                    offset=offset,
                    maximum_bytes=self.settings.inline_result_bytes,
                    lease_token=lease_token,
                    accepted_compression=query_pb.PAYLOAD_COMPRESSION_IDENTITY,
                ),
                timeout=self.settings.query_timeout_seconds,
            )
            final = False
            async for chunk in stream:
                if chunk.offset != offset or checksum(chunk.payload) != chunk.payload_checksum:
                    raise DaemonProtocolError("result chunk offset or checksum differs")
                chunks.append(chunk.payload)
                offset += len(chunk.payload)
                final = chunk.final_chunk
            if final:
                break
        payload = b"".join(chunks)
        if checksum(payload) != artifact_checksum or len(payload) != result_byte_count:
            raise DaemonProtocolError("assembled artifact identity differs")
        released = await self.stub.ReleaseResult(
            query_pb.ReleaseResultRequest(
                artifact_id=artifact_id,
                lease_token=lease_token,
            ),
            timeout=self.settings.query_timeout_seconds,
        )
        if not released.released:
            raise DaemonProtocolError("result artifact lease was not released")
        self._leased_artifacts.pop(artifact_id, None)
        return payload

    async def read_resource(self, artifact_id: str) -> bytes:
        """Resolve one process-owned result resource exactly once."""

        lease = self._leased_artifacts.get(artifact_id)
        if lease is None:
            raise DaemonProtocolError("result resource is absent or already released")
        lease_token, artifact_checksum, _expires_at, result_byte_count = lease
        payload = await self._read_and_release(
            artifact_id,
            lease_token,
            artifact_checksum,
            result_byte_count=result_byte_count,
        )
        canonical_payload = canonicalize_json(payload)
        if canonical_payload != payload:
            raise DaemonProtocolError("result resource is not canonical JSON")
        return payload


__all__ = ["CpgDaemonClient", "DaemonProtocolError", "DaemonQueryResult"]
