"""Strict presentation-only client for daemon-owned Arrow result resources.

The daemon remains authoritative for catalogs, schemas, relational execution, artifact identity,
authorization, and leases. This module validates the daemon's control projection, reassembles
bounded immutable resources, and returns canonical manifest metadata plus untouched Arrow IPC
bytes. It deliberately contains no PyArrow, DataFusion, semantic-row JSON, packaged registry, or
fingerprint authority.
"""

from __future__ import annotations

from typing import Annotated, Literal, Protocol, Self
from urllib.parse import quote

import blake3
from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    StringConstraints,
    TypeAdapter,
    model_validator,
)

from ..contracts.json import CanonicalJsonError, canonicalize_json

ARROW_STREAM_MEDIA_TYPE = "application/vnd.apache.arrow.stream"
CANONICAL_MANIFEST_MEDIA_TYPE = "application/json"
PUBLISHED_RESULT_FORMAT = "codefabric.published-arrow-result.v1"
ARROW_RESULT_RESOURCE_FORMAT = "codefabric.arrow-result-resource.v1"
ARROW_RELEASE = "59.2.0"

type Digest = Annotated[str, StringConstraints(pattern=r"^b3:[0-9a-f]{64}$")]
type HexId16 = Annotated[str, StringConstraints(pattern=r"^[0-9a-f]{32}$")]
type ModelRelationId = Annotated[str, StringConstraints(min_length=1, max_length=240)]
type OpaqueLeaseToken = Annotated[
    str,
    StringConstraints(min_length=16, max_length=512, pattern=r"^[A-Za-z0-9._~+-]+$"),
]
type NonNegativeInt = Annotated[int, Field(ge=0)]
type PositiveInt = Annotated[int, Field(gt=0)]
type CompletenessState = Literal["complete", "partial", "unknown"]
type ReleaseState = Literal["released", "already_released"]


class _StrictModel(BaseModel):
    """Closed immutable daemon boundary model."""

    model_config = ConfigDict(strict=True, extra="forbid", frozen=True)


class ArrowResultOwner(_StrictModel):
    """Authenticated owner projected by the Rust daemon."""

    workspace_id: HexId16
    agent_id: HexId16


class ArrowResultCoverage(_StrictModel):
    """Exact completeness accounting for one model relation."""

    state: CompletenessState
    requested_units: NonNegativeInt
    completed_units: NonNegativeInt
    remainder_units: NonNegativeInt
    unknown_cause: Annotated[str, StringConstraints(min_length=1, max_length=240)] | None

    @model_validator(mode="after")
    def validate_accounting(self) -> Self:
        """Require exact counts and explicit unknown/partial causes."""

        _validate_coverage(
            self.state,
            self.requested_units,
            self.completed_units,
            self.remainder_units,
            self.unknown_cause,
        )
        return self


class ArrowRelationDescriptor(_StrictModel):
    """Owner-bound read handle mapped to immutable Arrow relation content."""

    relation_id: ModelRelationId
    authorization_resource_id: Digest
    content_resource_id: Digest
    media_type: Literal["application/vnd.apache.arrow.stream"]
    schema_checksum: Digest
    schema_byte_length: PositiveInt
    content_checksum: Digest
    row_count: NonNegativeInt
    batch_count: NonNegativeInt
    byte_length: PositiveInt
    coverage: ArrowResultCoverage

    @model_validator(mode="after")
    def separate_authority_from_content(self) -> Self:
        if self.authorization_resource_id == self.content_resource_id:
            raise ValueError("relation authorization handle equals its content identity")
        return self


class ArrowManifestDescriptor(_StrictModel):
    """Owner-bound read handle mapped to the canonical public manifest."""

    authorization_resource_id: Digest
    content_resource_id: Digest
    media_type: Literal["application/json"]
    content_checksum: Digest
    byte_length: PositiveInt

    @model_validator(mode="after")
    def separate_authority_from_content(self) -> Self:
        if self.authorization_resource_id == self.content_resource_id:
            raise ValueError("manifest authorization handle equals its content identity")
        return self


