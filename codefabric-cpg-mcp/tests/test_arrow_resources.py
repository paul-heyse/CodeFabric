"""Focused proofs for presentation-only Arrow result resources."""

from __future__ import annotations

import asyncio
import inspect
import json
from collections.abc import Callable
from dataclasses import dataclass
from typing import cast

import pytest
from pydantic import ValidationError

import codefabric_cpg_mcp.daemon.arrow_resources as arrow_resources_module
from codefabric_cpg_mcp.contracts.json import JsonValue, canonicalize_value
from codefabric_cpg_mcp.daemon.arrow_resources import (
    ARROW_RELEASE,
    ARROW_RESULT_RESOURCE_FORMAT,
    ARROW_STREAM_MEDIA_TYPE,
    PUBLISHED_RESULT_FORMAT,
    ArrowResourceAccessError,
    ArrowResourceChunk,
    ArrowResourceContractError,
    ArrowResourceExpiredError,
    ArrowResourceIncompleteError,
    ArrowResourceIntegrityError,
    ArrowResourcePresenter,
    ArrowResourceReleasedError,
    ArrowResultAccess,
    ArrowResultOwner,
    ArrowResultPackageDescriptor,
    ArrowResultReleaseReceipt,
    framed_content_checksum,
    manifest_resource_uri,
    relation_resource_uri,
    validate_package_descriptor,
)


def _digest(seed: int) -> str:
    return f"b3:{seed:02x}" + f"{seed:02x}" * 31


@dataclass(frozen=True, slots=True)
class ResultFixture:
    descriptor: ArrowResultPackageDescriptor
    access: ArrowResultAccess
    manifest_bytes: bytes
    relation_bytes: bytes


ManifestMutation = Callable[[dict[str, object]], None]


def _fixture(
    *,
    mutate_manifest: ManifestMutation | None = None,
    canonical_manifest: bool = True,
) -> ResultFixture:
    relation_bytes = b"ARROW-IPC-STREAM\x00\xffraw-columnar-bytes"
    relation_checksum = framed_content_checksum(b"arrow-ipc-stream.v1", relation_bytes)
    coverage = {
        "state": "complete",
        "requested_units": 3,
        "completed_units": 3,
        "remainder_units": 0,
        "unknown_cause": None,
    }
    manifest: dict[str, object] = {
        "format": ARROW_RESULT_RESOURCE_FORMAT,
        "arrow_release": ARROW_RELEASE,
        "package_id": _digest(3),
        "epoch_id": "14" * 16,
        "query_execution": _digest(4),
        "completion_state": "complete",
        "complete": True,
        "truncated": False,
        "unknown": False,
        "relation_count": 1,
        "total_rows": 3,
        "total_batches": 1,
        "total_schema_bytes": 42,
        "total_ipc_bytes": len(relation_bytes),
        "subresources": [
            {
                "relation_id": "public.people",
                "resource_id": _digest(7),
                "media_type": ARROW_STREAM_MEDIA_TYPE,
                "schema_checksum": _digest(8),
                "schema_byte_length": 42,
                "content_checksum": relation_checksum,
                "row_count": 3,
                "batch_count": 1,
                "byte_length": len(relation_bytes),
                "completion_state": "complete",
                "requested_units": 3,
                "completed_units": 3,
                "remainder_units": 0,
                "complete": True,
                "truncated": False,
                "unknown": False,
            }
        ],
    }
    if mutate_manifest is not None:
        mutate_manifest(manifest)
    manifest_bytes = (
        canonicalize_value(cast(JsonValue, manifest))
        if canonical_manifest
        else json.dumps(manifest, indent=2).encode()
    )
    manifest_checksum = framed_content_checksum(b"result-manifest.v1", manifest_bytes)
    descriptor = validate_package_descriptor(
        {
            "format": PUBLISHED_RESULT_FORMAT,
            "artifact_id": _digest(1),
            "package_id": _digest(2),
            "content_package_id": _digest(3),
            "owner": {"workspace_id": "01" * 16, "agent_id": "02" * 16},
            "epoch_id": "14" * 16,
            "query_execution": _digest(4),
            "source_manifest_checksum": manifest_checksum,
            "source_manifest_byte_length": len(manifest_bytes),
            "completion": "complete",
            "total_rows": 3,
            "total_batches": 1,
            "total_schema_bytes": 42,
            "total_ipc_bytes": len(relation_bytes),
            "lease_expires_at_unix_ms": 2_000,
            "manifest": {
                "authorization_resource_id": _digest(5),
                "content_resource_id": _digest(6),
                "media_type": "application/json",
                "content_checksum": manifest_checksum,
                "byte_length": len(manifest_bytes),
            },
            "relations": (
                {
                    "relation_id": "public.people",
                    "authorization_resource_id": _digest(9),
                    "content_resource_id": _digest(7),
                    "media_type": ARROW_STREAM_MEDIA_TYPE,
                    "schema_checksum": _digest(8),
                    "schema_byte_length": 42,
                    "content_checksum": relation_checksum,
                    "row_count": 3,
                    "batch_count": 1,
                    "byte_length": len(relation_bytes),
                    "coverage": coverage,
                },
            ),
        }
    )
    access = ArrowResultAccess(
        artifact_id=descriptor.artifact_id,
        owner=descriptor.owner,
        lease_token="opaque-lease-token-0001",
    )
    return ResultFixture(descriptor, access, manifest_bytes, relation_bytes)


