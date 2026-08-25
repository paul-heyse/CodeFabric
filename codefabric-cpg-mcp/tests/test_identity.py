"""Cross-language acceptance oracles for CBEF-v1 identity contracts."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from codefabric_cpg_mcp.contracts.identity import (
    CbefField,
    CbefValue,
    IdentityDomain,
    IdentityError,
    TypeCode,
    WorkspacePath,
    canonical_type_term,
    decode_public_id,
    decode_record,
    derive_identity,
    encode_public_id,
    encode_record,
    validate_workspace_paths,
)

_ROOT = Path(__file__).parents[2]
_FIXTURES = _ROOT / "contracts" / "fixtures" / "identity"


def _load(name: str) -> dict[str, Any]:
    return json.loads((_FIXTURES / name).read_text())


def _value(source: dict[str, Any]) -> CbefValue:
    type_code = TypeCode(source["type_code"])
    value = source.get("value")
    match type_code:
        case TypeCode.ABSENT:
            return CbefValue(type_code)
        case TypeCode.BYTES | TypeCode.UNSIGNED | TypeCode.SIGNED | TypeCode.ID | TypeCode.DIGEST:
            assert isinstance(value, str)
            return CbefValue(type_code, bytes.fromhex(value))
        case TypeCode.UTF8:
            assert isinstance(value, str)
            return CbefValue(type_code, value, source.get("normalization", "NONE"))
        case TypeCode.RAW_PATH:
            assert isinstance(value, str)
            return CbefValue(type_code, bytes.fromhex(value), platform_code=source["platform_code"])
        case TypeCode.BOOLEAN:
            assert isinstance(value, bool)
            return CbefValue(type_code, value)
        case TypeCode.ORDERED_LIST | TypeCode.SET:
            assert isinstance(value, list)
            return CbefValue(type_code, tuple(_value(item) for item in value))
        case TypeCode.MAP:
            assert isinstance(value, list)
            return CbefValue(
                type_code,
                tuple((_value(entry["key"]), _value(entry["value"])) for entry in value),
            )
        case TypeCode.TAGGED_UNION:
            assert isinstance(value, dict)
            return CbefValue(
                type_code,
                _value(value["value"]),
                variant=value["variant"],
            )
    raise AssertionError("closed TypeCode match is exhaustive")


def _fields(source: list[dict[str, Any]]) -> tuple[CbefField, ...]:
    return tuple(CbefField(field["tag"], _value(field["value"])) for field in source)


def test_wp07_behavioral_acceptance() -> None:
    cbef = _load("cbef-v1-vectors.json")
    for case in cbef["cases"]:
        domain = IdentityDomain(case["domain_code"])
        fields = _fields(case["fields"])
        identity = derive_identity(domain, fields)
        assert identity.preimage.hex() == case["expected_preimage_hex"]
        assert identity.full_digest.hex() == case["expected_digest_hex"]
        assert identity.id_bytes.hex() == case["expected_id_hex"]
        assert (
            encode_public_id(domain, identity.id_bytes, case["kind_slug"])
            == case["expected_public_id"]
        )
        assert (
            decode_public_id(domain, case["expected_public_id"], case["kind_slug"])
            == identity.id_bytes
        )
        assert encode_record(domain, fields) == identity.preimage
        assert decode_record(identity.preimage).domain is domain

    all_types = cbef["all_type_codes"]
    identity = derive_identity(
        IdentityDomain(all_types["domain_code"]), _fields(all_types["fields"])
    )
    assert identity.preimage.hex() == all_types["expected_preimage_hex"]
    assert identity.full_digest.hex() == all_types["expected_digest_hex"]

    paths = _load("path-canonicalization-v1-vectors.json")
    for case in paths["cases"]:
        path = WorkspacePath.from_components(
            bytes.fromhex(case["workspace_id_hex"]),
            case["platform_code"],
            case["case_sensitivity_mode"],
            tuple(bytes.fromhex(value) for value in case["components_hex"]),
        )
        assert path.raw_relative_path_bytes.hex() == case["expected_raw_hex"]
        assert path.canonical_component_bytes.hex() == case["expected_canonical_hex"]
        assert path.comparison_key_bytes.hex() == case["expected_comparison_hex"]
        assert path.display_string == case["expected_display"]
        assert path.display_is_lossy is case["expected_display_is_lossy"]
        assert path.canonical_uri() == case["expected_uri"]
        assert path.decoded_components() == tuple(
            bytes.fromhex(value) for value in case["components_hex"]
        )

    types = _load("type-algebra-v1-vectors.json")
    for case in types["cases"]:
        fields = _fields(case["fields"])
        term = canonical_type_term(case["constructor_code"], fields)
        assert term.hex() == case["expected_canonical_term_hex"]
        identity = derive_identity(
            IdentityDomain.TYPE,
            (
                CbefField(1, CbefValue(TypeCode.ID, bytes.fromhex(case["workspace_id_hex"]))),
                CbefField(
                    2,
                    CbefValue(TypeCode.ID, bytes.fromhex(case["analysis_context_id_hex"])),
                ),
                CbefField(3, CbefValue(TypeCode.UNSIGNED, b"\x00\x01")),
                CbefField(4, CbefValue(TypeCode.BYTES, term)),
            ),
        )
        assert identity.preimage.hex() == case["expected_identity_preimage_hex"]
        assert identity.full_digest.hex() == case["expected_type_digest_hex"]
        assert identity.id_bytes.hex() == case["expected_type_id_hex"]


def test_wp07_structural_acceptance() -> None:
    assert [domain.value for domain in IdentityDomain] == list(range(1, 18))
    assert IdentityDomain.ROOT_AUTHORIZATION.value == 17
    assert [type_code.value for type_code in TypeCode] == list(range(13))
    assert len(_load("cbef-v1-vectors.json")["cases"]) == 16
    assert len(_load("type-algebra-v1-vectors.json")["cases"]) >= 4


def test_wp07_negative_zero_state() -> None:
    with pytest.raises(IdentityError):
        encode_record(
            IdentityDomain.WORKSPACE,
            (
                CbefField(2, CbefValue(TypeCode.BYTES, b"b")),
                CbefField(1, CbefValue(TypeCode.BYTES, b"a")),
            ),
        )
    with pytest.raises(IdentityError):
        decode_record(b"CFID\x01\x00\x01\x00\x00trailing")
    with pytest.raises(IdentityError):
        decode_public_id(IdentityDomain.ENTITY, "entity:Callable:" + "00" * 16, "callable")
    with pytest.raises(IdentityError):
        decode_public_id(IdentityDomain.WORKSPACE, "workspace:" + "AA" * 16)


def test_wp07_operational_acceptance() -> None:
    path_cases = {
        case["id"]: case for case in _load("path-canonicalization-v1-vectors.json")["cases"]
    }
    for left_id, right_id in _load("path-canonicalization-v1-vectors.json")["collision_pairs"]:
        paths = []
        for case in (path_cases[left_id], path_cases[right_id]):
            paths.append(
                WorkspacePath.from_components(
                    bytes.fromhex(case["workspace_id_hex"]),
                    case["platform_code"],
                    case["case_sensitivity_mode"],
                    tuple(bytes.fromhex(value) for value in case["components_hex"]),
                )
            )
        with pytest.raises(IdentityError, match="BLOCKED_PATH_COLLISION"):
            validate_workspace_paths(paths)

    one = paths[0]
    validate_workspace_paths((one, one))
    assert "not generated by production" in _load("cbef-v1-vectors.json")["construction"]