class ArrowResultPackageDescriptor(_StrictModel):
    """Daemon-issued owner-bound descriptor for one immutable Arrow package."""

    format: Literal["codefabric.published-arrow-result.v1"]
    artifact_id: Digest
    package_id: Digest
    content_package_id: Digest
    owner: ArrowResultOwner
    epoch_id: HexId16
    query_execution: Digest
    source_manifest_checksum: Digest
    source_manifest_byte_length: PositiveInt
    completion: CompletenessState
    total_rows: NonNegativeInt
    total_batches: NonNegativeInt
    total_schema_bytes: PositiveInt
    total_ipc_bytes: PositiveInt
    lease_expires_at_unix_ms: PositiveInt
    manifest: ArrowManifestDescriptor
    relations: Annotated[tuple[ArrowRelationDescriptor, ...], Field(min_length=1)]

    @model_validator(mode="after")
    def validate_projection_closure(self) -> Self:
        if len({self.artifact_id, self.package_id, self.content_package_id}) != 3:
            raise ValueError("artifact, package authorization, and content package IDs must differ")
        if (
            self.source_manifest_checksum != self.manifest.content_checksum
            or self.source_manifest_byte_length != self.manifest.byte_length
        ):
            raise ValueError("package and manifest descriptor identity differ")
        relation_ids = tuple(relation.relation_id for relation in self.relations)
        if relation_ids != tuple(sorted(set(relation_ids))):
            raise ValueError("relation descriptors are not unique and strictly sorted")
        authorization_ids = (
            self.manifest.authorization_resource_id,
            *(relation.authorization_resource_id for relation in self.relations),
        )
        if len(set(authorization_ids)) != len(authorization_ids):
            raise ValueError("result authorization handles are not unique")
        content_ids = (
            self.manifest.content_resource_id,
            *(relation.content_resource_id for relation in self.relations),
        )
        if len(set(content_ids)) != len(content_ids):
            raise ValueError("result content identities are not unique")
        if self.total_rows != sum(relation.row_count for relation in self.relations):
            raise ValueError("package total row count differs from relation descriptors")
        if self.total_batches != sum(relation.batch_count for relation in self.relations):
            raise ValueError("package total batch count differs from relation descriptors")
        if self.total_schema_bytes != sum(
            relation.schema_byte_length for relation in self.relations
        ):
            raise ValueError("package total schema bytes differ from relation descriptors")
        if self.total_ipc_bytes != sum(relation.byte_length for relation in self.relations):
            raise ValueError("package total IPC bytes differ from relation descriptors")
        if self.completion != _aggregate_completion(
            tuple(relation.coverage.state for relation in self.relations)
        ):
            raise ValueError("package completion differs from relation coverage")
        return self


class ArrowManifestSubresource(_StrictModel):
    """Canonical manifest metadata for one immutable Arrow IPC stream."""

    relation_id: ModelRelationId
    resource_id: Digest
    media_type: Literal["application/vnd.apache.arrow.stream"]
    schema_checksum: Digest
    schema_byte_length: PositiveInt
    content_checksum: Digest
    row_count: NonNegativeInt
    batch_count: NonNegativeInt
    byte_length: PositiveInt
    completion_state: CompletenessState
    requested_units: NonNegativeInt
    completed_units: NonNegativeInt
    remainder_units: NonNegativeInt
    complete: bool
    truncated: Literal[False]
    unknown: bool
    unknown_cause: Annotated[str, StringConstraints(min_length=1, max_length=240)] | None = None

    @model_validator(mode="after")
    def validate_coverage_flags(self) -> Self:
        _validate_coverage(
            self.completion_state,
            self.requested_units,
            self.completed_units,
            self.remainder_units,
            self.unknown_cause,
        )
        if self.complete != (self.completion_state == "complete"):
            raise ValueError("manifest relation complete flag differs from completeness")
        if self.unknown != (self.completion_state == "unknown"):
            raise ValueError("manifest relation unknown flag differs from completeness")
        return self


