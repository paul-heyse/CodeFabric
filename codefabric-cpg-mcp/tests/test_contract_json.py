"""Cross-language conformance tests for ``codefabric-jcs-v1``."""

import json
from collections.abc import Callable
from pathlib import Path

import pytest

from codefabric_cpg_mcp.contracts.generated import (
    CONTRACT_ARTIFACT_INDEX_DIGEST,
    CONTRACT_ARTIFACTS,
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


@pytest.mark.parametrize("vector", CORPUS["positive"], ids=lambda vector: vector["id"])
def test_shared_positive_vectors(vector: dict[str, str]) -> None:
    canonical = canonicalize_json(vector["input_json"])

    assert canonical == vector["canonical_utf8"].encode()
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


def test_utf8_bom_is_rejected() -> None:
    with pytest.raises(CanonicalJsonError, match="BOM"):
        canonicalize_json(b"\xef\xbb\xbf{}")


def test_generated_python_index_has_the_exact_source_census() -> None:
    assert len(CONTRACT_ARTIFACTS) == 50
    validate_checksum(CONTRACT_ARTIFACT_INDEX_DIGEST)
    for artifact in CONTRACT_ARTIFACTS:
        validate_checksum(artifact.canonical_digest)
