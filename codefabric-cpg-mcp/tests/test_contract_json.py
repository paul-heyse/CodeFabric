"""Cross-language conformance tests for ``codefabric-jcs-v1``."""

import json
from collections.abc import Callable
from pathlib import Path

import pytest
from blake3 import blake3

from codefabric_cpg_mcp.contracts.index import (
    decode_model_artifact_index,
    model_artifact_index,
    model_artifact_index_bytes,
    model_artifact_index_digest,
)
from codefabric_cpg_mcp.contracts.json import (
    CanonicalJsonError,
    canonicalize_json,
    canonicalize_value,
    checksum,
    non_string_map_records,
    validate_bytes,
    validate_checksum,
    validate_int64,
    validate_lowercase_public,
    validate_uint64,
)

ROOT = Path(__file__).resolve().parents[2]
CORPUS = json.loads((ROOT / "contracts/fixtures/jcs/vectors.json").read_text(encoding="utf-8"))
PROJECTION_CORPUS = json.loads(
    (ROOT / "contracts/fixtures/projections/vectors.json").read_text(encoding="utf-8")
)
DIFFERENTIAL_CORPUS = json.loads(
    (ROOT / "contracts/fixtures/jcs/differential-cases.json").read_text(encoding="utf-8")
)


@pytest.mark.parametrize("vector", CORPUS["positive"], ids=lambda vector: vector["id"])
def test_shared_positive_vectors(vector: dict[str, str]) -> None:
    canonical = canonicalize_json(vector["input_json"])
    expected_raw_digest = bytes.fromhex(vector["checksum"].removeprefix("b3:"))

    assert canonical == vector["canonical_utf8"].encode()
    assert blake3(canonical).digest() == expected_raw_digest
    assert vector["checksum"] == f"b3:{expected_raw_digest.hex()}"
    assert checksum(canonical) == vector["checksum"]


@pytest.mark.parametrize("vector", CORPUS["negative"], ids=lambda vector: vector["id"])
def test_shared_negative_vectors(vector: dict[str, str]) -> None:
    with pytest.raises(CanonicalJsonError) as caught:
        canonicalize_json(vector["input_json"])

    assert caught.value.failure_class == vector["error"]


@pytest.mark.parametrize(
    ("validator", "group"),
    [
        (validate_int64, "int64"),
        (validate_uint64, "uint64"),
        (validate_bytes, "bytes"),
        (validate_lowercase_public, "lowercase_public"),
        (validate_checksum, "checksum"),
    ],
)
def test_shared_schema_format_vectors(
    validator: Callable[[str], None],
    group: str,
) -> None:
    for value in CORPUS["formats"][group]["positive"]:
        validator(value)
    for value in CORPUS["formats"][group]["negative"]:
        with pytest.raises(CanonicalJsonError):
            validator(value)


def test_non_string_maps_are_sorted_as_records() -> None:
    fixture = CORPUS["non_string_map"]
    entries = [(record["key"], record["value"]) for record in fixture["entries"]]

    assert canonicalize_value(non_string_map_records(entries)) == fixture["canonical_utf8"].encode()


@pytest.mark.parametrize("case", DIFFERENTIAL_CORPUS["cases"], ids=lambda case: case["id"])
def test_output_free_differential_inputs_are_equivalent_and_idempotent(
    case: dict[str, object],
) -> None:
    inputs = case["inputs"]
    assert isinstance(inputs, list)
    outputs = [canonicalize_json(value) for value in inputs if isinstance(value, str)]

    assert outputs
    assert all(output == outputs[0] for output in outputs)
    assert canonicalize_json(outputs[0]) == outputs[0]


@pytest.mark.parametrize("vector", PROJECTION_CORPUS["vectors"], ids=lambda vector: vector["id"])
def test_shared_projection_vector_blake3_identities(vector: dict[str, str]) -> None:
    source = vector["source_utf8"].encode()
    canonical = vector["canonical_utf8"].encode()

    assert checksum(source) == vector["source_digest"]
    assert blake3(source).digest().hex() == vector["source_digest"].removeprefix("b3:")
    assert checksum(canonical) == vector["canonical_digest"]
    assert blake3(canonical).digest().hex() == vector["canonical_digest"].removeprefix("b3:")
    if identity := vector.get("bundle_identity_utf8"):
        assert checksum(identity.encode()) == vector["bundle_digest"]


def test_utf8_bom_is_rejected() -> None:
    with pytest.raises(CanonicalJsonError, match="BOM"):
        canonicalize_json(b"\xef\xbb\xbf{}")


def test_packaged_model_index_has_the_exact_compiled_census_and_bytes() -> None:
    compiled_manifest = json.loads(
        (ROOT / "contracts/manifests/suite-manifest.json").read_text(encoding="utf-8")
    )
    repository_resource = (
        ROOT / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json"
    ).read_bytes()
    index = model_artifact_index()

    assert model_artifact_index_bytes() == repository_resource
    assert [artifact.artifact_id for artifact in index.artifacts] == [
        artifact["artifact_id"] for artifact in compiled_manifest["artifacts"]
    ]
    assert [output.path for output in index.outputs] == [
        output["path"] for output in compiled_manifest["outputs"]
    ]
    assert len(index.outputs) == 78
    assert model_artifact_index_digest() == checksum(repository_resource)
    validate_checksum(model_artifact_index_digest())
    for artifact in index.artifacts:
        validate_checksum(artifact.canonical_digest)
        validate_checksum(artifact.source_digest)


def _model_index_mutation(mutation: str) -> bytes:
    resource = model_artifact_index_bytes()
    if mutation == "none":
        return resource
    value = json.loads(resource)
    if mutation == "unknown-root-field":
        value["unexpected"] = True
    elif mutation == "unsafe-authority-path":
        value["artifacts"][0]["authority_path"] = "../escape.json"
    elif mutation == "mixed-resource-profile":
        value["artifacts"][0]["resource_profile"]["max_source_bytes"] = 1
    elif mutation == "unsorted-artifacts":
        value["artifacts"][0], value["artifacts"][1] = (
            value["artifacts"][1],
            value["artifacts"][0],
        )
    elif mutation == "unsorted-output-paths":
        value["outputs"][0], value["outputs"][1] = (
            value["outputs"][1],
            value["outputs"][0],
        )
    else:
        raise AssertionError(f"unknown differential mutation {mutation}")
    return canonicalize_value(value)


def test_wp67_structural_acceptance_model_index_python_differential_corpus() -> None:
    cases = json.loads(
        (ROOT / "contracts/fixtures/model-index-decode-differential.json").read_text(
            encoding="utf-8"
        )
    )
    for case in cases["cases"]:
        try:
            decode_model_artifact_index(_model_index_mutation(case["mutation"]))
            accepted = True
        except CanonicalJsonError, ValueError:
            accepted = False
        assert accepted is case["accepted"], case["mutation"]