class ArrowResultManifest(_StrictModel):
    """Canonical JSON control manifest; it contains metadata and never semantic rows."""

    format: Literal["codefabric.arrow-result-resource.v1"]
    arrow_release: Literal["59.2.0"]
    package_id: Digest
    epoch_id: HexId16
    query_execution: Digest
    completion_state: CompletenessState
    complete: bool
    truncated: Literal[False]
    unknown: bool
    relation_count: PositiveInt
    total_rows: NonNegativeInt
    total_batches: NonNegativeInt
    total_schema_bytes: PositiveInt
    total_ipc_bytes: PositiveInt
    subresources: Annotated[tuple[ArrowManifestSubresource, ...], Field(min_length=1)]

    @model_validator(mode="after")
    def validate_manifest_closure(self) -> Self:
        if self.complete != (self.completion_state == "complete"):
            raise ValueError("manifest complete flag differs from completeness")
        if self.unknown != (self.completion_state == "unknown"):
            raise ValueError("manifest unknown flag differs from completeness")
        if self.relation_count != len(self.subresources):
            raise ValueError("manifest relation count differs from subresources")
        relation_ids = tuple(resource.relation_id for resource in self.subresources)
        if relation_ids != tuple(sorted(set(relation_ids))):
            raise ValueError("manifest relation resources are not unique and strictly sorted")
        if self.total_rows != sum(resource.row_count for resource in self.subresources):
            raise ValueError("manifest total row count differs from subresources")
        if self.total_batches != sum(resource.batch_count for resource in self.subresources):
            raise ValueError("manifest total batch count differs from subresources")
        if self.total_schema_bytes != sum(
            resource.schema_byte_length for resource in self.subresources
        ):
            raise ValueError("manifest total schema bytes differ from subresources")
        if self.total_ipc_bytes != sum(resource.byte_length for resource in self.subresources):
            raise ValueError("manifest total IPC bytes differ from subresources")
        if self.completion_state != _aggregate_completion(
            tuple(resource.completion_state for resource in self.subresources)
        ):
            raise ValueError("manifest completion differs from subresources")
        return self


class ArrowResultAccess(_StrictModel):
    """Owner and opaque token presented on every read/release operation."""

    artifact_id: Digest
    owner: ArrowResultOwner
    lease_token: OpaqueLeaseToken


class ArrowResourceChunk(_StrictModel):
    """One exact daemon chunk carrying the full immutable content checksum."""

    authorization_resource_id: Digest
    offset: NonNegativeInt
    next_offset: NonNegativeInt
    total_length: PositiveInt
    content_checksum: Digest
    payload: bytes
    complete: bool

    @model_validator(mode="after")
    def validate_range(self) -> Self:
        if self.next_offset != self.offset + len(self.payload):
            raise ValueError("chunk next offset differs from payload length")
        if self.next_offset > self.total_length:
            raise ValueError("chunk extends beyond its declared resource length")
        if self.complete != (self.next_offset == self.total_length):
            raise ValueError("chunk completion flag differs from its final offset")
        if not self.payload and not self.complete:
            raise ValueError("non-final chunk made no forward progress")
        return self


class ArrowResultReleaseReceipt(_StrictModel):
    """Idempotent daemon release/tombstone acknowledgement."""

    artifact_id: Digest
    state: ReleaseState


class PresentedArrowRelation(_StrictModel):
    """Validated metadata paired with untouched Arrow IPC bytes."""

    descriptor: ArrowRelationDescriptor
    ipc_bytes: bytes


class PresentedArrowPackage(_StrictModel):
    """Validated canonical manifest plus raw relation streams."""

    descriptor: ArrowResultPackageDescriptor
    manifest: ArrowResultManifest
    manifest_bytes: bytes
    relations: tuple[PresentedArrowRelation, ...]


PACKAGE_DESCRIPTOR_ADAPTER = TypeAdapter(ArrowResultPackageDescriptor)
RELATION_DESCRIPTOR_ADAPTER = TypeAdapter(ArrowRelationDescriptor)
MANIFEST_ADAPTER = TypeAdapter(ArrowResultManifest)
ACCESS_ADAPTER = TypeAdapter(ArrowResultAccess)
CHUNK_ADAPTER = TypeAdapter(ArrowResourceChunk)
RELEASE_RECEIPT_ADAPTER = TypeAdapter(ArrowResultReleaseReceipt)
OBSERVED_TIME_ADAPTER = TypeAdapter(NonNegativeInt)


class AsyncArrowResourceReader(Protocol):
    """Transport-neutral daemon port; a later gRPC adapter supplies this protocol."""

    async def read_chunk(
        self,
        *,
        access: ArrowResultAccess,
        authorization_resource_id: str,
        offset: int,
        maximum_bytes: int,
    ) -> ArrowResourceChunk: ...

    async def release(self, *, access: ArrowResultAccess) -> ArrowResultReleaseReceipt: ...


class ArrowResourcePresentationError(RuntimeError):
    """Base failure for the presentation-only Arrow resource boundary."""


