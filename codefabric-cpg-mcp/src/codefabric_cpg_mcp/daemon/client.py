"""Typed asynchronous client for the private CodeFabric daemon protocol."""

from __future__ import annotations

import asyncio
import platform
import secrets
import time
from contextlib import suppress
from dataclasses import dataclass
from importlib.metadata import version
from typing import Any, Never, cast

import grpc

from ..contracts.json import canonicalize_json, canonicalize_value, checksum
from ..contracts.model_registries import CpgdFeature
from ..contracts.schemas import schema_fingerprints
from ..contracts.wire_models import (
    JSON_OBJECT_ADAPTER,
    JsonObject,
    QueryToolInput,
    ValidateToolInput,
)
from ..settings import Settings
from .arrow_resources import (
    ARROW_RELEASE,
    PUBLISHED_RESULT_FORMAT,
    ArrowResourceAccessError,
    ArrowResourceChunk,
    ArrowResourceExpiredError,
    ArrowResourceLimitError,
    ArrowResourcePresenter,
    ArrowResourceReleasedError,
    ArrowResultAccess,
    ArrowResultPackageDescriptor,
    ArrowResultReleaseReceipt,
    validate_package_descriptor_json,
)
from .channel import create_local_channel
from .generated import cpg_query_service_pb2 as query_pb
from .generated import cpg_query_service_pb2_grpc as query_grpc

RPC_VERSION = "1.0"
SEMANTIC_QUERY_VERSION = "1.3"


def host_capability_profile_digest(maximum_frame_bytes: int) -> str:
    """Derive the governed digest from every typed host-capability field."""

    return checksum(
        canonicalize_value(
            {
                "compression_algorithms": ["identity"],
                "delivery_modes": ["automatic", "inline", "resource"],
                "maximum_frame_bytes": maximum_frame_bytes,
                "supports_resource_links": True,
                "supports_trace_context": True,
            }
        )
    )


class DaemonProtocolError(RuntimeError):
    """The daemon returned an internally inconsistent accepted-protocol result."""


class DaemonQueryError(DaemonProtocolError):
    """One daemon-authored canonical public error record."""

    def __init__(self, canonical_record: bytes) -> None:
        canonical = canonicalize_json(canonical_record)
        if canonical != canonical_record:
            raise DaemonProtocolError("daemon error record is not canonical JSON")
        self.canonical_bytes = canonical
        self.record = cast(JsonObject, JSON_OBJECT_ADAPTER.validate_json(canonical, strict=True))
        super().__init__(canonical.decode("utf-8"))


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
    execution_state: str
    freshness_state: str
    availability_state: str
    completeness_state: str
    limit_state: str
    truncated: bool
    query_statuses: tuple[JsonObject, ...]
    notices: tuple[str, ...]
    arrow_descriptor: ArrowResultPackageDescriptor | None = None


@dataclass(slots=True)
class _ArrowLeaseEntry:
    descriptor: ArrowResultPackageDescriptor
    access: ArrowResultAccess
    consumed_resource_ids: set[str]