class FakeArrowReader:
    """Exact transport double; faults alter protocol evidence, never presenter internals."""

    def __init__(self, fixture: ResultFixture, *, fault: str | None = None) -> None:
        self.expected_access = fixture.access
        self.resources = {
            fixture.descriptor.manifest.authorization_resource_id: (
                fixture.manifest_bytes,
                fixture.descriptor.manifest.content_checksum,
            ),
            fixture.descriptor.relations[0].authorization_resource_id: (
                fixture.relation_bytes,
                fixture.descriptor.relations[0].content_checksum,
            ),
        }
        self.fault = fault
        self.fault_used = False
        self.released = False

    async def read_chunk(
        self,
        *,
        access: ArrowResultAccess,
        authorization_resource_id: str,
        offset: int,
        maximum_bytes: int,
    ) -> ArrowResourceChunk:
        if access != self.expected_access:
            raise ArrowResourceAccessError("wrong owner or opaque lease token")
        if self.released:
            raise ArrowResourceReleasedError("released result tombstone")
        if authorization_resource_id not in self.resources:
            raise ArrowResourceAccessError("unknown authorization handle")
        if self.fault == "incomplete" and offset > 0:
            raise ArrowResourceIncompleteError("transport ended before final chunk")
        data, checksum = self.resources[authorization_resource_id]
        end = min(offset + maximum_bytes, len(data))
        payload = data[offset:end]
        returned_handle = authorization_resource_id
        returned_offset = offset
        returned_total = len(data)
        returned_checksum = checksum
        if not self.fault_used:
            if self.fault == "wrong_handle":
                returned_handle = _digest(240)
            elif self.fault == "wrong_checksum":
                returned_checksum = _digest(241)
            elif self.fault == "wrong_offset":
                returned_offset += 1
            elif self.fault == "wrong_length":
                returned_total += 1
            elif self.fault == "corrupt_payload":
                payload = bytes([payload[0] ^ 1]) + payload[1:]
            self.fault_used = self.fault is not None
        next_offset = returned_offset + len(payload)
        return ArrowResourceChunk(
            authorization_resource_id=returned_handle,
            offset=returned_offset,
            next_offset=next_offset,
            total_length=returned_total,
            content_checksum=returned_checksum,
            payload=payload,
            complete=next_offset == returned_total,
        )

    async def release(self, *, access: ArrowResultAccess) -> ArrowResultReleaseReceipt:
        if access != self.expected_access:
            raise ArrowResourceAccessError("wrong owner or opaque lease token")
        state = "already_released" if self.released else "released"
        self.released = True
        return ArrowResultReleaseReceipt(artifact_id=access.artifact_id, state=state)