class ArrowResourceContractError(ArrowResourcePresentationError):
    """Daemon control metadata violated the closed public contract."""


class ArrowResourceAccessError(ArrowResourcePresentationError):
    """The daemon rejected an owner, handle, or opaque lease token."""


class ArrowResourceIntegrityError(ArrowResourcePresentationError):
    """Immutable resource bytes differed from the daemon descriptor."""


class ArrowResourceIncompleteError(ArrowResourcePresentationError):
    """A bounded resource read ended without an exact final byte range."""


class ArrowResourceExpiredError(ArrowResourceAccessError):
    """The owner-bound artifact lease is no longer live."""


class ArrowResourceReleasedError(ArrowResourceAccessError):
    """The result is a released tombstone and cannot issue more bytes."""


class ArrowResourceLimitError(ArrowResourcePresentationError):
    """Descriptor or stream bytes exceeded the adapter presentation envelope."""


class ArrowResourcePresenter:
    """Reusable bounded client for one transport-backed daemon result authority."""

    __slots__ = (
        "_reader",
        "_max_chunk_bytes",
        "_max_manifest_bytes",
        "_max_relation_bytes",
        "_released",
    )

    def __init__(
        self,
        reader: AsyncArrowResourceReader,
        *,
        max_chunk_bytes: int,
        max_manifest_bytes: int,
        max_relation_bytes: int,
    ) -> None:
        if min(max_chunk_bytes, max_manifest_bytes, max_relation_bytes) <= 0:
            raise ValueError("Arrow presentation bounds must be positive")
        self._reader = reader
        self._max_chunk_bytes = max_chunk_bytes
        self._max_manifest_bytes = max_manifest_bytes
        self._max_relation_bytes = max_relation_bytes
        self._released: set[str] = set()

    async def read_package(
        self,
        descriptor: ArrowResultPackageDescriptor,
        access: ArrowResultAccess,
        *,
        observed_at_unix_ms: int,
    ) -> PresentedArrowPackage:
        """Read and verify a canonical manifest plus every raw Arrow IPC relation."""

        descriptor = PACKAGE_DESCRIPTOR_ADAPTER.validate_python(descriptor, strict=True)
        access = ACCESS_ADAPTER.validate_python(access, strict=True)
        observed_at_unix_ms = OBSERVED_TIME_ADAPTER.validate_python(
            observed_at_unix_ms, strict=True
        )
        self._validate_access(descriptor, access, observed_at_unix_ms)

        manifest_bytes = await self._read_resource(
            access,
            authorization_resource_id=descriptor.manifest.authorization_resource_id,
            expected_length=descriptor.manifest.byte_length,
            expected_checksum=descriptor.manifest.content_checksum,
            checksum_domain=b"result-manifest.v1",
            maximum_resource_bytes=self._max_manifest_bytes,
        )
        try:
            canonical_manifest = canonicalize_json(manifest_bytes)
        except CanonicalJsonError as error:
            raise ArrowResourceContractError(
                "result manifest is not valid canonical JSON"
            ) from error
        if canonical_manifest != manifest_bytes:
            raise ArrowResourceContractError("result manifest bytes are not canonical JSON")
        try:
            manifest = MANIFEST_ADAPTER.validate_json(manifest_bytes, strict=True)
        except ValueError as error:
            raise ArrowResourceContractError(
                "result manifest violates its strict contract"
            ) from error
        _validate_manifest_matches_descriptor(manifest, descriptor)

        relations: list[PresentedArrowRelation] = []
        for relation in descriptor.relations:
            ipc_bytes = await self._read_resource(
                access,
                authorization_resource_id=relation.authorization_resource_id,
                expected_length=relation.byte_length,
                expected_checksum=relation.content_checksum,
                checksum_domain=b"arrow-ipc-stream.v1",
                maximum_resource_bytes=self._max_relation_bytes,
            )
            relations.append(PresentedArrowRelation(descriptor=relation, ipc_bytes=ipc_bytes))
        return PresentedArrowPackage(
            descriptor=descriptor,
            manifest=manifest,
            manifest_bytes=manifest_bytes,
            relations=tuple(relations),
        )

    async def read_subresource(
        self,
        descriptor: ArrowResultPackageDescriptor,
        access: ArrowResultAccess,
        *,
        authorization_resource_id: str,
        observed_at_unix_ms: int,
    ) -> bytes:
        """Read one descriptor-authorized manifest or untouched Arrow IPC stream."""

        descriptor = PACKAGE_DESCRIPTOR_ADAPTER.validate_python(descriptor, strict=True)
        access = ACCESS_ADAPTER.validate_python(access, strict=True)
        observed_at_unix_ms = OBSERVED_TIME_ADAPTER.validate_python(
            observed_at_unix_ms, strict=True
        )
        self._validate_access(descriptor, access, observed_at_unix_ms)
        if authorization_resource_id == descriptor.manifest.authorization_resource_id:
            payload = await self._read_resource(
                access,
                authorization_resource_id=authorization_resource_id,
                expected_length=descriptor.manifest.byte_length,
                expected_checksum=descriptor.manifest.content_checksum,
                checksum_domain=b"result-manifest.v1",
                maximum_resource_bytes=self._max_manifest_bytes,
            )
            try:
                canonical = canonicalize_json(payload)
                manifest = MANIFEST_ADAPTER.validate_json(payload, strict=True)
            except (CanonicalJsonError, ValueError) as error:
                raise ArrowResourceContractError(
                    "result manifest violates its canonical strict contract"
                ) from error
            if canonical != payload:
                raise ArrowResourceContractError("result manifest bytes are not canonical JSON")
            _validate_manifest_matches_descriptor(manifest, descriptor)
            return payload
        relation = next(
            (
                relation
                for relation in descriptor.relations
                if relation.authorization_resource_id == authorization_resource_id
            ),
            None,
        )
        if relation is None:
            raise ArrowResourceAccessError("resource handle is absent from the descriptor")
        return await self._read_resource(
            access,
            authorization_resource_id=authorization_resource_id,
            expected_length=relation.byte_length,
            expected_checksum=relation.content_checksum,
            checksum_domain=b"arrow-ipc-stream.v1",
            maximum_resource_bytes=self._max_relation_bytes,
        )

    async def release(
        self,
        descriptor: ArrowResultPackageDescriptor,
        access: ArrowResultAccess,
        *,
        observed_at_unix_ms: int,
    ) -> ArrowResultReleaseReceipt:
        """Release the artifact; a second authenticated release preserves tombstone semantics."""

        descriptor = PACKAGE_DESCRIPTOR_ADAPTER.validate_python(descriptor, strict=True)
        access = ACCESS_ADAPTER.validate_python(access, strict=True)
        observed_at_unix_ms = OBSERVED_TIME_ADAPTER.validate_python(
            observed_at_unix_ms, strict=True
        )
        _validate_descriptor_access(descriptor, access)
        if observed_at_unix_ms >= descriptor.lease_expires_at_unix_ms:
            raise ArrowResourceExpiredError("result resource lease expired")
        receipt = RELEASE_RECEIPT_ADAPTER.validate_python(
            await self._reader.release(access=access), strict=True
        )
        if receipt.artifact_id != descriptor.artifact_id:
            raise ArrowResourceContractError("release receipt names another artifact")
        self._released.add(descriptor.artifact_id)
        return receipt

    def _validate_access(
        self,
        descriptor: ArrowResultPackageDescriptor,
        access: ArrowResultAccess,
        observed_at_unix_ms: int,
    ) -> None:
        _validate_descriptor_access(descriptor, access)
        if descriptor.artifact_id in self._released:
            raise ArrowResourceReleasedError("result resource is a released tombstone")
        if observed_at_unix_ms >= descriptor.lease_expires_at_unix_ms:
            raise ArrowResourceExpiredError("result resource lease expired")

    async def _read_resource(
        self,
        access: ArrowResultAccess,
        *,
        authorization_resource_id: str,
        expected_length: int,
        expected_checksum: str,
        checksum_domain: bytes,
        maximum_resource_bytes: int,
    ) -> bytes:
        if expected_length > maximum_resource_bytes:
            raise ArrowResourceLimitError("result resource exceeds the presentation byte bound")
        chunks: list[bytes] = []
        offset = 0
        maximum_chunks = (expected_length + self._max_chunk_bytes - 1) // self._max_chunk_bytes + 1
        for _ in range(maximum_chunks):
            requested = min(self._max_chunk_bytes, expected_length - offset)
            if requested <= 0:
                raise ArrowResourceIncompleteError("result stream did not mark its final range")
            chunk = CHUNK_ADAPTER.validate_python(
                await self._reader.read_chunk(
                    access=access,
                    authorization_resource_id=authorization_resource_id,
                    offset=offset,
                    maximum_bytes=requested,
                ),
                strict=True,
            )
            if chunk.authorization_resource_id != authorization_resource_id:
                raise ArrowResourceAccessError("daemon returned another resource handle")
            if chunk.offset != offset:
                raise ArrowResourceIncompleteError("result chunk offset is not monotone")
            if chunk.total_length != expected_length:
                raise ArrowResourceIntegrityError("result chunk length differs from descriptor")
            if chunk.content_checksum != expected_checksum:
                raise ArrowResourceIntegrityError("result chunk checksum differs from descriptor")
            if len(chunk.payload) > requested:
                raise ArrowResourceLimitError("daemon chunk exceeds the requested byte bound")
            chunks.append(chunk.payload)
            offset = chunk.next_offset
            if chunk.complete:
                break
        else:
            raise ArrowResourceIncompleteError("result stream exceeded its bounded chunk count")

        payload = b"".join(chunks)
        if offset != expected_length or len(payload) != expected_length:
            raise ArrowResourceIncompleteError("result stream ended before its exact final length")
        if framed_content_checksum(checksum_domain, payload) != expected_checksum:
            raise ArrowResourceIntegrityError("assembled resource checksum differs")
        return payload