class CpgDaemonClient:
    """One process-lifetime gRPC channel and its negotiated daemon contract."""

    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.channel: grpc.aio.Channel = create_local_channel(settings.daemon_target)
        self.stub = query_grpc.CpgQueryServiceStub(self.channel)
        self.host_profile_digest = host_capability_profile_digest(settings.max_request_bytes)
        self.handshake_response: Any | None = None
        self._connect_lock = asyncio.Lock()
        # Capability material cached only so a later resource read can ask the
        # daemon. Presence here never proves that the daemon still recognizes a lease.
        self._lease_cache: dict[str, tuple[str, str, int, int]] = {}
        self._arrow_leases: dict[str, _ArrowLeaseEntry] = {}

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
            required_feature_bits=int(CpgdFeature.REQUIRED),
            optional_feature_bits=int(CpgdFeature.SUPPORTED & ~CpgdFeature.REQUIRED),
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
                profile_digest=self.host_profile_digest,
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
            or response.negotiated_feature_bits & int(CpgdFeature.REQUIRED)
            != int(CpgdFeature.REQUIRED)
        ):
            raise DaemonProtocolError("daemon negotiated an unsupported protocol profile")
        self.handshake_response = response

    async def close(self) -> None:
        """Release the process-lifetime channel."""

        try:
            for artifact_id, entry in tuple(self._arrow_leases.items()):
                try:
                    await self.release(access=entry.access)
                except grpc.RpcError, DaemonProtocolError:
                    pass
                finally:
                    self._arrow_leases.pop(artifact_id, None)
            for artifact_id, (lease_token, _checksum, _expires_at, _byte_count) in tuple(
                self._lease_cache.items()
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
                    self._lease_cache.pop(artifact_id, None)
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

    def canonical_request(self, request: JsonObject) -> bytes:
        """Validate the bounded JSON domain and return RFC 8785 request bytes."""

        value = JSON_OBJECT_ADAPTER.validate_python(request, strict=True)
        canonical = canonicalize_value(value)
        if len(canonical) > self.settings.max_request_bytes:
            raise ValueError("semantic request exceeds the configured byte limit")
        return canonical

    async def validate(self, tool_input: ValidateToolInput) -> tuple[Any, JsonObject]:
        """Validate one canonical semantic request without executing it."""

        canonical = self.canonical_request(tool_input.request)
        response = await self.stub.ValidateQuery(
            query_pb.ValidateQueryRequest(
                agent_instance_id=self.settings.agent_instance_id,
                workspace_id=self.settings.workspace_id,
                semantic_query_version=SEMANTIC_QUERY_VERSION,
                canonical_request_json=canonical,
                request_checksum=checksum(canonical),
                freshness_policy=query_pb.FRESHNESS_POLICY_UNSPECIFIED,
                host_capability_profile_digest=self.host_profile_digest,
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

    async def execute(self, tool_input: QueryToolInput) -> DaemonQueryResult:
        """Execute, stream, verify, read, and release one immutable result artifact."""

        canonical = self.canonical_request(tool_input.request)
        request_digest = checksum(canonical)
        started = await self.stub.StartQuery(
            query_pb.StartQueryRequest(
                agent_instance_id=self.settings.agent_instance_id,
                workspace_id=self.settings.workspace_id,
                mcp_call_id=f"mcp:{secrets.token_hex(16)}",
                rpc_attempt_id=f"rpc:{secrets.token_hex(16)}",
                semantic_query_version=SEMANTIC_QUERY_VERSION,
                canonical_request_json=canonical,
                request_checksum=request_digest,
                freshness_policy=query_pb.FRESHNESS_POLICY_UNSPECIFIED,
                delivery_preference={
                    "inline": query_pb.DELIVERY_PREFERENCE_INLINE,
                    "resource": query_pb.DELIVERY_PREFERENCE_RESOURCE,
                    "automatic": query_pb.DELIVERY_PREFERENCE_AUTO,
                }[tool_input.delivery],
                host_capability_profile_digest=self.host_profile_digest,
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
                self.cancel(started.daemon_query_id, started.cancel_token, "adapter interrupted")
            )
            with suppress(BaseException):
                await asyncio.shield(cancellation)
            raise
        if (
            terminal is not None
            and terminal.execution_state != query_pb.QUERY_EXECUTION_STATE_SUCCEEDED
        ):
            if terminal.canonical_error_record_json:
                raise DaemonQueryError(terminal.canonical_error_record_json)
            raise DaemonProtocolError("daemon query terminated without a public error record")
        if artifact is None or terminal is None or snapshot is None:
            raise DaemonProtocolError(
                "query stream ended without snapshot, artifact, and terminal events"
            )

        if not terminal.semantic_execution_state or not terminal.completeness_state:
            raise DaemonProtocolError("terminal event omitted semantic response states")
        arrow_descriptor: ArrowResultPackageDescriptor | None = None
        arrow_access: ArrowResultAccess | None = None
        if artifact.canonical_result_descriptor_json:
            descriptor_bytes = bytes(artifact.canonical_result_descriptor_json)
            if (
                artifact.content_type != "application/vnd.codefabric.arrow-result-package+json"
                or artifact.encoding != query_pb.PAYLOAD_COMPRESSION_IDENTITY
                or artifact.result_contract_version != PUBLISHED_RESULT_FORMAT
                or artifact.arrow_release != ARROW_RELEASE
                or checksum(descriptor_bytes) != artifact.result_descriptor_checksum
                or artifact.artifact_checksum != artifact.result_descriptor_checksum
            ):
                raise DaemonProtocolError("Arrow result compatibility metadata differs")
            arrow_descriptor = validate_package_descriptor_json(descriptor_bytes)
            if (
                arrow_descriptor.artifact_id != artifact.artifact_id
                or arrow_descriptor.lease_expires_at_unix_ms != artifact.lease_expires_at_unix_ms
                or arrow_descriptor.total_rows != terminal.result_row_count
                or arrow_descriptor.total_ipc_bytes != terminal.result_byte_count
            ):
                raise DaemonProtocolError("Arrow result descriptor differs from terminal metadata")
            arrow_access = ArrowResultAccess(
                artifact_id=arrow_descriptor.artifact_id,
                owner=arrow_descriptor.owner,
                lease_token=artifact.lease_token,
            )
        elif artifact.result_descriptor_checksum:
            raise DaemonProtocolError("Arrow result descriptor bytes are absent")
        query_statuses = tuple(
            cast(
                JsonObject,
                {
                    "query_id": status.query_id,
                    "state": status.execution_state,
                    "message": self._query_status_message(status.canonical_error_record_json),
                },
            )
            for status in terminal.query_statuses
        )
        use_resource = (
            arrow_descriptor is not None
            or tool_input.delivery == "resource"
            or (
                tool_input.delivery == "automatic"
                and terminal.result_byte_count > self.settings.inline_result_bytes
            )
        )
        if use_resource:
            if arrow_descriptor is not None and arrow_access is not None:
                self._arrow_leases[artifact.artifact_id] = _ArrowLeaseEntry(
                    descriptor=arrow_descriptor,
                    access=arrow_access,
                    consumed_resource_ids=set(),
                )
            else:
                self._lease_cache[artifact.artifact_id] = (
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
                execution_state=terminal.semantic_execution_state,
                freshness_state=terminal.freshness_state,
                availability_state=terminal.availability_state,
                completeness_state=terminal.completeness_state,
                limit_state=terminal.limit_state,
                truncated=terminal.truncated,
                query_statuses=query_statuses,
                notices=tuple(terminal.notices),
                arrow_descriptor=arrow_descriptor,
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
        response = cast(JsonObject, JSON_OBJECT_ADAPTER.validate_json(payload, strict=True))
        expected_states = {
            "execution_state": terminal.semantic_execution_state,
            "availability_state": terminal.availability_state,
            "completeness_state": terminal.completeness_state,
            "freshness_state": terminal.freshness_state,
            "limit_state": terminal.limit_state,
        }
        if any(response.get(name) != value for name, value in expected_states.items()):
            raise DaemonProtocolError("terminal states differ from the canonical response")
        return DaemonQueryResult(
            semantic_request_id=started.effective_semantic_request_id,
            daemon_query_id=started.daemon_query_id,
            canonical_bytes=payload,
            response=response,
            snapshot=snapshot,
            checksum=artifact.artifact_checksum,
            artifact_id=artifact.artifact_id,
            lease_expires_at_unix_ms=artifact.lease_expires_at_unix_ms,
            result_row_count=terminal.result_row_count,
            result_byte_count=terminal.result_byte_count,
            execution_state=terminal.semantic_execution_state,
            freshness_state=terminal.freshness_state,
            availability_state=terminal.availability_state,
            completeness_state=terminal.completeness_state,
            limit_state=terminal.limit_state,
            truncated=terminal.truncated,
            query_statuses=query_statuses,
            notices=tuple(terminal.notices),
        )

    @staticmethod
    def _query_status_message(canonical_error_record: bytes) -> str | None:
        if not canonical_error_record:
            return None
        record = DaemonQueryError(canonical_error_record).record
        for field in ("safe_message", "detail"):
            value = record.get(field)
            if isinstance(value, str):
                return value
        return None

    async def read_chunk(
        self,
        *,
        access: ArrowResultAccess,
        authorization_resource_id: str,
        offset: int,
        maximum_bytes: int,
    ) -> ArrowResourceChunk:
        """Translate one strict presenter read into the owner-bound gRPC branch."""

        try:
            stream = self.stub.ReadResult(
                query_pb.ReadResultRequest(
                    artifact_id=access.artifact_id,
                    offset=offset,
                    maximum_bytes=maximum_bytes,
                    lease_token=access.lease_token,
                    accepted_compression=query_pb.PAYLOAD_COMPRESSION_IDENTITY,
                    authorization_resource_id=authorization_resource_id,
                    owner=query_pb.ResultOwner(
                        workspace_id=access.owner.workspace_id,
                        agent_id=access.owner.agent_id,
                    ),
                ),
                timeout=self.settings.query_timeout_seconds,
            )
            chunks = [chunk async for chunk in stream]
        except grpc.RpcError as error:
            self._raise_arrow_rpc_error(error)
        if len(chunks) != 1:
            raise DaemonProtocolError("Arrow resource read did not return exactly one range")
        chunk = chunks[0]
        if (
            chunk.artifact_id != access.artifact_id
            or chunk.authorization_resource_id != authorization_resource_id
            or chunk.encoding != query_pb.PAYLOAD_COMPRESSION_IDENTITY
            or chunk.uncompressed_length != len(chunk.payload)
            or checksum(chunk.payload) != chunk.payload_checksum
        ):
            raise DaemonProtocolError("Arrow resource transport metadata differs")
        return ArrowResourceChunk(
            authorization_resource_id=chunk.authorization_resource_id,
            offset=chunk.offset,
            next_offset=chunk.next_offset,
            total_length=chunk.total_length,
            content_checksum=chunk.content_checksum,
            payload=chunk.payload,
            complete=chunk.final_chunk,
        )

    async def release(self, *, access: ArrowResultAccess) -> ArrowResultReleaseReceipt:
        """Release an Arrow artifact while preserving daemon tombstone semantics."""

        try:
            response = await self.stub.ReleaseResult(
                query_pb.ReleaseResultRequest(
                    artifact_id=access.artifact_id,
                    lease_token=access.lease_token,
                    owner=query_pb.ResultOwner(
                        workspace_id=access.owner.workspace_id,
                        agent_id=access.owner.agent_id,
                    ),
                ),
                timeout=self.settings.query_timeout_seconds,
            )
        except grpc.RpcError as error:
            self._raise_arrow_rpc_error(error)
        if response.artifact_id != access.artifact_id or response.release_state not in {
            "released",
            "already_released",
        }:
            raise DaemonProtocolError("Arrow release receipt differs from the request")
        return ArrowResultReleaseReceipt(
            artifact_id=response.artifact_id,
            state=response.release_state,
        )

    async def read_arrow_resource(self, artifact_id: str, authorization_resource_id: str) -> bytes:
        """Read one descriptor-authorized resource and release after complete consumption."""

        entry = self._arrow_leases.get(artifact_id)
        if entry is None:
            raise DaemonProtocolError("Arrow result is absent, expired, or already released")
        handshake = self.handshake_response
        if handshake is None:
            raise DaemonProtocolError("daemon handshake is absent")
        maximum_chunk_bytes = min(
            self.settings.inline_result_bytes,
            int(handshake.effective_limits.maximum_payload_chunk_bytes),
        )
        presenter = ArrowResourcePresenter(
            self,
            max_chunk_bytes=maximum_chunk_bytes,
            max_manifest_bytes=self.settings.max_request_bytes,
            max_relation_bytes=64 * 1024 * 1024,
        )
        payload = await presenter.read_subresource(
            entry.descriptor,
            entry.access,
            authorization_resource_id=authorization_resource_id,
            observed_at_unix_ms=int(time.time() * 1000),
        )
        entry.consumed_resource_ids.add(authorization_resource_id)
        all_resource_ids = {
            entry.descriptor.manifest.authorization_resource_id,
            *(relation.authorization_resource_id for relation in entry.descriptor.relations),
        }
        if entry.consumed_resource_ids == all_resource_ids:
            await presenter.release(
                entry.descriptor,
                entry.access,
                observed_at_unix_ms=int(time.time() * 1000),
            )
            self._arrow_leases.pop(artifact_id, None)
        return payload

    def arrow_result_descriptor(self, artifact_id: str) -> ArrowResultPackageDescriptor:
        """Return the already validated control descriptor for URI routing only."""

        entry = self._arrow_leases.get(artifact_id)
        if entry is None:
            raise DaemonProtocolError("Arrow result is absent, expired, or already released")
        return entry.descriptor

    @staticmethod
    def _raise_arrow_rpc_error(error: grpc.RpcError) -> Never:
        code = error.code()
        detail = (error.details() or "").upper()
        if code == grpc.StatusCode.PERMISSION_DENIED:
            raise ArrowResourceAccessError("daemon rejected the Arrow result owner or token")
        if code == grpc.StatusCode.NOT_FOUND:
            raise ArrowResourceAccessError("daemon rejected the Arrow resource handle")
        if code == grpc.StatusCode.RESOURCE_EXHAUSTED:
            raise ArrowResourceLimitError("daemon rejected the Arrow resource bound")
        if code == grpc.StatusCode.FAILED_PRECONDITION and "EXPIRED" in detail:
            raise ArrowResourceExpiredError("Arrow result lease expired")
        if code == grpc.StatusCode.FAILED_PRECONDITION and "RELEASED" in detail:
            raise ArrowResourceReleasedError("Arrow result is a released tombstone")
        raise DaemonProtocolError("daemon rejected the Arrow result operation")

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
        maximum_bytes = self.settings.inline_result_bytes
        maximum_round_trips = max(1, (result_byte_count + maximum_bytes - 1) // maximum_bytes + 1)
        maximum_chunks = maximum_round_trips
        chunk_count = 0
        for _attempt in range(maximum_round_trips):
            stream = self.stub.ReadResult(
                query_pb.ReadResultRequest(
                    artifact_id=artifact_id,
                    offset=offset,
                    maximum_bytes=maximum_bytes,
                    lease_token=lease_token,
                    accepted_compression=query_pb.PAYLOAD_COMPRESSION_IDENTITY,
                ),
                timeout=self.settings.query_timeout_seconds,
            )
            final = False
            prior_offset = offset
            async for chunk in stream:
                chunk_count += 1
                if chunk_count > maximum_chunks:
                    raise DaemonProtocolError("result read exceeded its bounded chunk contract")
                if chunk.offset != offset or checksum(chunk.payload) != chunk.payload_checksum:
                    raise DaemonProtocolError("result chunk offset or checksum differs")
                if not chunk.payload and not chunk.final_chunk:
                    raise DaemonProtocolError("result stream made no forward progress")
                chunks.append(chunk.payload)
                offset += len(chunk.payload)
                final = chunk.final_chunk
            if final:
                break
            if offset == prior_offset:
                raise DaemonProtocolError("result read made no forward progress")
        else:
            raise DaemonProtocolError("result read exceeded its bounded retry contract")
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
        self._lease_cache.pop(artifact_id, None)
        return payload

    async def read_resource(self, artifact_id: str) -> bytes:
        """Resolve one process-owned result resource exactly once."""

        lease = self._lease_cache.get(artifact_id)
        if lease is None:
            raise DaemonProtocolError("result resource is absent or already released")
        lease_token, artifact_checksum, _expires_at, result_byte_count = lease
        try:
            payload = await self._read_and_release(
                artifact_id,
                lease_token,
                artifact_checksum,
                result_byte_count=result_byte_count,
            )
        except grpc.RpcError as error:
            self._lease_cache.pop(artifact_id, None)
            raise DaemonProtocolError("daemon rejected or revoked the result lease") from error
        canonical_payload = canonicalize_json(payload)
        if canonical_payload != payload:
            raise DaemonProtocolError("result resource is not canonical JSON")
        return payload


__all__ = [
    "CpgDaemonClient",
    "DaemonProtocolError",
    "DaemonQueryError",
    "DaemonQueryResult",
]