def _presenter(
    fixture: ResultFixture,
    *,
    fault: str | None = None,
) -> tuple[ArrowResourcePresenter, FakeArrowReader]:
    reader = FakeArrowReader(fixture, fault=fault)
    return (
        ArrowResourcePresenter(
            reader,
            max_chunk_bytes=7,
            max_manifest_bytes=64 * 1024,
            max_relation_bytes=64 * 1024,
        ),
        reader,
    )


def test_happy_reassembly_preserves_raw_ipc_and_canonical_manifest() -> None:
    fixture = _fixture()
    presenter, _ = _presenter(fixture)

    async def exercise() -> None:
        presented = await presenter.read_package(
            fixture.descriptor,
            fixture.access,
            observed_at_unix_ms=1_500,
        )
        assert presented.manifest_bytes == fixture.manifest_bytes
        assert (
            canonicalize_value(presented.manifest.model_dump(mode="json", exclude_none=True))
            == fixture.manifest_bytes
        )
        assert presented.relations[0].ipc_bytes == fixture.relation_bytes
        assert presented.relations[0].descriptor.schema_checksum == _digest(8)
        assert presented.relations[0].descriptor.row_count == 3
        assert presented.relations[0].descriptor.batch_count == 1
        assert presented.relations[0].descriptor.coverage.state == "complete"

    asyncio.run(exercise())


@pytest.mark.parametrize(
    ("fault", "error_type"),
    [
        ("wrong_handle", ArrowResourceAccessError),
        ("wrong_checksum", ArrowResourceIntegrityError),
        ("wrong_offset", ArrowResourceIncompleteError),
        ("wrong_length", ArrowResourceIntegrityError),
        ("corrupt_payload", ArrowResourceIntegrityError),
        ("incomplete", ArrowResourceIncompleteError),
    ],
)
def test_chunk_handle_checksum_offset_length_and_completion_fail_closed(
    fault: str,
    error_type: type[Exception],
) -> None:
    fixture = _fixture()
    presenter, _ = _presenter(fixture, fault=fault)

    async def exercise() -> None:
        with pytest.raises(error_type):
            await presenter.read_package(
                fixture.descriptor,
                fixture.access,
                observed_at_unix_ms=1_500,
            )

    asyncio.run(exercise())


def test_wrong_owner_token_release_tombstone_and_expiry_are_explicit() -> None:
    fixture = _fixture()
    presenter, _ = _presenter(fixture)
    wrong_owner = ArrowResultAccess(
        artifact_id=fixture.access.artifact_id,
        owner=ArrowResultOwner(workspace_id="01" * 16, agent_id="03" * 16),
        lease_token=fixture.access.lease_token,
    )
    wrong_token = fixture.access.model_copy(update={"lease_token": "opaque-lease-token-0002"})

    async def exercise() -> None:
        with pytest.raises(ArrowResourceAccessError):
            await presenter.read_package(
                fixture.descriptor,
                wrong_owner,
                observed_at_unix_ms=1_500,
            )
        with pytest.raises(ArrowResourceAccessError):
            await presenter.read_package(
                fixture.descriptor,
                wrong_token,
                observed_at_unix_ms=1_500,
            )
        first = await presenter.release(
            fixture.descriptor,
            fixture.access,
            observed_at_unix_ms=1_500,
        )
        second = await presenter.release(
            fixture.descriptor,
            fixture.access,
            observed_at_unix_ms=1_500,
        )
        assert first.state == "released"
        assert second.state == "already_released"
        with pytest.raises(ArrowResourceReleasedError):
            await presenter.read_package(
                fixture.descriptor,
                fixture.access,
                observed_at_unix_ms=1_500,
            )

        fresh, _ = _presenter(fixture)
        with pytest.raises(ArrowResourceExpiredError):
            await fresh.read_package(
                fixture.descriptor,
                fixture.access,
                observed_at_unix_ms=2_000,
            )

    asyncio.run(exercise())