def validate_package_descriptor(value: object) -> ArrowResultPackageDescriptor:
    """Validate an untrusted daemon descriptor through the reusable module adapter."""

    return PACKAGE_DESCRIPTOR_ADAPTER.validate_python(value, strict=True)


def validate_package_descriptor_json(value: bytes) -> ArrowResultPackageDescriptor:
    """Validate one canonical daemon control projection without Python coercion."""

    try:
        canonical = canonicalize_json(value)
    except CanonicalJsonError as error:
        raise ArrowResourceContractError("result descriptor is not valid JSON") from error
    if canonical != value:
        raise ArrowResourceContractError("result descriptor bytes are not canonical JSON")
    try:
        return PACKAGE_DESCRIPTOR_ADAPTER.validate_json(value, strict=True)
    except ValueError as error:
        raise ArrowResourceContractError(
            "result descriptor violates its strict contract"
        ) from error


def framed_content_checksum(domain: bytes, payload: bytes) -> str:
    """Recompute the Rust result-resource framed BLAKE3 integrity checksum."""

    hasher = blake3.blake3()
    for part in (domain, payload):
        hasher.update(len(part).to_bytes(8, "big"))
        hasher.update(part)
    return f"b3:{hasher.hexdigest()}"


def manifest_resource_uri(descriptor: ArrowResultPackageDescriptor) -> str:
    """Return a token-free FastMCP URI for the owner-bound manifest handle."""

    descriptor = PACKAGE_DESCRIPTOR_ADAPTER.validate_python(descriptor, strict=True)
    return (
        f"codefabric-result://{descriptor.owner.workspace_id}/"
        f"{_digest_path(descriptor.artifact_id)}/manifest/"
        f"{_digest_path(descriptor.manifest.authorization_resource_id)}"
    )


