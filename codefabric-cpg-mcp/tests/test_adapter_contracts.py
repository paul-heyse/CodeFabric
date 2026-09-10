"""Strict v2.3 presentation-model and live-schema tests."""

from __future__ import annotations

import pytest
from jsonschema import Draft202012Validator
from pydantic import ValidationError

from codefabric_cpg_mcp.contracts.wire_models import (
    PublicToolMeta,
    QueryBlockOutcome,
    QueryToolInput,
    QueryToolOutput,
    ReferenceToolOutput,
    ResourceReference,
    WireSchemaName,
    wire_schema,
    wire_schema_fingerprints,
)


def _resource(kind: str = "result_manifest") -> ResourceReference:
    return ResourceReference(
        uri="cpg://result/public%3Amanifest/manifest/0",
        kind=kind,  # type: ignore[arg-type]
        media_type="application/json",
        package_id="package:one",
        total_bytes=512,
        content_checksum="b3:" + "01" * 32,
        expires_at_unix_ms=4_000_000_000_000,
    )


def test_released_models_are_strict_closed_and_frozen() -> None:
    value = QueryToolInput(request={"form": "entities_by_kind", "limit": 10})
    assert value.delivery == "automatic"
    with pytest.raises(ValidationError):
        QueryToolInput.model_validate({"request": {}, "freshness": "await_latest"}, strict=True)
    with pytest.raises(ValidationError):
        QueryToolInput.model_validate({"request": {"bad": object()}}, strict=True)
    with pytest.raises(ValidationError):
        value.delivery = "resource"  # type: ignore[misc]


def test_query_projection_separates_presentation_and_daemon_identities() -> None:
    manifest = _resource()
    page = ResourceReference(
        uri="cpg://result/public%3Apage%3A0/page/0",
        kind="result_page",
        media_type="application/vnd.apache.arrow.stream",
        package_id="package:one",
        page_ordinal=0,
        total_bytes=128,
        content_checksum="b3:" + "03" * 32,
        expires_at_unix_ms=4_000_000_000_000,
    )
    output = QueryToolOutput(
        outcome="accepted",
        daemon_query_id="query:one",
        semantic_request_id="semantic:one",
        execution_state="SUCCEEDED",
        epoch_id="epoch:one",
        package_id="package:one",
        manifest=manifest,
        pages=(page,),
        total_rows=2,
        total_pages=1,
        total_bytes=512,
    )
    meta = PublicToolMeta(
        semantic_request_id=output.semantic_request_id,
        daemon_query_id=output.daemon_query_id,
        epoch_id=output.epoch_id,
        package_id=output.package_id,
    )

    dumped = output.model_dump(mode="json")
    assert meta.contract_version == "2.3"
    assert "mcp_call_id" not in dumped
    assert "rpc_attempt_id" not in dumped
    assert "lease" not in str(dumped)
    with pytest.raises(ValidationError):
        QueryToolOutput(
            outcome="accepted",
            semantic_request_id="semantic:one",
            execution_state="SUCCEEDED",
        )
    with pytest.raises(ValidationError):
        QueryToolOutput(
            outcome="accepted",
            daemon_query_id="query:one",
            semantic_request_id="semantic:one",
            execution_state="SUCCEEDED",
            package_id="package:one",
            manifest=manifest,
            total_pages=1,
        )


def test_query_outcomes_require_a_sealed_result_and_unique_request_ids() -> None:
    complete = QueryBlockOutcome(query_id="first", execution_state="COMPLETE")
    for outcomes, state in [
        ((), "SUCCEEDED"),
        ((complete, complete), "SUCCEEDED"),
        ((complete,), "FAILED"),
    ]:
        with pytest.raises(ValidationError, match="sealed result"):
            QueryToolOutput(
                outcome="accepted",
                daemon_query_id="query:one",
                semantic_request_id="semantic:one",
                execution_state=state,  # type: ignore[arg-type]
                query_results=outcomes,
            )


def test_reference_projects_only_a_public_daemon_resource() -> None:
    resource = ResourceReference(
        uri="cpg://reference/public%3Areference/guide/current",
        kind="reference",
        media_type="application/json",
        total_bytes=128,
        content_checksum="b3:" + "02" * 32,
        expires_at_unix_ms=4_000_000_000_000,
    )
    reference = ReferenceToolOutput(reference_id="reference:one", resource=resource)

    dumped = reference.model_dump(mode="json")
    assert dumped["resource"]["uri"].startswith("cpg://reference/")
    assert "content" not in dumped
    assert "lease" not in str(dumped)


def test_all_wire_schemas_are_draft_2020_12_and_fingerprinted() -> None:
    assert len(WireSchemaName) == 13
    for mode in ("validation", "serialization"):
        fingerprints = dict(wire_schema_fingerprints(mode))
        assert set(fingerprints) == set(WireSchemaName)
        assert all(value.startswith("b3:") and len(value) == 67 for value in fingerprints.values())
        for name in WireSchemaName:
            schema = wire_schema(name, mode)
            assert schema["$schema"] == Draft202012Validator.META_SCHEMA["$schema"]
            assert schema["$id"].startswith("https://codefabric.dev/schema/adapter/2.3/")
            Draft202012Validator.check_schema(schema)