def test_strict_descriptor_rejects_extra_coercion_and_wrong_media() -> None:
    fixture = _fixture()
    extra = fixture.descriptor.model_dump(mode="python")
    extra["semantic_registry"] = {"forbidden": True}
    with pytest.raises(ValidationError, match="extra_forbidden"):
        validate_package_descriptor(extra)

    coerced = fixture.descriptor.model_dump(mode="python")
    coerced["total_rows"] = "3"
    with pytest.raises(ValidationError, match="int_type"):
        validate_package_descriptor(coerced)

    wrong_media = fixture.descriptor.model_dump(mode="python")
    wrong_media["relations"][0]["media_type"] = "application/json"
    with pytest.raises(ValidationError, match="literal_error"):
        validate_package_descriptor(wrong_media)


@pytest.mark.parametrize("metadata", ["schema", "row", "batch", "coverage"])
def test_manifest_metadata_must_match_descriptor(metadata: str) -> None:
    def mutate(manifest: dict[str, object]) -> None:
        resources = manifest["subresources"]
        assert isinstance(resources, list)
        resource = resources[0]
        assert isinstance(resource, dict)
        if metadata == "schema":
            resource["schema_checksum"] = _digest(42)
        elif metadata == "row":
            resource["row_count"] = 4
            manifest["total_rows"] = 4
        elif metadata == "batch":
            resource["batch_count"] = 2
            manifest["total_batches"] = 2
        else:
            resource["requested_units"] = 4
            resource["completed_units"] = 4

    fixture = _fixture(mutate_manifest=mutate)
    presenter, _ = _presenter(fixture)

    async def exercise() -> None:
        with pytest.raises(ArrowResourceContractError, match="metadata differs"):
            await presenter.read_package(
                fixture.descriptor,
                fixture.access,
                observed_at_unix_ms=1_500,
            )

    asyncio.run(exercise())


def test_noncanonical_or_semantic_row_manifest_is_rejected() -> None:
    noncanonical = _fixture(canonical_manifest=False)
    presenter, _ = _presenter(noncanonical)

    def add_semantic_rows(manifest: dict[str, object]) -> None:
        manifest["semantic_rows"] = [{"name": "Ada"}]

    semantic = _fixture(mutate_manifest=add_semantic_rows)
    semantic_presenter, _ = _presenter(semantic)

    async def exercise() -> None:
        with pytest.raises(ArrowResourceContractError, match="not canonical"):
            await presenter.read_package(
                noncanonical.descriptor,
                noncanonical.access,
                observed_at_unix_ms=1_500,
            )
        with pytest.raises(ArrowResourceContractError, match="strict contract"):
            await semantic_presenter.read_package(
                semantic.descriptor,
                semantic.access,
                observed_at_unix_ms=1_500,
            )

    asyncio.run(exercise())


def test_fastmcp_uris_expose_handles_without_tokens_or_content_authority() -> None:
    fixture = _fixture()
    manifest_uri = manifest_resource_uri(fixture.descriptor)
    relation_uri = relation_resource_uri(fixture.descriptor, fixture.descriptor.relations[0])
    assert manifest_uri.startswith("codefabric-result://")
    assert "/manifest/" in manifest_uri
    assert "/relation/public.people/" in relation_uri
    assert fixture.access.lease_token not in manifest_uri + relation_uri
    assert fixture.descriptor.manifest.content_resource_id.removeprefix("b3:") not in manifest_uri
    assert (
        fixture.descriptor.relations[0].content_resource_id.removeprefix("b3:") not in relation_uri
    )


def test_module_has_no_semantic_json_or_packaged_registry_authority() -> None:
    source = inspect.getsource(arrow_resources_module).casefold()
    for forbidden in (
        "import pyarrow",
        "from pyarrow",
        "import datafusion",
        "from datafusion",
        "model_registries",
        "query_forms",
        "json_object_adapter",
        "semantic_response",
    ):
        assert forbidden not in source