def relation_resource_uri(
    descriptor: ArrowResultPackageDescriptor,
    relation: ArrowRelationDescriptor,
) -> str:
    """Return a token-free FastMCP URI for one owner-bound relation handle."""

    descriptor = PACKAGE_DESCRIPTOR_ADAPTER.validate_python(descriptor, strict=True)
    relation = RELATION_DESCRIPTOR_ADAPTER.validate_python(relation, strict=True)
    if relation not in descriptor.relations:
        raise ArrowResourceContractError("relation is absent from the package descriptor")
    return (
        f"codefabric-result://{descriptor.owner.workspace_id}/"
        f"{_digest_path(descriptor.artifact_id)}/relation/"
        f"{quote(relation.relation_id, safe='')}/"
        f"{_digest_path(relation.authorization_resource_id)}"
    )


def _validate_descriptor_access(
    descriptor: ArrowResultPackageDescriptor,
    access: ArrowResultAccess,
) -> None:
    if access.artifact_id != descriptor.artifact_id or access.owner != descriptor.owner:
        raise ArrowResourceAccessError("result access owner or artifact differs")


def _validate_manifest_matches_descriptor(
    manifest: ArrowResultManifest,
    descriptor: ArrowResultPackageDescriptor,
) -> None:
    if (
        manifest.package_id != descriptor.content_package_id
        or manifest.epoch_id != descriptor.epoch_id
        or manifest.query_execution != descriptor.query_execution
        or manifest.completion_state != descriptor.completion
        or manifest.total_rows != descriptor.total_rows
        or manifest.total_batches != descriptor.total_batches
        or manifest.total_schema_bytes != descriptor.total_schema_bytes
        or manifest.total_ipc_bytes != descriptor.total_ipc_bytes
        or len(manifest.subresources) != len(descriptor.relations)
    ):
        raise ArrowResourceContractError("manifest package metadata differs from descriptor")
    for resource, relation in zip(manifest.subresources, descriptor.relations, strict=True):
        coverage = relation.coverage
        if (
            resource.relation_id != relation.relation_id
            or resource.resource_id != relation.content_resource_id
            or resource.media_type != relation.media_type
            or resource.schema_checksum != relation.schema_checksum
            or resource.schema_byte_length != relation.schema_byte_length
            or resource.content_checksum != relation.content_checksum
            or resource.row_count != relation.row_count
            or resource.batch_count != relation.batch_count
            or resource.byte_length != relation.byte_length
            or resource.completion_state != coverage.state
            or resource.requested_units != coverage.requested_units
            or resource.completed_units != coverage.completed_units
            or resource.remainder_units != coverage.remainder_units
            or resource.unknown_cause != coverage.unknown_cause
        ):
            raise ArrowResourceContractError(
                f"manifest relation metadata differs for {relation.relation_id}"
            )


def _validate_coverage(
    state: CompletenessState,
    requested_units: int,
    completed_units: int,
    remainder_units: int,
    unknown_cause: str | None,
) -> None:
    if completed_units + remainder_units != requested_units:
        raise ValueError("completed plus remainder units must equal requested units")
    if unknown_cause is not None and not unknown_cause.isascii():
        raise ValueError("unknown cause must be ASCII")
    valid = (
        (
            state == "complete"
            and remainder_units == 0
            and completed_units == requested_units
            and unknown_cause is None
        )
        or (
            state == "partial"
            and requested_units > 0
            and completed_units > 0
            and remainder_units > 0
            and unknown_cause is not None
        )
        or (
            state == "unknown"
            and requested_units > 0
            and remainder_units > 0
            and unknown_cause is not None
        )
    )
    if not valid:
        raise ValueError("coverage state, counts, and unknown cause disagree")


def _aggregate_completion(states: tuple[CompletenessState, ...]) -> CompletenessState:
    if "unknown" in states:
        return "unknown"
    if "partial" in states:
        return "partial"
    return "complete"


def _digest_path(value: str) -> str:
    return value.removeprefix("b3:")


__all__ = [
    "ACCESS_ADAPTER",
    "ARROW_RELEASE",
    "ARROW_RESULT_RESOURCE_FORMAT",
    "ARROW_STREAM_MEDIA_TYPE",
    "ArrowManifestDescriptor",
    "ArrowManifestSubresource",
    "ArrowRelationDescriptor",
    "ArrowResourceAccessError",
    "ArrowResourceChunk",
    "ArrowResourceContractError",
    "ArrowResourceExpiredError",
    "ArrowResourceIncompleteError",
    "ArrowResourceIntegrityError",
    "ArrowResourceLimitError",
    "ArrowResourcePresentationError",
    "ArrowResourcePresenter",
    "ArrowResourceReleasedError",
    "ArrowResultAccess",
    "ArrowResultCoverage",
    "ArrowResultManifest",
    "ArrowResultOwner",
    "ArrowResultPackageDescriptor",
    "ArrowResultReleaseReceipt",
    "AsyncArrowResourceReader",
    "CANONICAL_MANIFEST_MEDIA_TYPE",
    "CHUNK_ADAPTER",
    "MANIFEST_ADAPTER",
    "PACKAGE_DESCRIPTOR_ADAPTER",
    "PUBLISHED_RESULT_FORMAT",
    "PresentedArrowPackage",
    "PresentedArrowRelation",
    "RELATION_DESCRIPTOR_ADAPTER",
    "framed_content_checksum",
    "manifest_resource_uri",
    "relation_resource_uri",
    "validate_package_descriptor",
    "validate_package_descriptor_json",
]
