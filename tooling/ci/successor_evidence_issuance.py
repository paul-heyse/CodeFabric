"""Validate the frozen, first-principles relational-fabric v3 evidence issuance.

The validator is deliberately independent of production Rust/Python code and of all
historical acceptance corpora.  It validates human-authored decoded expectations,
semantic causal/negative fixtures, independent row review, immutable identities, and
consumer ordering.  It never executes or imports the behavior whose later acceptance
will consume these artifacts.
"""

from __future__ import annotations

import argparse
import ast
import copy
import hashlib
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

import blake3
import jsonschema
import rfc8785

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_ROOT = Path("contracts/acceptance/relational-fabric-v3")
EXPECTATIONS_PATH = EVIDENCE_ROOT / "expectations.jsonl"
FIXTURES_PATH = EVIDENCE_ROOT / "negative-fixtures.jsonl"
ISSUANCE_PATH = EVIDENCE_ROOT / "evidence-issuance.json"
PLAN_PATH = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md"
)

CLAIM_ID = re.compile(r"RFV3-CLAIM-\d{3}\Z")
FIXTURE_ID = re.compile(r"RFV3-FIX-\d{3}-(?:C|N)\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
B3 = re.compile(r"b3:[0-9a-f]{64}\Z")
PACKET_HEADING = re.compile(r"^### (WP\d{2}) — .+$", re.MULTILINE)

CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE = 4
CBEF_SOURCE_FILE_DOMAIN_CODE = 6
CBEF_ENTITY_DOMAIN_CODE = 8
CBEF_RELATION_FACT_DOMAIN_CODE = 9
CBEF_PATH_RESULT_DOMAIN_CODE = 18
CBEF_QUERY_SOURCE_CONTEXT_DOMAIN_CODE = 21
PYTHON_FUNCTION_KIND_CODE = 120
PYTHON_CALL_SITE_KIND_CODE = 130
CALLS_RELATION_KIND_CODE = 50

EXPECTED_FAMILIES = frozenset(
    {
        "exact_provider_facts",
        "programmatic_transformations",
        "derived_analyses",
        "query_find_code_entities",
        "query_retrieve_facts",
        "query_follow_relationships",
        "query_connecting_paths",
        "query_match_pattern",
        "query_combine_results",
        "query_summarize_facts",
        "query_source_context",
        "delta_exact_version_protocol",
        "activation_recovery",
        "authorization",
        "resource_terminals",
        "security_denial",
        "released_wire_projection",
        "clean_incremental_equivalence",
    }
)
EXPECTED_QUERY_FAMILIES = frozenset(
    {
        "query_find_code_entities",
        "query_retrieve_facts",
        "query_follow_relationships",
        "query_connecting_paths",
        "query_match_pattern",
        "query_combine_results",
        "query_summarize_facts",
        "query_source_context",
    }
)
PIN_CONTRACT = {
    "suite": "codefabric-relational-data-fabric@2.1.0",
    "arrow": "59.2.0",
    "datafusion": "55.0.0",
    "object_store": "0.13.2",
    "deltalake": "git:43a0cf10a313e5077c48637ad786a05359136bbb",
}
PROVIDER_PIN_CONTRACT = {
    **PIN_CONTRACT,
    "tree_sitter_runtime": "0.26.12",
    "tree_sitter_python": "0.25.0",
    "tree_sitter_rust": "0.24.2",
    "ruff_stable_root": "0.0.7",
    "pyrefly": "1.2.0@1933169ad8ee9e4d4114112eb56ef0811fb0a094",
    "rustc_extractor_toolchain": "nightly-2026-08-18",
    "rustc_compiler": ("1.100.0-nightly@8fa1c96cfd489e4c27654c144ae871ce2c4db6c6"),
}
PROVIDER_RELEASE_INPUT_PINS = {
    "tree_sitter_runtime": "0.26.12",
    "tree_sitter_python": "0.25.0",
    "ruff_stable_root": "0.0.7",
    "pyrefly": "1.2.0@1933169ad8ee9e4d4114112eb56ef0811fb0a094",
    "rustc_extractor_toolchain": "nightly-2026-08-18",
    "rustc_compiler": "1.100.0-nightly@8fa1c96cfd489e4c27654c144ae871ce2c4db6c6",
}
WIRE_PIN_CONTRACT = {
    **PIN_CONTRACT,
    "public_response_schema": (
        "sha256:0c3c29141d58bf7c7a91556320ea53e7ad001486128cfb1368dd31e2118897fc"
    ),
    "query_service_proto": (
        "sha256:ec5c773af9f9cc3ba503ca3ed0b10a91471c8696d0a8aedbcd1afedd8bfa10b8"
    ),
    "public_response_version": "1.3",
    "protobuf_package": "codefabric.cpgd.v1",
}
QUERY_INPUT_ROLES = frozenset(
    {
        "request_envelope",
        "negotiated_profile",
        "pinned_epoch",
        "authorized_child_catalog",
        "admitted_relations",
        "program_binding",
        "access_scope",
        "producer_coverage",
        "resource_limits",
    }
)
RETURN_KEYS = {
    "include",
    "exclude",
    "result_shape",
    "group_by",
    "order_by",
    "deduplicate_by",
    "supporting_facts",
    "include_query_result",
    "limit",
}
QUERY_REQUEST_KEY_CONTRACT = {
    "query_find_code_entities": {
        "query_id",
        "request",
        "looking_for",
        "within",
        "where",
        "return",
    },
    "query_retrieve_facts": {
        "query_id",
        "request",
        "about",
        "facts",
        "where",
        "return",
    },
    "query_follow_relationships": {
        "query_id",
        "request",
        "starting_from",
        "relationship",
        "direction",
        "distance",
        "stop_when",
        "where",
        "return",
    },
    "query_connecting_paths": {
        "query_id",
        "request",
        "from",
        "to",
        "using",
        "path_policy",
        "where",
        "return",
    },
    "query_match_pattern": {
        "query_id",
        "request",
        "pattern",
        "where",
        "return",
    },
    "query_combine_results": {
        "query_id",
        "request",
        "inputs",
        "operation",
        "where",
        "return",
    },
    "query_summarize_facts": {
        "query_id",
        "request",
        "about",
        "measure",
        "group_by",
        "where",
        "return",
    },
    "query_source_context": {
        "query_id",
        "request",
        "about",
        "context",
        "where",
        "return",
    },
}
NORMATIVE_TAG_PATHS = {
    "SUITE": Path(
        "docs/authoritative_design/"
        "codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.1.md"
    ),
    "ONT": Path(
        "docs/authoritative_design/"
        "code_property_graph_present_state_fact_ontology_specification_v2.1.md"
    ),
    "GEN": Path(
        "docs/authoritative_design/"
        "present_state_cpg_fact_generation_specification_python_rust_v2.1.md"
    ),
    "FAB": Path(
        "docs/authoritative_design/"
        "present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.1.md"
    ),
    "QRY": Path(
        "docs/authoritative_design/"
        "code_property_graph_semantic_query_specification_v2.1.md"
    ),
    "LIFE": Path(
        "docs/authoritative_design/"
        "codefabric_continuous_cpg_update_lifecycle_management_specification_v2.1.md"
    ),
    "SRV": Path(
        "docs/authoritative_design/"
        "present_state_cpg_fastmcp_serving_specification_v2.1.md"
    ),
}
PRINCIPLES_PATH = Path("docs/library_ref/full_data_fabric_design_principles_v2.md")
INPUT_ROLE_CONTRACT = {
    "exact_provider_facts": frozenset(
        {
            "source_images",
            "provider_requests",
            "semantic_context",
            "provider_release_pins",
            "requested_family_set",
            "coverage_terminals",
            "execution_policy",
            "protocol_schema_identity",
        }
    ),
    "programmatic_transformations": frozenset(
        {
            "admitted_input_relation",
            "input_schema",
            "transformation_definition",
            "transformation_release",
            "authority_context",
            "resource_policy",
        }
    ),
    "derived_analyses": frozenset(
        {
            "accepted_family_census",
            "python_cfg_inputs",
            "rust_mir_cfg_inputs",
            "rust_control_native_inputs",
            "provider_call_targets",
            "canonical_call_occurrences",
            "canonical_callable_lookup",
            "analysis_definitions",
            "precision_profiles",
            "authority_context",
            "coverage_terminals",
        }
    ),
    **{family: QUERY_INPUT_ROLES for family in EXPECTED_QUERY_FAMILIES},
    "delta_exact_version_protocol": frozenset(
        {
            "delta_table_history",
            "selected_version_vector",
            "protocol_support",
            "table_root_identity",
            "runtime_configuration",
            "proof_input",
        }
    ),
    "activation_recovery": frozenset(
        {
            "activation_chain",
            "recovery_policy",
            "receipt_cache_observation",
            "candidate_memory_observation",
        }
    ),
    "authorization": frozenset(
        {
            "access_scope",
            "authorization_policy",
            "epoch_provider_catalog",
            "child_catalog_bindings",
            "bound_plan",
            "provider_rows",
            "resource_policy",
        }
    ),
    "resource_terminals": frozenset(
        {
            "query_identity",
            "bound_plan",
            "resource_budget",
            "reservation",
            "cancellation_state",
            "registry_state",
            "lease_policy",
            "cpu_budget",
            "actual_output_batch",
            "delivery_policy",
        }
    ),
    "security_denial": frozenset(
        {
            "provider_jobs",
            "trust_policy",
            "explicit_authorization",
            "launcher_evidence_contract",
            "launcher_constraints",
            "hostile_actions",
            "resource_limits",
        }
    ),
    "released_wire_projection": frozenset(
        {
            "candidate_released_projection",
            "daemon_canonical_response_results",
            "internal_terminal",
            "request_context",
            "private_diagnostics",
            "public_json_contract",
            "protobuf_contract",
            "public_projection_policy",
            "access_scope",
            "redaction_policy",
        }
    ),
    "clean_incremental_equivalence": frozenset(
        {
            "source_images",
            "incremental_base_state",
            "provider_release_vector",
            "transformation_analysis_release",
            "change_derivation",
            "route_definitions",
            "exact_table_vector",
            "coverage_proof_inputs",
            "policy_proof_pins",
        }
    ),
}
FAULT_DIMENSIONS = frozenset(
    {
        "authoritative_input",
        "provider_batch",
        "transformation",
        "coverage",
        "exact_version_feature",
        "authorization",
        "resource",
        "security",
        "protocol",
        "public_output",
        "objectivity",
    }
)
FORBIDDEN_IMPORT_ROOTS = (
    "src/",
    "target/",
    "tests/golden/",
    "rustc-extractor/src/",
    "pyrefly-sidecar/src/",
    "codefabric-cpg-mcp/src/",
    "contracts/acceptance/relational-fabric-v1/",
)
FORBIDDEN_ISSUANCE_EDGES = (
    "bootstrap_model_semantics",
    "replay-agreement",
    "replay_agreement",
    "model-digest",
    "model_digest",
    "comparator",
)

EXPECTATION_KEYS = {
    "claim_id",
    "claim_family",
    "subject",
    "author_id",
    "source_anchor",
    "governing_clauses",
    "complete_input_universe",
    "exact_pins",
    "decoded_expectation",
    "semantics",
    "limitations",
    "future_consumer",
    "causal_fixture_id",
    "negative_fixture_id",
}
FIXTURE_KEYS = {
    "fixture_id",
    "claim_id",
    "kind",
    "author_id",
    "source_anchor",
    "fault_dimension",
    "authoritative_change",
    "expected_terminal",
    "expected_decoded",
    "semantic_basis",
    "semantic",
    "integrity_only",
    "imports",
    "mutation",
}


class SuccessorEvidenceError(ValueError):
    """The successor evidence issuance is incomplete, mutable, or circular."""


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise SuccessorEvidenceError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def _load_json(path: Path, context: str) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise SuccessorEvidenceError(f"missing or invalid {context}: {path}") from error
    if not isinstance(value, dict):
        raise SuccessorEvidenceError(f"{context} must be a JSON object: {path}")
    return value


def _load_jsonl(path: Path, context: str) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise SuccessorEvidenceError(f"missing or invalid {context}: {path}") from error
    if not lines:
        raise SuccessorEvidenceError(f"{context} selected zero rows: {path}")
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, 1):
        if not line.strip():
            raise SuccessorEvidenceError(
                f"{context} contains a blank JSONL row at line {line_number}"
            )
        try:
            value = json.loads(line, object_pairs_hook=_reject_duplicates)
        except json.JSONDecodeError as error:
            raise SuccessorEvidenceError(
                f"invalid {context} JSON at line {line_number}"
            ) from error
        if not isinstance(value, dict):
            raise SuccessorEvidenceError(
                f"{context} line {line_number} must be an object"
            )
        rows.append(value)
    return rows


def _strict_keys(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    actual = set(value)
    if actual != expected:
        raise SuccessorEvidenceError(
            f"{context} keys differ: missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        raise SuccessorEvidenceError(f"{context} must be an object")
    return value


def _nonempty_string(value: object, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise SuccessorEvidenceError(f"{context} must be a non-empty string")
    return value


def _string_list(value: object, context: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise SuccessorEvidenceError(f"{context} must be a non-empty list")
    result = [_nonempty_string(item, f"{context} item") for item in value]
    if len(result) != len(set(result)):
        raise SuccessorEvidenceError(f"{context} contains duplicates")
    return result


def _relative_path(value: object, context: str) -> str:
    text = _nonempty_string(value, context)
    path = PurePosixPath(text)
    if path.is_absolute() or ".." in path.parts or "\\" in text or "\x00" in text:
        raise SuccessorEvidenceError(f"{context} is not a safe relative path: {text}")
    return text


def _sha256(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as error:
        raise SuccessorEvidenceError(
            f"cannot hash evidence artifact: {path}"
        ) from error


def _canonical_sha256(value: object) -> str:
    payload = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _canonical_b3(value: object) -> str:
    return f"b3:{blake3.blake3(rfc8785.dumps(value)).hexdigest()}"


def _bytes_b3(value: bytes) -> str:
    return f"b3:{blake3.blake3(value).hexdigest()}"


def _cbef_field(tag: int, type_code: int, payload: bytes) -> bytes:
    return (
        tag.to_bytes(2, "big")
        + type_code.to_bytes(1, "big")
        + len(payload).to_bytes(4, "big")
        + payload
    )


def _cbef_record(domain_code: int, fields: Sequence[tuple[int, int, bytes]]) -> bytes:
    if (
        not fields
        or fields[0][0] == 0
        or [field[0] for field in fields] != sorted({field[0] for field in fields})
    ):
        raise SuccessorEvidenceError(
            "CBEF-v1 fields must have unique ascending nonzero tags"
        )
    return (
        b"CFID"
        + b"\x01"
        + domain_code.to_bytes(2, "big")
        + len(fields).to_bytes(2, "big")
        + b"".join(_cbef_field(*field) for field in fields)
    )


def _public_id_bytes(value: object, prefix: str) -> bytes:
    text = str(value)
    expected = f"{prefix}:"
    encoded = text.removeprefix(expected)
    if not text.startswith(expected) or len(encoded) != 32:
        raise SuccessorEvidenceError(
            f"{prefix} identity must contain exactly 16 lowercase bytes"
        )
    try:
        decoded = bytes.fromhex(encoded)
    except ValueError as error:
        raise SuccessorEvidenceError(
            f"{prefix} identity is not lowercase hexadecimal"
        ) from error
    if encoded != encoded.lower():
        raise SuccessorEvidenceError(f"{prefix} identity is not lowercase hexadecimal")
    return decoded


def _analysis_context_id_bytes(value: object) -> bytes:
    text = str(value)
    if text == "context:source":
        return b"\xff" * 16
    if text.startswith("context:"):
        return _public_id_bytes(text, "context")
    if len(text) == 64 and text == text.lower():
        try:
            # Provider relations retain the full 256-bit context digest; CBEF ID
            # fields carry its governed 128-bit public identity.
            return bytes.fromhex(text)[:16]
        except ValueError as error:
            raise SuccessorEvidenceError(
                "analysis context digest is not hexadecimal"
            ) from error
    raise SuccessorEvidenceError(
        "analysis context identity is neither context:source nor a CBEF ID"
    )


def _digest_bytes(value: object, prefix: str) -> bytes:
    text = str(value)
    expected = f"{prefix}:"
    encoded = text.removeprefix(expected)
    if not text.startswith(expected) or len(encoded) != 64:
        raise SuccessorEvidenceError(
            f"{prefix} digest must contain exactly 32 lowercase bytes"
        )
    try:
        decoded = bytes.fromhex(encoded)
    except ValueError as error:
        raise SuccessorEvidenceError(
            f"{prefix} digest is not lowercase hexadecimal"
        ) from error
    if encoded != encoded.lower():
        raise SuccessorEvidenceError(f"{prefix} digest is not lowercase hexadecimal")
    return decoded


def _cbef_analysis_context_id(
    *, workspace_id: object, language_slug: str, environment_digest: object
) -> str:
    """Build the closed CBEF-v1 ANALYSIS_CONTEXT recipe."""

    if not language_slug.isascii():
        raise SuccessorEvidenceError("analysis-context language slug must be ASCII")
    preimage = _cbef_record(
        CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE,
        [
            (1, 7, _public_id_bytes(workspace_id, "workspace")),
            (2, 2, language_slug.lower().encode("utf-8")),
            (3, 8, _digest_bytes(environment_digest, "b3")),
        ],
    )
    return f"context:{blake3.blake3(preimage).digest()[:16].hex()}"


def _cbef_analysis_context_digest(
    *, workspace_id: object, language_slug: str, environment_digest: object
) -> str:
    if not language_slug.isascii():
        raise SuccessorEvidenceError("analysis-context language slug must be ASCII")
    preimage = _cbef_record(
        CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE,
        [
            (1, 7, _public_id_bytes(workspace_id, "workspace")),
            (2, 2, language_slug.lower().encode("utf-8")),
            (3, 8, _digest_bytes(environment_digest, "b3")),
        ],
    )
    return blake3.blake3(preimage).hexdigest()


def _cbef_source_file_id(*, workspace_id: object, comparison_key: bytes) -> str:
    if not comparison_key:
        raise SuccessorEvidenceError("source-file comparison key must be non-empty")
    preimage = _cbef_record(
        CBEF_SOURCE_FILE_DOMAIN_CODE,
        [
            (1, 7, _public_id_bytes(workspace_id, "workspace")),
            (2, 7, _analysis_context_id_bytes("context:source")),
            (3, 1, comparison_key),
        ],
    )
    return blake3.blake3(preimage).digest()[:16].hex()


def _provider_semantic_environment_digest(request: Mapping[str, Any]) -> str:
    return _canonical_b3(
        {
            "provider_id": request["provider_id"],
            "provider_release": request["provider_release"],
            "relation_id": request["relation_id"],
            "crate_manifest_target_inputs": request["crate_manifest_target_inputs"],
            "trust_inputs": request["trust_inputs"],
        }
    )


def _provider_schema_identity(contract: Mapping[str, Any]) -> str:
    """Reconstruct the provider's governed Arrow schema identity boundary."""

    if contract.get("relation_id") != "provider.pyrefly.call_target.v1":
        return _canonical_b3(contract)
    provider_release = str(contract.get("provider_release", ""))
    try:
        release, revision = provider_release.removeprefix("pyrefly@").split("#", 1)
        fields = ";".join(
            f"{field_value['name']}:{field_value['data_type']}:"
            f"{'nullable' if field_value['nullable'] else 'required'}"
            for field_value in _list(contract.get("fields"), "Pyrefly schema fields")
            if isinstance(field_value, Mapping)
        )
    except (KeyError, ValueError) as error:
        raise SuccessorEvidenceError("Pyrefly schema descriptor differs") from error
    descriptor = (
        f"{contract['relation_id']}|protocol=1|"
        f"arrow={contract['arrow_type_universe']}|"
        f"provider={release}@{revision}|{fields}"
    )
    return _bytes_b3(descriptor.encode("utf-8"))


def _cbef_entity_preimage(
    *,
    workspace_id: object,
    analysis_context_id: object,
    kind_code: int,
    owner_id: object,
    owner_prefix: str,
    semantic_key: bytes,
) -> bytes:
    """Build the closed CBEF-v1 ENTITY recipe; callers cannot author tags."""

    return _cbef_record(
        CBEF_ENTITY_DOMAIN_CODE,
        [
            (1, 7, _public_id_bytes(workspace_id, "workspace")),
            (2, 7, _analysis_context_id_bytes(analysis_context_id)),
            (3, 4, kind_code.to_bytes(2, "big")),
            (4, 7, _public_id_bytes(owner_id, owner_prefix)),
            (5, 1, semantic_key),
        ],
    )


def _cbef_relation_fact_preimage(
    *,
    workspace_id: object,
    analysis_context_id: object,
    relation_kind_code: int,
    subject_id: object,
    object_id: object,
    role: str,
) -> bytes:
    """Build the closed CBEF-v1 RELATION_FACT recipe; callers cannot author tags."""

    role_payload = role.encode("utf-8")
    typed_role = b"\x02" + len(role_payload).to_bytes(4, "big") + role_payload
    tagged_role = b"\x00\x01" + len(typed_role).to_bytes(4, "big") + typed_role
    return _cbef_record(
        CBEF_RELATION_FACT_DOMAIN_CODE,
        [
            (1, 7, _public_id_bytes(workspace_id, "workspace")),
            (2, 7, _analysis_context_id_bytes(analysis_context_id)),
            (3, 4, relation_kind_code.to_bytes(2, "big")),
            (4, 7, _public_id_bytes(subject_id, "entity:function")),
            (5, 7, _public_id_bytes(object_id, "entity:function")),
            (6, 12, tagged_role),
        ],
    )


def _python_function_id(
    identity_context: Mapping[str, Any], qualified_lexical_path: Sequence[str]
) -> str:
    semantic_key = rfc8785.dumps(
        {
            "module_id": identity_context["module_id"],
            "qualified_lexical_path": list(qualified_lexical_path),
            "kind": "function",
            "schema_version": 1,
        }
    )
    preimage = _cbef_entity_preimage(
        workspace_id=identity_context["workspace_id"],
        analysis_context_id=identity_context["analysis_context_id"],
        kind_code=PYTHON_FUNCTION_KIND_CODE,
        owner_id=identity_context["module_id"],
        owner_prefix="entity:module",
        semantic_key=semantic_key,
    )
    return f"entity:function:{blake3.blake3(preimage).digest()[:16].hex()}"


def _python_call_site_id(
    identity_context: Mapping[str, Any],
    *,
    owner_id: str,
    owner_relative_role: str,
    owner_relative_ordinal: int,
    start_byte: int,
    end_byte: int,
) -> str:
    semantic_key = rfc8785.dumps(
        {
            "module_id": identity_context["module_id"],
            "owner_relative_role": owner_relative_role,
            "owner_relative_ordinal": owner_relative_ordinal,
            "file_id": identity_context["file_id"],
            "content_digest": identity_context["content_digest"],
            "source_range": {"start_byte": start_byte, "end_byte": end_byte},
            "kind": "call_site",
            "schema_version": 1,
        }
    )
    preimage = _cbef_entity_preimage(
        workspace_id=identity_context["workspace_id"],
        analysis_context_id=identity_context["analysis_context_id"],
        kind_code=PYTHON_CALL_SITE_KIND_CODE,
        owner_id=owner_id,
        owner_prefix="entity:function",
        semantic_key=semantic_key,
    )
    return f"entity:call-site:{blake3.blake3(preimage).digest()[:16].hex()}"


def _calls_fact_id(
    identity_context: Mapping[str, Any],
    *,
    caller_id: str,
    callee_id: str,
    call_site_id: str,
) -> str:
    role = rfc8785.dumps(
        {
            "module_id": identity_context["module_id"],
            "call_site_id": call_site_id,
            "schema_version": 1,
        }
    ).decode("utf-8")
    preimage = _cbef_relation_fact_preimage(
        workspace_id=identity_context["workspace_id"],
        analysis_context_id=identity_context["analysis_context_id"],
        relation_kind_code=CALLS_RELATION_KIND_CODE,
        subject_id=caller_id,
        object_id=callee_id,
        role=role,
    )
    return f"fact:calls:{blake3.blake3(preimage).digest()[:16].hex()}"


def _node_byte_range(source: str, node: ast.AST, context: str) -> tuple[int, int]:
    if not all(
        hasattr(node, name)
        for name in ("lineno", "col_offset", "end_lineno", "end_col_offset")
    ):
        raise SuccessorEvidenceError(f"{context} node lacks an exact source range")
    lines = source.splitlines(keepends=True)
    start_line = int(node.lineno) - 1
    end_line = int(node.end_lineno) - 1
    start = sum(len(line.encode("utf-8")) for line in lines[:start_line]) + int(
        node.col_offset
    )
    end = sum(len(line.encode("utf-8")) for line in lines[:end_line]) + int(
        node.end_col_offset
    )
    return start, end


def _validate_public_identity_recipe(
    value: object,
    *,
    domain: str,
    prefix: str,
    preimage: Mapping[str, Any],
    excluded: Sequence[str],
    context: str,
) -> str:
    recipe = _mapping(value, f"{context} identity recipe")
    envelope = {
        "domain": domain,
        "recipe_version": "codefabric.canonical-public-id.v1",
        "preimage": dict(preimage),
    }
    canonical = rfc8785.dumps(envelope)
    expected_id = f"{prefix}:{blake3.blake3(canonical).digest()[:16].hex()}"
    expected = {
        "recipe_version": "codefabric.canonical-public-id.v1",
        "strict_ingress": "reject duplicate members and non-I-JSON values before mapping",
        "canonicalization": "RFC8785 JCS UTF-8 bytes",
        "framing": "one canonical object with domain, recipe_version, and preimage members; no byte concatenation",
        "digest": {
            "algorithm": "BLAKE3",
            "mode": "unkeyed",
            "full_output_bytes": 32,
            "truncation": "first 16 digest bytes",
            "text_encoding": "lowercase hexadecimal",
        },
        "envelope": envelope,
        "canonical_jcs_utf8": canonical.decode("utf-8"),
        "excluded": list(excluded),
        "output_id": expected_id,
    }
    if recipe != expected:
        raise SuccessorEvidenceError(f"{context} public identity recipe differs")
    return expected_id


def _cbef_typed_value(type_code: int, payload: bytes) -> bytes:
    return type_code.to_bytes(1, "big") + len(payload).to_bytes(4, "big") + payload


def _public_domain_id_bytes(value: object, domain: str) -> bytes:
    text = str(value)
    prefix, separator, _ = text.rpartition(":")
    if not separator or not prefix.startswith(f"{domain}:"):
        raise SuccessorEvidenceError(f"identity is outside the {domain} domain")
    return _public_id_bytes(text, prefix)


def _cbef_sequence_value(
    members: Sequence[tuple[int, bytes]], *, canonical_set: bool, context: str
) -> bytes:
    encoded = [_cbef_typed_value(type_code, payload) for type_code, payload in members]
    if canonical_set:
        encoded.sort()
        if len(set(encoded)) != len(encoded):
            raise SuccessorEvidenceError(f"{context} CBEF SET contains duplicates")
    return len(encoded).to_bytes(4, "big") + b"".join(
        len(member).to_bytes(4, "big") + member for member in encoded
    )


def _cbef_map_value(
    entries: Sequence[tuple[tuple[int, bytes], tuple[int, bytes]]], *, context: str
) -> bytes:
    encoded = [
        (_cbef_typed_value(*key), _cbef_typed_value(*value)) for key, value in entries
    ]
    encoded.sort(key=lambda entry: entry[0])
    if len({key for key, _ in encoded}) != len(encoded):
        raise SuccessorEvidenceError(f"{context} CBEF MAP contains duplicate keys")
    return len(encoded).to_bytes(4, "big") + b"".join(
        len(key).to_bytes(4, "big") + key + len(value).to_bytes(4, "big") + value
        for key, value in encoded
    )


def _cbef_utf8_value(
    value: object, *, ascii_lower: bool = False, context: str
) -> bytes:
    text = _nonempty_string(value, context)
    if ascii_lower and (not text.isascii() or text != text.lower()):
        raise SuccessorEvidenceError(f"{context} must already be canonical ASCII_LOWER")
    return text.encode("utf-8")


def _cbef_scalar_union_value(
    value: object, *, context: str
) -> tuple[bytes, dict[str, Any]]:
    if isinstance(value, bool):
        variant = 6
        member_type = "BOOLEAN"
        payload = bytes([int(value)])
    elif isinstance(value, int):
        if value >= 0:
            variant = 4
            member_type = "UNSIGNED"
            payload = value.to_bytes(8, "big")
        else:
            variant = 5
            member_type = "SIGNED"
            payload = value.to_bytes(8, "big", signed=True)
    elif isinstance(value, str) and value:
        variant = 2
        member_type = "UTF8"
        payload = value.encode("utf-8")
    else:
        raise SuccessorEvidenceError(
            f"{context} is not a supported CBEF scalar group-key value"
        )
    typed = _cbef_typed_value(variant, payload)
    tagged = variant.to_bytes(2, "big") + len(typed).to_bytes(4, "big") + typed
    return tagged, {"variant": variant, "member_type": member_type, "value": value}


def _expected_cbef_recipe(
    *,
    domain_code: int,
    domain_name: str,
    output_prefix: str,
    fields: Sequence[tuple[int, str, int, str, bytes, Any]],
    excluded: Sequence[str],
) -> dict[str, Any]:
    preimage = _cbef_record(
        domain_code,
        [(tag, type_code, payload) for tag, _, type_code, _, payload, _ in fields],
    )
    digest = blake3.blake3(preimage).digest()
    return {
        "recipe_version": "CBEF-v1",
        "contract": {
            "artifact_id": "codefabric.identity.cbef-v1",
            "version": "1.1",
        },
        "record_domain": {"code": domain_code, "name": domain_name},
        "fields": [
            {
                "tag": tag,
                "name": name,
                "type_code": {"code": type_code, "name": type_name},
                "value": copy.deepcopy(value),
                "payload_hex": payload.hex(),
            }
            for tag, name, type_code, type_name, payload, value in fields
        ],
        "canonical_preimage_hex": preimage.hex(),
        "digest": {
            "algorithm": "BLAKE3-256",
            "mode": "unkeyed",
            "full_digest_hex": digest.hex(),
            "id_derivation": "first 16 digest bytes",
            "text_encoding": "lowercase hexadecimal",
        },
        "excluded": list(excluded),
        "output_id": f"{output_prefix}:{digest[:16].hex()}",
    }


def _validate_cbef_recipe(
    value: object,
    *,
    domain_code: int,
    domain_name: str,
    output_prefix: str,
    fields: Sequence[tuple[int, str, int, str, bytes, Any]],
    excluded: Sequence[str],
    context: str,
) -> str:
    recipe = _mapping(value, f"{context} CBEF identity recipe")
    expected = _expected_cbef_recipe(
        domain_code=domain_code,
        domain_name=domain_name,
        output_prefix=output_prefix,
        fields=fields,
        excluded=excluded,
    )
    if recipe != expected:
        raise SuccessorEvidenceError(f"{context} CBEF identity recipe differs")
    return str(expected["output_id"])


def _validate_path_result_recipe(
    value: object,
    *,
    workspace_id: object,
    analysis_context_id: object,
    fabric_epoch_id: object,
    policy_identity: object,
    ordered_entity_ids: Sequence[str],
    ordered_fact_ids: Sequence[str],
    context: str,
) -> str:
    """Independently derive the closed CBEF-v1.1 PATH_RESULT recipe."""

    entity_ids = list(ordered_entity_ids)
    fact_ids = list(ordered_fact_ids)
    entity_payload = _cbef_sequence_value(
        [(7, _public_domain_id_bytes(item, "entity")) for item in entity_ids],
        canonical_set=False,
        context=f"{context} ordered entity IDs",
    )
    fact_payload = _cbef_sequence_value(
        [(7, _public_domain_id_bytes(item, "fact")) for item in fact_ids],
        canonical_set=False,
        context=f"{context} ordered fact IDs",
    )
    return _validate_cbef_recipe(
        value,
        domain_code=CBEF_PATH_RESULT_DOMAIN_CODE,
        domain_name="PATH_RESULT",
        output_prefix="path",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                _public_id_bytes(workspace_id, "workspace"),
                workspace_id,
            ),
            (
                2,
                "analysis_context_id",
                7,
                "ID",
                _analysis_context_id_bytes(analysis_context_id),
                analysis_context_id,
            ),
            (
                3,
                "fabric_epoch_id",
                7,
                "ID",
                _public_id_bytes(fabric_epoch_id, "fabric-epoch"),
                fabric_epoch_id,
            ),
            (
                4,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8_value(policy_identity, context=f"{context} policy identity"),
                policy_identity,
            ),
            (
                5,
                "ordered_entity_ids",
                9,
                "ORDERED_LIST",
                entity_payload,
                entity_ids,
            ),
            (
                6,
                "ordered_fact_ids",
                9,
                "ORDERED_LIST",
                fact_payload,
                fact_ids,
            ),
        ],
        excluded=["path length", "witness provenance", "certainty summary"],
        context=context,
    )


def _validate_query_source_context_recipe(
    value: object,
    *,
    workspace_id: object,
    analysis_context_id: object,
    snapshot_id: object,
    entity_id: object,
    source_file_id: object,
    source_generation: int,
    source_content_digest: object,
    delivered_start_byte: int,
    delivered_end_byte: int,
    delivered_content_digest: object,
    disclosure_scope_id: object,
    policy_identity: object,
    context_kind: object,
    context: str,
) -> str:
    """Independently derive the closed CBEF-v1.1 source-context identity."""

    for name, number in (
        ("source generation", source_generation),
        ("delivered start byte", delivered_start_byte),
        ("delivered end byte", delivered_end_byte),
    ):
        if (
            not isinstance(number, int)
            or isinstance(number, bool)
            or not 0 <= number <= 0xFFFF_FFFF_FFFF_FFFF
        ):
            raise SuccessorEvidenceError(f"{context} {name} is not unsigned")
    if delivered_start_byte > delivered_end_byte:
        raise SuccessorEvidenceError(f"{context} delivered source range is inverted")
    canonical_context_kind = _nonempty_string(
        context_kind, f"{context} source context kind"
    )
    if (
        not canonical_context_kind.isascii()
        or canonical_context_kind != canonical_context_kind.lower()
    ):
        raise SuccessorEvidenceError(
            f"{context} source context kind is not canonical ASCII_LOWER"
        )
    return _validate_cbef_recipe(
        value,
        domain_code=CBEF_QUERY_SOURCE_CONTEXT_DOMAIN_CODE,
        domain_name="QUERY_SOURCE_CONTEXT",
        output_prefix="context",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                _public_id_bytes(workspace_id, "workspace"),
                workspace_id,
            ),
            (
                2,
                "analysis_context_id",
                7,
                "ID",
                _analysis_context_id_bytes(analysis_context_id),
                analysis_context_id,
            ),
            (
                3,
                "snapshot_id",
                7,
                "ID",
                _public_id_bytes(snapshot_id, "snapshot"),
                snapshot_id,
            ),
            (
                4,
                "entity_id",
                7,
                "ID",
                _public_domain_id_bytes(entity_id, "entity"),
                entity_id,
            ),
            (
                5,
                "source_file_id",
                7,
                "ID",
                _public_id_bytes(source_file_id, "file"),
                source_file_id,
            ),
            (
                6,
                "source_generation",
                4,
                "UNSIGNED",
                source_generation.to_bytes(8, "big"),
                source_generation,
            ),
            (
                7,
                "source_content_digest",
                8,
                "DIGEST",
                _digest_bytes(source_content_digest, "b3"),
                source_content_digest,
            ),
            (
                8,
                "delivered_start_byte",
                4,
                "UNSIGNED",
                delivered_start_byte.to_bytes(8, "big"),
                delivered_start_byte,
            ),
            (
                9,
                "delivered_end_byte",
                4,
                "UNSIGNED",
                delivered_end_byte.to_bytes(8, "big"),
                delivered_end_byte,
            ),
            (
                10,
                "delivered_content_digest",
                8,
                "DIGEST",
                _digest_bytes(delivered_content_digest, "b3"),
                delivered_content_digest,
            ),
            (
                11,
                "disclosure_scope_id",
                7,
                "ID",
                _public_id_bytes(disclosure_scope_id, "access-scope"),
                disclosure_scope_id,
            ),
            (
                12,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8_value(policy_identity, context=f"{context} policy identity"),
                policy_identity,
            ),
            (
                13,
                "context_kind",
                2,
                "UTF8",
                _cbef_utf8_value(
                    canonical_context_kind,
                    ascii_lower=True,
                    context=f"{context} source context kind",
                ),
                canonical_context_kind,
            ),
        ],
        excluded=["omitted byte count", "truncation state"],
        context=context,
    )


def _retrieve_fact_coverage_state(
    coverage_rows: Sequence[Mapping[str, Any]], context: str
) -> str:
    states = {
        _nonempty_string(row.get("state"), f"{context} coverage state")
        for row in coverage_rows
    }
    if not states or not states <= {"COMPLETE", "PARTIAL", "UNAVAILABLE"}:
        raise SuccessorEvidenceError(
            f"{context} retrieve-facts coverage is outside its closed vocabulary"
        )
    if states == {"COMPLETE"}:
        return "complete"
    if "UNAVAILABLE" in states:
        return "indeterminate"
    return "partial"


def _validate_retrieve_fact_input_set(
    relations: Mapping[str, Any], policy_identity: object, context: str
) -> str:
    rows = [
        _mapping(value, f"{context} retrieve-facts input row")
        for value in _list(relations.get("fact_rows"), f"{context} fact rows")
    ]
    coverage_rows = [
        _mapping(value, f"{context} retrieve-facts coverage row")
        for value in _list(relations.get("coverage_rows"), f"{context} coverage rows")
    ]
    if not rows or not coverage_rows:
        raise SuccessorEvidenceError(f"{context} retrieve-facts input set is empty")
    workspace_ids = {str(row.get("workspace_id")) for row in rows}
    context_ids = sorted(
        {_nonempty_string(row.get("analysis_context_id"), context) for row in rows}
    )
    if len(workspace_ids) != 1 or not context_ids:
        raise SuccessorEvidenceError(
            f"{context} retrieve-facts input set crosses a CBEF identity boundary"
        )
    fact_ids = sorted(
        _nonempty_string(row.get("fact_id"), f"{context} input fact ID") for row in rows
    )
    producer_ids = sorted(
        {
            *(
                _nonempty_string(
                    _mapping(row.get("producer"), f"{context} fact producer").get(
                        "producer_id"
                    ),
                    f"{context} fact producer identity",
                )
                for row in rows
            ),
            *(
                f"coverage:{_nonempty_string(row.get('family'), context)}"
                for row in coverage_rows
                if row.get("state") != "COMPLETE"
            ),
        }
    )
    coverage_state = _retrieve_fact_coverage_state(coverage_rows, context)
    context_set_payload = _cbef_sequence_value(
        [(7, _analysis_context_id_bytes(value)) for value in context_ids],
        canonical_set=True,
        context=f"{context} retrieve-facts analysis contexts",
    )
    fact_set_payload = _cbef_sequence_value(
        [(7, _public_domain_id_bytes(value, "fact")) for value in fact_ids],
        canonical_set=True,
        context=f"{context} retrieve-facts fact IDs",
    )
    producer_set_payload = _cbef_sequence_value(
        [(2, value.encode("utf-8")) for value in producer_ids],
        canonical_set=True,
        context=f"{context} retrieve-facts producers",
    )
    input_set_id = _validate_cbef_recipe(
        relations.get("input_set_identity"),
        domain_code=19,
        domain_name="OBJECTIVE_INPUT_SET",
        output_prefix="input-set",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                _public_id_bytes(rows[0]["workspace_id"], "workspace"),
                rows[0]["workspace_id"],
            ),
            (
                2,
                "analysis_context_ids",
                10,
                "SET",
                context_set_payload,
                context_ids,
            ),
            (3, "fact_ids", 10, "SET", fact_set_payload, fact_ids),
            (
                4,
                "producer_identities",
                10,
                "SET",
                producer_set_payload,
                producer_ids,
            ),
            (
                5,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8_value(
                    policy_identity, context=f"{context} retrieve-facts policy"
                ),
                policy_identity,
            ),
            (
                6,
                "coverage_state",
                2,
                "UTF8",
                coverage_state.encode("utf-8"),
                coverage_state,
            ),
        ],
        excluded=[
            "fact ordering",
            "support ids",
            "mutable coverage counters",
            "diagnostic evidence",
        ],
        context=f"{context} retrieve-facts input set",
    )
    if any(
        _mapping(row.get("direct_provenance"), f"{context} fact provenance").get(
            "input_set_id"
        )
        != input_set_id
        for row in rows
    ):
        raise SuccessorEvidenceError(
            f"{context} retrieve-facts provenance does not bind its CBEF input set"
        )
    return input_set_id


def _validate_retrieve_source_identity(
    coverage: Mapping[str, Any], workspace_id: object, context: str
) -> str:
    source = _mapping(
        coverage.get("source_identity"), f"{context} unknown source identity"
    )
    _strict_keys(
        source,
        {
            "canonical_path_bytes_hex",
            "content_digest",
            "content_utf8",
            "file_id",
        },
        f"{context} unknown source identity",
    )
    encoded_path = _nonempty_string(
        source["canonical_path_bytes_hex"], f"{context} canonical source path"
    )
    try:
        comparison_key = bytes.fromhex(encoded_path)
    except ValueError as error:
        raise SuccessorEvidenceError(
            f"{context} canonical source path is not hexadecimal bytes"
        ) from error
    if not comparison_key or comparison_key.hex() != encoded_path:
        raise SuccessorEvidenceError(
            f"{context} canonical source path is not lowercase byte-exact hex"
        )
    file_id = _nonempty_string(source["file_id"], f"{context} source file identity")
    expected_file_id = "file:" + _cbef_source_file_id(
        workspace_id=workspace_id, comparison_key=comparison_key
    )
    source_text = _nonempty_string(source["content_utf8"], f"{context} source bytes")
    if file_id != expected_file_id or source["content_digest"] != _bytes_b3(
        source_text.encode("utf-8")
    ):
        raise SuccessorEvidenceError(
            f"{context} unknown source identity is not its closed CBEF/content recipe"
        )
    return file_id


def _property_kind_allocation(
    relations: Mapping[str, Any], context: str
) -> dict[str, int]:
    registry = _mapping(
        relations.get("property_kind_registry"),
        f"{context} input.property_kind registry",
    )
    _strict_keys(
        registry,
        {"relation_id", "closed_universe", "rows"},
        f"{context} input.property_kind registry",
    )
    if (
        registry["relation_id"] != "input.property_kind"
        or registry["closed_universe"] is not True
    ):
        raise SuccessorEvidenceError(
            f"{context} input.property_kind registry is not closed"
        )
    allocation: dict[str, int] = {}
    used_codes: set[int] = set()
    for value in _list(registry["rows"], f"{context} property-kind rows"):
        row = _mapping(value, f"{context} property-kind row")
        _strict_keys(
            row,
            {"property_kind", "property_kind_code"},
            f"{context} property-kind row",
        )
        name = _nonempty_string(row["property_kind"], f"{context} property kind")
        code = row["property_kind_code"]
        if (
            not isinstance(code, int)
            or isinstance(code, bool)
            or not 0 < code <= 0xFFFF
            or name in allocation
            or code in used_codes
        ):
            raise SuccessorEvidenceError(
                f"{context} property-kind allocation is zero, duplicate, or invalid"
            )
        allocation[name] = code
        used_codes.add(code)
    if not allocation:
        raise SuccessorEvidenceError(f"{context} property-kind registry is empty")
    return allocation


def _authorization_scope_recipe(
    access_scope: Mapping[str, Any], authorization_policy: Mapping[str, Any]
) -> dict[str, Any]:
    def canonical_strings(name: str) -> list[str]:
        values = [
            _nonempty_string(value, f"access-scope {name} value")
            for value in _list(access_scope[name], f"access-scope {name}")
        ]
        if len(values) != len(set(values)):
            raise SuccessorEvidenceError(f"access-scope {name} contains duplicates")
        return sorted(values)

    columns = _mapping(access_scope["allowed_columns"], "access-scope columns")
    allowed_relations = canonical_strings("allowed_relations")
    allowed_columns = {
        relation: sorted(_string_list(values, f"scope columns for {relation}"))
        for relation, values in sorted(columns.items())
    }
    if set(allowed_columns) != set(allowed_relations):
        raise SuccessorEvidenceError(
            "access-scope relation and column grants do not close"
        )
    relation_payload = _cbef_sequence_value(
        [(2, relation.encode("utf-8")) for relation in allowed_relations],
        canonical_set=True,
        context="access-scope relations",
    )
    columns_payload = _cbef_map_value(
        [
            (
                (2, relation.encode("utf-8")),
                (
                    10,
                    _cbef_sequence_value(
                        [(2, column.encode("utf-8")) for column in values],
                        canonical_set=True,
                        context=f"access-scope columns for {relation}",
                    ),
                ),
            )
            for relation, values in allowed_columns.items()
        ],
        context="access-scope column map",
    )
    grant_sets = {
        name: canonical_strings(name)
        for name in (
            "allowed_functions",
            "allowed_extensions",
            "allowed_variables",
            "allowed_object_stores",
            "allowed_metadata",
            "row_policies",
            "execution_posture",
        )
    }
    grant_payloads = {
        name: _cbef_sequence_value(
            [(2, value.encode("utf-8")) for value in values],
            canonical_set=True,
            context=f"access-scope {name}",
        )
        for name, values in grant_sets.items()
    }
    source_file_ids = canonical_strings("source_file_ids")
    source_files_payload = _cbef_sequence_value(
        [(7, _public_id_bytes(value, "file")) for value in source_file_ids],
        canonical_set=True,
        context="access-scope source files",
    )
    canonical_ranges: list[list[Any]] = []
    range_members: list[tuple[int, bytes]] = []
    for raw in _list(access_scope["authorized_ranges"], "authorized ranges"):
        if (
            not isinstance(raw, list)
            or len(raw) != 3
            or not isinstance(raw[1], int)
            or isinstance(raw[1], bool)
            or not isinstance(raw[2], int)
            or isinstance(raw[2], bool)
            or not 0 <= raw[1] < raw[2] <= 0xFFFF_FFFF_FFFF_FFFF
        ):
            raise SuccessorEvidenceError("access-scope authorized range is invalid")
        file_id = _nonempty_string(raw[0], "authorized range source file")
        canonical_ranges.append([file_id, raw[1], raw[2]])
        range_members.append(
            (
                9,
                _cbef_sequence_value(
                    [
                        (7, _public_id_bytes(file_id, "file")),
                        (4, raw[1].to_bytes(8, "big")),
                        (4, raw[2].to_bytes(8, "big")),
                    ],
                    canonical_set=False,
                    context="authorized range tuple",
                ),
            )
        )
    ranges_payload = _cbef_sequence_value(
        range_members, canonical_set=True, context="authorized range set"
    )
    source_access = access_scope["source_access"]
    if not isinstance(source_access, bool):
        raise SuccessorEvidenceError("access-scope source_access must be Boolean")
    return _expected_cbef_recipe(
        domain_code=22,
        domain_name="ACCESS_SCOPE",
        output_prefix="access-scope",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                _public_id_bytes(access_scope["workspace"], "workspace"),
                access_scope["workspace"],
            ),
            (
                2,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8_value(
                    authorization_policy["policy_id"], context="scope policy identity"
                ),
                authorization_policy["policy_id"],
            ),
            (
                3,
                "principal_id",
                7,
                "ID",
                _public_id_bytes(access_scope["principal_id"], "principal"),
                access_scope["principal_id"],
            ),
            (
                4,
                "agent_id",
                7,
                "ID",
                _public_id_bytes(access_scope["agent_id"], "agent"),
                access_scope["agent_id"],
            ),
            (
                5,
                "credential_digest",
                8,
                "DIGEST",
                _digest_bytes(access_scope["credential_digest"], "b3"),
                access_scope["credential_digest"],
            ),
            (
                6,
                "role",
                2,
                "UTF8",
                _cbef_utf8_value(
                    access_scope["role"],
                    ascii_lower=True,
                    context="access-scope role",
                ),
                access_scope["role"],
            ),
            (
                7,
                "operation",
                2,
                "UTF8",
                _cbef_utf8_value(
                    access_scope["operation"],
                    ascii_lower=True,
                    context="access-scope operation",
                ),
                access_scope["operation"],
            ),
            (8, "allowed_relations", 10, "SET", relation_payload, allowed_relations),
            (9, "allowed_columns", 11, "MAP", columns_payload, allowed_columns),
            (
                10,
                "allowed_functions",
                10,
                "SET",
                grant_payloads["allowed_functions"],
                grant_sets["allowed_functions"],
            ),
            (
                11,
                "allowed_extensions",
                10,
                "SET",
                grant_payloads["allowed_extensions"],
                grant_sets["allowed_extensions"],
            ),
            (
                12,
                "allowed_variables",
                10,
                "SET",
                grant_payloads["allowed_variables"],
                grant_sets["allowed_variables"],
            ),
            (
                13,
                "allowed_object_stores",
                10,
                "SET",
                grant_payloads["allowed_object_stores"],
                grant_sets["allowed_object_stores"],
            ),
            (
                14,
                "allowed_metadata",
                10,
                "SET",
                grant_payloads["allowed_metadata"],
                grant_sets["allowed_metadata"],
            ),
            (
                15,
                "row_policies",
                10,
                "SET",
                grant_payloads["row_policies"],
                grant_sets["row_policies"],
            ),
            (
                16,
                "execution_posture",
                10,
                "SET",
                grant_payloads["execution_posture"],
                grant_sets["execution_posture"],
            ),
            (
                17,
                "source_access",
                6,
                "BOOLEAN",
                bytes([int(source_access)]),
                source_access,
            ),
            (
                18,
                "source_file_ids",
                10,
                "SET",
                source_files_payload,
                source_file_ids,
            ),
            (
                19,
                "authorized_ranges",
                10,
                "SET",
                ranges_payload,
                canonical_ranges,
            ),
        ],
        excluded=[
            "derived child catalog",
            "bound physical plan",
            "resource limits",
            "mutable request counters",
        ],
    )


def _authorization_scope_id(
    access_scope: Mapping[str, Any], authorization_policy: Mapping[str, Any]
) -> str:
    return str(
        _authorization_scope_recipe(access_scope, authorization_policy)["output_id"]
    )


_NATIVE_KIND_ENTITY_CONTRACT = {
    "call": "call_site",
    "function_definition": "function_syntax",
    "identifier": "identifier_syntax",
}


def _validate_objective_fact_inputs(
    relations: Mapping[str, Any], policy_identity: str, context: str
) -> tuple[str, list[Mapping[str, Any]], dict[str, list[Mapping[str, Any]]]]:
    entities = _mapping(relations["entity_dictionary"], f"{context} entity dictionary")
    property_kinds = _property_kind_allocation(relations, context)
    coverage_state = _nonempty_string(
        relations.get("coverage_state"), f"{context} objective coverage state"
    )
    if coverage_state not in {
        "complete",
        "partial",
        "indeterminate",
        "unavailable",
    }:
        raise SuccessorEvidenceError(
            f"{context} objective coverage state is outside its closed vocabulary"
        )
    rows = [
        _mapping(value, f"{context} objective fact")
        for value in _list(relations["syntax_rows"], f"{context} objective facts")
    ]
    if not rows:
        raise SuccessorEvidenceError(f"{context} objective fact input is empty")
    grouped: dict[str, list[Mapping[str, Any]]] = {}
    fact_ids: list[str] = []
    for row in rows:
        statement = _mapping(row.get("statement"), f"{context} native-kind statement")
        subject_id = _nonempty_string(
            statement.get("subject"), f"{context} native-kind subject"
        )
        entity = _mapping(
            entities.get(subject_id), f"{context} native-kind subject entity"
        )
        native_kind = _nonempty_string(
            statement.get("object"), f"{context} native-kind value"
        )
        if (
            row.get("fact_form") != "property"
            or row.get("fact_kind") != "native_kind"
            or row.get("owner_id") != subject_id
            or statement.get("predicate") != "native_kind"
            or _NATIVE_KIND_ENTITY_CONTRACT.get(native_kind)
            != entity.get("semantic_kind")
            or entity.get("representation") != "syntax_occurrence"
        ):
            raise SuccessorEvidenceError(
                f"{context} native-kind property does not match its syntax occurrence"
            )
        _mapping(row.get("direct_provenance"), f"{context} objective fact provenance")
        _mapping(row.get("producer"), f"{context} objective fact producer")
        property_kind = _nonempty_string(
            statement.get("predicate"), f"{context} property kind"
        )
        property_kind_code = property_kinds.get(property_kind)
        if (
            property_kind_code is None
            or row.get("property_kind_code") != property_kind_code
        ):
            raise SuccessorEvidenceError(
                f"{context} property fact does not consume its declared kind allocation"
            )
        native_payload = native_kind.encode("utf-8")
        typed_value = _cbef_typed_value(2, native_payload)
        tagged_value = (
            (50).to_bytes(2, "big") + len(typed_value).to_bytes(4, "big") + typed_value
        )
        fact_id = _validate_cbef_recipe(
            row.get("identity_recipe"),
            domain_code=10,
            domain_name="PROPERTY_FACT",
            output_prefix="fact:native-kind",
            fields=[
                (
                    1,
                    "workspace_id",
                    7,
                    "ID",
                    _public_id_bytes(row["workspace_id"], "workspace"),
                    row["workspace_id"],
                ),
                (
                    2,
                    "analysis_context_id",
                    7,
                    "ID",
                    _analysis_context_id_bytes(row["analysis_context_id"]),
                    row["analysis_context_id"],
                ),
                (
                    3,
                    "property_kind_code",
                    4,
                    "UNSIGNED",
                    property_kind_code.to_bytes(2, "big"),
                    property_kind_code,
                ),
                (
                    4,
                    "subject_entity_id",
                    7,
                    "ID",
                    _public_domain_id_bytes(subject_id, "entity"),
                    subject_id,
                ),
                (
                    5,
                    "canonical_value",
                    12,
                    "TAGGED_UNION",
                    tagged_value,
                    {"variant": 50, "member_type": "UTF8", "value": native_kind},
                ),
            ],
            excluded=[
                "input-set identity",
                "support ids",
                "source and producer provenance",
                "policy identity",
                "mutable coverage counters",
                "diagnostic evidence",
            ],
            context=f"{context} native-kind fact",
        )
        if row.get("fact_id") != fact_id or fact_id in fact_ids:
            raise SuccessorEvidenceError(
                f"{context} native-kind fact identity differs or collides"
            )
        fact_ids.append(fact_id)
        grouped.setdefault(native_kind, []).append(row)
    workspace_ids = {str(row["workspace_id"]) for row in rows}
    if len(workspace_ids) != 1:
        raise SuccessorEvidenceError(
            f"{context} objective input set crosses workspaces"
        )
    context_ids = sorted({str(row["analysis_context_id"]) for row in rows})
    producer_ids = sorted({str(row["producer"]["producer_id"]) for row in rows})
    sorted_fact_ids = sorted(fact_ids)
    context_set_payload = _cbef_sequence_value(
        [(7, _analysis_context_id_bytes(value)) for value in context_ids],
        canonical_set=True,
        context=f"{context} objective analysis contexts",
    )
    fact_set_payload = _cbef_sequence_value(
        [
            (7, _public_id_bytes(fact_id, "fact:native-kind"))
            for fact_id in sorted_fact_ids
        ],
        canonical_set=True,
        context=f"{context} objective fact IDs",
    )
    producer_set_payload = _cbef_sequence_value(
        [(2, producer.encode("utf-8")) for producer in producer_ids],
        canonical_set=True,
        context=f"{context} objective producers",
    )
    input_set_id = _validate_cbef_recipe(
        relations.get("input_set_identity"),
        domain_code=19,
        domain_name="OBJECTIVE_INPUT_SET",
        output_prefix="input-set",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                _public_id_bytes(rows[0]["workspace_id"], "workspace"),
                rows[0]["workspace_id"],
            ),
            (
                2,
                "analysis_context_ids",
                10,
                "SET",
                context_set_payload,
                context_ids,
            ),
            (3, "fact_ids", 10, "SET", fact_set_payload, sorted_fact_ids),
            (
                4,
                "producer_identities",
                10,
                "SET",
                producer_set_payload,
                producer_ids,
            ),
            (
                5,
                "policy_identity",
                2,
                "UTF8",
                _cbef_utf8_value(
                    policy_identity, context=f"{context} objective policy"
                ),
                policy_identity,
            ),
            (
                6,
                "coverage_state",
                2,
                "UTF8",
                coverage_state.encode("utf-8"),
                coverage_state,
            ),
        ],
        excluded=[
            "fact ordering",
            "support ids",
            "mutable coverage counters",
            "diagnostic evidence",
        ],
        context=f"{context} objective input set",
    )
    if any(
        row.get("direct_provenance", {}).get("input_set_id") != input_set_id
        for row in rows
    ):
        raise SuccessorEvidenceError(
            f"{context} objective facts do not bind the rederived input set"
        )
    return input_set_id, rows, grouped


def _validate_objective_groups(
    groups: Sequence[Mapping[str, Any]],
    *,
    input_set_id: str,
    grouped_facts: Mapping[str, Sequence[Mapping[str, Any]]],
    context: str,
) -> list[str]:
    by_kind: dict[str, Mapping[str, Any]] = {}
    group_ids: list[str] = []
    for group in groups:
        key = _mapping(group.get("group_key"), f"{context} group key")
        native_kind = _nonempty_string(
            key.get("native_kind"), f"{context} group native kind"
        )
        members = grouped_facts.get(native_kind)
        objective = _mapping(group.get("objective_value"), f"{context} objective value")
        if (
            native_kind in by_kind
            or members is None
            or group.get("input_set_id") != input_set_id
            or group.get("grouping") != ["native_kind"]
            or group.get("aggregation") != "count"
            or objective != {"measure": "count", "value": len(members)}
            or group.get("producer_id") != members[0]["producer"]["producer_id"]
            or group.get("support_fact_ids")
            != sorted(str(member["fact_id"]) for member in members)
            or any(
                member["workspace_id"] != members[0]["workspace_id"]
                or member["analysis_context_id"] != members[0]["analysis_context_id"]
                or member["producer"]["producer_id"]
                != members[0]["producer"]["producer_id"]
                for member in members
            )
        ):
            raise SuccessorEvidenceError(
                f"{context} objective group is not derived from its exact input set"
            )
        if group["aggregation"] not in {
            "count",
            "count_distinct",
            "sum",
            "average",
            "minimum",
            "maximum",
        }:
            raise SuccessorEvidenceError(
                f"{context} aggregate is outside the closed objective vocabulary"
            )
        grouping_payload = _cbef_sequence_value(
            [(2, str(value).encode("utf-8")) for value in group["grouping"]],
            canonical_set=False,
            context=f"{context} grouping dimensions",
        )
        tagged_key = {
            str(name): _cbef_scalar_union_value(
                value, context=f"{context} group key {name}"
            )
            for name, value in key.items()
        }
        key_payload = _cbef_map_value(
            [
                ((2, name.encode("utf-8")), (12, payload))
                for name, (payload, _) in tagged_key.items()
            ],
            context=f"{context} canonical group key",
        )
        group_key_evidence = {
            name: evidence for name, (_, evidence) in tagged_key.items()
        }
        aggregate = _cbef_utf8_value(
            group["aggregation"],
            ascii_lower=True,
            context=f"{context} aggregate function",
        )
        measure = _cbef_utf8_value(
            objective["measure"], context=f"{context} aggregate measure"
        )
        producer_identity = _cbef_utf8_value(
            group["producer_id"], context=f"{context} group producer"
        )
        group_id = _validate_cbef_recipe(
            group.get("identity_recipe"),
            domain_code=20,
            domain_name="OBJECTIVE_GROUP",
            output_prefix="group",
            fields=[
                (
                    1,
                    "workspace_id",
                    7,
                    "ID",
                    _public_id_bytes(members[0]["workspace_id"], "workspace"),
                    members[0]["workspace_id"],
                ),
                (
                    2,
                    "analysis_context_id",
                    7,
                    "ID",
                    _analysis_context_id_bytes(members[0]["analysis_context_id"]),
                    members[0]["analysis_context_id"],
                ),
                (
                    3,
                    "input_set_id",
                    7,
                    "ID",
                    _public_id_bytes(input_set_id, "input-set"),
                    input_set_id,
                ),
                (
                    4,
                    "grouping_dimensions",
                    9,
                    "ORDERED_LIST",
                    grouping_payload,
                    group["grouping"],
                ),
                (
                    5,
                    "canonical_group_key",
                    11,
                    "MAP",
                    key_payload,
                    group_key_evidence,
                ),
                (
                    6,
                    "aggregate_function",
                    2,
                    "UTF8",
                    aggregate,
                    group["aggregation"],
                ),
                (7, "measure", 2, "UTF8", measure, objective["measure"]),
                (
                    8,
                    "producer_identity",
                    2,
                    "UTF8",
                    producer_identity,
                    group["producer_id"],
                ),
            ],
            excluded=[
                "support_fact_ids",
                "group members",
                "member count",
                "objective count value",
                "mutable coverage counters",
            ],
            context=f"{context} objective group",
        )
        if group.get("group_id") != group_id:
            raise SuccessorEvidenceError(f"{context} objective group identity differs")
        by_kind[native_kind] = group
        group_ids.append(group_id)
    if set(by_kind) != set(grouped_facts):
        raise SuccessorEvidenceError(f"{context} objective group coverage differs")
    return group_ids


def _framed_b3_hex(domain: str, parts: Sequence[str]) -> str:
    """Derive an identity from exact typed byte parts without JSON mediation."""
    hasher = blake3.blake3()
    for part in (domain, *parts):
        encoded = part.encode("utf-8")
        hasher.update(len(encoded).to_bytes(8, "big"))
        hasher.update(encoded)
    return hasher.hexdigest()


def _read_text(path: Path, context: str) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise SuccessorEvidenceError(f"cannot read {context}: {path}") from error


def _validate_source_references(root: Path, value: object, context: str) -> None:
    text = _nonempty_string(value, context)
    references = list(
        re.finditer(
            r"\b(SUITE|ONT|GEN|FAB|QRY|LIFE|SRV)\s+"
            r"(?:§(?P<section>\d+(?:\.\d+)?[A-Z]?)|(?P<acceptance>AC-[A-Z]-\d+))",
            text,
        )
    )
    principles = list(re.finditer(r"\bP(?P<number>\d{1,2})\b", text))
    if not references and not principles:
        raise SuccessorEvidenceError(f"{context} has no resolvable normative citation")
    for reference in references:
        tag = reference.group(1)
        document = _read_text(root / NORMATIVE_TAG_PATHS[tag], f"{tag} authority")
        section = reference.group("section")
        acceptance = reference.group("acceptance")
        if section is not None:
            heading = re.compile(
                rf"^#{{1,6}}\s+{re.escape(section)}(?:\.|\s|—)", re.MULTILINE
            )
            if heading.search(document) is None:
                raise SuccessorEvidenceError(
                    f"{context} references unresolved {tag} §{section}"
                )
        elif (
            re.search(
                rf"^#{{1,6}}\s+{re.escape(str(acceptance))}(?:\s|—)",
                document,
                re.MULTILINE,
            )
            is None
        ):
            raise SuccessorEvidenceError(
                f"{context} references unresolved {tag} {acceptance}"
            )
    if principles:
        document = _read_text(root / PRINCIPLES_PATH, "data-fabric principles")
        for principle in principles:
            number = principle.group("number")
            if (
                re.search(rf"^#{{1,6}}\s+P{number}(?:\s|—)", document, re.MULTILINE)
                is None
            ):
                raise SuccessorEvidenceError(
                    f"{context} references unresolved principle P{number}"
                )


def _list(value: object, context: str) -> list[Any]:
    if not isinstance(value, list):
        raise SuccessorEvidenceError(f"{context} must be a list")
    return value


def _bool(value: object, context: str) -> bool:
    if not isinstance(value, bool):
        raise SuccessorEvidenceError(f"{context} must be a boolean")
    return value


def _positive_int(value: object, context: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise SuccessorEvidenceError(f"{context} must be a positive integer")
    return value


def _decoded_rows(decoded: Mapping[str, Any], context: str) -> list[list[Any]]:
    rows = _list(decoded["rows"], f"{context} rows")
    if not rows:
        raise SuccessorEvidenceError(f"{context} has zero rows")
    return rows  # type: ignore[return-value]


def _validate_provider_inputs(
    root: Path,
    inputs: Mapping[str, Any],
    decoded: Mapping[str, Any],
    context: str,
) -> None:
    images = _list(inputs["source_images"], f"{context} source_images")
    source_by_id: dict[str, Mapping[str, Any]] = {}
    for index, image_value in enumerate(images, 1):
        image = _mapping(image_value, f"{context} source image {index}")
        common_keys = {
            "source_id",
            "workspace_id",
            "file_id",
            "canonical_path_bytes_hex",
            "language",
            "encoding",
            "source_kind",
            "source_generation",
            "access_authorization_id",
            "content_digest",
        }
        payload_keys = (
            {"bytes_utf8"}
            if image.get("source_kind") == "source_file"
            else {"immutable_descriptor"}
        )
        _strict_keys(
            image, common_keys | payload_keys, f"{context} source image {index}"
        )
        source_id = _nonempty_string(image["source_id"], f"{context} source_id")
        if source_id in source_by_id:
            raise SuccessorEvidenceError(f"{context} has duplicate source_id")
        if (
            image["workspace_id"] != "workspace:00000000000000000000000000000000"
            or re.fullmatch(r"[0-9a-f]{32}", str(image["file_id"])) is None
            or image["encoding"] != "utf-8"
            or image["source_generation"] != 2
            or not _nonempty_string(
                image["access_authorization_id"], f"{context} source authorization"
            )
        ):
            raise SuccessorEvidenceError(f"{context} source identity contract differs")
        try:
            canonical_path = bytes.fromhex(
                _nonempty_string(
                    image["canonical_path_bytes_hex"], f"{context} canonical path"
                )
            )
        except ValueError as error:
            raise SuccessorEvidenceError(
                f"{context} canonical path is not hexadecimal bytes"
            ) from error
        if not canonical_path:
            raise SuccessorEvidenceError(f"{context} canonical path is empty")
        if image["file_id"] != _cbef_source_file_id(
            workspace_id=image["workspace_id"], comparison_key=canonical_path
        ):
            raise SuccessorEvidenceError(
                f"{context} source file identity is not the closed CBEF recipe"
            )
        digest = _nonempty_string(image["content_digest"], f"{context} content digest")
        if B3.fullmatch(digest) is None:
            raise SuccessorEvidenceError(f"{context} source digest is not BLAKE3")
        if "bytes_utf8" in image:
            source_bytes = _nonempty_string(
                image["bytes_utf8"], f"{context} source bytes"
            ).encode("utf-8")
            if (
                digest != _bytes_b3(source_bytes)
                or image["source_kind"] != "source_file"
            ):
                raise SuccessorEvidenceError(
                    f"{context} source digest or source kind differs"
                )
        else:
            descriptor = _mapping(
                image["immutable_descriptor"], f"{context} immutable descriptor"
            )
            _strict_keys(
                descriptor,
                {"path", "content_digest"},
                f"{context} immutable descriptor",
            )
            descriptor_path = root / _relative_path(
                descriptor["path"], f"{context} immutable source path"
            )
            expected_digest = _bytes_b3(descriptor_path.read_bytes())
            if (
                descriptor["content_digest"] != expected_digest
                or digest != expected_digest
                or image["source_kind"] != "source_file_descriptor"
            ):
                raise SuccessorEvidenceError(
                    f"{context} immutable source descriptor differs"
                )
        source_by_id[source_id] = image

    semantic_context = _mapping(
        inputs["semantic_context"], f"{context} semantic context"
    )
    _strict_keys(
        semantic_context,
        {
            "workspace_id",
            "analysis_context_ids",
            "semantic_environment_ids",
            "source_generation",
            "authority",
        },
        f"{context} semantic context",
    )
    if (
        semantic_context["workspace_id"] != "workspace:00000000000000000000000000000000"
        or semantic_context["source_generation"] != 2
        or semantic_context["authority"]
        != "exact immutable source image plus application-owned relation contract and declared semantic-environment inputs"
    ):
        raise SuccessorEvidenceError(f"{context} semantic context differs")

    requests = _list(inputs["provider_requests"], f"{context} provider requests")
    terminals = _list(inputs["coverage_terminals"], f"{context} coverage terminals")
    if len(requests) != 4 or len(terminals) != 4:
        raise SuccessorEvidenceError(f"{context} must close four provider batches")
    request_keys = {
        "provider_id",
        "provider_run_id",
        "provider_release",
        "source_id",
        "relation_id",
        "requested_scope",
        "analysis_context_id",
        "semantic_environment_id",
        "schema_contract",
        "crate_manifest_target_inputs",
        "trust_inputs",
    }
    expected_provider_relations = {
        "tree-sitter-python": "provider.tree_sitter.cst_node",
        "ruff": "provider.ruff.ast_node",
        "pyrefly": "provider.pyrefly.call_target.v1",
        "rustc": "provider.rustc.public_item.v1",
    }
    request_identities: set[tuple[str, str]] = set()
    request_by_provider: dict[str, Mapping[str, Any]] = {}
    for index, request_value in enumerate(requests, 1):
        request = _mapping(request_value, f"{context} provider request {index}")
        _strict_keys(request, request_keys, f"{context} provider request {index}")
        if request["source_id"] not in source_by_id:
            raise SuccessorEvidenceError(
                f"{context} provider request has unknown source"
            )
        provider_id = _nonempty_string(request["provider_id"], f"{context} provider id")
        if (
            provider_id not in expected_provider_relations
            or request["relation_id"] != expected_provider_relations[provider_id]
            or provider_id in request_by_provider
        ):
            raise SuccessorEvidenceError(f"{context} provider relation closure differs")
        identity = (
            _nonempty_string(request["provider_run_id"], f"{context} provider run"),
            _nonempty_string(request["relation_id"], f"{context} relation id"),
        )
        if (
            identity in request_identities
            or re.fullmatch(r"[0-9a-f]{32}", identity[0]) is None
            or identity[0] == "0" * 32
            or re.fullmatch(r"[0-9a-f]{64}", str(request["analysis_context_id"]))
            is None
            or re.fullmatch(r"[0-9a-f]{64}", str(request["semantic_environment_id"]))
            is None
        ):
            raise SuccessorEvidenceError(
                f"{context} duplicates a provider batch identity"
            )
        environment_digest = _provider_semantic_environment_digest(request)
        source_image = source_by_id[str(request["source_id"])]
        if request["semantic_environment_id"] != environment_digest.removeprefix(
            "b3:"
        ) or request["analysis_context_id"] != _cbef_analysis_context_digest(
            workspace_id=source_image["workspace_id"],
            language_slug=str(source_image["language"]),
            environment_digest=environment_digest,
        ):
            raise SuccessorEvidenceError(
                f"{context} provider context/environment identity differs"
            )
        request_identities.add(identity)
        scope = _mapping(request["requested_scope"], f"{context} requested scope")
        if scope != {"scope_kind": "source_file", "scope_id": request["source_id"]}:
            raise SuccessorEvidenceError(f"{context} provider scope differs")
        contract = _mapping(request["schema_contract"], f"{context} schema contract")
        contract_without_identity = dict(contract)
        schema_identity = contract_without_identity.pop("schema_identity", None)
        _strict_keys(
            contract_without_identity,
            {
                "relation_id",
                "schema_contract_version",
                "provider_release",
                "arrow_type_universe",
                "semantic_encoding",
                "fields",
            },
            f"{context} schema contract",
        )
        if (
            contract_without_identity["relation_id"] != request["relation_id"]
            or contract_without_identity["provider_release"]
            != request["provider_release"]
            or contract_without_identity["schema_contract_version"] != 1
            or contract_without_identity["arrow_type_universe"]
            != "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5"
            or contract_without_identity["semantic_encoding"]
            != "typed-arrow-relation-stream"
            or schema_identity != _provider_schema_identity(contract_without_identity)
        ):
            raise SuccessorEvidenceError(f"{context} provider schema contract differs")
        fields = _list(contract_without_identity["fields"], f"{context} schema fields")
        field_names: set[str] = set()
        for field_index, field_value in enumerate(fields, 1):
            field = _mapping(field_value, f"{context} schema field {field_index}")
            _strict_keys(
                field,
                {"name", "data_type", "nullable", "metadata"},
                f"{context} schema field {field_index}",
            )
            name = _nonempty_string(field["name"], f"{context} schema field name")
            if name in field_names or not isinstance(field["nullable"], bool):
                raise SuccessorEvidenceError(f"{context} schema field closure differs")
            field_names.add(name)
            metadata = _mapping(field["metadata"], f"{context} field metadata")
            _strict_keys(
                metadata,
                {"codefabric.meaning", "codefabric.semantic_representation"},
                f"{context} field metadata",
            )
            if metadata["codefabric.semantic_representation"] != "typed-arrow-field":
                raise SuccessorEvidenceError(f"{context} field representation differs")
        required_provenance = {
            "tree-sitter-python": {
                "provider_run_id",
                "provider_id",
                "provider_release",
                "analysis_context_id",
                "semantic_environment_id",
                "file_id",
                "content_digest",
                "source_generation",
            },
            "ruff": {
                "provider_run_id",
                "provider_id",
                "provider_release",
                "analysis_context_id",
                "semantic_environment_id",
                "file_id",
                "content_digest",
                "source_generation",
            },
            "pyrefly": {
                "provider_run_id",
                "analysis_context_id",
                "semantic_environment_id",
                "file_id",
                "content_digest",
                "source_generation",
            },
            "rustc": {
                "provider_run_id",
                "compilation_unit_id",
                "owner_id",
                "source_generation",
                "source_file_id",
                "source_content_digest",
                "stable_crate_id",
                "def_path_hash",
            },
        }[provider_id]
        if not required_provenance <= field_names:
            raise SuccessorEvidenceError(f"{context} provider schema omits provenance")
        if provider_id == "rustc":
            crate_inputs = _mapping(
                request["crate_manifest_target_inputs"], f"{context} rust crate inputs"
            )
            trust_inputs = _mapping(
                request["trust_inputs"], f"{context} rust trust inputs"
            )
            if (
                crate_inputs
                != {
                    "package": {
                        "name": "wp33-rust-fixture",
                        "version": "0.0.0",
                        "edition": "2024",
                        "publish": False,
                        "dependencies": [],
                    },
                    "target": {
                        "name": "wp33_rust_fixture",
                        "kind": "lib",
                        "crate_types": ["rlib"],
                        "source_id": "rs:fixture",
                        "target_triple": "x86_64-unknown-linux-gnu",
                        "requested_features": [],
                    },
                    "toolchain": {
                        "channel": "nightly-2026-08-18",
                        "rustc_release": "1.100.0-nightly",
                        "rustc_commit": ("8fa1c96cfd489e4c27654c144ae871ce2c4db6c6"),
                    },
                }
                or trust_inputs.get("requested_profile") != "untrusted"
                or trust_inputs.get("direct_host_cargo_allowed") is not False
                or trust_inputs.get("admission") != "unavailable"
            ):
                raise SuccessorEvidenceError(f"{context} rust trust boundary differs")
        elif (
            request["crate_manifest_target_inputs"] is not None
            or request["trust_inputs"] is not None
        ):
            raise SuccessorEvidenceError(
                f"{context} non-rust provider carries compiler trust inputs"
            )
        request_by_provider[provider_id] = request
    if set(request_by_provider) != set(expected_provider_relations):
        raise SuccessorEvidenceError(f"{context} provider set differs")
    expected_analysis_contexts = {
        provider: request["analysis_context_id"]
        for provider, request in request_by_provider.items()
    }
    expected_environments = {
        provider: request["semantic_environment_id"]
        for provider, request in request_by_provider.items()
    }
    if (
        semantic_context["analysis_context_ids"] != expected_analysis_contexts
        or semantic_context["semantic_environment_ids"] != expected_environments
    ):
        raise SuccessorEvidenceError(
            f"{context} semantic context does not close provider identities"
        )
    terminal_keys = {
        "provider_id",
        "provider_run_id",
        "relation_id",
        "requested_units",
        "completed_units",
        "remainders",
        "state",
    }
    terminal_identities = set()
    for index, terminal_value in enumerate(terminals, 1):
        terminal = _mapping(terminal_value, f"{context} coverage terminal {index}")
        _strict_keys(terminal, terminal_keys, f"{context} coverage terminal {index}")
        provider_id = str(terminal["provider_id"])
        terminal_identities.add((terminal["provider_run_id"], terminal["relation_id"]))
        if provider_id == "rustc":
            remainders = _list(terminal["remainders"], f"{context} rust remainder")
            if (
                terminal["state"] != "unavailable"
                or terminal["requested_units"] != 1
                or terminal["completed_units"] != 0
                or remainders
                != [
                    {
                        "units": 1,
                        "reason": "TRUST_SUBSTRATE_UNAVAILABLE",
                        "retryable": True,
                    }
                ]
            ):
                raise SuccessorEvidenceError(f"{context} rust remainder differs")
        elif (
            terminal["state"] != "complete"
            or terminal["requested_units"] != 1
            or terminal["completed_units"] != 1
            or terminal["remainders"] != []
        ):
            raise SuccessorEvidenceError(f"{context} provider coverage is not complete")
    if terminal_identities != request_identities:
        raise SuccessorEvidenceError(
            f"{context} provider request/terminal closure differs"
        )
    if inputs["provider_release_pins"] != PROVIDER_RELEASE_INPUT_PINS:
        raise SuccessorEvidenceError(f"{context} provider release vector differs")
    if set(inputs["requested_family_set"]) != set(expected_provider_relations.values()):
        raise SuccessorEvidenceError(f"{context} requested provider family set differs")
    protocol = _mapping(
        inputs["protocol_schema_identity"], f"{context} protocol schema identity"
    )
    _strict_keys(
        protocol,
        {
            "control_schema",
            "control_schema_b3",
            "payload_encoding",
            "metadata_version",
            "protocol_id",
            "schema_contract",
        },
        f"{context} protocol schema identity",
    )
    control_path = root / _relative_path(
        protocol["control_schema"], f"{context} control schema path"
    )
    if protocol["control_schema_b3"] != _bytes_b3(control_path.read_bytes()):
        raise SuccessorEvidenceError(
            f"{context} provider control schema digest differs"
        )
    if (
        protocol["payload_encoding"] != "Arrow IPC stream"
        or protocol["metadata_version"] != "V5"
    ):
        raise SuccessorEvidenceError(f"{context} does not issue Arrow IPC payloads")
    schema_contract = _mapping(
        protocol["schema_contract"], f"{context} provider schema contract"
    )
    _strict_keys(
        schema_contract,
        {
            "request_message",
            "chunk_message",
            "terminal_message",
            "semantic_payload_field",
            "stream_schema_authority",
            "dictionary_scope",
        },
        f"{context} provider schema contract",
    )
    if (
        schema_contract["request_message"] != "codefabric.provider.v1.ProviderJobSpec"
        or schema_contract["chunk_message"]
        != "codefabric.provider.v1.ProviderObservationChunkEvent"
        or schema_contract["terminal_message"]
        != "codefabric.provider.v1.ProviderTerminalEvent"
        or schema_contract["semantic_payload_field"] != "arrow_ipc"
        or schema_contract["stream_schema_authority"] != "schema_digest"
        or schema_contract["dictionary_scope"] != "one IPC stream"
    ):
        raise SuccessorEvidenceError(f"{context} provider message identity differs")
    rows = _decoded_rows(decoded, f"{context} decoded expectation")
    pyrefly_rows = [row for row in rows if row[0] == "pyrefly"]
    typed_source = source_by_id.get("py:typed")
    if typed_source is None:
        raise SuccessorEvidenceError(f"{context} lacks the typed Python source")
    source = _nonempty_string(
        typed_source.get("bytes_utf8"), f"{context} typed Python source"
    )
    call = re.search(r"return\s+([A-Za-z_]\w*)\(", source)
    if call is None:
        raise SuccessorEvidenceError(f"{context} typed Python call is not decoded")
    call_name = call.group(1)
    call_start = source.index(call_name, call.start())
    expected_native = {
        "call_occurrence_ordinal": 0,
        "start_byte": call_start,
        "end_byte": call_start + len(call_name),
        "target_ordinal": 0,
        "callee_kind": "function",
        "qualified_target": "builtins.abs" if call_name == "abs" else call_name,
        "class_name": None,
        "resolution_state": "resolved",
    }
    if (
        len(rows) != 4
        or len(pyrefly_rows) != 1
        or pyrefly_rows[0][1] != request_by_provider["pyrefly"]["provider_run_id"]
        or pyrefly_rows[0][2] != request_by_provider["pyrefly"]["relation_id"]
        or pyrefly_rows[0][4] != "py:typed"
        or pyrefly_rows[0][5] != typed_source["content_digest"]
        or pyrefly_rows[0][6] != expected_native
        or pyrefly_rows[0][7] != "complete"
    ):
        raise SuccessorEvidenceError(
            f"{context} Pyrefly callable expectation is not derived from the exact source"
        )
    syntax_source = source_by_id.get("py:syntax")
    if syntax_source is None:
        raise SuccessorEvidenceError(f"{context} lacks the Python syntax source")
    syntax_text = _nonempty_string(
        syntax_source.get("bytes_utf8"), f"{context} Python syntax source"
    )
    syntax_end = len(syntax_text.removesuffix("\n").encode("utf-8"))
    expected_syntax_native = {
        "ruff": {
            "raw_kind": "FunctionDef",
            "ast_category": "statement",
            "start_byte": 0,
            "end_byte": syntax_end,
            "raw_kind_disposition": "known",
        },
        "tree-sitter-python": {
            "provider_local_node_id": 2,
            "parent_provider_local_node_id": 1,
            "raw_kind": "function_definition",
            "start_byte": 0,
            "end_byte": syntax_end,
            "named": True,
            "error": False,
            "missing": False,
            "raw_kind_disposition": "known",
        },
    }
    for provider_id, native in expected_syntax_native.items():
        provider_rows = [row for row in rows if row[0] == provider_id]
        request = request_by_provider[provider_id]
        if (
            len(provider_rows) != 1
            or provider_rows[0][1] != request["provider_run_id"]
            or provider_rows[0][2] != request["relation_id"]
            or provider_rows[0][4] != "py:syntax"
            or provider_rows[0][5] != syntax_source["content_digest"]
            or provider_rows[0][6] != native
            or provider_rows[0][7] != "complete"
        ):
            raise SuccessorEvidenceError(
                f"{context} {provider_id} syntax expectation is not derived from exact pinned coordinates"
            )
    if rows != sorted(rows, key=lambda row: (str(row[0]), str(row[2]))):
        raise SuccessorEvidenceError(
            f"{context} provider rows violate declared canonical ordering"
        )


def _validate_transformation_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    definition = _mapping(
        inputs["transformation_definition"], f"{context} transformation definition"
    )
    _strict_keys(
        definition,
        {
            "semantic_id",
            "semantic_version",
            "typed_input_relation_ids",
            "resource_class",
            "determinism_policy",
            "order_policy",
            "plan_building_function",
            "output_schema_assertion",
        },
        f"{context} transformation definition",
    )
    typed_inputs = _list(
        definition["typed_input_relation_ids"], f"{context} typed inputs"
    )
    if len(typed_inputs) != 1:
        raise SuccessorEvidenceError(
            f"{context} must bind exactly one root transformation input"
        )
    for typed_input in typed_inputs:
        _strict_keys(
            _mapping(typed_input, f"{context} typed input"),
            {"relation_id", "schema_id"},
            f"{context} typed input",
        )
    builder = _mapping(
        definition["plan_building_function"], f"{context} plan building function"
    )
    _strict_keys(
        builder,
        {
            "trait_entrypoint",
            "builder_identity",
            "root_input_relation_id",
            "operations",
        },
        f"{context} plan building function",
    )
    if builder["builder_identity"] != (
        "datafusion::logical_expr::LogicalPlanBuilder@55.0.0"
    ):
        raise SuccessorEvidenceError(f"{context} plan-builder identity differs")
    relation = _mapping(
        inputs["admitted_input_relation"], f"{context} admitted input relation"
    )
    input_schema = _mapping(inputs["input_schema"], f"{context} input schema")
    input_columns = _string_list(relation["columns"], f"{context} input columns")
    if input_columns != list(input_schema):
        raise SuccessorEvidenceError(
            f"{context} admitted columns do not match the typed input schema"
        )
    typed_input = _mapping(typed_inputs[0], f"{context} typed input")
    if (
        typed_input["relation_id"] != relation["relation"]
        or builder["root_input_relation_id"] != relation["relation"]
    ):
        raise SuccessorEvidenceError(
            f"{context} root plan input is not the admitted typed relation"
        )
    rows: list[dict[str, Any]] = []
    for index, raw_row in enumerate(
        _list(relation["rows"], f"{context} admitted rows")
    ):
        row = _list(raw_row, f"{context} admitted row {index}")
        if len(row) != len(input_columns):
            raise SuccessorEvidenceError(
                f"{context} admitted row {index} does not match its schema"
            )
        rows.append(dict(zip(input_columns, row, strict=True)))

    def column_expression(
        value: object,
        available: Mapping[str, str],
        expression_context: str,
    ) -> tuple[str, str]:
        expression = _mapping(value, expression_context)
        _strict_keys(
            expression,
            {"kind", "name", "data_type"},
            expression_context,
        )
        name = _nonempty_string(expression["name"], f"{expression_context} name")
        data_type = _nonempty_string(
            expression["data_type"], f"{expression_context} type"
        )
        if expression["kind"] != "column" or available.get(name) != data_type:
            raise SuccessorEvidenceError(
                f"{expression_context} is not a declared typed column"
            )
        return name, data_type

    normalized_schema = {
        name: _nonempty_string(declaration, f"{context} input field {name}").split()[0]
        for name, declaration in input_schema.items()
    }
    operations = _list(builder["operations"], f"{context} typed operations")
    if [
        operation.get("operator")
        for operation in operations
        if isinstance(operation, dict)
    ] != [
        "filter",
        "project",
        "sort",
    ] or any(not isinstance(operation, dict) for operation in operations):
        raise SuccessorEvidenceError(
            f"{context} plan operations are not closed typed objects"
        )
    filter_operation = _mapping(operations[0], f"{context} filter operation")
    _strict_keys(
        filter_operation, {"operator", "predicate"}, f"{context} filter operation"
    )
    predicate = _mapping(filter_operation["predicate"], f"{context} filter predicate")
    _strict_keys(
        predicate,
        {"expression", "left", "right"},
        f"{context} filter predicate",
    )
    left_name, left_type = column_expression(
        predicate["left"], normalized_schema, f"{context} filter left expression"
    )
    right = _mapping(predicate["right"], f"{context} filter right expression")
    _strict_keys(
        right,
        {"kind", "data_type", "value"},
        f"{context} filter right expression",
    )
    if (
        predicate["expression"] != "eq"
        or right["kind"] != "literal"
        or right["data_type"] != left_type
    ):
        raise SuccessorEvidenceError(
            f"{context} filter is not the declared typed equality expression"
        )
    filtered = [row for row in rows if row[left_name] == right["value"]]

    project_operation = _mapping(operations[1], f"{context} project operation")
    _strict_keys(
        project_operation,
        {"operator", "expressions"},
        f"{context} project operation",
    )
    projected_schema: dict[str, str] = {}
    projected: list[dict[str, Any]] = [{} for _ in filtered]
    for index, raw_expression in enumerate(
        _list(project_operation["expressions"], f"{context} project expressions")
    ):
        expression = _mapping(raw_expression, f"{context} project expression {index}")
        if expression.get("kind") == "alias":
            _strict_keys(
                expression,
                {"kind", "source", "alias"},
                f"{context} project expression {index}",
            )
            source_name, data_type = column_expression(
                expression["source"],
                normalized_schema,
                f"{context} project source {index}",
            )
            output_name = _nonempty_string(
                expression["alias"], f"{context} project alias {index}"
            )
        elif expression.get("kind") == "column":
            source_name, data_type = column_expression(
                expression,
                normalized_schema,
                f"{context} project expression {index}",
            )
            output_name = source_name
        else:
            raise SuccessorEvidenceError(
                f"{context} project expression {index} is unsupported"
            )
        if output_name in projected_schema:
            raise SuccessorEvidenceError(f"{context} project aliases are not unique")
        projected_schema[output_name] = data_type
        for source_row, output_row in zip(filtered, projected, strict=True):
            output_row[output_name] = source_row[source_name]

    sort_operation = _mapping(operations[2], f"{context} sort operation")
    _strict_keys(sort_operation, {"operator", "keys"}, f"{context} sort operation")
    sort_keys: list[tuple[str, bool, bool]] = []
    for index, raw_key in enumerate(
        _list(sort_operation["keys"], f"{context} sort keys")
    ):
        key = _mapping(raw_key, f"{context} sort key {index}")
        _strict_keys(
            key,
            {"expression", "ascending", "nulls_first"},
            f"{context} sort key {index}",
        )
        name, _ = column_expression(
            key["expression"], projected_schema, f"{context} sort expression {index}"
        )
        if not isinstance(key["ascending"], bool) or not isinstance(
            key["nulls_first"], bool
        ):
            raise SuccessorEvidenceError(f"{context} sort flags must be Boolean")
        sort_keys.append((name, key["ascending"], key["nulls_first"]))
    for name, ascending, nulls_first in reversed(sort_keys):
        projected.sort(
            key=lambda row, field=name, first=nulls_first: (
                0 if (row[field] is None) == first else 1,
                row[field],
            ),
            reverse=not ascending,
        )

    output = _mapping(
        definition["output_schema_assertion"], f"{context} output schema assertion"
    )
    _strict_keys(
        output, {"relation_id", "fields"}, f"{context} output schema assertion"
    )
    output_fields = [
        _mapping(field, f"{context} output field {index}")
        for index, field in enumerate(
            _list(output["fields"], f"{context} output fields")
        )
    ]
    for index, field in enumerate(output_fields):
        _strict_keys(
            field,
            {"name", "type", "nullable"},
            f"{context} output field {index}",
        )
    output_columns = [field["name"] for field in output_fields]
    if (
        output["relation_id"] != decoded["relation"]
        or output_columns != list(projected_schema)
        or decoded["columns"] != output_columns
        or any(
            field["type"] != projected_schema[field["name"]]
            or field["nullable"] is not False
            for field in output_fields
        )
    ):
        raise SuccessorEvidenceError(
            f"{context} decoded output does not satisfy its typed schema assertion"
        )
    selected = [[row[column] for column in output_columns] for row in projected]
    if decoded["rows"] != selected:
        raise SuccessorEvidenceError(
            f"{context} decoded transformation is not derived from its full typed plan"
        )


DERIVED_EXPECTATION_FAMILIES = (
    ("python.cfg_node", "GEN §24"),
    ("python.cfg_edge", "GEN §24"),
    ("python.evaluation_order", "GEN §24"),
    ("python.def_use", "GEN §25"),
    ("python.reaching_definition", "GEN §25"),
    ("python.liveness", "GEN §25"),
    ("python.value_flow", "GEN §25"),
    ("python.memory_location", "GEN §26"),
    ("python.alias_points_to", "GEN §26"),
    ("python.effect", "GEN §27"),
    ("python.exceptional_flow", "GEN §28"),
    ("python.resource_lifecycle", "GEN §29"),
    ("python.async_suspension", "GEN §30"),
    ("python.closure_capture", "GEN §31"),
    ("python.unknown", "GEN §33"),
    ("rust_mir.ownership_state", "GEN §44"),
    ("rust_mir.def_use", "GEN §45"),
    ("rust_mir.reaching_definition", "GEN §45"),
    ("rust_mir.liveness", "GEN §45"),
    ("rust_mir.value_flow", "GEN §45"),
    ("rust_mir.alias_points_to", "GEN §46"),
    ("rust_mir.resource_lifecycle", "GEN §47"),
    ("rust_mir.async_lowering", "GEN §48"),
    ("rust_mir.unsafe_ffi_candidate", "GEN §50"),
    ("rust_mir.control_dependence", "GEN §57"),
    ("rust_mir.unknown", "GEN §51"),
    ("common.reachability", "GEN §54"),
    ("common.scc_membership", "GEN §55"),
    ("common.dominator", "GEN §56"),
    ("common.post_dominator", "GEN §56"),
    ("common.control_dependence", "GEN §57"),
    ("common.loop", "GEN §58"),
    ("common.reaching_definition", "GEN §59"),
    ("common.liveness", "GEN §60"),
    ("common.alias_points_to", "GEN §61"),
    ("common.shortest_distance", "GEN §62"),
    ("common.connected_component", "GEN §63"),
    ("common.transitive_closure", "GEN §64"),
    ("common.transitive_reduction", "GEN §64"),
    ("common.structural_metric", "GEN §65"),
    ("common.callable_summary", "GEN §66"),
    ("common.call_graph", "GEN §72"),
    ("common.callable_effect", "GEN §77"),
    ("common.callable_resource", "GEN §77B"),
    ("common.unknown", "GEN §84"),
    ("common.completeness", "GEN §85"),
    ("common.invalidation", "GEN §87"),
)
DERIVED_EXPECTATION_PROPERTIES = [
    "independently authored examples",
    "row-order permutation",
    "addition deletion and change",
    "exceptional dynamic and partial inputs",
    "clean incremental equivalence",
    "causal input mutations",
    "convergence and resource bounds",
    "exact provenance closure",
]


def _derived_call_graph_rows(
    provider_value: object,
    occurrence_value: object,
    callable_value: object,
    context: str,
    *,
    require_complete: bool,
) -> list[list[Any]]:
    provider_targets = _mapping(provider_value, f"{context} provider call targets")
    _strict_keys(
        provider_targets,
        {
            "relation",
            "columns",
            "rows",
            "source_image",
            "provider_run_id",
            "coverage_terminal",
        },
        f"{context} provider call targets",
    )
    expected_columns = [
        "call_occurrence_ordinal",
        "start_byte",
        "end_byte",
        "target_ordinal",
        "callee_kind",
        "qualified_target",
        "class_name",
        "resolution_state",
    ]
    source_image = _mapping(
        provider_targets["source_image"], f"{context} call-target source image"
    )
    _strict_keys(
        source_image,
        {
            "source_id",
            "workspace_id",
            "module_id",
            "analysis_context_id",
            "file_id",
            "canonical_path_bytes_hex",
            "content_digest",
            "semantic_environment_inputs",
            "semantic_environment_id",
            "source_generation",
            "bytes_utf8",
        },
        f"{context} call-target source image",
    )
    source = _nonempty_string(
        source_image["bytes_utf8"], f"{context} call-target source"
    )
    try:
        comparison_key = bytes.fromhex(
            _nonempty_string(
                source_image["canonical_path_bytes_hex"],
                f"{context} call-target comparison key",
            )
        )
    except ValueError as error:
        raise SuccessorEvidenceError(
            f"{context} call-target comparison key is not hexadecimal bytes"
        ) from error
    environment_inputs = _mapping(
        source_image["semantic_environment_inputs"],
        f"{context} call-target semantic environment inputs",
    )
    expected_environment_inputs = {
        "provider_id": "pyrefly",
        "provider_release": "pyrefly@1.2.0#1933169ad8ee9e4d4114112eb56ef0811fb0a094",
        "relation_id": "provider.pyrefly.call_target.v1",
        "language": "python",
    }
    environment_digest = _canonical_b3(environment_inputs)
    identity_context = {
        "workspace_id": source_image["workspace_id"],
        "module_id": source_image["module_id"],
        "analysis_context_id": source_image["analysis_context_id"],
        "file_id": source_image["file_id"],
        "content_digest": source_image["content_digest"],
    }
    if (
        provider_targets["relation"] != "provider.pyrefly.call_target.v1"
        or provider_targets["columns"] != expected_columns
        or source_image["content_digest"] != _bytes_b3(source.encode("utf-8"))
        or re.fullmatch(r"workspace:[0-9a-f]{32}", str(source_image["workspace_id"]))
        is None
        or re.fullmatch(r"entity:module:[0-9a-f]{32}", str(source_image["module_id"]))
        is None
        or re.fullmatch(r"[0-9a-f]{32}", str(source_image["file_id"])) is None
        or source_image["file_id"]
        != _cbef_source_file_id(
            workspace_id=source_image["workspace_id"],
            comparison_key=comparison_key,
        )
        or re.fullmatch(r"[0-9a-f]{64}", str(source_image["analysis_context_id"]))
        is None
        or environment_inputs != expected_environment_inputs
        or source_image["semantic_environment_id"]
        != environment_digest.removeprefix("b3:")
        or source_image["analysis_context_id"]
        != _cbef_analysis_context_digest(
            workspace_id=source_image["workspace_id"],
            language_slug="python",
            environment_digest=environment_digest,
        )
        or re.fullmatch(r"[0-9a-f]{64}", str(source_image["semantic_environment_id"]))
        is None
        or not isinstance(source_image["source_generation"], int)
        or isinstance(source_image["source_generation"], bool)
        or source_image["source_generation"] < 1
        or re.fullmatch(r"[0-9a-f]{32}", str(provider_targets["provider_run_id"]))
        is None
        or provider_targets["provider_run_id"] == "0" * 32
    ):
        raise SuccessorEvidenceError(f"{context} call-target authority differs")

    callables = _mapping(callable_value, f"{context} canonical callable lookup")
    _strict_keys(
        callables,
        {"relation", "columns", "rows"},
        f"{context} canonical callable lookup",
    )
    callable_columns = [
        "qualified_target",
        "callable_id",
        "kind",
        "qualified_lexical_path",
    ]
    if (
        callables["relation"] != "canonical.python_callable_lookup.v1"
        or callables["columns"] != callable_columns
    ):
        raise SuccessorEvidenceError(f"{context} canonical callable contract differs")
    callable_by_target: dict[str, str] = {}
    callable_by_path: dict[tuple[str, ...], str] = {}
    for index, row_value in enumerate(
        _list(callables["rows"], f"{context} canonical callable rows"), 1
    ):
        if not isinstance(row_value, list) or len(row_value) != len(callable_columns):
            raise SuccessorEvidenceError(
                f"{context} canonical callable row {index} differs"
            )
        qualified_target, callable_id, kind, path_value = row_value
        path = _string_list(path_value, f"{context} callable path {index}")
        expected_id = _python_function_id(identity_context, path)
        if (
            not isinstance(qualified_target, str)
            or not qualified_target
            or kind != "function"
            or callable_id != expected_id
            or qualified_target in callable_by_target
            or tuple(path) in callable_by_path
        ):
            raise SuccessorEvidenceError(
                f"{context} canonical callable identity or lookup differs"
            )
        callable_by_target[qualified_target] = callable_id
        callable_by_path[tuple(path)] = callable_id

    occurrences = _mapping(occurrence_value, f"{context} canonical call occurrences")
    occurrence_columns = [
        "call_occurrence_ordinal",
        "call_site_id",
        "owner_id",
        "owner_relative_role",
        "owner_relative_ordinal",
        "file_id",
        "content_digest",
        "start_byte",
        "end_byte",
        "syntactic_callee",
    ]
    _strict_keys(
        occurrences,
        {"relation", "columns", "rows"},
        f"{context} canonical call occurrences",
    )
    if (
        occurrences["relation"] != "canonical.python_call_site.v1"
        or occurrences["columns"] != occurrence_columns
    ):
        raise SuccessorEvidenceError(f"{context} canonical call-site contract differs")
    try:
        tree = ast.parse(source)
    except SyntaxError as error:
        raise SuccessorEvidenceError(
            f"{context} call-target source is not valid Python"
        ) from error
    occurrence_drafts: list[tuple[int, int, str, int, str]] = []
    for function in tree.body:
        if not isinstance(function, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        calls = sorted(
            (node for node in ast.walk(function) if isinstance(node, ast.Call)),
            key=lambda node: _node_byte_range(source, node.func, context),
        )
        for owner_ordinal, call in enumerate(calls):
            if not isinstance(call.func, ast.Name):
                raise SuccessorEvidenceError(
                    f"{context} canonical call occurrence lacks a named callee"
                )
            start_byte, end_byte = _node_byte_range(source, call.func, context)
            occurrence_drafts.append(
                (start_byte, end_byte, function.name, owner_ordinal, call.func.id)
            )
    expected_occurrences = []
    for occurrence_ordinal, draft in enumerate(sorted(occurrence_drafts)):
        start_byte, end_byte, owner_name, owner_ordinal, syntactic_callee = draft
        owner_id = callable_by_path.get((owner_name,))
        if owner_id is None:
            raise SuccessorEvidenceError(
                f"{context} call occurrence owner has no canonical callable"
            )
        call_site_id = _python_call_site_id(
            identity_context,
            owner_id=owner_id,
            owner_relative_role="body.call.callee",
            owner_relative_ordinal=owner_ordinal,
            start_byte=start_byte,
            end_byte=end_byte,
        )
        expected_occurrences.append(
            [
                occurrence_ordinal,
                call_site_id,
                owner_id,
                "body.call.callee",
                owner_ordinal,
                source_image["file_id"],
                source_image["content_digest"],
                start_byte,
                end_byte,
                syntactic_callee,
            ]
        )
    supplied_occurrences = _list(
        occurrences["rows"], f"{context} canonical call occurrence rows"
    )
    if supplied_occurrences != expected_occurrences:
        raise SuccessorEvidenceError(
            f"{context} canonical call occurrence/owner identity differs"
        )
    occurrence_by_join = {(row[0], row[7], row[8]): row for row in expected_occurrences}

    rows = _list(provider_targets["rows"], f"{context} provider call-target rows")
    seen: set[tuple[str, int]] = set()
    call_sites: set[str] = set()
    decoded: list[list[Any]] = []
    provenance = {
        "input_relation": provider_targets["relation"],
        "provider_run_id": provider_targets["provider_run_id"],
        "analysis_context_id": source_image["analysis_context_id"],
        "source_content_digest": source_image["content_digest"],
        "analysis_semantic_id": "analysis.common.call-graph.candidate-preserving.v1",
    }
    for index, row_value in enumerate(rows, 1):
        if not isinstance(row_value, list) or len(row_value) != len(expected_columns):
            raise SuccessorEvidenceError(
                f"{context} call-target row {index} differs from its typed columns"
            )
        occurrence_ordinal, start_byte, end_byte, target_ordinal = row_value[:4]
        callee_kind, qualified_target, class_name, resolution = row_value[4:]
        occurrence = occurrence_by_join.get((occurrence_ordinal, start_byte, end_byte))
        callee = callable_by_target.get(str(qualified_target))
        if (
            occurrence is None
            or callee is None
            or callee_kind != "function"
            or class_name is not None
            or resolution not in {"resolved", "candidate"}
            or not isinstance(target_ordinal, int)
            or isinstance(target_ordinal, bool)
            or target_ordinal < 0
            or (occurrence[1], target_ordinal) in seen
        ):
            raise SuccessorEvidenceError(
                f"{context} call-target row {index} does not close both enforced joins"
            )
        seen.add((occurrence[1], target_ordinal))
        call_sites.add(occurrence[1])
        decoded.append(
            [
                occurrence[1],
                occurrence[2],
                callee,
                resolution,
                target_ordinal,
                provenance,
            ]
        )
    terminal = _mapping(
        provider_targets["coverage_terminal"], f"{context} call-target coverage"
    )
    _strict_keys(
        terminal,
        {
            "requested_call_sites",
            "completed_call_sites",
            "remainders",
            "state",
        },
        f"{context} call-target coverage",
    )
    requested_call_sites = len(expected_occurrences)
    if require_complete:
        if (
            terminal
            != {
                "requested_call_sites": requested_call_sites,
                "completed_call_sites": requested_call_sites,
                "remainders": [],
                "state": "complete",
            }
            or len(call_sites) != requested_call_sites
        ):
            raise SuccessorEvidenceError(
                f"{context} call-target coverage is not complete"
            )
    else:
        remainder_values = _list(
            terminal["remainders"], f"{context} call-target remainders"
        )
        remainders = [
            _mapping(value, f"{context} call-target remainder")
            for value in remainder_values
        ]
        expected_missing = {
            row[1] for row in expected_occurrences if row[1] not in call_sites
        }
        if (
            terminal["requested_call_sites"] != requested_call_sites
            or terminal["completed_call_sites"] != len(call_sites)
            or terminal["state"] != "partial"
            or {row.get("call_site_id") for row in remainders} != expected_missing
            or any(
                set(row) != {"call_site_id", "reason", "retryable"}
                or row.get("reason") != "PROVIDER_TARGET_SET_INCOMPLETE"
                or row.get("retryable") is not True
                for row in remainders
            )
        ):
            raise SuccessorEvidenceError(
                f"{context} partial call-target coverage differs"
            )
    return sorted(decoded, key=lambda row: (row[0], row[4], row[2]))


def _validate_derived_expectation_contract(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    census = [
        _mapping(value, f"{context} family census")
        for value in _list(inputs["accepted_family_census"], f"{context} family census")
    ]
    expected_ids = {family_id for family_id, _ in DERIVED_EXPECTATION_FAMILIES}
    census_by_id: dict[str, Mapping[str, Any]] = {}
    for entry in census:
        _strict_keys(
            entry,
            {
                "family_id",
                "normative_owner",
                "authority",
                "expected_disposition",
                "required_properties",
            },
            f"{context} family census entry",
        )
        family_id = _nonempty_string(entry["family_id"], f"{context} family id")
        if family_id in census_by_id:
            raise SuccessorEvidenceError(f"{context} duplicates a derived family")
        if (
            entry["authority"] != "application_owned"
            or entry["expected_disposition"] != "independent_expectation_required"
            or entry["required_properties"] != DERIVED_EXPECTATION_PROPERTIES
        ):
            raise SuccessorEvidenceError(
                f"{context} {family_id} proof obligation differs"
            )
        census_by_id[family_id] = entry
    if set(census_by_id) != expected_ids or any(
        census_by_id[family_id]["normative_owner"] != owner
        for family_id, owner in DERIVED_EXPECTATION_FAMILIES
    ):
        raise SuccessorEvidenceError(f"{context} normative family closure differs")

    definitions = [
        _mapping(value, f"{context} analysis definition")
        for value in _list(inputs["analysis_definitions"], f"{context} definitions")
    ]
    definition_by_id = {str(value.get("family_id")): value for value in definitions}
    if (
        len(definition_by_id) != len(definitions)
        or set(definition_by_id) != expected_ids
    ):
        raise SuccessorEvidenceError(f"{context} analysis-definition closure differs")
    for family_id, owner in DERIVED_EXPECTATION_FAMILIES:
        definition = definition_by_id[family_id]
        _strict_keys(
            definition,
            {
                "family_id",
                "normative_owner",
                "authority",
                "typed_input_contract",
                "precision",
                "proof_contract",
            },
            f"{context} {family_id} definition",
        )
        if (
            definition["normative_owner"] != owner
            or definition["authority"] != "application_owned"
            or definition["proof_contract"] != DERIVED_EXPECTATION_PROPERTIES
            or not _nonempty_string(
                definition["typed_input_contract"], f"{context} typed input contract"
            )
            or not _nonempty_string(
                definition["precision"], f"{context} precision contract"
            )
        ):
            raise SuccessorEvidenceError(f"{context} {family_id} definition differs")
    if definition_by_id["common.call_graph"] != {
        "family_id": "common.call_graph",
        "normative_owner": "GEN §72",
        "authority": "application_owned",
        "typed_input_contract": (
            "provider.pyrefly.call_target.v1 joined by exact source range to canonical call occurrences/owners and by qualified_target to canonical callable entities"
        ),
        "precision": (
            "candidate-preserving; partial target sets materialize common.unknown"
        ),
        "proof_contract": DERIVED_EXPECTATION_PROPERTIES,
    }:
        raise SuccessorEvidenceError(
            f"{context} concrete call-graph definition differs"
        )

    python_cfg = _mapping(inputs["python_cfg_inputs"], f"{context} Python CFG inputs")
    if python_cfg != {
        "normative_relations": [
            "python.cfg_node",
            "python.cfg_edge",
            "python.evaluation_order",
        ],
        "authority": "application_owned",
        "provider_inputs": [
            "provider.ruff.ast_node",
            "provider.ruff.semantic_edge",
        ],
    }:
        raise SuccessorEvidenceError(f"{context} Python CFG authority differs")
    if _mapping(inputs["rust_mir_cfg_inputs"], f"{context} Rust MIR CFG inputs") != {
        "provider_native_relation": "provider.rustc.cfg_edge.v1",
        "application_output_families": ["rust_mir.control_dependence"],
        "authority_split": "provider-native CFG is input and never relabeled as application-derived output",
    }:
        raise SuccessorEvidenceError(f"{context} Rust MIR authority split differs")
    if _mapping(
        inputs["rust_control_native_inputs"], f"{context} Rust native inputs"
    ) != {
        "relations": [
            "provider.rustc.mir_block.v1",
            "provider.rustc.mir_operand.v1",
            "provider.rustc.mir_terminator.v1",
            "provider.rustc.cfg_edge.v1",
        ]
    }:
        raise SuccessorEvidenceError(f"{context} Rust control input closure differs")
    precision = _mapping(inputs["precision_profiles"], f"{context} precision profiles")
    if set(precision) != {"required", "partial_input", "forbidden"} or any(
        not _nonempty_string(value, f"{context} precision profile")
        for value in precision.values()
    ):
        raise SuccessorEvidenceError(f"{context} precision profile closure differs")
    if _mapping(inputs["authority_context"], f"{context} authority context") != {
        "suite": "codefabric-relational-data-fabric@2.1.0",
        "derivation_rule": "GEN family owner headings identify the complete expectation scope; decoded rows are mechanically derived from exact typed inputs",
    }:
        raise SuccessorEvidenceError(f"{context} derived authority context differs")

    terminals = [
        _mapping(value, f"{context} coverage terminal")
        for value in _list(
            inputs["coverage_terminals"], f"{context} coverage terminals"
        )
    ]
    provider_targets = _mapping(
        inputs["provider_call_targets"], f"{context} provider call targets"
    )
    if (
        len(terminals) != 2
        or terminals[0]
        != {
            "scope": "accepted_family_expectations",
            "family_ids": [family_id for family_id, _ in DERIVED_EXPECTATION_FAMILIES],
            "state": "closed",
        }
        or terminals[1]
        != {
            "family_id": "common.call_graph",
            **_mapping(
                provider_targets["coverage_terminal"], f"{context} call-target coverage"
            ),
        }
    ):
        raise SuccessorEvidenceError(f"{context} derived coverage closure differs")

    expected_rows = _derived_call_graph_rows(
        provider_targets,
        inputs["canonical_call_occurrences"],
        inputs["canonical_callable_lookup"],
        context,
        require_complete=True,
    )
    if (
        decoded["terminal"] != "pass"
        or decoded["relation"] != "analysis.common_call_graph.v1"
        or decoded["columns"]
        != [
            "call_site_id",
            "caller_id",
            "callee_id",
            "resolution_state",
            "target_ordinal",
            "provenance",
        ]
        or decoded["rows"] != expected_rows
    ):
        raise SuccessorEvidenceError(
            f"{context} decoded call graph is not derived from typed provider inputs"
        )


def _validate_derived_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    """Validate only the successor-native concrete analysis expectation.

    The predecessor producer-count/remainder schema was intentionally removed: complete family
    coverage remains metadata, while acceptance is grounded in decoded typed behavior.
    """
    _validate_derived_expectation_contract(inputs, decoded, context)


QUERY_STATE_VOCABULARY = {
    "execution_state": frozenset(
        {
            "COMPLETE",
            "FAILED",
            "CANCELLED",
            "DEADLINE_EXCEEDED",
            "NOT_EXECUTED_DEPENDENCY",
        }
    ),
    "availability_state": frozenset(
        {"AVAILABLE", "PARTIAL", "UNAVAILABLE", "NOT_APPLICABLE"}
    ),
    "completeness_state": frozenset(
        {"COMPLETE", "PARTIAL", "INDETERMINATE", "UNAVAILABLE", "NOT_APPLICABLE"}
    ),
    "freshness_state": frozenset({"CURRENT", "STALE", "UNKNOWN"}),
    "limit_state": frozenset(
        {"NOT_APPLIED", "EXPLICIT_LIMIT_REACHED", "HARD_LIMIT_REJECTED"}
    ),
    "dependency_state": frozenset(
        {"READY", "SATISFIED", "FAILED_DEPENDENCY", "NOT_EXECUTED"}
    ),
}
CANONICAL_ID = re.compile(r"(?:[a-z][a-z0-9-]*:)+[0-9a-f]{32}\Z")
PUBLIC_SNAPSHOT_PROJECTION_KEYS = {
    "snapshot_id",
    "workspace_id",
    "repository_id",
    "worktree_id",
    "source_generation",
    "source_inventory_digest",
    "durable_base_publication",
    "base_table_version_digest",
    "overlay_generation",
    "overlay_checksum",
    "analysis_context_set_id",
    "analysis_context_ids",
    "freshness_state",
    "source_trust_state",
    "event_stream_health",
    "git_acceleration_status",
    "git_operation_summary",
    "pending_update_count",
    "ontology_version",
    "schema_bundle_version",
    "provider_bundle_version",
    "derivation_bundle_version",
    "query_language_version",
    "capability_summaries",
    "diagnostic_references",
    "fabric_epoch_id",
    "application_release",
    "provider_release_vector",
    "programmatic_assembly_identity",
    "proof_receipt",
    "policy_identity",
    "exact_epoch_compatibility_class",
}
QUERY_RESPONSE_KEYS = {
    "specification",
    "version",
    "semantic_request_id",
    "execution_state",
    "availability_state",
    "completeness_state",
    "freshness_state",
    "limit_state",
    "successful_query_count",
    "failed_query_count",
    "not_executed_dependency_count",
    "snapshot",
    "entities",
    "facts",
    "paths",
    "groups",
    "source_contexts",
    "query_results",
    "errors",
}
QUERY_COLLECTION_ID_KEYS = {
    "entities": "entity_id",
    "facts": "fact_id",
    "paths": "path_id",
    "groups": "group_id",
    "source_contexts": "source_context_id",
}
QUERY_COLLECTION_RESULT_KEYS = {
    "entities": "entity_ids",
    "facts": "fact_ids",
    "paths": "path_ids",
    "groups": "group_ids",
    "source_contexts": "source_context_ids",
}
QUERY_RESULT_KEYS = {
    "query_id",
    "request",
    "execution_state",
    "availability_state",
    "completeness_state",
    "freshness_state",
    "limit_state",
    "dependency_state",
    "resolved_semantics",
    "result_role",
    "entity_ids",
    "fact_ids",
    "path_ids",
    "group_ids",
    "source_context_ids",
    "bindings",
    "coverage",
    "provenance",
    "errors",
    "notices",
}


def _validate_query_states(
    value: Mapping[str, Any], context: str, *, include_dependency: bool = True
) -> None:
    for key, allowed in QUERY_STATE_VOCABULARY.items():
        if key == "dependency_state" and not include_dependency:
            continue
        state = _nonempty_string(value.get(key), f"{context} {key}")
        if state not in allowed:
            raise SuccessorEvidenceError(
                f"{context} {key} is outside the released vocabulary: {state}"
            )


def _query_provenance_from_inputs(inputs: Mapping[str, Any]) -> dict[str, Any]:
    epoch = _mapping(inputs["pinned_epoch"], "query provenance epoch")
    binding = _mapping(inputs["program_binding"], "query provenance binding")
    return {
        "epoch_id": epoch["fabric_epoch_id"],
        "snapshot_id": epoch["snapshot_id"],
        "query_program_release": binding["query_program_release"],
        "producer_closure_id": binding["producer_closure_id"],
        "policy_release": epoch["policy_release"],
        "expectation_issuance": epoch["expectation_issuance"],
    }


def _derive_combine_producer_results(
    *,
    producer_blocks: Sequence[Mapping[str, Any]],
    relations: Mapping[str, Any],
    provenance: Mapping[str, Any],
    admitted_entities: Mapping[str, Any],
    context: str,
) -> dict[str, dict[str, Any]]:
    if "producer_results" in relations:
        raise SuccessorEvidenceError(
            f"{context} preauthored producer output is circular authority"
        )
    producer_inputs = _mapping(
        relations.get("producer_inputs"), f"{context} producer base inputs"
    )
    producer_ids = [str(block["query_id"]) for block in producer_blocks]
    if set(producer_inputs) != set(producer_ids):
        raise SuccessorEvidenceError(
            f"{context} combine dependency has a dangling result reference because its base producer input is absent"
        )
    results: dict[str, dict[str, Any]] = {}
    for block in producer_blocks:
        producer_id = str(block["query_id"])
        source = _mapping(
            producer_inputs[producer_id], f"{context} producer input {producer_id}"
        )
        _strict_keys(
            source,
            {
                "relation_id",
                "workspace_id",
                "analysis_context_id",
                "representation_layer",
                "certainty_class",
                "semantic_role",
                "rows",
            },
            f"{context} producer input {producer_id}",
        )
        relation_id = _nonempty_string(
            source["relation_id"], f"{context} producer input relation"
        )
        expected_relation = f"input.query.entities.{producer_id}"
        if (
            relation_id != expected_relation
            or relation_id.startswith("query.prior_result.")
            or block.get("looking_for") != "function declarations"
            or block.get("within") != [source["workspace_id"]]
            or block.get("where") != [{"relation": relation_id, "predicate": "member"}]
        ):
            raise SuccessorEvidenceError(
                f"{context} producer block does not read its independent base relation"
            )
        entity_ids = _string_list(
            source["rows"], f"{context} producer input rows {producer_id}"
        )
        if not set(entity_ids) <= set(admitted_entities):
            raise SuccessorEvidenceError(
                f"{context} producer input references an unadmitted entity"
            )
        compatibility = {
            key: source[key]
            for key in (
                "workspace_id",
                "analysis_context_id",
                "representation_layer",
                "certainty_class",
                "semantic_role",
            )
        }
        results[producer_id] = {
            "query_id": producer_id,
            "request": "find code entities",
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "looking_for": "function declarations",
                "producer_input_relation": relation_id,
                "compatibility_dimensions": compatibility,
            },
            "result_role": "entities",
            "entity_ids": sorted(entity_ids),
            "fact_ids": [],
            "path_ids": [],
            "group_ids": [],
            "source_context_ids": [],
            "bindings": [],
            "coverage": {
                "state": "COMPLETE",
                "producer_query_id": producer_id,
                "producer_input_relation": relation_id,
                "completed_entities": len(entity_ids),
            },
            "provenance": dict(provenance),
            "errors": [],
            "notices": [],
        }
    return results


def _public_query_entity(row: Mapping[str, Any]) -> dict[str, Any]:
    record = copy.deepcopy(dict(row))
    record.pop("alias", None)
    return record


def _validate_query_child_catalog(
    inputs: Mapping[str, Any], context: str
) -> Mapping[str, Any]:
    child = _mapping(
        inputs["authorized_child_catalog"], f"{context} authorized child catalog"
    )
    _strict_keys(
        child,
        {
            "catalog_id",
            "workspace_id",
            "recursive_dependency_closure",
            "parent_catalog_visible",
            "visible_relations",
            "visible_functions",
            "visible_object_stores",
        },
        f"{context} authorized child catalog",
    )
    workspace_id = str(inputs["request_envelope"]["decoded"]["scope"]["workspace_id"])
    _nonempty_string(child["catalog_id"], f"{context} child catalog identity")
    for key in ("visible_relations", "visible_functions", "visible_object_stores"):
        values = _list(child[key], f"{context} child catalog {key}")
        if any(not isinstance(value, str) or not value for value in values):
            raise SuccessorEvidenceError(
                f"{context} child catalog {key} contains a non-string identity"
            )
    if (
        child["workspace_id"] != workspace_id
        or child["recursive_dependency_closure"] != "complete"
        or child["parent_catalog_visible"] is not False
    ):
        raise SuccessorEvidenceError(
            f"{context} child catalog is not workspace-bound, closed, and reduced"
        )
    return child


def _validate_find_entity_authority(
    inputs: Mapping[str, Any], block: Mapping[str, Any], context: str
) -> tuple[list[Mapping[str, Any]], Mapping[str, Any]]:
    relations = _mapping(inputs["admitted_relations"], f"{context} admitted relations")
    _strict_keys(
        relations,
        {"entity_rows", "entity_dictionary"},
        f"{context} find-entity admitted relations",
    )
    rows = [
        _mapping(value, f"{context} admitted entity row {index}")
        for index, value in enumerate(
            _list(relations["entity_rows"], f"{context} admitted entity rows"), 1
        )
    ]
    if not rows:
        raise SuccessorEvidenceError(f"{context} admitted entity rows are empty")
    workspace_id = str(inputs["request_envelope"]["decoded"]["scope"]["workspace_id"])
    row_ids: list[str] = []
    aliases: list[str] = []
    for row in rows:
        entity_id = _nonempty_string(row.get("entity_id"), f"{context} entity row id")
        alias = _nonempty_string(row.get("alias"), f"{context} private entity alias")
        if (
            CANONICAL_ID.fullmatch(entity_id) is None
            or row.get("workspace_id") != workspace_id
            or not {
                "semantic_kind",
                "representation",
                "analysis_context_id",
                "certainty",
                "resolution",
                "direct_provenance",
            }
            <= set(row)
        ):
            raise SuccessorEvidenceError(
                f"{context} admitted find-entity row is not ontology-closed"
            )
        row_ids.append(entity_id)
        aliases.append(alias)
    if len(set(row_ids)) != len(row_ids) or len(set(aliases)) != len(aliases):
        raise SuccessorEvidenceError(
            f"{context} admitted find-entity rows have duplicate identities or aliases"
        )
    dictionary = _mapping(
        relations["entity_dictionary"], f"{context} admitted entity dictionary"
    )
    expected_dictionary = {
        str(row["entity_id"]): _public_query_entity(row) for row in rows
    }
    if dictionary != expected_dictionary:
        raise SuccessorEvidenceError(
            f"{context} admitted find-entity dictionary does not close all candidate rows"
        )

    binding = _mapping(inputs["program_binding"], f"{context} program binding")
    _strict_keys(
        binding,
        {
            "binding_relation_id",
            "description_resolution",
            "form",
            "live_capability_state",
            "phrase_resolution_relation_id",
            "producer_closure_id",
            "projection_policy_id",
            "query_program_release",
            "result_role",
        },
        f"{context} find-entity program binding",
    )
    resolution = _mapping(
        binding["description_resolution"], f"{context} description resolution"
    )
    if (
        binding["form"] != block.get("request")
        or binding["live_capability_state"] != "available"
        or binding["result_role"] != "entities"
        or binding["binding_relation_id"] != "query.binding"
        or binding["phrase_resolution_relation_id"] != "query.phrase_resolution"
        or binding["projection_policy_id"] != "projection:fixture-v1"
        or set(resolution) != {"function", "function syntax"}
        or not all(isinstance(value, str) and value for value in resolution.values())
    ):
        raise SuccessorEvidenceError(f"{context} find-entity program authority differs")
    _nonempty_string(
        binding["query_program_release"], f"{context} query program release"
    )
    _nonempty_string(
        binding["producer_closure_id"], f"{context} producer closure identity"
    )
    child = _validate_query_child_catalog(inputs, context)
    if (
        set(child["visible_relations"]) != {"canonical.entity", "proof.entity_coverage"}
        or child["visible_functions"] != []
        or child["visible_object_stores"] != []
    ):
        raise SuccessorEvidenceError(
            f"{context} find-entity child catalog exposes the wrong authority"
        )
    access = _mapping(inputs["access_scope"], f"{context} find-entity access scope")
    _strict_keys(
        access,
        {"workspace_id", "fact_access", "source_access"},
        f"{context} find-entity access scope",
    )
    if access != {
        "workspace_id": workspace_id,
        "fact_access": True,
        "source_access": False,
    }:
        raise SuccessorEvidenceError(f"{context} find-entity access scope differs")
    coverage = _mapping(
        inputs["producer_coverage"], f"{context} find-entity producer coverage"
    )
    _strict_keys(
        coverage,
        {"state", "family", "scope", "covered_entity_ids"},
        f"{context} find-entity producer coverage",
    )
    if (
        coverage["state"] != "COMPLETE"
        or coverage["family"] != "entity_kind"
        or coverage["scope"] != workspace_id
        or set(_string_list(coverage["covered_entity_ids"], context)) != set(row_ids)
    ):
        raise SuccessorEvidenceError(
            f"{context} find-entity coverage does not close admitted rows"
        )
    limits = _mapping(inputs["resource_limits"], f"{context} resource limits")
    _strict_keys(
        limits,
        {"deadline_ms", "max_bytes", "max_rows"},
        f"{context} find-entity resource limits",
    )
    maximum_results = int(block["return"]["limit"]["maximum_results"])
    if (
        limits["deadline_ms"]
        != inputs["request_envelope"]["decoded"]["freshness"]["deadline_ms"]
        or _positive_int(limits["max_rows"], context) < maximum_results
        or _positive_int(limits["max_bytes"], context)
        < len(rfc8785.dumps([_public_query_entity(row) for row in rows]))
        or block.get("where") != []
        or block.get("within") != [workspace_id]
    ):
        raise SuccessorEvidenceError(
            f"{context} find-entity bounds or selectors differ"
        )
    return rows, resolution


def _derive_find_entity_selection(
    inputs: Mapping[str, Any], block: Mapping[str, Any], context: str
) -> tuple[list[dict[str, Any]], dict[str, Any], dict[str, Any]]:
    rows, resolution = _validate_find_entity_authority(inputs, block, context)
    looking_for = str(block["looking_for"])
    semantic_kind = resolution.get(looking_for)
    if semantic_kind is None:
        raise SuccessorEvidenceError(
            f"{context} requested description is outside the typed query program"
        )
    selected = [
        _public_query_entity(row)
        for row in rows
        if row.get("semantic_kind") == semantic_kind
        and row.get("workspace_id") in block["within"]
    ]
    selected.sort(
        key=lambda row: (
            row.get("source_reference", {}).get("byte_safe_path", ""),
            row.get("source_reference", {}).get("start_byte", -1),
            row["semantic_kind"],
            row.get("qualified_name", ""),
            row["entity_id"],
        )
    )
    representations = {row["representation"] for row in selected}
    if not selected or len(representations) != 1:
        raise SuccessorEvidenceError(
            f"{context} typed entity selection is empty or representation-ambiguous"
        )
    maximum = int(block["return"]["limit"]["maximum_results"])
    if len(selected) > maximum:
        raise SuccessorEvidenceError(
            f"{context} fixture unexpectedly exceeds the explicit entity bound"
        )
    coverage = _mapping(inputs["producer_coverage"], f"{context} producer coverage")
    return (
        selected,
        {
            "looking_for": looking_for,
            "representation": representations.pop(),
            "semantic_kind": semantic_kind,
        },
        {
            "state": coverage["state"],
            "family": coverage["family"],
            "scope": coverage["scope"],
            "completed_inputs": len(coverage["covered_entity_ids"]),
        },
    )


def _derive_follow_edges(
    inputs: Mapping[str, Any], block: Mapping[str, Any], context: str
) -> list[Mapping[str, Any]]:
    relations = _mapping(inputs["admitted_relations"], f"{context} admitted relations")
    _strict_keys(
        relations,
        {"call_edges", "entity_dictionary"},
        f"{context} admitted follow relations",
    )
    edges = [
        _mapping(value, f"{context} admitted relationship edge {index}")
        for index, value in enumerate(
            _list(relations["call_edges"], f"{context} admitted call edges"), 1
        )
    ]
    dictionary = _mapping(
        relations["entity_dictionary"], f"{context} admitted entity dictionary"
    )
    workspace_id = str(inputs["request_envelope"]["decoded"]["scope"]["workspace_id"])
    for entity_id, record_value in dictionary.items():
        record = _mapping(record_value, f"{context} follow entity record")
        if (
            CANONICAL_ID.fullmatch(str(entity_id)) is None
            or record.get("entity_id") != entity_id
            or record.get("workspace_id") != workspace_id
        ):
            raise SuccessorEvidenceError(
                f"{context} follow entity dictionary is not identity/workspace closed"
            )
    selected: list[Mapping[str, Any]] = []
    for edge in edges:
        statement = _mapping(edge.get("statement"), f"{context} call statement")
        if (
            edge.get("fact_form") != "relationship"
            or edge.get("fact_kind") != block.get("relationship")
            or statement.get("predicate") != block.get("relationship")
            or statement.get("subject") not in dictionary
            or statement.get("object") not in dictionary
        ):
            raise SuccessorEvidenceError(
                f"{context} admitted relationship edge is not ontology-closed"
            )
        if statement["subject"] in block["starting_from"]:
            selected.append(edge)
    selected.sort(
        key=lambda edge: (
            str(edge["statement"]["object"]),
            str(edge["fact_id"]),
        )
    )
    return selected


def _canonical_shortest_query_witness(
    edges: Sequence[Mapping[str, Any]],
    *,
    start: str,
    target: str,
    families: set[str],
    maximum_length: int,
    context: str,
) -> tuple[list[str], list[str]]:
    adjacency: dict[str, list[tuple[str, str]]] = {}
    for edge in edges:
        statement = _mapping(edge.get("statement"), f"{context} path edge statement")
        if statement.get("predicate") in families:
            adjacency.setdefault(str(statement["subject"]), []).append(
                (str(statement["object"]), str(edge["fact_id"]))
            )
    for values in adjacency.values():
        values.sort()
    frontier: list[tuple[list[str], list[str]]] = [([start], [])]
    for _ in range(maximum_length + 1):
        matches = [value for value in frontier if value[0][-1] == target]
        if matches:
            matches.sort(key=lambda value: (value[1], value[0]))
            return matches[0]
        next_frontier: list[tuple[list[str], list[str]]] = []
        for entity_ids, fact_ids in frontier:
            for successor, fact_id in adjacency.get(entity_ids[-1], []):
                if successor not in entity_ids:
                    next_frontier.append(
                        ([*entity_ids, successor], [*fact_ids, fact_id])
                    )
        frontier = next_frontier
        if not frontier:
            break
    raise SuccessorEvidenceError(
        f"{context} bounded admitted graph has no connecting witness"
    )


def _validate_path_authority(
    inputs: Mapping[str, Any], block: Mapping[str, Any], context: str
) -> tuple[list[Mapping[str, Any]], Mapping[str, Any], Mapping[str, Any]]:
    relations = _mapping(inputs["admitted_relations"], f"{context} admitted relations")
    _strict_keys(
        relations,
        {"edges", "entity_dictionary"},
        f"{context} path admitted relations",
    )
    dictionary = _mapping(
        relations["entity_dictionary"], f"{context} path entity dictionary"
    )
    edges = [
        _mapping(value, f"{context} path edge {index}")
        for index, value in enumerate(
            _list(relations["edges"], f"{context} path edges"), 1
        )
    ]
    fact_ids: list[str] = []
    analysis_contexts: set[str] = set()
    families = set(_string_list(block["using"], f"{context} path families"))
    for edge in edges:
        statement = _mapping(edge.get("statement"), f"{context} path statement")
        fact_id = _nonempty_string(edge.get("fact_id"), f"{context} path fact id")
        if (
            CANONICAL_ID.fullmatch(fact_id) is None
            or edge.get("fact_form") != "relationship"
            or edge.get("fact_kind") not in families
            or statement.get("predicate") not in families
            or statement.get("subject") not in dictionary
            or statement.get("object") not in dictionary
        ):
            raise SuccessorEvidenceError(f"{context} path edge is not ontology-closed")
        fact_ids.append(fact_id)
        analysis_contexts.add(str(edge.get("analysis_context_id")))
    if len(set(fact_ids)) != len(fact_ids) or len(analysis_contexts) != 1:
        raise SuccessorEvidenceError(
            f"{context} path graph duplicates facts or crosses analysis contexts"
        )
    binding = _mapping(inputs["program_binding"], f"{context} path program binding")
    _strict_keys(
        binding,
        {
            "binding_relation_id",
            "form",
            "live_capability_state",
            "path_policy",
            "phrase_resolution_relation_id",
            "producer_closure_id",
            "projection_policy_id",
            "query_program_release",
            "result_role",
        },
        f"{context} path program binding",
    )
    if (
        binding["form"] != block.get("request")
        or binding["live_capability_state"] != "available"
        or binding["path_policy"] != "shortest"
        or binding["result_role"] != "paths"
        or binding["binding_relation_id"] != "query.binding"
        or binding["phrase_resolution_relation_id"] != "query.phrase_resolution"
        or binding["projection_policy_id"] != "projection:fixture-v1"
    ):
        raise SuccessorEvidenceError(f"{context} path program authority differs")
    _nonempty_string(
        binding["query_program_release"], f"{context} path query program release"
    )
    _nonempty_string(
        binding["producer_closure_id"], f"{context} path producer closure identity"
    )
    access = _mapping(inputs["access_scope"], f"{context} path access scope")
    _strict_keys(
        access,
        {"workspace_id", "relationship_families"},
        f"{context} path access scope",
    )
    child = _validate_query_child_catalog(inputs, context)
    if (
        access["workspace_id"]
        != inputs["request_envelope"]["decoded"]["scope"]["workspace_id"]
        or set(access["relationship_families"]) != families
        or set(child.get("visible_relations", []))
        != {"canonical.call_fact", "proof.call_graph_coverage"}
        or child.get("visible_functions") != []
        or child.get("visible_object_stores") != []
    ):
        raise SuccessorEvidenceError(f"{context} path access/catalog authority differs")
    coverage = _mapping(inputs["producer_coverage"], f"{context} path coverage")
    _strict_keys(
        coverage,
        {"state", "family", "analysis_context_id", "entity_ids", "fact_ids"},
        f"{context} path coverage",
    )
    if (
        coverage["state"] != "COMPLETE"
        or {coverage["family"]} != families
        or coverage["analysis_context_id"] not in analysis_contexts
        or set(coverage["entity_ids"]) != set(dictionary)
        or set(coverage["fact_ids"]) != set(fact_ids)
    ):
        raise SuccessorEvidenceError(
            f"{context} path coverage does not close the admitted graph"
        )
    limits = _mapping(inputs["resource_limits"], f"{context} path resource limits")
    _strict_keys(
        limits,
        {"deadline_ms", "max_path_length", "max_paths"},
        f"{context} path resource limits",
    )
    if (
        limits["deadline_ms"]
        != inputs["request_envelope"]["decoded"]["freshness"]["deadline_ms"]
        or _positive_int(limits["max_path_length"], context) < 1
        or limits["max_paths"] != block["return"]["limit"]["maximum_results"]
        or block.get("where") != []
    ):
        raise SuccessorEvidenceError(f"{context} path bounds differ")
    return edges, dictionary, coverage


def _derive_typed_pattern_matches(
    inputs: Mapping[str, Any], block: Mapping[str, Any], context: str
) -> tuple[
    list[dict[str, Any]],
    list[str],
    list[str],
    dict[str, Any],
    Mapping[str, Any],
]:
    pattern = _mapping(block["pattern"], f"{context} typed pattern")
    _strict_keys(
        pattern,
        {"nodes", "facts", "alternatives", "scoped_negation"},
        f"{context} typed pattern",
    )
    nodes = [
        _mapping(value, f"{context} pattern node {index}")
        for index, value in enumerate(_list(pattern["nodes"], context), 1)
    ]
    facts = [
        _mapping(value, f"{context} pattern fact {index}")
        for index, value in enumerate(_list(pattern["facts"], context), 1)
    ]
    negations = [
        _mapping(value, f"{context} pattern negation {index}")
        for index, value in enumerate(_list(pattern["scoped_negation"], context), 1)
    ]
    if (
        len(nodes) != 2
        or len(facts) != 1
        or len(negations) != 1
        or pattern["alternatives"] != []
    ):
        raise SuccessorEvidenceError(
            f"{context} pattern does not exercise nodes, a positive fact, and scoped negation"
        )
    variables: set[str] = set()
    for node in nodes:
        _strict_keys(
            node,
            {"binding", "module_id", "name", "semantic_kind"},
            f"{context} pattern node",
        )
        variables.add(str(node["binding"]))
    fact = facts[0]
    _strict_keys(
        fact,
        {"subject_binding", "relationship", "object_binding", "direction"},
        f"{context} pattern fact",
    )
    negation = negations[0]
    _strict_keys(
        negation,
        {
            "subject_binding",
            "relationship",
            "direction",
            "owner_scope",
            "analysis_context_id",
            "required_coverage",
        },
        f"{context} pattern negation",
    )
    if (
        {fact["subject_binding"], fact["object_binding"]} != variables
        or fact["direction"] != "outgoing"
        or negation["subject_binding"] not in variables
        or negation["direction"] != "outgoing"
        or negation["required_coverage"] != "COMPLETE"
    ):
        raise SuccessorEvidenceError(f"{context} typed pattern bindings differ")

    binding = _mapping(inputs["program_binding"], f"{context} pattern program")
    _strict_keys(
        binding,
        {
            "binding_relation_id",
            "form",
            "live_capability_state",
            "negation_policy",
            "pattern_contract",
            "phrase_resolution_relation_id",
            "producer_closure_id",
            "projection_policy_id",
            "query_program_release",
            "result_role",
        },
        f"{context} pattern program",
    )
    if (
        binding["form"] != block.get("request")
        or binding["live_capability_state"] != "available"
        or binding["result_role"] != "pattern_bindings"
        or binding["negation_policy"] != "requires complete scoped family"
        or binding["pattern_contract"] != pattern
        or binding["binding_relation_id"] != "query.binding"
        or binding["phrase_resolution_relation_id"] != "query.phrase_resolution"
        or binding["projection_policy_id"] != "projection:fixture-v1"
    ):
        raise SuccessorEvidenceError(f"{context} pattern program authority differs")
    _nonempty_string(
        binding["query_program_release"], f"{context} pattern query program release"
    )
    _nonempty_string(
        binding["producer_closure_id"],
        f"{context} pattern producer closure identity",
    )
    access = _mapping(inputs["access_scope"], f"{context} pattern access")
    _strict_keys(
        access,
        {"workspace_id", "modules", "relationship_families"},
        f"{context} pattern access",
    )
    modules = {str(node["module_id"]) for node in nodes}
    families = {str(fact["relationship"]), str(negation["relationship"])}
    child = _validate_query_child_catalog(inputs, context)
    if (
        access["workspace_id"]
        != inputs["request_envelope"]["decoded"]["scope"]["workspace_id"]
        or set(access["modules"]) != modules
        or set(access["relationship_families"]) != families
        or set(child.get("visible_relations", []))
        != {"canonical.entity", "canonical.call_fact", "proof.negative_universe"}
        or child.get("visible_functions") != []
        or child.get("visible_object_stores") != []
    ):
        raise SuccessorEvidenceError(
            f"{context} pattern access/catalog authority differs"
        )
    relations = _mapping(inputs["admitted_relations"], f"{context} pattern relations")
    _strict_keys(
        relations,
        {"entities", "call_edges", "entity_dictionary"},
        f"{context} pattern admitted relations",
    )
    entity_relation = _mapping(relations["entities"], f"{context} entity relation")
    _strict_keys(
        entity_relation,
        {"relation", "rows"},
        f"{context} entity relation",
    )
    rows = [
        _mapping(value, f"{context} pattern entity row {index}")
        for index, value in enumerate(_list(entity_relation["rows"], context), 1)
    ]
    dictionary = _mapping(
        relations["entity_dictionary"], f"{context} pattern entity dictionary"
    )
    if dictionary != {str(row["entity_id"]): _public_query_entity(row) for row in rows}:
        raise SuccessorEvidenceError(
            f"{context} pattern entity dictionary is not derived from admitted rows"
        )
    edges = [
        _mapping(value, f"{context} pattern edge {index}")
        for index, value in enumerate(_list(relations["call_edges"], context), 1)
    ]
    for edge in edges:
        statement = _mapping(edge.get("statement"), f"{context} pattern statement")
        if (
            edge.get("fact_form") != "relationship"
            or edge.get("fact_kind") not in families
            or statement.get("predicate") not in families
            or statement.get("subject") not in dictionary
            or statement.get("object") not in dictionary
        ):
            raise SuccessorEvidenceError(
                f"{context} pattern edge is not ontology-closed"
            )
    coverage = _mapping(inputs["producer_coverage"], f"{context} pattern coverage")
    expected_coverage_keys = {
        "state",
        "family",
        "analysis_context_id",
        "covered_subject_ids",
        "covered_fact_ids",
        "owner_scope",
        "negative_proof_universe_id",
    }
    if coverage.get("state") == "PARTIAL":
        expected_coverage_keys.add("remainders")
    _strict_keys(coverage, expected_coverage_keys, f"{context} pattern coverage")
    covered_subjects = {
        str(row["entity_id"])
        for row in rows
        if row.get("module_id") == negation["owner_scope"]
        and row.get("analysis_context_id") == negation["analysis_context_id"]
        and row.get("semantic_kind") == "function_declaration"
    }
    covered_facts = {
        str(edge["fact_id"])
        for edge in edges
        if edge.get("analysis_context_id") == negation["analysis_context_id"]
        and edge.get("statement", {}).get("subject") in covered_subjects
        and edge.get("statement", {}).get("predicate") == negation["relationship"]
    }
    _nonempty_string(
        coverage["negative_proof_universe_id"],
        f"{context} negative-proof universe identity",
    )
    if (
        coverage["family"] != negation["relationship"]
        or coverage["analysis_context_id"] != negation["analysis_context_id"]
        or coverage["owner_scope"] != negation["owner_scope"]
        or set(coverage["covered_subject_ids"]) != covered_subjects
        or set(coverage["covered_fact_ids"]) != covered_facts
    ):
        raise SuccessorEvidenceError(
            f"{context} pattern coverage does not close owner/family/context facts"
        )
    if coverage["state"] == "PARTIAL":
        remainders = _list(coverage["remainders"], f"{context} pattern remainders")
        if len(remainders) != 1:
            raise SuccessorEvidenceError(
                f"{context} partial pattern coverage lacks one explicit remainder"
            )
    elif coverage["state"] != "COMPLETE":
        raise SuccessorEvidenceError(f"{context} pattern coverage state differs")
    limits = _mapping(inputs["resource_limits"], f"{context} pattern limits")
    _strict_keys(
        limits,
        {"deadline_ms", "max_bindings", "max_pattern_nodes"},
        f"{context} pattern limits",
    )
    if (
        limits["deadline_ms"]
        != inputs["request_envelope"]["decoded"]["freshness"]["deadline_ms"]
        or limits["max_pattern_nodes"] < len(nodes)
        or limits["max_bindings"] < block["return"]["limit"]["maximum_results"]
        or block.get("where") != []
    ):
        raise SuccessorEvidenceError(f"{context} pattern resource bounds differ")

    candidates: dict[str, list[Mapping[str, Any]]] = {}
    for node in nodes:
        candidates[str(node["binding"])] = [
            row
            for row in rows
            if row.get("module_id") == node["module_id"]
            and row.get("semantic_kind") == node["semantic_kind"]
            and str(row.get("qualified_name", "")).rsplit(".", 1)[-1] == node["name"]
        ]
    assignments: list[dict[str, Mapping[str, Any]]] = [{}]
    for variable in sorted(candidates):
        assignments = [
            {**assignment, variable: candidate}
            for assignment in assignments
            for candidate in candidates[variable]
        ]
    assignments = [
        assignment
        for assignment in assignments
        if any(
            edge.get("statement")
            == {
                "subject": assignment[str(fact["subject_binding"])]["entity_id"],
                "predicate": fact["relationship"],
                "object": assignment[str(fact["object_binding"])]["entity_id"],
            }
            for edge in edges
        )
    ]
    indeterminate = coverage["state"] == "PARTIAL"
    matches: list[dict[str, Any]] = []
    for assignment in assignments:
        subject_id = str(assignment[str(negation["subject_binding"])]["entity_id"])
        has_outgoing = any(
            edge.get("analysis_context_id") == negation["analysis_context_id"]
            and edge.get("statement", {}).get("subject") == subject_id
            and edge.get("statement", {}).get("predicate") == negation["relationship"]
            for edge in edges
        )
        if has_outgoing and not indeterminate:
            continue
        support_ids = sorted(
            str(edge["fact_id"])
            for edge in edges
            if edge.get("statement")
            == {
                "subject": assignment[str(fact["subject_binding"])]["entity_id"],
                "predicate": fact["relationship"],
                "object": assignment[str(fact["object_binding"])]["entity_id"],
            }
        )
        matches.append(
            {
                "matched_branch": "primary",
                "binding_state": "INDETERMINATE" if indeterminate else "MATCH",
                "bindings": {
                    variable: {
                        "binding_type": "entity:function",
                        "entity_id": record["entity_id"],
                        "semantic_kind": record["semantic_kind"],
                    }
                    for variable, record in sorted(assignment.items())
                },
                "supporting_fact_ids": support_ids,
                "scoped_negation": [
                    {
                        "subject_binding": negation["subject_binding"],
                        "subject_entity_id": subject_id,
                        "relationship": negation["relationship"],
                        "direction": negation["direction"],
                        "owner_scope": negation["owner_scope"],
                        "analysis_context_id": negation["analysis_context_id"],
                        "coverage_witness": coverage["negative_proof_universe_id"],
                        "state": (
                            "INDETERMINATE" if indeterminate else "PROVED_ABSENT"
                        ),
                    }
                ],
            }
        )
    matches.sort(
        key=lambda result: tuple(
            value["entity_id"] for value in result["bindings"].values()
        )
    )
    if len(matches) > limits["max_bindings"]:
        raise SuccessorEvidenceError(f"{context} pattern exceeds its explicit bound")
    entity_ids = sorted(
        {
            str(value["entity_id"])
            for match in matches
            for value in match["bindings"].values()
        }
    )
    fact_ids = sorted(
        {str(fact_id) for match in matches for fact_id in match["supporting_fact_ids"]}
    )
    result_coverage = {
        "state": coverage["state"],
        "owner_scope": coverage["owner_scope"],
        "analysis_context_id": coverage["analysis_context_id"],
        "family": coverage["family"],
        "covered_subject_ids": coverage["covered_subject_ids"],
        "covered_fact_ids": coverage["covered_fact_ids"],
        "negative_proof_universe_id": coverage["negative_proof_universe_id"],
    }
    if indeterminate:
        result_coverage["remainders"] = coverage["remainders"]
    else:
        result_coverage["outcome"] = "MATCH" if matches else "NO_MATCH_AFTER_FILTERS"
    return matches, entity_ids, fact_ids, result_coverage, dictionary


def _validate_query_inputs(
    family: str,
    inputs: Mapping[str, Any],
    decoded: Mapping[str, Any],
    context: str,
) -> None:
    request_value = _mapping(
        inputs["request_envelope"], f"{context} encoded request envelope"
    )
    _strict_keys(
        request_value,
        {"decoded", "canonical_json"},
        f"{context} encoded request envelope",
    )
    envelope = _mapping(request_value["decoded"], f"{context} request envelope")
    _strict_keys(
        envelope,
        {
            "specification",
            "version",
            "semantic_request_id",
            "scope",
            "freshness",
            "defaults",
            "queries",
        },
        f"{context} request envelope",
    )
    if (
        envelope["specification"] != "composable semantic CPG fact query"
        or envelope["version"] != "2.0"
    ):
        raise SuccessorEvidenceError(f"{context} negotiated query profile differs")
    _nonempty_string(envelope["semantic_request_id"], f"{context} semantic request id")
    scope = _mapping(envelope["scope"], f"{context} scope")
    _strict_keys(
        scope,
        {
            "workspace_id",
            "codebase",
            "languages",
            "source_boundaries",
            "analysis_contexts",
            "representations",
            "external_entities",
        },
        f"{context} scope",
    )
    if CANONICAL_ID.fullmatch(str(scope["workspace_id"])) is None:
        raise SuccessorEvidenceError(f"{context} workspace identity is not canonical")
    freshness = _mapping(envelope["freshness"], f"{context} freshness")
    _strict_keys(
        freshness,
        {"policy", "target_scope", "deadline_ms"},
        f"{context} freshness",
    )
    if freshness["policy"] != "require_current_for_targets":
        raise SuccessorEvidenceError(f"{context} freshness policy differs")
    queries = _list(envelope["queries"], f"{context} queries")
    if family == "query_combine_results":
        if len(queries) != 3:
            raise SuccessorEvidenceError(
                f"{context} combine request must contain two producers and one consumer"
            )
    elif len(queries) != 1:
        raise SuccessorEvidenceError(f"{context} must contain one fixture query block")
    blocks = [
        _mapping(value, f"{context} query block {index}")
        for index, value in enumerate(queries, 1)
    ]
    block_ids = [
        _nonempty_string(value.get("query_id"), f"{context} query block identity")
        for value in blocks
    ]
    if len(set(block_ids)) != len(block_ids):
        raise SuccessorEvidenceError(f"{context} query block identities are not unique")
    if family == "query_combine_results":
        combine_blocks = [
            value for value in blocks if value.get("request") == "combine result sets"
        ]
        producer_blocks = [
            value for value in blocks if value.get("request") == "find code entities"
        ]
        if (
            len(combine_blocks) != 1
            or len(producer_blocks) != 2
            or blocks[-1] is not combine_blocks[0]
        ):
            raise SuccessorEvidenceError(
                f"{context} combine request is not in deterministic topological order"
            )
        block = combine_blocks[0]
    else:
        producer_blocks = []
        block = blocks[0]
    for index, candidate in enumerate(blocks, 1):
        contract_family = (
            "query_find_code_entities"
            if family == "query_combine_results"
            and candidate.get("request") == "find code entities"
            else family
        )
        _strict_keys(
            candidate,
            QUERY_REQUEST_KEY_CONTRACT[contract_family],
            f"{context} query block {index}",
        )
        returns = _mapping(
            candidate["return"], f"{context} query block {index} return contract"
        )
        _strict_keys(
            returns, RETURN_KEYS, f"{context} query block {index} return contract"
        )
        for key in ("include", "exclude", "group_by", "order_by"):
            if not isinstance(returns[key], list):
                raise SuccessorEvidenceError(
                    f"{context} query block {index} return {key} must be a list"
                )
        for key in ("result_shape", "deduplicate_by", "supporting_facts"):
            _nonempty_string(
                returns[key], f"{context} query block {index} return {key}"
            )
        _bool(
            returns["include_query_result"],
            f"{context} query block {index} include_query_result",
        )
        limit = _mapping(
            returns["limit"], f"{context} query block {index} return limit"
        )
        _strict_keys(
            limit,
            {"maximum_results", "per", "when_exceeded"},
            f"{context} query block {index} return limit",
        )
        _positive_int(
            limit["maximum_results"], f"{context} query block {index} return maximum"
        )
        if limit["when_exceeded"] != "EXPLICIT_LIMIT_REACHED":
            raise SuccessorEvidenceError(f"{context} explicit limit state differs")

    profile = _mapping(inputs["negotiated_profile"], f"{context} negotiated profile")
    _strict_keys(
        profile,
        {
            "profile_id",
            "wire_version",
            "schema_authority",
            "media_type",
        },
        f"{context} negotiated profile",
    )
    if (
        profile["profile_id"] != "composable-semantic-cpg-query@2.0"
        or profile["wire_version"] != "2.0"
        or profile["schema_authority"] != "QRY §2.1"
        or profile["media_type"] != "application/json"
    ):
        raise SuccessorEvidenceError(f"{context} negotiated profile is not exact")
    try:
        encoded = json.loads(
            str(request_value["canonical_json"]), object_pairs_hook=_reject_duplicates
        )
    except json.JSONDecodeError as error:
        raise SuccessorEvidenceError(
            f"{context} canonical request bytes are invalid"
        ) from error
    if encoded != envelope:
        raise SuccessorEvidenceError(
            f"{context} canonical request bytes differ from decoded envelope"
        )
    if request_value["canonical_json"] != rfc8785.dumps(envelope).decode("utf-8"):
        raise SuccessorEvidenceError(
            f"{context} request bytes are not exact canonical JSON"
        )
    epoch = _mapping(inputs["pinned_epoch"], f"{context} pinned epoch")
    _strict_keys(
        epoch,
        {
            "fabric_epoch_id",
            "snapshot_id",
            "source_generation",
            "table_version_vector",
            "program_release",
            "application_release",
            "provider_release_vector",
            "policy_release",
            "expectation_issuance",
            "public_snapshot_projection",
        },
        f"{context} pinned epoch",
    )
    public_snapshot = _mapping(
        epoch["public_snapshot_projection"], f"{context} public snapshot projection"
    )
    _strict_keys(
        public_snapshot,
        PUBLIC_SNAPSHOT_PROJECTION_KEYS,
        f"{context} public snapshot projection",
    )
    child = _validate_query_child_catalog(inputs, context)

    if decoded["columns"] != ["response_state"]:
        raise SuccessorEvidenceError(
            f"{context} query expectation is not decoded response state"
        )
    rows = _decoded_rows(decoded, f"{context} decoded expectation")
    if len(rows) != 1 or len(rows[0]) != 1:
        raise SuccessorEvidenceError(f"{context} requires one decoded response object")
    response = _mapping(rows[0][0], f"{context} response state")
    _strict_keys(response, QUERY_RESPONSE_KEYS, f"{context} response state")
    _validate_query_states(
        response, f"{context} response state", include_dependency=False
    )
    if (
        response["specification"] != "composable semantic CPG fact query response"
        or response["version"] != "2.0"
        or response["semantic_request_id"] != envelope["semantic_request_id"]
        or response["execution_state"] != "COMPLETE"
        or response["freshness_state"] != "CURRENT"
        or decoded["terminal"] != response["execution_state"]
    ):
        raise SuccessorEvidenceError(f"{context} response uses noncanonical state")
    if (
        response["successful_query_count"] != len(blocks)
        or response["failed_query_count"] != 0
        or response["not_executed_dependency_count"] != 0
    ):
        raise SuccessorEvidenceError(f"{context} response counters differ")
    snapshot = _mapping(response["snapshot"], f"{context} response snapshot")
    if snapshot != public_snapshot:
        raise SuccessorEvidenceError(f"{context} response snapshot differs from epoch")
    collections = {
        name: _mapping(response[name], f"{context} response {name}")
        for name in QUERY_COLLECTION_ID_KEYS
    }
    admitted_entities = _mapping(
        _mapping(inputs["admitted_relations"], f"{context} admitted relations")[
            "entity_dictionary"
        ],
        f"{context} admitted entity dictionary",
    )
    access = _mapping(inputs["access_scope"], f"{context} query access scope")
    workspace_id = str(scope["workspace_id"])
    access_workspace_id = access.get("workspace_id", access.get("workspace"))
    if (
        access_workspace_id != workspace_id
        or public_snapshot.get("workspace_id") != workspace_id
        or child.get("workspace_id") != workspace_id
    ):
        raise SuccessorEvidenceError(f"{context} query workspace binding differs")
    producer_coverage = _mapping(
        inputs["producer_coverage"], f"{context} producer coverage"
    )
    if "scope" in producer_coverage and producer_coverage.get("scope") != workspace_id:
        raise SuccessorEvidenceError(
            f"{context} producer coverage crosses workspace scope"
        )
    if any(
        not isinstance(entity, dict) or entity.get("workspace_id") != workspace_id
        for entity in admitted_entities.values()
    ):
        raise SuccessorEvidenceError(
            f"{context} admitted entity crosses workspace scope"
        )

    def require_admitted_entities(values: object, label: str) -> set[str]:
        selectors = {
            _nonempty_string(value, f"{context} {label} selector")
            for value in _list(values, f"{context} {label} selectors")
        }
        if not selectors <= set(admitted_entities):
            raise SuccessorEvidenceError(
                f"{context} {label} selector lacks an admitted identity binding"
            )
        return selectors

    if family == "query_find_code_entities":
        within = set(_list(block["within"], f"{context} within selectors"))
        if within != {workspace_id}:
            raise SuccessorEvidenceError(f"{context} within workspace binding differs")
    elif family == "query_retrieve_facts":
        about = require_admitted_entities(block["about"], "about")
        if about != set(access.get("subjects", [])):
            raise SuccessorEvidenceError(
                f"{context} about/access subject binding differs"
            )
    elif family == "query_follow_relationships":
        require_admitted_entities(block["starting_from"], "starting_from")
    elif family == "query_connecting_paths":
        require_admitted_entities(block["from"], "from")
        require_admitted_entities(block["to"], "to")
    elif family == "query_match_pattern":
        pattern = _mapping(block["pattern"], f"{context} pattern selector")
        nodes = _list(pattern["nodes"], f"{context} pattern nodes")
        negations = _list(pattern["scoped_negation"], f"{context} scoped negations")
        module_selectors = {
            str(node.get("module_id")) for node in nodes if isinstance(node, dict)
        } | {
            str(negation.get("owner_scope"))
            for negation in negations
            if isinstance(negation, dict)
        }
        if (
            not module_selectors
            or module_selectors != set(access.get("modules", []))
            or not module_selectors <= set(admitted_entities)
            or any(
                admitted_entities[module].get("semantic_kind") != "module"
                for module in module_selectors
            )
        ):
            raise SuccessorEvidenceError(
                f"{context} pattern module selector lacks an admitted identity binding"
            )
    elif family == "query_summarize_facts":
        if set(_list(block["about"], f"{context} summary selectors")) != {workspace_id}:
            raise SuccessorEvidenceError(f"{context} summary workspace binding differs")
    elif family == "query_source_context":
        require_admitted_entities(block["about"], "source about")
    if (
        family != "query_find_code_entities"
        and collections["entities"] != admitted_entities
    ):
        raise SuccessorEvidenceError(
            f"{context} response entity dictionary differs from admitted input"
        )
    _list(response["errors"], f"{context} response errors")
    results = _list(response["query_results"], f"{context} query results")
    if len(results) != len(blocks):
        raise SuccessorEvidenceError(
            f"{context} query-result count differs from the request DAG"
        )
    block_by_id = {str(value["query_id"]): value for value in blocks}
    result_by_id: dict[str, Mapping[str, Any]] = {}
    for index, result_value in enumerate(results, 1):
        candidate = _mapping(result_value, f"{context} query result {index}")
        _strict_keys(candidate, QUERY_RESULT_KEYS, f"{context} query result {index}")
        _validate_query_states(candidate, f"{context} query result {index}")
        query_id = _nonempty_string(
            candidate["query_id"], f"{context} query result {index} identity"
        )
        candidate_block = block_by_id.get(query_id)
        if query_id in result_by_id or candidate_block is None:
            raise SuccessorEvidenceError(
                f"{context} query result is duplicate or outside the request DAG"
            )
        if candidate["request"] != candidate_block["request"] or any(
            candidate[key] != response[key]
            for key in (
                "execution_state",
                "availability_state",
                "completeness_state",
                "freshness_state",
                "limit_state",
            )
        ):
            raise SuccessorEvidenceError(f"{context} query-result envelope differs")
        for key in (
            "entity_ids",
            "fact_ids",
            "path_ids",
            "group_ids",
            "source_context_ids",
            "bindings",
            "errors",
            "notices",
        ):
            _list(candidate[key], f"{context} query result {index} {key}")
        for collection_name in QUERY_COLLECTION_ID_KEYS:
            result_key = QUERY_COLLECTION_RESULT_KEYS[collection_name]
            for identity in candidate[result_key]:
                if CANONICAL_ID.fullmatch(str(identity)) is None:
                    raise SuccessorEvidenceError(
                        f"{context} has noncanonical public result id"
                    )
                if identity not in collections[collection_name]:
                    raise SuccessorEvidenceError(
                        f"{context} query result references missing {collection_name} record"
                    )
        result_by_id[query_id] = candidate
    result = result_by_id[str(block["query_id"])]
    for collection_name, identity_key in QUERY_COLLECTION_ID_KEYS.items():
        for identity, record_value in collections[collection_name].items():
            record = _mapping(record_value, f"{context} {collection_name} record")
            if (
                CANONICAL_ID.fullmatch(str(identity)) is None
                or record.get(identity_key) != identity
            ):
                raise SuccessorEvidenceError(
                    f"{context} {collection_name} dictionary identity differs"
                )
    required_entity_fields = {
        "entity_id",
        "semantic_kind",
        "representation",
        "workspace_id",
        "analysis_context_id",
        "certainty",
        "resolution",
        "direct_provenance",
    }
    for entity_value in collections["entities"].values():
        entity = _mapping(entity_value, f"{context} entity record")
        if not required_entity_fields <= set(entity):
            raise SuccessorEvidenceError(f"{context} entity record is incomplete")
        if (
            entity["representation"] == "syntax_occurrence"
            and "source_reference" not in entity
        ):
            raise SuccessorEvidenceError(
                f"{context} syntax occurrence lacks exact source identity"
            )

    entity_ids = set(collections["entities"])
    fact_ids = set(collections["facts"])
    for fact_value in collections["facts"].values():
        fact = _mapping(fact_value, f"{context} fact record")
        statement = _mapping(fact.get("statement"), f"{context} fact statement")
        if fact.get("fact_form") not in {
            "entity_existence",
            "relationship",
            "property",
        }:
            raise SuccessorEvidenceError(
                f"{context} fact form is outside the ontology contract"
            )
        references = [
            fact.get("owner_id"),
            statement.get("subject"),
            statement.get("object"),
        ]
        for reference in references:
            if (
                isinstance(reference, str)
                and reference.startswith("entity:")
                and reference not in entity_ids
            ):
                raise SuccessorEvidenceError(
                    f"{context} fact references a missing entity record"
                )
    for path_value in collections["paths"].values():
        path = _mapping(path_value, f"{context} path record")
        if (
            not set(path.get("ordered_entity_ids", [])) <= entity_ids
            or not set(path.get("ordered_fact_ids", [])) <= fact_ids
        ):
            raise SuccessorEvidenceError(
                f"{context} path references a missing entity or fact record"
            )
    for group_value in collections["groups"].values():
        group = _mapping(group_value, f"{context} group record")
        if not set(group.get("support_fact_ids", [])) <= fact_ids:
            raise SuccessorEvidenceError(
                f"{context} group references a missing support fact"
            )
    for source_value in collections["source_contexts"].values():
        source_context = _mapping(source_value, f"{context} source-context record")
        if source_context.get("entity_id") not in entity_ids:
            raise SuccessorEvidenceError(
                f"{context} source context references a missing entity record"
            )
    _mapping(result["coverage"], f"{context} structured coverage")
    provenance = _mapping(result["provenance"], f"{context} structured provenance")
    for key in (
        "epoch_id",
        "snapshot_id",
        "query_program_release",
        "producer_closure_id",
        "policy_release",
        "expectation_issuance",
    ):
        _nonempty_string(provenance.get(key), f"{context} provenance {key}")

    if family == "query_find_code_entities":
        selected, expected_semantics, expected_coverage = _derive_find_entity_selection(
            inputs, block, context
        )
        expected_entities = {row["entity_id"]: row for row in selected}
        if (
            collections["entities"] != expected_entities
            or result["entity_ids"] != [row["entity_id"] for row in selected]
            or result["resolved_semantics"] != expected_semantics
            or result["coverage"] != expected_coverage
            or result["result_role"] != inputs["program_binding"]["result_role"]
            or result["request"] != inputs["program_binding"]["form"]
            or provenance["query_program_release"]
            != inputs["program_binding"]["query_program_release"]
            or provenance["producer_closure_id"]
            != inputs["program_binding"]["producer_closure_id"]
            or any(
                collections[name]
                for name in ("facts", "paths", "groups", "source_contexts")
            )
            or any(
                result[key]
                for key in (
                    "fact_ids",
                    "path_ids",
                    "group_ids",
                    "source_context_ids",
                    "bindings",
                    "errors",
                    "notices",
                )
            )
        ):
            raise SuccessorEvidenceError(
                f"{context} find-entity result is not exactly derived from request, program, admitted rows, coverage, access, and catalog"
            )
    elif family == "query_retrieve_facts":
        relations = _mapping(
            inputs["admitted_relations"], f"{context} admitted relations"
        )
        property_kinds = _property_kind_allocation(relations, context)
        admitted_type_rows = [
            _mapping(value, f"{context} admitted type fact")
            for value in _list(relations["fact_rows"], f"{context} fact rows")
            if isinstance(value, dict) and value.get("fact_kind") == "type"
        ]
        if len(admitted_type_rows) != 1:
            raise SuccessorEvidenceError(
                f"{context} must admit exactly one known type fact"
            )
        admitted_type = admitted_type_rows[0]
        admitted_projection = copy.deepcopy(dict(admitted_type))
        admitted_projection.pop("alias", None)
        facts = list(collections["facts"].values())
        known_types = [
            fact
            for fact in facts
            if isinstance(fact, dict) and fact.get("fact_kind") == "type"
        ]
        unknowns = [
            fact
            for fact in facts
            if isinstance(fact, dict) and fact.get("fact_kind") == "unknown"
        ]
        if len(known_types) != 1 or len(unknowns) != 1:
            raise SuccessorEvidenceError(f"{context} lacks typed known/unknown facts")
        known_type = known_types[0]
        unknown = unknowns[0]
        if known_type != admitted_projection:
            raise SuccessorEvidenceError(
                f"{context} known type fact is not derived from the admitted fact row"
            )
        coverage_rows = _list(
            relations["coverage_rows"],
            f"{context} coverage rows",
        )
        effect_coverage = next(
            (
                row
                for row in coverage_rows
                if isinstance(row, dict) and row.get("family") == "effects"
            ),
            None,
        )
        if effect_coverage is None:
            raise SuccessorEvidenceError(f"{context} lacks effects coverage")
        input_set_id = _validate_retrieve_fact_input_set(
            relations, inputs["pinned_epoch"]["policy_release"], context
        )
        _validate_retrieve_source_identity(
            effect_coverage,
            inputs["request_envelope"]["decoded"]["scope"]["workspace_id"],
            context,
        )
        known_statement = _mapping(
            known_type.get("statement"), f"{context} known type statement"
        )
        type_code = property_kinds.get("type")
        if (
            type_code is None
            or known_type.get("property_kind_code") != type_code
            or known_statement.get("predicate") != "type"
            or known_statement.get("subject") != known_type.get("owner_id")
        ):
            raise SuccessorEvidenceError(
                f"{context} known type does not consume its property-kind allocation"
            )
        type_scalar = rfc8785.dumps(known_statement["object"]).decode("utf-8")
        type_typed = _cbef_typed_value(2, type_scalar.encode("utf-8"))
        type_tagged = (
            (50).to_bytes(2, "big") + len(type_typed).to_bytes(4, "big") + type_typed
        )
        known_type_id = _validate_cbef_recipe(
            known_type.get("identity_recipe"),
            domain_code=10,
            domain_name="PROPERTY_FACT",
            output_prefix="fact:type",
            fields=[
                (
                    1,
                    "workspace_id",
                    7,
                    "ID",
                    _public_id_bytes(known_type["workspace_id"], "workspace"),
                    known_type["workspace_id"],
                ),
                (
                    2,
                    "analysis_context_id",
                    7,
                    "ID",
                    _analysis_context_id_bytes(known_type["analysis_context_id"]),
                    known_type["analysis_context_id"],
                ),
                (
                    3,
                    "property_kind_code",
                    4,
                    "UNSIGNED",
                    type_code.to_bytes(2, "big"),
                    type_code,
                ),
                (
                    4,
                    "subject_entity_id",
                    7,
                    "ID",
                    _public_domain_id_bytes(known_statement["subject"], "entity"),
                    known_statement["subject"],
                ),
                (
                    5,
                    "canonical_value",
                    12,
                    "TAGGED_UNION",
                    type_tagged,
                    {"variant": 50, "member_type": "UTF8", "value": type_scalar},
                ),
            ],
            excluded=[
                "source and producer provenance",
                "input-set and policy identity",
                "diagnostic evidence",
                "mutable coverage counters",
            ],
            context=context,
        )
        if known_type.get("fact_id") != known_type_id:
            raise SuccessorEvidenceError(f"{context} property fact identity differs")
        unknown_code = property_kinds.get("UNKNOWN_EFFECT")
        expected_statement = {
            "subject": unknown["owner_id"],
            "predicate": "UNKNOWN_EFFECT",
            "object": effect_coverage["family"],
        }
        expected_coverage = {
            "state": effect_coverage["state"],
            "reason": effect_coverage["reason"],
            "retryable": False,
        }
        if unknown_code is None or unknown.get("property_kind_code") != unknown_code:
            raise SuccessorEvidenceError(
                f"{context} UNKNOWN_EFFECT does not consume its property-kind allocation"
            )
        unknown_scalar = _nonempty_string(
            effect_coverage["family"], f"{context} unknown effect family"
        )
        unknown_typed = _cbef_typed_value(2, unknown_scalar.encode("utf-8"))
        unknown_tagged = (
            (50).to_bytes(2, "big")
            + len(unknown_typed).to_bytes(4, "big")
            + unknown_typed
        )
        expected_id = _validate_cbef_recipe(
            unknown.get("identity_recipe"),
            domain_code=10,
            domain_name="PROPERTY_FACT",
            output_prefix="fact:unknown-effect",
            fields=[
                (
                    1,
                    "workspace_id",
                    7,
                    "ID",
                    _public_id_bytes(unknown["workspace_id"], "workspace"),
                    unknown["workspace_id"],
                ),
                (
                    2,
                    "analysis_context_id",
                    7,
                    "ID",
                    _analysis_context_id_bytes(unknown["analysis_context_id"]),
                    unknown["analysis_context_id"],
                ),
                (
                    3,
                    "property_kind_code",
                    4,
                    "UNSIGNED",
                    unknown_code.to_bytes(2, "big"),
                    unknown_code,
                ),
                (
                    4,
                    "subject_entity_id",
                    7,
                    "ID",
                    _public_domain_id_bytes(unknown["owner_id"], "entity"),
                    unknown["owner_id"],
                ),
                (
                    5,
                    "canonical_value",
                    12,
                    "TAGGED_UNION",
                    unknown_tagged,
                    {
                        "variant": 50,
                        "member_type": "UTF8",
                        "value": unknown_scalar,
                    },
                ),
            ],
            excluded=[
                "coverage state",
                "coverage reason",
                "retryability",
                "source and producer provenance",
                "input-set and policy identity",
                "diagnostic evidence",
                "mutable coverage counters",
            ],
            context=context,
        )
        if (
            unknown.get("fact_form") != "property"
            or unknown.get("statement") != expected_statement
            or unknown.get("direct_provenance", {}).get("coverage") != expected_coverage
            or unknown.get("direct_provenance", {}).get("input_set_id") != input_set_id
            or unknown.get("fact_id") != expected_id
        ):
            raise SuccessorEvidenceError(
                f"{context} typed unknown proposition or provenance differs"
            )
    elif family == "query_follow_relationships":
        relations = _mapping(
            inputs["admitted_relations"], f"{context} admitted relations"
        )
        admitted_edges = [
            _mapping(edge, f"{context} admitted relationship edge {index}")
            for index, edge in enumerate(
                _list(relations["call_edges"], f"{context} admitted call edges")
            )
        ]
        start = block["starting_from"][0]
        selected_edges: list[Mapping[str, Any]] = []
        for edge in admitted_edges:
            statement = _mapping(edge.get("statement"), f"{context} call statement")
            if (
                edge.get("fact_form") != "relationship"
                or edge.get("fact_kind") != block["relationship"]
                or statement.get("predicate") != block["relationship"]
                or edge.get("fact_id") is None
                or statement.get("subject") not in admitted_entities
                or statement.get("object") not in admitted_entities
            ):
                raise SuccessorEvidenceError(
                    f"{context} admitted relationship edge is not ontology-closed"
                )
            if statement["subject"] == start:
                selected_edges.append(edge)
        selected_edges.sort(
            key=lambda edge: (
                str(edge["statement"]["object"]),
                str(edge["fact_id"]),
            )
        )
        maximum_results = block["return"]["limit"]["maximum_results"]
        if len(selected_edges) > maximum_results:
            raise SuccessorEvidenceError(
                f"{context} fixture unexpectedly exceeds its explicit result bound"
            )
        selected_ids = [str(edge["fact_id"]) for edge in selected_edges]
        selected_contexts = {
            str(edge["analysis_context_id"]) for edge in selected_edges
        }
        binding = _mapping(inputs["program_binding"], f"{context} program binding")
        _strict_keys(
            binding,
            {
                "binding_relation_id",
                "distance_policy",
                "fact_identity_required",
                "form",
                "live_capability_state",
                "phrase_resolution_relation_id",
                "producer_closure_id",
                "projection_policy_id",
                "query_program_release",
                "relationship_resolution",
                "result_role",
            },
            f"{context} follow program binding",
        )
        relationship_resolution = _mapping(
            binding.get("relationship_resolution"),
            f"{context} relationship resolution",
        )
        coverage_proof = _mapping(
            inputs["producer_coverage"], f"{context} producer coverage"
        )
        result_coverage = _mapping(result["coverage"], f"{context} result coverage")
        visible_relations = set(
            _list(child.get("visible_relations"), f"{context} visible relations")
        )
        expected_facts = {str(edge["fact_id"]): dict(edge) for edge in selected_edges}
        limits = _mapping(inputs["resource_limits"], f"{context} follow limits")
        _strict_keys(
            limits,
            {"deadline_ms", "max_distance", "max_rows"},
            f"{context} follow limits",
        )
        _strict_keys(
            coverage_proof,
            {
                "state",
                "owner",
                "analysis_context_id",
                "family",
                "covered_fact_ids",
            },
            f"{context} follow coverage",
        )
        if (
            block.get("request") != binding.get("form")
            or binding.get("binding_relation_id") != "query.binding"
            or binding.get("phrase_resolution_relation_id") != "query.phrase_resolution"
            or binding.get("projection_policy_id") != "projection:fixture-v1"
            or binding.get("live_capability_state") != "available"
            or binding.get("result_role") != "facts"
            or binding.get("distance_policy") != "exactly one step"
            or binding.get("fact_identity_required") is not True
            or relationship_resolution
            != {
                "phrase": block["relationship"],
                "family_id": block["relationship"],
                "resolution": "exact",
            }
            or block.get("direction") != "outgoing"
            or block.get("distance") != 1
            or limits.get("deadline_ms") != envelope["freshness"]["deadline_ms"]
            or limits.get("max_distance") != 1
            or limits.get("max_rows") < len(selected_ids)
            or set(access.get("relationship_families", [])) != {block["relationship"]}
            or visible_relations != {"canonical.call_fact", "proof.call_coverage"}
            or coverage_proof.get("family") != block["relationship"]
            or coverage_proof.get("owner") != start
            or coverage_proof.get("state") != "COMPLETE"
            or {coverage_proof.get("analysis_context_id")} != selected_contexts
            or set(coverage_proof.get("covered_fact_ids", [])) != set(selected_ids)
            or result["resolved_semantics"]
            != {
                "starting_from": block["starting_from"],
                "relationship": block["relationship"],
                "direction": block["direction"],
                "distance": block["distance"],
            }
            or result.get("fact_ids") != selected_ids
            or collections["facts"] != expected_facts
            or result_coverage
            != {
                "state": coverage_proof["state"],
                "owner": coverage_proof["owner"],
                "analysis_context_id": coverage_proof["analysis_context_id"],
                "distance": block["distance"],
                "completed_family": coverage_proof["family"],
            }
            or provenance.get("query_program_release")
            != binding.get("query_program_release")
            or provenance.get("producer_closure_id")
            != binding.get("producer_closure_id")
        ):
            raise SuccessorEvidenceError(
                f"{context} follow result is not derived from its typed program, access, coverage, catalog, and admitted edges"
            )
    elif family == "query_connecting_paths":
        edges, dictionary, coverage_proof = _validate_path_authority(
            inputs, block, context
        )
        expected_entities, expected_facts = _canonical_shortest_query_witness(
            edges,
            start=str(block["from"][0]),
            target=str(block["to"][0]),
            families=set(block["using"]),
            maximum_length=int(inputs["resource_limits"]["max_path_length"]),
            context=context,
        )
        paths = list(collections["paths"].values())
        required = {
            "path_id",
            "ordered_entity_ids",
            "ordered_fact_ids",
            "length",
            "path_policy",
            "certainty_summary",
            "supporting_provenance",
            "identity_recipe",
        }
        if len(paths) != 1 or not required <= set(paths[0]):
            raise SuccessorEvidenceError(f"{context} path record is incomplete")
        path = paths[0]
        expected_fact_dictionary = {str(edge["fact_id"]): dict(edge) for edge in edges}
        producer_releases = sorted(
            {
                f"{edge['producer']['producer_id']}:{edge['producer']['release']}"
                for edge in edges
            }
        )
        expected_result_coverage = {
            "state": coverage_proof["state"],
            "graph_projection": (
                f"{coverage_proof['family']}@{coverage_proof['analysis_context_id']}"
            ),
            "searched_entity_count": len(coverage_proof["entity_ids"]),
            "searched_fact_count": len(coverage_proof["fact_ids"]),
        }
        expected_semantics = {
            "from": block["from"],
            "to": block["to"],
            "relationship_families": block["using"],
            "path_policy": block["path_policy"],
            "maximum_path_length": inputs["resource_limits"]["max_path_length"],
        }
        if (
            block["path_policy"] != inputs["program_binding"]["path_policy"]
            or path["ordered_entity_ids"] != expected_entities
            or path["ordered_fact_ids"] != expected_facts
            or path["length"] != len(expected_facts)
            or path["path_policy"] != block["path_policy"]
            or path["certainty_summary"] != "exact"
            or path["supporting_provenance"]
            != {
                "analysis_context_id": coverage_proof["analysis_context_id"],
                "coverage_state": coverage_proof["state"],
                "producer_releases": producer_releases,
            }
            or collections["entities"] != dictionary
            or collections["facts"] != expected_fact_dictionary
            or result["path_ids"] != [path["path_id"]]
            or result["entity_ids"] != []
            or result["fact_ids"] != []
            or result["coverage"] != expected_result_coverage
            or result["resolved_semantics"] != expected_semantics
            or result["result_role"] != inputs["program_binding"]["result_role"]
            or provenance["query_program_release"]
            != inputs["program_binding"]["query_program_release"]
            or provenance["producer_closure_id"]
            != inputs["program_binding"]["producer_closure_id"]
        ):
            raise SuccessorEvidenceError(
                f"{context} path result is not the exact canonical shortest witness derived from admitted edges"
            )
        expected_id = _validate_path_result_recipe(
            path["identity_recipe"],
            workspace_id=response["snapshot"]["workspace_id"],
            analysis_context_id=path["supporting_provenance"]["analysis_context_id"],
            fabric_epoch_id=epoch["fabric_epoch_id"],
            policy_identity=epoch["policy_release"],
            ordered_entity_ids=path["ordered_entity_ids"],
            ordered_fact_ids=path["ordered_fact_ids"],
            context=context,
        )
        if path["path_id"] != expected_id:
            raise SuccessorEvidenceError(f"{context} path witness identity differs")
    elif family == "query_match_pattern":
        matches, entity_ids, fact_ids, expected_coverage, dictionary = (
            _derive_typed_pattern_matches(inputs, block, context)
        )
        expected_facts = {
            str(edge["fact_id"]): dict(edge)
            for edge in inputs["admitted_relations"]["call_edges"]
        }
        pattern = block["pattern"]
        expected_semantics = {
            "pattern_id": "pattern:typed-edge-no-outgoing-call-v1",
            "typed_bindings": {
                str(node["binding"]): str(node["semantic_kind"])
                for node in pattern["nodes"]
            },
            "positive_fact_count": len(pattern["facts"]),
            "scoped_negation_universe": inputs["producer_coverage"][
                "negative_proof_universe_id"
            ],
        }
        if (
            inputs["producer_coverage"]["state"] != "COMPLETE"
            or not matches
            or result["bindings"] != matches
            or result["entity_ids"] != entity_ids
            or result["fact_ids"] != fact_ids
            or result["coverage"] != expected_coverage
            or result["resolved_semantics"] != expected_semantics
            or collections["entities"] != dictionary
            or collections["facts"] != expected_facts
            or result["result_role"] != inputs["program_binding"]["result_role"]
            or provenance["query_program_release"]
            != inputs["program_binding"]["query_program_release"]
            or provenance["producer_closure_id"]
            != inputs["program_binding"]["producer_closure_id"]
        ):
            raise SuccessorEvidenceError(
                f"{context} pattern result is not derived from typed nodes, edges, scoped negation, access, catalog, and exact coverage"
            )
    elif family == "query_combine_results":
        selections = [
            _mapping(value, f"{context} prior-result selection")
            for value in _list(block["inputs"], f"{context} prior-result selections")
        ]
        for selection in selections:
            _strict_keys(
                selection,
                {"results_of", "select"},
                f"{context} prior-result selection",
            )
        producer_ids = [str(value["query_id"]) for value in producer_blocks]
        referenced_ids = [
            _nonempty_string(
                selection["results_of"], f"{context} prior-result reference"
            )
            for selection in selections
        ]
        if (
            referenced_ids != producer_ids
            or any(selection["select"] != "entities" for selection in selections)
            or [str(value["query_id"]) for value in results] != block_ids
        ):
            raise SuccessorEvidenceError(
                f"{context} combine references do not match the topological producer order"
            )
        relations = _mapping(
            inputs["admitted_relations"], f"{context} admitted relations"
        )
        producer_results = _derive_combine_producer_results(
            producer_blocks=producer_blocks,
            relations=relations,
            provenance=_query_provenance_from_inputs(inputs),
            admitted_entities=admitted_entities,
            context=context,
        )
        expected_edges = [
            {
                "producer_query_id": producer_id,
                "consumer_query_id": str(block["query_id"]),
                "selection": "entities",
            }
            for producer_id in producer_ids
        ]
        if relations.get("dependency_edges") != expected_edges:
            raise SuccessorEvidenceError(
                f"{context} combine dependency DAG differs from typed selections"
            )
        dimensions = inputs["program_binding"].get("compatibility_dimensions")
        if set(dimensions or []) != {
            "workspace_id",
            "analysis_context_id",
            "representation_layer",
            "certainty_class",
            "semantic_role",
        }:
            raise SuccessorEvidenceError(
                f"{context} compatibility dimensions are incomplete"
            )
        compatibility_values: list[Mapping[str, Any]] = []
        for producer_id in producer_ids:
            producer_result = _mapping(
                producer_results[producer_id],
                f"{context} producer result {producer_id}",
            )
            _strict_keys(
                producer_result,
                QUERY_RESULT_KEYS,
                f"{context} producer result {producer_id}",
            )
            if producer_result != result_by_id[producer_id]:
                raise SuccessorEvidenceError(
                    f"{context} response producer result is not derived from its base input"
                )
            resolved_producer = _mapping(
                producer_result["resolved_semantics"],
                f"{context} producer result semantics {producer_id}",
            )
            compatibility = _mapping(
                resolved_producer.get("compatibility_dimensions"),
                f"{context} producer compatibility {producer_id}",
            )
            if (
                set(compatibility) != set(dimensions)
                or producer_result["query_id"] != producer_id
                or producer_result["result_role"] != "entities"
                or producer_result["execution_state"] != "COMPLETE"
                or not set(producer_result["entity_ids"]) <= set(admitted_entities)
            ):
                raise SuccessorEvidenceError(
                    f"{context} producer result envelope is not compatible and closed"
                )
            compatibility_values.append(compatibility)
        if any(value != compatibility_values[0] for value in compatibility_values[1:]):
            raise SuccessorEvidenceError(
                f"{context} combine producer compatibility dimensions differ"
            )
        expected_entity_ids = sorted(
            set(producer_results[producer_ids[0]]["entity_ids"])
            & set(producer_results[producer_ids[1]]["entity_ids"])
        )
        resolved_combine = _mapping(
            result["resolved_semantics"], f"{context} combine result semantics"
        )
        if (
            result["entity_ids"] != expected_entity_ids
            or resolved_combine.get("operation") != block["operation"]
            or resolved_combine.get("inputs") != selections
            or resolved_combine.get("compatibility_dimensions")
            != compatibility_values[0]
            or result.get("coverage", {}).get("upstream_results") != producer_ids
            or inputs["access_scope"].get("input_results") != producer_ids
        ):
            raise SuccessorEvidenceError(
                f"{context} combine result is not derived from its producer envelopes"
            )
    elif family == "query_summarize_facts":
        relations = _mapping(
            inputs["admitted_relations"], f"{context} admitted objective relations"
        )
        if relations.get("coverage_state") != inputs["producer_coverage"].get("syntax"):
            raise SuccessorEvidenceError(
                f"{context} objective input-set coverage is detached from producer coverage"
            )
        input_set_id, objective_rows, grouped_facts = _validate_objective_fact_inputs(
            relations, str(epoch["policy_release"]), context
        )
        groups = [
            _mapping(value, f"{context} objective group")
            for value in collections["groups"].values()
        ]
        required = {
            "group_id",
            "group_key",
            "objective_value",
            "input_set_id",
            "grouping",
            "aggregation",
            "producer_id",
            "precision",
            "completeness",
            "support_fact_ids",
            "identity_recipe",
        }
        if any(not required <= set(group) for group in groups):
            raise SuccessorEvidenceError(f"{context} summary provenance is incomplete")
        group_ids = _validate_objective_groups(
            groups,
            input_set_id=input_set_id,
            grouped_facts=grouped_facts,
            context=context,
        )
        objective_facts = {str(row["fact_id"]): dict(row) for row in objective_rows}
        coverage = _mapping(result["coverage"], f"{context} summary coverage")
        resolved = _mapping(
            result["resolved_semantics"], f"{context} summary semantics"
        )
        if (
            collections["facts"] != objective_facts
            or result["group_ids"] != group_ids
            or resolved.get("input_set_id") != input_set_id
            or coverage.get("input_set_id") != input_set_id
            or coverage.get("input_count") != len(objective_rows)
            or coverage.get("group_count") != len(groups)
        ):
            raise SuccessorEvidenceError(
                f"{context} objective summary identities are not transitively derived"
            )
    elif family == "query_source_context":
        context_kind = _nonempty_string(
            block["context"], f"{context} source context kind"
        )
        resource_limits = _mapping(
            inputs["resource_limits"], f"{context} source resource limits"
        )
        source_byte_limit = _positive_int(
            resource_limits["max_source_bytes"],
            f"{context} source byte limit",
        )
        return_limit = _mapping(
            block["return"]["limit"], f"{context} result-record limit"
        )
        if return_limit["maximum_results"] != 1 or return_limit["per"] != "query block":
            raise SuccessorEvidenceError(
                f"{context} source byte bound is conflated with result-record limit"
            )
        contexts = list(collections["source_contexts"].values())
        required = {
            "source_context_id",
            "entity_id",
            "context_kind",
            "source_reference",
            "content",
            "returned_bytes",
            "omitted_bytes",
            "complete",
            "authorization_scope",
            "limit",
            "identity_recipe",
        }
        if len(contexts) != 1 or not required <= set(contexts[0]):
            raise SuccessorEvidenceError(f"{context} source context is incomplete")
        if resource_limits.get("max_output_bytes", 0) < contexts[0]["returned_bytes"]:
            raise SuccessorEvidenceError(
                f"{context} conflates explicit truncation with hard byte budget"
            )
        admitted = _mapping(
            inputs["admitted_relations"], f"{context} admitted relations"
        )
        span = _mapping(admitted["entity_span"], f"{context} admitted entity span")
        expected_source_file_id = "file:" + _cbef_source_file_id(
            workspace_id=span["workspace_id"],
            comparison_key=_nonempty_string(
                span["byte_safe_path"], f"{context} byte-safe path"
            ).encode("utf-8"),
        )
        if span.get("source_file_id") != expected_source_file_id:
            raise SuccessorEvidenceError(
                f"{context} source file is not the exact CBEF SOURCE_FILE identity"
            )
        admitted_entity = _mapping(
            admitted_entities[span["entity_id"]], f"{context} admitted source entity"
        )
        source_reference = {
            key: value for key, value in span.items() if key != "entity_id"
        }
        response_entity = _mapping(
            collections["entities"][span["entity_id"]],
            f"{context} response source entity",
        )
        source_context = _mapping(contexts[0], f"{context} source context")
        source_access = _mapping(inputs["access_scope"], f"{context} source access")
        _strict_keys(
            source_access,
            {
                "workspace",
                "principal_id",
                "agent_id",
                "credential_digest",
                "role",
                "operation",
                "allowed_relations",
                "allowed_columns",
                "allowed_functions",
                "allowed_extensions",
                "allowed_variables",
                "allowed_object_stores",
                "allowed_metadata",
                "row_policies",
                "execution_posture",
                "source_access",
                "source_file_ids",
                "authorized_ranges",
                "scope_id",
                "identity_recipe",
            },
            f"{context} source access",
        )
        scope_recipe = _authorization_scope_recipe(
            source_access, {"policy_id": epoch["policy_release"]}
        )
        if (
            source_access["workspace"] != span["workspace_id"]
            or source_access["source_access"] is not True
            or source_access["source_file_ids"] != [span["source_file_id"]]
            or source_access["authorized_ranges"]
            != [[span["source_file_id"], span["start_byte"], span["end_byte"]]]
            or set(source_access["allowed_relations"])
            != set(child["visible_relations"])
            or set(source_access["allowed_columns"])
            != set(source_access["allowed_relations"])
            or source_access["scope_id"] != scope_recipe["output_id"]
            or source_access["identity_recipe"] != scope_recipe
            or source_context.get("authorization_scope") != source_access["scope_id"]
        ):
            raise SuccessorEvidenceError(
                f"{context} source disclosure scope is not exact and CBEF-bound"
            )
        source_bytes = _mapping(admitted["source_bytes"], f"{context} source bytes")
        content = _mapping(source_context["content"], f"{context} source content")
        delivered = _nonempty_string(
            content.get("text"), f"{context} delivered source"
        ).encode("utf-8")
        encoded_source = _nonempty_string(
            source_bytes["value"], f"{context} source image"
        ).encode("utf-8")
        authorized_bytes = encoded_source[span["start_byte"] : span["end_byte"]]
        expected_delivered = authorized_bytes[:source_byte_limit]
        expected_omitted = len(authorized_bytes) - len(expected_delivered)
        expected_complete = expected_omitted == 0
        expected_limit_state = (
            "NOT_APPLIED" if expected_complete else "EXPLICIT_LIMIT_REACHED"
        )
        expected_completeness = "COMPLETE" if expected_complete else "PARTIAL"
        resolved = _mapping(
            result["resolved_semantics"], f"{context} source resolved semantics"
        )
        result_coverage = _mapping(
            result["coverage"], f"{context} source result coverage"
        )
        if (
            admitted_entity.get("source_reference") != source_reference
            or response_entity.get("source_reference") != source_reference
            or source_context.get("source_reference") != source_reference
            or source_context.get("entity_id") != span["entity_id"]
            or source_context.get("context_kind") != context_kind
            or delivered != expected_delivered
            or source_context.get("returned_bytes") != len(delivered)
            or source_context.get("omitted_bytes") != expected_omitted
            or source_context.get("complete") is not expected_complete
            or source_context.get("limit")
            != {
                "kind": "explicit",
                "state": expected_limit_state,
                "maximum_source_bytes": source_byte_limit,
            }
            or resolved.get("about") != block["about"]
            or resolved.get("context") != context_kind
            or resolved.get("explicit_source_byte_limit") != source_byte_limit
            or result.get("completeness_state") != expected_completeness
            or result.get("limit_state") != expected_limit_state
            or result_coverage.get("authorized_span_bytes") != len(authorized_bytes)
            or result_coverage.get("returned_bytes") != len(delivered)
            or result_coverage.get("omitted_bytes") != expected_omitted
        ):
            raise SuccessorEvidenceError(
                f"{context} exact source context is detached from its selected entity span"
            )
        expected_id = _validate_query_source_context_recipe(
            source_context["identity_recipe"],
            workspace_id=span["workspace_id"],
            analysis_context_id=admitted_entity["analysis_context_id"],
            snapshot_id=response["snapshot"]["snapshot_id"],
            entity_id=source_context["entity_id"],
            source_file_id=span["source_file_id"],
            source_generation=span["source_generation"],
            source_content_digest=span["content_digest"],
            delivered_start_byte=span["start_byte"],
            delivered_end_byte=span["start_byte"] + len(delivered),
            delivered_content_digest=_bytes_b3(delivered),
            disclosure_scope_id=source_access["scope_id"],
            policy_identity=epoch["policy_release"],
            context_kind=context_kind.lower(),
            context=context,
        )
        if source_context["source_context_id"] != expected_id:
            raise SuccessorEvidenceError(f"{context} source-context identity differs")


def _delta_protocol(value: object, context: str) -> Mapping[str, Any]:
    protocol = _mapping(value, context)
    _strict_keys(
        protocol,
        {
            "min_reader_version",
            "min_writer_version",
            "reader_features",
            "writer_features",
        },
        context,
    )
    for name in ("min_reader_version", "min_writer_version"):
        _positive_int(protocol[name], f"{context} {name}")
    for name in ("reader_features", "writer_features"):
        features = _list(protocol[name], f"{context} {name}")
        normalized = [
            _nonempty_string(feature, f"{context} {name} item") for feature in features
        ]
        if len(normalized) != len(set(normalized)):
            raise SuccessorEvidenceError(f"{context} {name} contains duplicates")
    return protocol


def _delta_file_rows(value: object, context: str) -> tuple[str, list[list[str]]]:
    file = _mapping(value, context)
    _strict_keys(file, {"path", "rows"}, context)
    path = _nonempty_string(file["path"], f"{context} path")
    rows = _list(file["rows"], f"{context} rows")
    if not rows:
        raise SuccessorEvidenceError(f"{context} has zero rows")
    decoded_rows: list[list[str]] = []
    for row_index, row_value in enumerate(rows, 1):
        row = _list(row_value, f"{context} row {row_index}")
        if len(row) != 2:
            raise SuccessorEvidenceError(f"{context} row {row_index} width differs")
        decoded_rows.append(
            [
                _nonempty_string(row[0], f"{context} row {row_index} entity_id"),
                _nonempty_string(row[1], f"{context} row {row_index} value"),
            ]
        )
    return path, decoded_rows


def _reconstruct_delta_snapshots(
    history_value: object, context: str
) -> tuple[dict[int, Mapping[str, Any]], dict[int, list[list[str]]]]:
    history = _mapping(history_value, f"{context} Delta history")
    _strict_keys(
        history, {"table", "checkpoint", "commits"}, f"{context} Delta history"
    )
    _nonempty_string(history["table"], f"{context} Delta table")

    checkpoint = _mapping(history["checkpoint"], f"{context} Delta checkpoint")
    _strict_keys(
        checkpoint,
        {"version", "protocol", "active_files"},
        f"{context} Delta checkpoint",
    )
    checkpoint_version = _positive_int(
        checkpoint["version"], f"{context} Delta checkpoint version"
    )
    _delta_protocol(checkpoint["protocol"], f"{context} Delta checkpoint protocol")
    active_files: dict[str, list[list[str]]] = {}
    for file_index, file_value in enumerate(
        _list(checkpoint["active_files"], f"{context} Delta checkpoint active files"),
        1,
    ):
        path, rows = _delta_file_rows(
            file_value, f"{context} Delta checkpoint active file {file_index}"
        )
        if path in active_files:
            raise SuccessorEvidenceError(
                f"{context} Delta checkpoint duplicates an active file"
            )
        active_files[path] = rows

    commits = _list(history["commits"], f"{context} Delta commits")
    if not commits:
        raise SuccessorEvidenceError(f"{context} Delta history has zero commits")
    versions: dict[int, Mapping[str, Any]] = {}
    snapshots: dict[int, list[list[str]]] = {
        checkpoint_version: sorted(
            (row for rows in active_files.values() for row in rows),
            key=lambda row: (row[0], row[1]),
        )
    }
    predecessor_version = checkpoint_version
    for index, value in enumerate(commits, 1):
        commit_context = f"{context} Delta commit {index}"
        commit = _mapping(value, commit_context)
        _strict_keys(
            commit,
            {"version", "operation", "protocol", "actions"},
            commit_context,
        )
        version = commit["version"]
        if (
            not isinstance(version, int)
            or isinstance(version, bool)
            or version != predecessor_version + 1
        ):
            raise SuccessorEvidenceError(
                f"{context} Delta log versions are not contiguous"
            )
        operation = _mapping(commit["operation"], f"{commit_context} operation")
        _strict_keys(
            operation,
            {"name", "mode", "base_version"},
            f"{commit_context} operation",
        )
        if (
            operation["name"] != "WRITE"
            or operation["mode"] not in {"append", "overwrite"}
            or operation["base_version"] != predecessor_version
        ):
            raise SuccessorEvidenceError(
                f"{commit_context} does not bind its exact predecessor write"
            )
        _delta_protocol(commit["protocol"], f"{commit_context} protocol")
        actions = _mapping(commit["actions"], f"{commit_context} actions")
        _strict_keys(actions, {"add", "remove"}, f"{commit_context} actions")
        remove_values = _list(actions["remove"], f"{commit_context} remove actions")
        remove_paths = [
            _nonempty_string(path, f"{commit_context} remove path")
            for path in remove_values
        ]
        if len(remove_paths) != len(set(remove_paths)):
            raise SuccessorEvidenceError(f"{commit_context} duplicates a remove action")
        if any(path not in active_files for path in remove_paths):
            raise SuccessorEvidenceError(
                f"{commit_context} removes a file outside the predecessor snapshot"
            )

        add_files: dict[str, list[list[str]]] = {}
        for file_index, file_value in enumerate(
            _list(actions["add"], f"{commit_context} add actions"), 1
        ):
            path, rows = _delta_file_rows(
                file_value, f"{commit_context} add file {file_index}"
            )
            if path in add_files or path in active_files:
                raise SuccessorEvidenceError(
                    f"{commit_context} reuses an active or duplicate file path"
                )
            add_files[path] = rows
        if not add_files:
            raise SuccessorEvidenceError(f"{commit_context} has zero add actions")

        active_paths = set(active_files)
        if operation["mode"] == "append" and remove_paths:
            raise SuccessorEvidenceError(
                f"{commit_context} append masquerades as replacement"
            )
        if operation["mode"] == "overwrite" and set(remove_paths) != active_paths:
            raise SuccessorEvidenceError(
                f"{commit_context} overwrite does not replace the exact predecessor snapshot"
            )

        for path in remove_paths:
            del active_files[path]
        active_files.update(add_files)
        versions[version] = commit
        snapshots[version] = sorted(
            (row for rows in active_files.values() for row in rows),
            key=lambda row: (row[0], row[1]),
        )
        predecessor_version = version
    return versions, snapshots


def _delta_selection_proof(
    inputs: Mapping[str, Any],
    history: Mapping[str, Any],
    selected: int,
    commit: Mapping[str, Any],
    context: str,
) -> dict[str, Any]:
    root_identity = _mapping(
        inputs["table_root_identity"], f"{context} Delta table root identity"
    )
    _strict_keys(
        root_identity,
        {"table", "root"},
        f"{context} Delta table root identity",
    )
    if root_identity["table"] != history["table"]:
        raise SuccessorEvidenceError(f"{context} Delta table root table differs")
    table_root = _nonempty_string(root_identity["root"], f"{context} Delta table root")
    checkpoint = _mapping(history["checkpoint"], f"{context} Delta checkpoint")
    return {
        "table": history["table"],
        "table_root": table_root,
        "checkpoint_version": checkpoint["version"],
        "checkpoint_identity": _canonical_b3(checkpoint),
        "selected_version": selected,
        "selected_commit_identity": _canonical_b3(commit),
    }


def _validate_materialized_delta_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    history = _mapping(inputs["delta_table_history"], f"{context} Delta history")
    _strict_keys(
        history,
        {"table", "materialization", "schema", "versions"},
        f"{context} Delta history",
    )
    if history["table"] != "fact.entity":
        raise SuccessorEvidenceError(f"{context} Delta table identity differs")
    materialization = _mapping(
        history["materialization"], f"{context} Delta materialization"
    )
    if materialization != {
        "root_binding": "runtime-created private local-filesystem URL",
        "creation_api": "deltalake::operations::create::CreateBuilder",
        "write_api": "DeltaTable::write",
        "authority": "Delta transaction log interpreted by delta-rs",
        "frozen_physical_uri": False,
    }:
        raise SuccessorEvidenceError(f"{context} Delta materialization differs")
    if history["schema"] != [
        {"name": "entity_id", "delta_type": "string", "nullable": False},
        {"name": "value", "delta_type": "string", "nullable": False},
    ]:
        raise SuccessorEvidenceError(f"{context} Delta logical schema differs")

    versions = [
        _mapping(value, f"{context} Delta version")
        for value in _list(history["versions"], f"{context} Delta versions")
    ]
    if len(versions) < 3:
        raise SuccessorEvidenceError(f"{context} Delta version history is incomplete")
    snapshot: list[list[str]] = []
    snapshots: dict[int, list[list[str]]] = {}
    for expected_version, version in enumerate(versions):
        _strict_keys(
            version,
            {"version", "operation", "input_rows", "expected_snapshot", "protocol"},
            f"{context} Delta version {expected_version}",
        )
        expected_operation = "CREATE" if expected_version == 0 else "WRITE_APPEND"
        if (
            version["version"] != expected_version
            or version["operation"] != expected_operation
        ):
            raise SuccessorEvidenceError(f"{context} Delta transition differs")
        input_rows = _list(version["input_rows"], f"{context} Delta version input rows")
        if any(not isinstance(row, list) or len(row) != 2 for row in input_rows):
            raise SuccessorEvidenceError(f"{context} Delta input row shape differs")
        if (expected_version == 0 and input_rows) or (
            expected_version > 0 and not input_rows
        ):
            raise SuccessorEvidenceError(
                f"{context} CREATE cannot author data and each fixture WRITE must append rows"
            )
        snapshot = [*snapshot, *copy.deepcopy(input_rows)]
        if version["expected_snapshot"] != snapshot:
            raise SuccessorEvidenceError(
                f"{context} Delta snapshot is not derived from transitions"
            )
        protocol = _mapping(version["protocol"], f"{context} Delta version protocol")
        _strict_keys(
            protocol,
            {
                "min_reader_version",
                "min_writer_version",
                "reader_features",
                "writer_features",
                "table_properties",
            },
            f"{context} Delta version protocol",
        )
        if (
            protocol["min_reader_version"] != 1
            or protocol["min_writer_version"] != 4
            or protocol["reader_features"] != []
            or protocol["writer_features"] != []
            or protocol["table_properties"] != {"delta.enableChangeDataFeed": "true"}
        ):
            raise SuccessorEvidenceError(f"{context} Delta protocol differs")
        snapshots[expected_version] = copy.deepcopy(snapshot)

    selected_vector = _mapping(
        inputs["selected_version_vector"], f"{context} selected version vector"
    )
    selected = selected_vector.get("table_versions", {}).get("fact.entity")
    latest = max(snapshots)
    if (
        selected_vector.get("selection") != "exact"
        or selected not in snapshots
        or selected == latest
    ):
        raise SuccessorEvidenceError(f"{context} exact version selector differs")
    protocol_support = _mapping(
        inputs["protocol_support"], f"{context} protocol support"
    )
    _strict_keys(
        protocol_support,
        {
            "delta_rs_revision",
            "supported_reader_features",
            "supported_writer_features",
            "unsupported_writer_features",
        },
        f"{context} protocol support",
    )
    if (
        protocol_support["delta_rs_revision"]
        != "43a0cf10a313e5077c48637ad786a05359136bbb"
        or "rowTracking" in protocol_support["supported_writer_features"]
        or protocol_support["unsupported_writer_features"] != ["rowTracking"]
        or "changeDataFeed" not in protocol_support["supported_writer_features"]
    ):
        raise SuccessorEvidenceError(f"{context} Delta feature support differs")
    root_identity = _mapping(
        inputs["table_root_identity"], f"{context} table-root identity"
    )
    if root_identity != {
        "binding": "runtime-created local-filesystem URL",
        "canonicalization": "ExactDeltaPin canonical URL after materialization",
        "frozen_uri": False,
    }:
        raise SuccessorEvidenceError(f"{context} fabricates a physical Delta root")
    if inputs["runtime_configuration"] != {
        "datafusion": "55.0.0",
        "arrow": "59.2.0",
        "object_store": "0.13.2",
        "deltalake": "git:43a0cf10a313e5077c48637ad786a05359136bbb",
    }:
        raise SuccessorEvidenceError(f"{context} Delta runtime universe differs")
    proof = _mapping(inputs["proof_input"], f"{context} Delta proof input")
    _strict_keys(
        proof,
        {
            "exact_snapshot_read",
            "latest_snapshot_read",
            "cdf_read",
            "raw_object_listing_authority",
        },
        f"{context} Delta proof input",
    )
    if (
        proof["exact_snapshot_read"]
        != {"api": "DeltaTableBuilder::with_version", "version": selected}
        or proof["latest_snapshot_read"]
        != {"api": "DeltaTable::update_state", "expected_version": latest}
        or proof["raw_object_listing_authority"] is not False
    ):
        raise SuccessorEvidenceError(f"{context} Delta selection APIs differ")
    cdf = _mapping(proof["cdf_read"], f"{context} CDF proof")
    if cdf != {
        "api": "DeltaTable::scan_cdf",
        "starting_version": latest,
        "ending_version": latest,
        "inclusive_bounds": True,
        "allow_out_of_range": False,
        "required_metadata_columns": [
            "_change_type",
            "_commit_version",
            "_commit_timestamp",
        ],
    }:
        raise SuccessorEvidenceError(f"{context} CDF proof contract differs")
    cdf_rows = [
        [*row, "insert", version["version"]]
        for version in versions
        if cdf["starting_version"] <= version["version"] <= cdf["ending_version"]
        for row in version["input_rows"]
    ]
    expected_observation = {
        "selected_version": selected,
        "latest_version": latest,
        "protocol": versions[selected]["protocol"],
        "snapshot_rows": snapshots[selected],
        "cdf_window": {
            "starting_version": latest,
            "ending_version": latest,
            "inclusive": True,
        },
        "cdf_columns": ["entity_id", "value", "_change_type", "_commit_version"],
        "cdf_rows": cdf_rows,
    }
    if decoded["columns"] != ["table", "exact_observation"] or decoded["rows"] != [
        ["fact.entity", expected_observation]
    ]:
        raise SuccessorEvidenceError(
            f"{context} programmatic Delta observation differs"
        )


def _validate_delta_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    history = _mapping(inputs["delta_table_history"], f"{context} Delta history")
    if set(history) == {"table", "materialization", "schema", "versions"}:
        _validate_materialized_delta_inputs(inputs, decoded, context)
        return
    versions, snapshots = _reconstruct_delta_snapshots(history, context)
    selected = inputs["selected_version_vector"]["table_versions"][history["table"]]
    if selected not in versions or selected == max(versions):
        raise SuccessorEvidenceError(
            f"{context} does not discriminate exact from latest"
        )
    commit = versions[selected]
    selection_proof = _delta_selection_proof(inputs, history, selected, commit, context)
    expected = [
        [
            selection_proof,
            commit["protocol"]["min_reader_version"],
            commit["protocol"]["min_writer_version"],
            commit["operation"]["name"],
            *row,
        ]
        for row in snapshots[selected]
    ]
    if (
        decoded["columns"]
        != [
            "exact_selection",
            "min_reader_version",
            "min_writer_version",
            "operation",
            "entity_id",
            "value",
        ]
        or decoded["rows"] != expected
    ):
        raise SuccessorEvidenceError(
            f"{context} exact-version selection proof or decoded rows differ"
        )
    proof_input = _mapping(inputs["proof_input"], f"{context} Delta proof input")
    _strict_keys(
        proof_input,
        {
            "expected_version",
            "latest_version",
            "raw_listing_authority",
            "operations_under_test",
        },
        f"{context} Delta proof input",
    )
    operations = _mapping(
        proof_input["operations_under_test"], f"{context} Delta operations under test"
    )
    _strict_keys(operations, {"read", "write"}, f"{context} Delta operations")
    latest = max(versions)
    latest_operation = versions[latest]["operation"]
    if (
        operations["read"] != {"selection": "exact", "version": selected}
        or operations["write"]
        != {
            "mode": "overwrite",
            "base_version": latest_operation["base_version"],
            "committed_version": latest,
            "replacement_scope": "full_table",
            "require_supported_writer_features": True,
        }
        or latest_operation["mode"] != "overwrite"
        or proof_input["expected_version"] != selected
        or proof_input["latest_version"] != latest
        or proof_input["raw_listing_authority"] is not False
    ):
        raise SuccessorEvidenceError(f"{context} Delta operation proof differs")
    if "rowTracking" in inputs["protocol_support"]["supported_writer_features"]:
        raise SuccessorEvidenceError(
            f"{context} falsely advertises rowTracking support"
        )


ACTIVATION_EVENT_KEYS = {
    "sequence",
    "epoch",
    "predecessor",
    "operation_marker",
    "writer_fence",
    "input_set_id",
    "program_release",
    "application_release",
    "source_generation",
    "provider_release_vector",
    "table_version_vector",
    "policy_release",
    "proof_set",
    "durable_event_identity",
}

ACTIVATION_CHAIN_KEYS = {"durable_relation_readback", "events"}
ACTIVATION_RELATION_READBACK_KEYS = {
    "relation_id",
    "table_root",
    "table_id",
    "readback_version",
    "readback_checksum",
    "operation_marker_source",
}
ACTIVATION_DURABLE_EVENT_KEYS = {
    "table_id",
    "delta_version",
    "event_payload_checksum",
}

PROGRAMMATIC_ACTIVATION_CHAIN_KEYS = {
    "relation_contract",
    "typed_identity_contract",
    "events",
}
PROGRAMMATIC_ACTIVATION_RELATION_KEYS = {
    "relation_id",
    "storage_relation_id",
    "schema_identity",
    "provider_binding_id",
    "schema_version",
    "arrow_type_universe",
    "append_only",
    "change_data_feed",
    "fields",
}
PROGRAMMATIC_ACTIVATION_FIELDS = [
    "control_root",
    "control_predecessor_version",
    "control_commit_version",
    "control_session_id",
    "control_provider_binding_id",
    "control_binding_fingerprint",
    "logical_schema_digest",
    "storage_schema_digest",
    "event_id",
    "workspace_id",
    "operation_id",
    "predecessor_event_id",
    "predecessor_epoch",
    "ordinal",
    "lease_id",
    "writer_generation",
    "epoch_id",
    "input_release",
    "program_release",
    "application_release",
    "source_authority",
    "source_generation",
    "provider_release",
    "provider_set",
    "table_versions",
    "table_version_components",
    "overlay_segments",
    "policy_set",
    "resource_envelope",
    "proof_receipt",
    "compatibility_class",
    "retention_policy",
    "operation_selection",
    "transaction",
    "row_digest",
]
PROGRAMMATIC_ACTIVATION_EVENT_KEYS = {
    "event_id",
    "workspace_id",
    "operation_id",
    "predecessor_event_id",
    "predecessor_epoch",
    "ordinal",
    "execution_fence",
    "pins",
    "compatibility_class",
    "retention_policy",
    "command",
    "durable_commit",
    "backend_observation",
    "readback",
}
PROGRAMMATIC_OBSERVATION_TABLE_VERSION_RELATIONS = {
    "system.programmatic_dependency_observation",
    "system.programmatic_field_observation",
    "system.programmatic_provenance_observation",
    "system.programmatic_relation_observation",
    "system.programmatic_schema_observation",
}
PROGRAMMATIC_ACTIVATION_PIN_KEYS = {
    "epoch",
    "input_release",
    "program_release",
    "application_release",
    "source_authority",
    "source_generation",
    "provider_release",
    "provider_set",
    "table_versions",
    "overlay_segments",
    "policy_set",
    "resource_envelope",
    "proof_receipt",
}
PROGRAMMATIC_COMMAND_PIN_KEYS = PROGRAMMATIC_ACTIVATION_PIN_KEYS - {
    "epoch",
    "table_versions",
    "overlay_segments",
    "policy_set",
    "proof_receipt",
}


def _validate_programmatic_table_version_binding(value: Any, context: str) -> None:
    """Require a reversible runtime binding without accepting an authored reference.

    The five roots and exact versions do not exist until production seals the
    `ProgrammaticObservationDeltaPublication`.  Consequently WP33 may identify the
    required relations and production derivation only; a literal digest, URI,
    version, fallback, or sentinel would be false evidence.
    """

    binding = _mapping(value, f"{context} table-version binding")
    _strict_keys(
        binding,
        {"kind", "source", "constructor", "reference_projection", "components"},
        f"{context} table-version binding",
    )
    if (
        binding["kind"] != "runtime_derived_table_version_set"
        or binding["source"] != "sealed_programmatic_observation_delta_publication"
        or binding["constructor"] != "TableVersionSet::try_new"
        or binding["reference_projection"] != "TableVersionSet::reference"
    ):
        raise SuccessorEvidenceError(
            f"{context} table-version binding is not production-derived"
        )
    components = [
        _mapping(component, f"{context} table-version component")
        for component in _list(
            binding["components"], f"{context} table-version components"
        )
    ]
    observed_relations: set[str] = set()
    for component in components:
        _strict_keys(
            component,
            {"relation_id", "exact_delta_pin"},
            f"{context} table-version component",
        )
        relation_id = _nonempty_string(
            component["relation_id"], f"{context} table-version relation"
        )
        pin = _mapping(
            component["exact_delta_pin"], f"{context} exact Delta pin binding"
        )
        _strict_keys(pin, {"root", "version"}, f"{context} exact Delta pin binding")
        if pin != {
            "root": "publication_runtime_root",
            "version": "publication_exact_version",
        }:
            raise SuccessorEvidenceError(
                f"{context} table-version component authors a literal or sentinel pin"
            )
        if relation_id in observed_relations:
            raise SuccessorEvidenceError(
                f"{context} table-version binding duplicates a relation"
            )
        observed_relations.add(relation_id)
    if observed_relations != PROGRAMMATIC_OBSERVATION_TABLE_VERSION_RELATIONS:
        raise SuccessorEvidenceError(
            f"{context} table-version binding must contain exactly the five "
            "programmatic observation histories"
        )
    if any(
        relation_id.startswith("control.activation_event")
        for relation_id in observed_relations
    ):
        raise SuccessorEvidenceError(
            f"{context} activation-control backend is not an epoch table-version component"
        )


def _validate_programmatic_activation_chain(
    chain_value: Any, context: str
) -> list[Mapping[str, Any]]:
    """Validate the live typed activation relation and return its ordered chain."""
    chain = _mapping(chain_value, f"{context} activation chain")
    _strict_keys(
        chain, PROGRAMMATIC_ACTIVATION_CHAIN_KEYS, f"{context} activation chain"
    )
    relation = _mapping(
        chain["relation_contract"], f"{context} activation relation contract"
    )
    _strict_keys(
        relation,
        PROGRAMMATIC_ACTIVATION_RELATION_KEYS,
        f"{context} activation relation contract",
    )
    if relation != {
        "relation_id": "control.activation_event.v3",
        "storage_relation_id": "storage.delta.activation_event.v3",
        "schema_identity": "programmatic:control.activation_event.v3:arrow59-delta1",
        "provider_binding_id": "binding.delta.exact-snapshot.activation-event.v3",
        "schema_version": 3,
        "arrow_type_universe": (
            "arrow-array@59.2.0|arrow-schema@59.2.0|datafusion@55.0.0|"
            "deltalake@43a0cf10"
        ),
        "append_only": True,
        "change_data_feed": True,
        "fields": PROGRAMMATIC_ACTIVATION_FIELDS,
    }:
        raise SuccessorEvidenceError(
            f"{context} programmatic activation relation contract differs"
        )
    identity = _mapping(
        chain["typed_identity_contract"],
        f"{context} activation typed-identity contract",
    )
    if identity != {
        "contract_id": "codefabric.activation.typed-fixture-inputs.v1",
        "authority": (
            "explicit canonical bytes supplied to the production Rust byte-identity types"
        ),
        "widths": {
            "WorkspaceId": 16,
            "OperationId": 16,
            "PrincipalId": 16,
            "LeaseId": 16,
            "EpochId": 16,
            "ActivationEventId": 32,
            "IdempotencyKey": 32,
            "AuthorizationRef": 32,
            "release_and_policy_refs": 32,
            "OperationSelectionRef": 32,
            "TransactionRef": 32,
        },
        "classification": {
            "command_and_control_ids": "opaque typed inputs, not content hashes",
            "release_and_policy_refs": (
                "exact fixture release inputs, not acceptance checksums"
            ),
            "row_and_readback_integrity": (
                "computed and verified by the production activation-control codec; "
                "deliberately not authored by WP33"
            ),
        },
        "programmatic_readback": (
            "exact operation, transaction, fence, commit version, and row event "
            "equality; no digest surrogate"
        ),
    }:
        raise SuccessorEvidenceError(
            f"{context} activation typed-identity contract differs"
        )

    events = [
        _mapping(value, f"{context} activation event")
        for value in _list(chain["events"], f"{context} activation events")
    ]
    if not events:
        raise SuccessorEvidenceError(f"{context} activation relation is empty")
    seen_event_ids: set[str] = set()
    workspace_id: str | None = None
    predecessor: Mapping[str, Any] | None = None
    for ordinal, event in enumerate(events, 1):
        _strict_keys(
            event,
            PROGRAMMATIC_ACTIVATION_EVENT_KEYS,
            f"{context} activation event {ordinal}",
        )
        event_id = _nonempty_string(
            event["event_id"], f"{context} activation event identity"
        )
        operation_id = _nonempty_string(
            event["operation_id"], f"{context} activation operation identity"
        )
        event_workspace = _nonempty_string(
            event["workspace_id"], f"{context} activation workspace identity"
        )
        if (
            re.fullmatch(r"[0-9a-f]{64}", event_id) is None
            or re.fullmatch(r"[0-9a-f]{32}", operation_id) is None
            or re.fullmatch(r"[0-9a-f]{32}", event_workspace) is None
            or int(event_id, 16) == 0
            or int(operation_id, 16) == 0
            or int(event_workspace, 16) == 0
            or event_id in seen_event_ids
            or event["ordinal"] != ordinal
        ):
            raise SuccessorEvidenceError(
                f"{context} activation event identity/order differs"
            )
        seen_event_ids.add(event_id)
        if workspace_id is None:
            workspace_id = event_workspace
        elif workspace_id != event_workspace:
            raise SuccessorEvidenceError(
                f"{context} activation workspace identity differs"
            )

        pins = _mapping(event["pins"], f"{context} activation pins")
        _strict_keys(
            pins, PROGRAMMATIC_ACTIVATION_PIN_KEYS, f"{context} activation pins"
        )
        _validate_programmatic_table_version_binding(
            pins["table_versions"], f"{context} activation event {ordinal}"
        )
        if (
            pins["source_generation"] != ordinal
            or re.fullmatch(r"[0-9a-f]{32}", str(pins["epoch"])) is None
            or int(str(pins["epoch"]), 16) == 0
            or any(
                re.fullmatch(r"[0-9a-f]{64}", str(value)) is None
                or int(str(value), 16) == 0
                for key, value in pins.items()
                if key not in {"epoch", "source_generation", "table_versions"}
            )
        ):
            raise SuccessorEvidenceError(f"{context} activation pins differ")
        expected_head = (
            {"kind": "empty"}
            if predecessor is None
            else {"kind": "epoch", "epoch": predecessor["pins"]["epoch"]}
        )
        if (
            event["predecessor_event_id"]
            != (None if predecessor is None else predecessor["event_id"])
            or event["predecessor_epoch"] != expected_head
        ):
            raise SuccessorEvidenceError(
                f"{context} activation predecessor chain differs"
            )

        fence = _mapping(event["execution_fence"], f"{context} writer fence")
        _strict_keys(fence, {"lease_id", "generation"}, f"{context} writer fence")
        if (
            fence["generation"] != ordinal
            or re.fullmatch(r"[0-9a-f]{32}", str(fence["lease_id"])) is None
            or int(str(fence["lease_id"]), 16) == 0
        ):
            raise SuccessorEvidenceError(f"{context} writer fence differs")

        command = _mapping(event["command"], f"{context} FabricCommand")
        _strict_keys(
            command,
            {"identity", "ownership", "expected_head", "pins", "resources", "payload"},
            f"{context} FabricCommand",
        )
        command_identity = _mapping(command["identity"], f"{context} command identity")
        command_ownership = _mapping(
            command["ownership"], f"{context} command ownership"
        )
        command_pins = _mapping(command["pins"], f"{context} command pins")
        command_payload = _mapping(command["payload"], f"{context} command payload")
        _strict_keys(
            command_identity,
            {"operation_id", "idempotency_key"},
            f"{context} command identity",
        )
        _strict_keys(
            command_ownership,
            {"workspace_id", "principal_id", "authorization"},
            f"{context} command ownership",
        )
        if (
            command_identity.get("operation_id") != operation_id
            or re.fullmatch(
                r"[0-9a-f]{64}", str(command_identity.get("idempotency_key"))
            )
            is None
            or int(str(command_identity.get("idempotency_key")), 16) == 0
            or command_ownership.get("workspace_id") != event_workspace
            or re.fullmatch(r"[0-9a-f]{32}", str(command_ownership.get("principal_id")))
            is None
            or int(str(command_ownership.get("principal_id")), 16) == 0
            or re.fullmatch(
                r"[0-9a-f]{64}", str(command_ownership.get("authorization"))
            )
            is None
            or int(str(command_ownership.get("authorization")), 16) == 0
            or command["expected_head"] != expected_head
            or set(command_pins) != PROGRAMMATIC_COMMAND_PIN_KEYS
            or command_pins != {key: pins[key] for key in PROGRAMMATIC_COMMAND_PIN_KEYS}
            or command["resources"] != pins["resource_envelope"]
            or command_payload
            != {
                "kind": "ActivateEpoch",
                "candidate_epoch": pins["epoch"],
                "proof_receipt": pins["proof_receipt"],
            }
        ):
            raise SuccessorEvidenceError(f"{context} FabricCommand binding differs")

        durable = _mapping(event["durable_commit"], f"{context} durable commit")
        _strict_keys(
            durable, {"operation_selection", "transaction"}, f"{context} durable commit"
        )
        operation_selection = _nonempty_string(
            durable["operation_selection"], f"{context} operation selection"
        )
        transaction = _nonempty_string(
            durable["transaction"], f"{context} activation transaction"
        )
        if (
            re.fullmatch(r"[0-9a-f]{64}", operation_selection) is None
            or int(operation_selection, 16) == 0
            or re.fullmatch(r"[0-9a-f]{64}", transaction) is None
            or int(transaction, 16) == 0
            or re.fullmatch(r"[0-9a-f]{64}", str(event["compatibility_class"])) is None
            or int(str(event["compatibility_class"]), 16) == 0
            or re.fullmatch(r"[0-9a-f]{64}", str(event["retention_policy"])) is None
            or int(str(event["retention_policy"]), 16) == 0
        ):
            raise SuccessorEvidenceError(
                f"{context} durable typed control identity differs"
            )
        backend = _mapping(
            event["backend_observation"], f"{context} backend observation"
        )
        readback = _mapping(event["readback"], f"{context} activation readback")
        predecessor_version = ordinal - 1
        if backend != {
            "control_root_binding": "runtime-created private activation-control URL",
            "control_predecessor_version": predecessor_version,
            "control_commit_version": ordinal,
            "marker_observed_version": ordinal,
            "operation_id": operation_id,
            "transaction": transaction,
            "writer_fence": fence,
            "read_version": {"kind": "exact", "version": predecessor_version},
            "num_retries": 0,
        }:
            raise SuccessorEvidenceError(
                f"{context} backend activation observation differs"
            )
        if readback != {
            "relation_id": "control.activation_event.v3",
            "storage_relation_id": "storage.delta.activation_event.v3",
            "control_commit_version": ordinal,
            "operation_id": operation_id,
            "transaction": transaction,
            "writer_fence": fence,
            "row_event_id": event_id,
        }:
            raise SuccessorEvidenceError(
                f"{context} durable activation readback differs"
            )
        predecessor = event
    return events


def _programmatic_activation_outcome(
    inputs: Mapping[str, Any], head: Mapping[str, Any]
) -> dict[str, Any]:
    return {
        "selected_event_id": head["event_id"],
        "selected_epoch": head["pins"]["epoch"],
        "predecessor_event_id": head["predecessor_event_id"],
        "command": head["command"],
        "fabric_epoch_pins": head["pins"],
        "durable_commit": head["durable_commit"],
        "backend_observation": head["backend_observation"],
        "readback": head["readback"],
        "installation": {
            "state": "installed",
            "epoch": head["pins"]["epoch"],
            "readback_event_id": head["readback"]["row_event_id"],
            "control_commit_version": head["readback"]["control_commit_version"],
        },
        "receipt_cache_reconciliation": inputs["receipt_cache_observation"][
            "reconciliation"
        ],
        "acknowledgement": {
            "state": "acknowledged",
            "event_id": head["event_id"],
            "control_commit_version": head["readback"]["control_commit_version"],
        },
        "admission_state": "open",
        "candidate_present_during_reconcile": inputs["candidate_memory_observation"][
            "candidate_present"
        ],
        "receipt_cache_authoritative": inputs["receipt_cache_observation"][
            "authoritative"
        ],
    }


def _activation_events(
    chain_value: Any, context: str
) -> tuple[list[Mapping[str, Any]], Mapping[str, Any]]:
    chain = _mapping(chain_value, f"{context} activation chain")
    _strict_keys(chain, ACTIVATION_CHAIN_KEYS, f"{context} activation chain")
    readback = _mapping(
        chain["durable_relation_readback"], f"{context} activation relation readback"
    )
    _strict_keys(
        readback,
        ACTIVATION_RELATION_READBACK_KEYS,
        f"{context} activation relation readback",
    )
    if (
        readback["relation_id"] != "lifecycle.activation_event"
        or not str(readback["table_root"]).startswith("file:///")
        or re.fullmatch(r"b3:[0-9a-f]{64}", str(readback["table_id"])) is None
        or readback["operation_marker_source"]
        != "activation_event.operation_marker.command_id"
    ):
        raise SuccessorEvidenceError(
            f"{context} durable activation relation identity differs"
        )
    events = [
        _mapping(value, f"{context} activation event")
        for value in _list(chain["events"], f"{context} activation events")
    ]
    if not events:
        raise SuccessorEvidenceError(f"{context} activation relation is empty")
    versions: list[int] = []
    for index, event in enumerate(events, 1):
        durable = _mapping(
            event.get("durable_event_identity"),
            f"{context} activation durable event {index}",
        )
        _strict_keys(
            durable,
            ACTIVATION_DURABLE_EVENT_KEYS,
            f"{context} activation durable event {index}",
        )
        version = durable["delta_version"]
        if (
            durable["table_id"] != readback["table_id"]
            or not isinstance(version, int)
            or isinstance(version, bool)
        ):
            raise SuccessorEvidenceError(
                f"{context} activation event is not bound to its durable relation"
            )
        payload = dict(event)
        del payload["durable_event_identity"]
        if durable["event_payload_checksum"] != _canonical_b3(payload):
            raise SuccessorEvidenceError(
                f"{context} activation event payload checksum differs"
            )
        versions.append(version)
    if versions != list(range(len(events))):
        raise SuccessorEvidenceError(
            f"{context} durable activation versions are not contiguous"
        )
    if readback["readback_version"] != versions[-1] or readback[
        "readback_checksum"
    ] != _canonical_b3(events):
        raise SuccessorEvidenceError(
            f"{context} durable activation readback identity differs"
        )
    return events, readback


def _activation_head(
    events: Sequence[Mapping[str, Any]], context: str
) -> Mapping[str, Any]:
    by_epoch: dict[str, Mapping[str, Any]] = {}
    predecessors: set[str] = set()
    workspace_id: str | None = None
    for index, event in enumerate(events, 1):
        _strict_keys(
            event, ACTIVATION_EVENT_KEYS, f"{context} activation event {index}"
        )
        epoch = _nonempty_string(event["epoch"], f"{context} activation epoch")
        if epoch in by_epoch:
            raise SuccessorEvidenceError(f"{context} duplicates activation epoch")
        by_epoch[epoch] = event
        predecessor = event["predecessor"]
        if predecessor is not None:
            predecessors.add(_nonempty_string(predecessor, f"{context} predecessor"))
        for key in (
            "operation_marker",
            "provider_release_vector",
            "table_version_vector",
            "proof_set",
        ):
            if not _mapping(event[key], f"{context} activation {key}"):
                raise SuccessorEvidenceError(f"{context} activation {key} is empty")
        writer_fence = _mapping(
            event["writer_fence"], f"{context} activation writer fence"
        )
        _strict_keys(
            writer_fence,
            {"workspace_id", "generation", "holder"},
            f"{context} activation writer fence",
        )
        event_workspace = _nonempty_string(
            writer_fence["workspace_id"], f"{context} activation workspace"
        )
        if re.fullmatch(r"workspace:[0-9a-f]{32}", event_workspace) is None:
            raise SuccessorEvidenceError(
                f"{context} activation workspace identity differs"
            )
        if workspace_id is None:
            workspace_id = event_workspace
        elif event_workspace != workspace_id:
            raise SuccessorEvidenceError(
                f"{context} activation workspace identity differs"
            )
        if writer_fence["generation"] != event["sequence"]:
            raise SuccessorEvidenceError(
                f"{context} activation fence generation differs"
            )
        _nonempty_string(writer_fence["holder"], f"{context} activation fence holder")
        if event["proof_set"].get("state") != "complete":
            raise SuccessorEvidenceError(f"{context} activation proof is incomplete")
    if not predecessors <= set(by_epoch):
        raise SuccessorEvidenceError(f"{context} activation predecessor is missing")
    heads = set(by_epoch) - predecessors
    if len(heads) != 1:
        raise SuccessorEvidenceError(f"{context} activation chain lacks unique head")
    return by_epoch[heads.pop()]


def _validate_activation_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    chain = _mapping(inputs["activation_chain"], f"{context} activation chain")
    if set(chain) == PROGRAMMATIC_ACTIVATION_CHAIN_KEYS:
        events = _validate_programmatic_activation_chain(chain, context)
        policy = _mapping(inputs["recovery_policy"], f"{context} recovery policy")
        if policy != {
            "admission_during_recovery": "closed",
            "admission_after_install_readback": "open",
            "acknowledgement_after_install_readback": "required",
            "candidate_epoch_allowed": False,
            "selection": "unique predecessor-event-linked complete head",
            "receipt_cache_authority": False,
            "required_relation": "control.activation_event.v3",
        }:
            raise SuccessorEvidenceError(
                f"{context} activation recovery policy differs"
            )
        if inputs["candidate_memory_observation"] != {
            "candidate_present": False
        } or inputs["receipt_cache_observation"] != {
            "present": False,
            "authoritative": False,
            "reconciliation": "complete_non_authoritative",
        }:
            raise SuccessorEvidenceError(
                f"{context} recovery relies on candidate or receipt authority"
            )
        expected = _programmatic_activation_outcome(inputs, events[-1])
        if (
            decoded["terminal"]
            != "selected_epoch_installed_admission_reopened_acknowledged"
            or decoded["columns"] != ["outcome"]
            or decoded["rows"] != [[expected]]
        ):
            raise SuccessorEvidenceError(
                f"{context} programmatic recovery outcome differs"
            )
        return
    events, readback = _activation_events(inputs["activation_chain"], context)
    head = _activation_head(events, context)
    if inputs["recovery_policy"].get("admission_during_recovery") != "closed":
        raise SuccessorEvidenceError(
            f"{context} recovery does not keep admission closed"
        )
    row = decoded["rows"][0]
    selected = dict(zip(decoded["columns"], row, strict=True))
    if decoded[
        "terminal"
    ] != "selected_epoch_installed_admission_closed" or selected != {
        "selected_epoch": head["epoch"],
        "predecessor": head["predecessor"],
        "selected_event": head,
        "operation_marker": head["operation_marker"],
        "writer_fence": head["writer_fence"],
        "input_set_id": head["input_set_id"],
        "program_release": head["program_release"],
        "application_release": head["application_release"],
        "source_generation": head["source_generation"],
        "provider_release_vector": head["provider_release_vector"],
        "table_version_vector": head["table_version_vector"],
        "policy_release": head["policy_release"],
        "proof_set": head["proof_set"],
        "durable_event_identity": head["durable_event_identity"],
        "activation_relation_readback": readback,
        "candidate_present_during_reconcile": inputs["candidate_memory_observation"][
            "candidate_present"
        ],
        "receipt_authoritative": inputs["receipt_cache_observation"]["authoritative"],
        "admission_state": "closed",
    }:
        raise SuccessorEvidenceError(f"{context} recovery overclaims its selected head")


def _validate_resource_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    batch = _mapping(inputs["actual_output_batch"], f"{context} output batch")
    if set(batch) == {
        "schema_contract",
        "canonical_response",
        "rows",
        "row_count",
        "ipc_contract",
        "measured_ipc_bytes",
        "artifact_identity",
    }:
        _validate_programmatic_resource_inputs(inputs, decoded, context)
        return
    _strict_keys(
        batch,
        {
            "schema",
            "rows",
            "row_count",
            "canonical_byte_count",
            "byte_accounting",
            "artifact_identity",
        },
        f"{context} output batch",
    )
    if batch["row_count"] != len(batch["rows"]):
        raise SuccessorEvidenceError(f"{context} output row count is not derived")
    accounting = _mapping(batch["byte_accounting"], f"{context} byte accounting")
    components = (
        accounting.get("offset_buffer_bytes"),
        accounting.get("value_buffer_bytes"),
        accounting.get("validity_buffer_bytes"),
        accounting.get("alignment_padding_bytes"),
    )
    if (
        any(not isinstance(value, int) or value < 0 for value in components)
        or sum(components) != accounting.get("derived_total_bytes")
        or accounting.get("derived_total_bytes") != batch["canonical_byte_count"]
    ):
        raise SuccessorEvidenceError(f"{context} output byte count is not derived")
    artifact = _mapping(batch["artifact_identity"], f"{context} artifact identity")
    _strict_keys(
        artifact,
        {"artifact_id", "schema_digest", "canonical_batch_checksum", "immutable"},
        f"{context} artifact identity",
    )
    if (
        artifact["schema_digest"] != _canonical_b3(batch["schema"])
        or artifact["canonical_batch_checksum"]
        != _canonical_b3({"schema": batch["schema"], "rows": batch["rows"]})
        or artifact["immutable"] is not True
    ):
        raise SuccessorEvidenceError(f"{context} immutable artifact identity differs")
    cpu = _mapping(inputs["cpu_budget"], f"{context} CPU budget")
    if (
        cpu.get("cpu_milliseconds", 0) <= 0
        or cpu.get("observed_cpu_milliseconds", 0) <= 0
        or cpu["observed_cpu_milliseconds"] > cpu["cpu_milliseconds"]
        or cpu.get("observed_runtime_milliseconds", 0) <= 0
        or cpu["observed_runtime_milliseconds"]
        > inputs["resource_budget"]["milliseconds"]
    ):
        raise SuccessorEvidenceError(f"{context} CPU budget is absent")
    if inputs["delivery_policy"].get("delivery") != "resource":
        raise SuccessorEvidenceError(f"{context} resource publication is not selected")
    cancellation = _mapping(
        inputs["cancellation_state"], f"{context} cancellation state"
    )
    if set(cancellation) != {"cancelled", "cancellation_ordinal"}:
        raise SuccessorEvidenceError(f"{context} cancellation input is incomplete")
    lease = _mapping(inputs["lease_policy"], f"{context} lease policy")
    registry = _mapping(inputs["registry_state"], f"{context} registry state")
    if (
        lease.get("artifact_id") != artifact["artifact_id"]
        or lease.get("release_on_cancel") is not True
        or registry.get("artifact_id") != artifact["artifact_id"]
        or registry.get("next_publication_generation")
        != registry.get("published_before", -1) + 1
        or not str(registry.get("resource_uri", "")).startswith(
            "codefabric://resource/"
        )
    ):
        raise SuccessorEvidenceError(f"{context} lease/publication identity differs")
    terminal = dict(zip(decoded["columns"], decoded["rows"][0], strict=True))
    if terminal != {
        "query_id": inputs["query_identity"]["query_id"],
        "state": "complete",
        "rows": batch["row_count"],
        "bytes": batch["canonical_byte_count"],
        "published_resources": 1,
        "terminal_provenance": _resource_terminal_provenance(
            inputs, "complete", "active", "published"
        ),
    }:
        raise SuccessorEvidenceError(
            f"{context} terminal is not derived from output/delivery"
        )


def _programmatic_resource_terminal(
    inputs: Mapping[str, Any], state: str, published: bool
) -> dict[str, Any]:
    batch = inputs["actual_output_batch"]
    if state == "complete":
        terminal_state = "complete"
        public_error = None
    elif state == "hard_limit_exceeded":
        terminal_state = "failed"
        public_error = "QUERY_HARD_LIMIT_EXCEEDED"
    elif state == "cancelled":
        terminal_state = "cancelled"
        public_error = "CANCELLED"
    else:
        raise SuccessorEvidenceError(f"unsupported resource terminal state: {state}")
    return {
        "query_id": inputs["query_identity"]["query_id"],
        "state": terminal_state,
        "public_error": public_error,
        "published_rows": batch["row_count"] if published else 0,
        "byte_observation": "required_at_execution",
        "published_resources": 1 if published else 0,
        "resource_uri": inputs["registry_state"]["resource_uri"] if published else None,
        "complete_partial_list": False,
        "terminal_provenance": {
            "schema_contract": batch["schema_contract"],
            "ipc_contract": batch["ipc_contract"],
            "artifact_identity": batch["artifact_identity"],
            "reservation_state": "held" if published else "released",
            "cancellation": inputs["cancellation_state"],
            "lease_state": "active" if published else "released",
            "publication_state": "published" if published else "not_published",
        },
    }


def _validate_programmatic_resource_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    batch = _mapping(inputs["actual_output_batch"], f"{context} output batch")
    schema = _mapping(batch["schema_contract"], f"{context} Arrow schema contract")
    _strict_keys(
        schema,
        {
            "relation_id",
            "arrow_type_universe",
            "fields",
            "metadata",
            "canonical_schema_identity",
        },
        f"{context} Arrow schema contract",
    )
    schema_preimage = dict(schema)
    schema_identity = schema_preimage.pop("canonical_schema_identity")
    if (
        schema["relation_id"] != "query.result.ordinals.v1"
        or schema["arrow_type_universe"]
        != ("arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5")
        or schema["fields"]
        != [
            {
                "name": "ordinal",
                "data_type": "uint64",
                "nullable": False,
                "metadata": {
                    "codefabric.field_id": "query.result.ordinals.v1.ordinal",
                    "codefabric.field_ordinal": "0",
                    "codefabric.semantic_role": "deterministic_result_ordinal",
                },
            }
        ]
        or schema["metadata"]
        != {
            "codefabric.relation_id": "query.result.ordinals.v1",
            "codefabric.semantic_encoding": "typed-arrow-result-resource",
            "codefabric.schema_contract_version": "1",
        }
        or schema_identity != _canonical_b3(schema_preimage)
    ):
        raise SuccessorEvidenceError(f"{context} Arrow schema contract differs")
    plan = _mapping(inputs["bound_plan"], f"{context} bound plan")
    if plan != {
        "output_schema_contract": schema,
        "plan_schema_authority": "DataFusion LogicalPlan::schema",
        "estimated_rows": batch["row_count"],
    }:
        raise SuccessorEvidenceError(
            f"{context} DataFusion plan/schema binding differs"
        )
    rows = _list(batch["rows"], f"{context} Arrow rows")
    if (
        batch["row_count"] != len(rows)
        or not rows
        or any(
            not isinstance(row, list)
            or len(row) != 1
            or not isinstance(row[0], int)
            or isinstance(row[0], bool)
            or not 0 <= row[0] <= 0xFFFF_FFFF_FFFF_FFFF
            for row in rows
        )
        or [row[0] for row in rows] != sorted({row[0] for row in rows})
    ):
        raise SuccessorEvidenceError(
            f"{context} Arrow rows violate the deterministic UInt64 schema"
        )
    query_identity = _mapping(inputs["query_identity"], f"{context} query identity")
    _strict_keys(
        query_identity,
        {"query_id", "workspace", "epoch", "owning_agent_id", "snapshot_id"},
        f"{context} query identity",
    )
    canonical_response = _mapping(
        batch["canonical_response"], f"{context} canonical response"
    )
    expected_response = {
        "query_id": query_identity["query_id"],
        "workspace_id": query_identity["workspace"],
        "fabric_epoch_id": query_identity["epoch"],
        "snapshot_id": query_identity["snapshot_id"],
        "relation_id": schema["relation_id"],
        "columns": ["ordinal"],
        "rows": rows,
        "coverage": {"state": "COMPLETE"},
        "errors": [],
    }
    if canonical_response != expected_response:
        raise SuccessorEvidenceError(
            f"{context} canonical response is not derived from the bound result"
        )
    canonical_response_checksum = _canonical_b3(canonical_response)
    ipc = _mapping(batch["ipc_contract"], f"{context} IPC contract")
    if (
        ipc
        != {
            "format": "Arrow IPC stream",
            "metadata_version": "V5",
            "schema_message_count": 1,
            "dictionary_scope": "one stream",
            "physical_end_of_stream_required": True,
            "identity_input": "exact emitted IPC bytes observed at execution",
        }
        or batch["measured_ipc_bytes"] is not None
    ):
        raise SuccessorEvidenceError(
            f"{context} freezes an IPC byte observation before execution"
        )
    artifact = _mapping(batch["artifact_identity"], f"{context} artifact identity")
    identity_exclusions = [
        "resource URI",
        "physical storage path",
        "publication generation",
        "lease identity and expiry",
        "mutable access counters",
        "transport-specific emitted byte checksum",
    ]
    artifact_id = _validate_cbef_recipe(
        artifact.get("identity_recipe"),
        domain_code=23,
        domain_name="RESULT_ARTIFACT_V2",
        output_prefix="artifact",
        fields=[
            (
                1,
                "workspace_id",
                7,
                "ID",
                _public_id_bytes(query_identity["workspace"], "workspace"),
                query_identity["workspace"],
            ),
            (
                2,
                "owning_agent_id",
                2,
                "UTF8",
                _cbef_utf8_value(
                    query_identity["owning_agent_id"],
                    context=f"{context} owning agent",
                ),
                query_identity["owning_agent_id"],
            ),
            (
                3,
                "fabric_epoch_id",
                7,
                "ID",
                _public_id_bytes(query_identity["epoch"], "fabric-epoch"),
                query_identity["epoch"],
            ),
            (
                4,
                "snapshot_id",
                7,
                "ID",
                _public_id_bytes(query_identity["snapshot_id"], "snapshot"),
                query_identity["snapshot_id"],
            ),
            (
                5,
                "canonical_response_checksum",
                8,
                "DIGEST",
                _digest_bytes(canonical_response_checksum, "b3"),
                canonical_response_checksum,
            ),
            (
                6,
                "format",
                2,
                "UTF8",
                _cbef_utf8_value(
                    "arrow-ipc",
                    ascii_lower=True,
                    context=f"{context} artifact format",
                ),
                "arrow-ipc",
            ),
            (
                7,
                "format_version",
                2,
                "UTF8",
                _cbef_utf8_value("V5", context=f"{context} format version"),
                "V5",
            ),
        ],
        excluded=identity_exclusions,
        context=f"{context} result artifact",
    )
    resource_uri = (
        "codefabric-result://"
        f"{str(query_identity['workspace']).removeprefix('workspace:')}/"
        f"{artifact_id.removeprefix('artifact:')}"
    )
    if artifact != {
        "artifact_id": artifact_id,
        "identity_recipe": artifact["identity_recipe"],
        "resource_uri": resource_uri,
        "workspace_id": query_identity["workspace"],
        "owning_agent_id": query_identity["owning_agent_id"],
        "fabric_epoch_id": query_identity["epoch"],
        "snapshot_id": query_identity["snapshot_id"],
        "canonical_schema_identity": schema_identity,
        "canonical_response_checksum": canonical_response_checksum,
        "format": "arrow-ipc",
        "format_version": "V5",
        "expected_ipc_identity_recipe": {
            "algorithm": "unkeyed BLAKE3",
            "input": "exact Arrow IPC V5 stream bytes",
            "output": "b3 lowercase hex",
            "frozen_without_bytes": False,
        },
        "immutable": True,
    }:
        raise SuccessorEvidenceError(
            f"{context} typed resource artifact identity differs"
        )
    budget = _mapping(inputs["resource_budget"], f"{context} resource budget")
    reservation = _mapping(inputs["reservation"], f"{context} reservation")
    cpu = _mapping(inputs["cpu_budget"], f"{context} CPU budget")
    cancellation = _mapping(
        inputs["cancellation_state"], f"{context} cancellation state"
    )
    registry = _mapping(inputs["registry_state"], f"{context} registry state")
    lease = _mapping(inputs["lease_policy"], f"{context} lease policy")
    if (
        budget.get("rows", 0) < batch["row_count"]
        or budget.get("bytes", 0) <= 0
        or budget.get("milliseconds", 0) <= 0
        or budget.get("concurrency", 0) <= 0
        or reservation.get("memory_bytes") != budget["bytes"]
        or reservation.get("concurrency_slots") != budget["concurrency"]
        or cpu.get("cpu_milliseconds") != budget.get("cpu_milliseconds")
        or not 0 < cpu.get("observed_cpu_milliseconds", 0) <= budget["cpu_milliseconds"]
        or not 0 < cpu.get("observed_runtime_milliseconds", 0) <= budget["milliseconds"]
        or cancellation != {"cancelled": False, "cancellation_ordinal": None}
        or inputs["delivery_policy"].get("delivery") != "resource"
        or inputs["delivery_policy"].get("resource_externalization_required")
        is not True
        or registry.get("artifact_id") != artifact["artifact_id"]
        or registry.get("resource_uri") != resource_uri
        or registry.get("owning_agent_id") != query_identity["owning_agent_id"]
        or registry.get("workspace_id") != query_identity["workspace"]
        or registry.get("fabric_epoch_id") != query_identity["epoch"]
        or registry.get("snapshot_id") != query_identity["snapshot_id"]
        or registry.get("canonical_response_checksum") != canonical_response_checksum
        or registry.get("format") != "arrow-ipc"
        or registry.get("format_version") != "V5"
        or registry.get("next_publication_generation")
        != registry.get("published_before", -1) + 1
        or lease.get("artifact_id") != artifact["artifact_id"]
        or lease.get("release_on_cancel") is not True
    ):
        raise SuccessorEvidenceError(
            f"{context} resource admission/publication contract differs"
        )
    if (
        decoded["terminal"] != "complete"
        or decoded["columns"] != ["terminal"]
        or decoded["rows"]
        != [[_programmatic_resource_terminal(inputs, "complete", True)]]
    ):
        raise SuccessorEvidenceError(
            f"{context} programmatic resource terminal differs"
        )


def _resource_terminal_provenance(
    inputs: Mapping[str, Any],
    artifact_state: str,
    lease_state: str,
    publication_state: str,
) -> dict[str, Any]:
    batch = inputs["actual_output_batch"]
    artifact = batch["artifact_identity"]
    lease = inputs["lease_policy"]
    registry = inputs["registry_state"]
    cpu = inputs["cpu_budget"]
    return {
        "artifact": {
            **artifact,
            "row_count": batch["row_count"],
            "canonical_byte_count": batch["canonical_byte_count"],
            "state": artifact_state,
        },
        "delivery_policy": inputs["delivery_policy"],
        "reservation": {
            **inputs["reservation"],
            "state": "held" if publication_state == "published" else "released",
        },
        "cpu_runtime_accounting": {
            "cpu_budget_milliseconds": cpu["cpu_milliseconds"],
            "observed_cpu_milliseconds": cpu["observed_cpu_milliseconds"],
            "runtime_budget_milliseconds": inputs["resource_budget"]["milliseconds"],
            "observed_runtime_milliseconds": cpu["observed_runtime_milliseconds"],
            "accounting_scope": cpu["accounting_scope"],
        },
        "cancellation": inputs["cancellation_state"],
        "lease": {
            "lease_id": lease["lease_id"],
            "artifact_id": lease["artifact_id"],
            "result_lease_seconds": lease["result_lease_seconds"],
            "release_on_cancel": lease["release_on_cancel"],
            "state": lease_state,
        },
        "publication": {
            "publication_id": registry["publication_id"],
            "artifact_id": registry["artifact_id"],
            "resource_uri": registry["resource_uri"],
            "registry_generation": registry["next_publication_generation"],
            "published_before": registry["published_before"],
            "state": publication_state,
        },
    }


SECURITY_TERMINAL_COLUMNS = [
    "job_id",
    "scenario",
    "state",
    "public_error",
    "trust_policy_id",
    "authorization_id",
    "launcher_receipt_id",
    "launcher_proof_id",
    "provenance_id",
    "public_visibility",
    "capability_state",
    "attempted_action_count",
    "contained_action_count",
    "hostile_action_closure",
    "secret_bytes",
    "surviving_children",
]

SECURITY_REQUIRED_HOSTILE_ACTION_IDS = [
    "credential-read",
    "network-open",
    "source-parent-symlink-path-escape",
    "inherited-fd-use",
    "surviving-child",
    "output-explosion",
    "process-exhaustion",
    "wall-timeout",
    "cpu-exhaustion",
    "memory-exhaustion",
]

SECURITY_REQUIRED_HOST_PREREQUISITES = {
    "containment_substrate_available": True,
    "compiled_seccomp_policy_authorized": True,
    "no_new_privileges": True,
    "network_namespace_isolated": True,
    "credentials_stripped": True,
    "workspace_read_only": True,
    "unrelated_file_descriptors_closed": True,
    "process_group_and_cgroup_cleanup": True,
    "hostile_escape_matrix_executed": True,
}

SECURITY_LAUNCHER_EVIDENCE_CONTRACT = {
    "contract_id": "rust-launcher-evidence-v1",
    "identity_algorithm": "blake3-rfc8785-v1",
    "receipt_fields": [
        "contract_id",
        "trust_policy_id",
        "job_id",
        "authorization_id",
        "requested_profile",
        "launcher_constraints_digest",
        "resource_limits_digest",
    ],
    "proof_fields": [
        "launcher_receipt_id",
        "attempted_action_count",
        "contained_action_count",
        "hostile_action_closure_digest",
        "upstream_hostile_suite_proof_id",
    ],
    "provenance_fields": [
        "trust_policy_id",
        "job_id",
        "authorization_id",
        "launcher_receipt_id",
        "launcher_proof_id",
        "terminal_state",
        "capability_state",
        "public_visibility",
    ],
    "observation_fields": [
        "action_id",
        "attempted",
        "contained",
        "expected_terminal",
        "observed_terminal",
        "containment",
        "observation_id",
    ],
    "required_action_ids": SECURITY_REQUIRED_HOSTILE_ACTION_IDS,
}


def _security_launcher_receipt_id(
    inputs: Mapping[str, Any],
    job: Mapping[str, Any],
    authorization_id: str,
) -> str:
    contract = inputs["launcher_evidence_contract"]
    policy = inputs["trust_policy"]
    receipt_payload = {
        "contract_id": contract["contract_id"],
        "trust_policy_id": policy["policy_id"],
        "job_id": job["job_id"],
        "authorization_id": authorization_id,
        "requested_profile": job["requested_profile"],
        "launcher_constraints_digest": _canonical_b3(inputs["launcher_constraints"]),
        "resource_limits_digest": _canonical_b3(inputs["resource_limits"]),
    }
    return f"launcher-receipt:{_canonical_b3(receipt_payload)}"


def _security_launcher_identities(
    inputs: Mapping[str, Any],
    job: Mapping[str, Any],
    authorization_id: str,
    action_closure: Sequence[Mapping[str, Any]],
    terminal_state: str,
    capability_state: str,
    public_visibility: str,
    upstream_hostile_suite_proof_id: str | None = None,
) -> tuple[str, str, str]:
    policy = inputs["trust_policy"]
    launcher_receipt_id = _security_launcher_receipt_id(inputs, job, authorization_id)
    attempted_action_count = sum(
        observation.get("attempted") is True for observation in action_closure
    )
    contained_action_count = sum(
        observation.get("contained") is True for observation in action_closure
    )
    proof_payload = {
        "launcher_receipt_id": launcher_receipt_id,
        "attempted_action_count": attempted_action_count,
        "contained_action_count": contained_action_count,
        "hostile_action_closure_digest": _canonical_b3(list(action_closure)),
        "upstream_hostile_suite_proof_id": upstream_hostile_suite_proof_id,
    }
    launcher_proof_id = f"launcher-proof:{_canonical_b3(proof_payload)}"
    provenance_payload = {
        "trust_policy_id": policy["policy_id"],
        "job_id": job["job_id"],
        "authorization_id": authorization_id,
        "launcher_receipt_id": launcher_receipt_id,
        "launcher_proof_id": launcher_proof_id,
        "terminal_state": terminal_state,
        "capability_state": capability_state,
        "public_visibility": public_visibility,
    }
    provenance_id = f"provenance:{_canonical_b3(provenance_payload)}"
    return launcher_receipt_id, launcher_proof_id, provenance_id


def _security_preflight_provenance_id(
    inputs: Mapping[str, Any],
    job: Mapping[str, Any],
    authorization_id: str,
) -> str:
    payload = {
        "trust_policy_id": inputs["trust_policy"]["policy_id"],
        "job_id": job["job_id"],
        "authorization_id": authorization_id,
        "launcher_receipt_id": None,
        "launcher_proof_id": None,
        "terminal_state": "denied",
        "capability_state": "not_advertised",
        "public_visibility": "PUBLIC_DENIAL_ONLY",
    }
    return f"provenance:{_canonical_b3(payload)}"


def _security_job_by_profile(
    jobs: Sequence[Mapping[str, Any]], profile: str, context: str
) -> Mapping[str, Any]:
    selected = [job for job in jobs if job.get("requested_profile") == profile]
    if len(selected) != 1:
        raise SuccessorEvidenceError(
            f"{context} must contain exactly one {profile} provider job"
        )
    return selected[0]


def _security_action_closure(
    actions: Sequence[Mapping[str, Any]],
    *,
    launcher_receipt_id: str | None,
    attempted: bool,
    contained: bool | str,
) -> list[dict[str, Any]]:
    closure: list[dict[str, Any]] = []
    for action in actions:
        observed_terminal = action["expected_terminal"] if attempted else "not_executed"
        observation_payload = {
            "launcher_receipt_id": launcher_receipt_id,
            "action_id": action["action_id"],
            "executable_digest": _canonical_b3(action["executable"]),
            "observed_terminal": observed_terminal,
            "contained": contained,
        }
        closure.append(
            {
                "action_id": action["action_id"],
                "attempted": attempted,
                "contained": contained,
                "expected_terminal": action["expected_terminal"],
                "observed_terminal": observed_terminal,
                "containment": action["expected_containment"],
                "observation_id": (
                    f"launcher-observation:{_canonical_b3(observation_payload)}"
                    if attempted
                    else None
                ),
            }
        )
    return closure


def _validate_unavailable_security_inputs(
    inputs: Mapping[str, Any],
    decoded: Mapping[str, Any],
    jobs: Sequence[Mapping[str, Any]],
    trusted_job: Mapping[str, Any],
    hostile_job: Mapping[str, Any],
    context: str,
) -> None:
    """Validate fail-closed host preflight without inventing hostile execution."""
    policy = _mapping(inputs["trust_policy"], f"{context} trust policy")
    if policy != {
        "policy_id": "rust-compilation-trust-v1",
        "default_profile": "untrusted",
        "trusted_local_requires_distinct_authorization": True,
        "trusted_local_is_degraded": True,
        "untrusted_requires_all_host_prerequisites": True,
        "fail_closed": True,
    }:
        raise SuccessorEvidenceError(f"{context} successor trust policy differs")
    authorization = _mapping(
        inputs["explicit_authorization"], f"{context} explicit authorization"
    )
    _strict_keys(
        authorization,
        {"job_id", "trusted_local", "authorization_id"},
        f"{context} explicit authorization",
    )
    if authorization != {
        "job_id": trusted_job["job_id"],
        "trusted_local": False,
        "authorization_id": "authorization:none",
    }:
        raise SuccessorEvidenceError(
            f"{context} baseline trusted-local authorization differs"
        )
    evidence = _mapping(
        inputs["launcher_evidence_contract"], f"{context} launcher evidence contract"
    )
    if evidence != {
        "contract_id": "rust-launcher-evidence-v2",
        "required_prerequisites": list(SECURITY_REQUIRED_HOST_PREREQUISITES),
        "receipt_required_before_execution": True,
        "proof_required_for_capability": True,
        "absence_terminal": "SANDBOX_UNAVAILABLE",
        "hostile_actions_are_not_assumed_executed": True,
    }:
        raise SuccessorEvidenceError(
            f"{context} successor launcher evidence contract differs"
        )
    constraints = _mapping(
        inputs["launcher_constraints"], f"{context} launcher constraints"
    )
    _strict_keys(
        constraints,
        {"required", "observed_host", "untrusted_admission"},
        f"{context} launcher constraints",
    )
    required = _mapping(
        constraints["required"], f"{context} required host prerequisites"
    )
    observed = _mapping(
        constraints["observed_host"], f"{context} observed host prerequisites"
    )
    if (
        required != SECURITY_REQUIRED_HOST_PREREQUISITES
        or constraints["untrusted_admission"] != "unavailable"
    ):
        raise SuccessorEvidenceError(
            f"{context} untrusted host prerequisite contract differs"
        )
    if not set(required) <= set(observed) or all(
        observed[key] == value for key, value in required.items()
    ):
        raise SuccessorEvidenceError(
            f"{context} claims unavailable without an observed prerequisite gap"
        )
    cgroup = _mapping(observed.get("cgroup_v2"), f"{context} cgroup-v2 observation")
    if cgroup != {
        "delegated_controllers": ["cpu", "memory", "pids"],
        "pre_exec_self_placement": True,
        "cpu_memory_pids_swap_bounds": True,
        "aggregate_kernel_accounting": True,
    }:
        raise SuccessorEvidenceError(
            f"{context} cgroup-v2 substrate observation differs"
        )

    resource_limits = _mapping(inputs["resource_limits"], f"{context} resource limits")
    if set(resource_limits) != {
        "wall_ms",
        "cpu_ms",
        "memory_bytes",
        "processes",
        "open_files",
        "output_bytes",
        "arrow_output_bytes",
    } or any(
        not isinstance(value, int) or isinstance(value, bool) or value <= 0
        for value in resource_limits.values()
    ):
        raise SuccessorEvidenceError(f"{context} resource limits are not positive")

    action_values = _list(inputs["hostile_actions"], f"{context} hostile actions")
    actions = [_mapping(value, f"{context} hostile action") for value in action_values]
    action_ids: list[str] = []
    for action in actions:
        _strict_keys(
            action,
            {"action_id", "executable", "expected_terminal", "expected_containment"},
            f"{context} hostile action",
        )
        action_id = _nonempty_string(action["action_id"], f"{context} action id")
        executable = _mapping(action["executable"], f"{context} hostile executable")
        if (
            set(executable) != {"program", "argv"}
            or not _nonempty_string(
                executable["program"], f"{context} hostile executable program"
            )
            or not _string_list(executable["argv"], f"{context} hostile argv")
            or not _nonempty_string(
                action["expected_terminal"], f"{context} hostile terminal"
            )
            or not _nonempty_string(
                action["expected_containment"], f"{context} hostile containment"
            )
        ):
            raise SuccessorEvidenceError(f"{context} hostile action is prose-only")
        action_ids.append(action_id)
    if (
        action_ids != SECURITY_REQUIRED_HOSTILE_ACTION_IDS
        or len(action_ids) != len(set(action_ids))
        or hostile_job["payload"]["action_ids"] != action_ids
    ):
        raise SuccessorEvidenceError(f"{context} hostile action closure differs")

    preflight = {
        "policy_id": policy["policy_id"],
        "job_id": trusted_job["job_id"],
        "authorization": authorization,
        "state": "denied",
    }
    unavailable = {
        "policy_id": policy["policy_id"],
        "job_id": hostile_job["job_id"],
        "required": required,
        "observed": observed,
        "state": "sandbox_unavailable",
    }
    expected_rows = [
        [
            trusted_job["job_id"],
            "trusted_local_preflight",
            "denied",
            "CAPABILITY_UNAVAILABLE",
            None,
            None,
            _canonical_b3(preflight),
            "not_advertised",
            0,
            [],
            "unknown",
        ],
        [
            hostile_job["job_id"],
            "untrusted_preflight",
            "sandbox_unavailable",
            "SANDBOX_UNAVAILABLE",
            None,
            None,
            _canonical_b3(unavailable),
            "not_advertised",
            0,
            [
                {
                    "action_id": action["action_id"],
                    "attempted": False,
                    "contained": "unknown",
                    "expected_terminal": action["expected_terminal"],
                }
                for action in actions
            ],
            "unknown",
        ],
    ]
    expected_columns = [
        "job_id",
        "scenario",
        "state",
        "public_error",
        "launcher_receipt_id",
        "launcher_proof_id",
        "provenance_id",
        "capability_state",
        "attempted_action_count",
        "hostile_action_closure",
        "surviving_children",
    ]
    if (
        decoded["terminal"] != "pass_with_security_denials"
        or decoded["columns"] != expected_columns
        or decoded["rows"] != expected_rows
        or len(jobs) != 2
    ):
        raise SuccessorEvidenceError(
            f"{context} unavailable security terminal overclaims execution"
        )


def _validate_security_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    job_values = _list(inputs["provider_jobs"], f"{context} provider jobs")
    if len(job_values) != 2:
        raise SuccessorEvidenceError(
            f"{context} must split exactly two trust scenarios"
        )
    jobs: list[Mapping[str, Any]] = []
    job_ids: set[str] = set()
    for index, job_value in enumerate(job_values, 1):
        job = _mapping(job_value, f"{context} provider job {index}")
        _strict_keys(
            job,
            {"job_id", "provider", "workspace", "requested_profile", "payload"},
            f"{context} provider job {index}",
        )
        job_id = _nonempty_string(job["job_id"], f"{context} provider job id")
        if job_id in job_ids or job["provider"] != "rustc-extractor":
            raise SuccessorEvidenceError(f"{context} provider job identity differs")
        job_ids.add(job_id)
        _nonempty_string(job["workspace"], f"{context} provider job workspace")
        jobs.append(job)
    trusted_job = _security_job_by_profile(jobs, "trusted_local", context)
    hostile_job = _security_job_by_profile(jobs, "untrusted", context)
    trusted_payload = _mapping(trusted_job["payload"], f"{context} trusted job payload")
    hostile_payload = _mapping(hostile_job["payload"], f"{context} hostile job payload")
    _strict_keys(
        trusted_payload,
        {"kind", "source_id"},
        f"{context} trusted job payload",
    )
    _strict_keys(
        hostile_payload,
        {"kind", "action_ids"},
        f"{context} hostile job payload",
    )
    if (
        trusted_payload["kind"] != "compile_fixture"
        or hostile_payload["kind"] != "hostile_fixture"
    ):
        raise SuccessorEvidenceError(f"{context} provider job payload kind differs")
    if (
        isinstance(inputs["launcher_evidence_contract"], dict)
        and inputs["launcher_evidence_contract"].get("contract_id")
        == "rust-launcher-evidence-v2"
    ):
        _validate_unavailable_security_inputs(
            inputs, decoded, jobs, trusted_job, hostile_job, context
        )
        return

    policy = _mapping(inputs["trust_policy"], f"{context} trust policy")
    _strict_keys(
        policy,
        {
            "policy_id",
            "default_profile",
            "trusted_local_requires_distinct_authorization",
            "untrusted_requires_hostile_execution_receipt",
            "fail_closed",
        },
        f"{context} trust policy",
    )
    if (
        not _nonempty_string(policy["policy_id"], f"{context} trust policy id")
        or policy["default_profile"] != "untrusted"
        or policy["trusted_local_requires_distinct_authorization"] is not True
        or policy["untrusted_requires_hostile_execution_receipt"] is not True
        or policy["fail_closed"] is not True
    ):
        raise SuccessorEvidenceError(f"{context} trust policy is not fail closed")

    authorization = _mapping(
        inputs["explicit_authorization"], f"{context} explicit authorization"
    )
    _strict_keys(
        authorization,
        {"job_id", "trusted_local", "authorization_id"},
        f"{context} explicit authorization",
    )
    if (
        authorization["job_id"] != trusted_job["job_id"]
        or authorization["trusted_local"] is not False
        or authorization["authorization_id"] != "authorization:none"
    ):
        raise SuccessorEvidenceError(
            f"{context} baseline trusted-local authorization differs"
        )

    evidence_contract = _mapping(
        inputs["launcher_evidence_contract"],
        f"{context} launcher evidence contract",
    )
    _strict_keys(
        evidence_contract,
        set(SECURITY_LAUNCHER_EVIDENCE_CONTRACT),
        f"{context} launcher evidence contract",
    )
    contract_without_requirements = dict(evidence_contract)
    required_action_ids = _string_list(
        contract_without_requirements.pop("required_action_ids"),
        f"{context} required hostile action ids",
    )
    expected_contract_without_requirements = dict(SECURITY_LAUNCHER_EVIDENCE_CONTRACT)
    expected_contract_without_requirements.pop("required_action_ids")
    if contract_without_requirements != expected_contract_without_requirements:
        raise SuccessorEvidenceError(
            f"{context} launcher evidence identity contract differs"
        )

    constraints = _mapping(
        inputs["launcher_constraints"], f"{context} launcher constraints"
    )
    _strict_keys(
        constraints,
        {
            "allowed_environment",
            "network",
            "workspace_mode",
            "private_output_only",
            "unrelated_file_descriptors",
            "process_group_kill",
        },
        f"{context} launcher constraints",
    )
    if constraints != {
        "allowed_environment": [],
        "network": False,
        "workspace_mode": "read_only",
        "private_output_only": True,
        "unrelated_file_descriptors": False,
        "process_group_kill": True,
    }:
        raise SuccessorEvidenceError(f"{context} launcher constraints are incomplete")

    resource_limits = _mapping(inputs["resource_limits"], f"{context} resource limits")
    _strict_keys(
        resource_limits,
        {
            "wall_ms",
            "cpu_ms",
            "memory_bytes",
            "processes",
            "open_files",
            "output_bytes",
            "arrow_output_bytes",
        },
        f"{context} resource limits",
    )
    if any(
        not isinstance(value, int) or isinstance(value, bool) or value <= 0
        for value in resource_limits.values()
    ):
        raise SuccessorEvidenceError(f"{context} resource limits are not positive")

    action_values = _list(inputs["hostile_actions"], f"{context} hostile actions")
    if len(action_values) != len(SECURITY_REQUIRED_HOSTILE_ACTION_IDS):
        raise SuccessorEvidenceError(f"{context} hostile action corpus is incomplete")
    actions: list[Mapping[str, Any]] = []
    action_ids: list[str] = []
    for index, action_value in enumerate(action_values, 1):
        action = _mapping(action_value, f"{context} hostile action {index}")
        _strict_keys(
            action,
            {
                "action_id",
                "executable",
                "expected_terminal",
                "expected_containment",
            },
            f"{context} hostile action {index}",
        )
        action_id = _nonempty_string(action["action_id"], f"{context} action id")
        executable = _mapping(
            action["executable"], f"{context} executable action {action_id}"
        )
        _strict_keys(
            executable,
            {"program", "argv"},
            f"{context} executable action {action_id}",
        )
        if not _nonempty_string(
            executable["program"], f"{context} executable program"
        ) or not _string_list(executable["argv"], f"{context} executable argv"):
            raise SuccessorEvidenceError(f"{context} hostile action is prose-only")
        _nonempty_string(action["expected_terminal"], f"{context} expected terminal")
        _nonempty_string(
            action["expected_containment"], f"{context} expected containment"
        )
        actions.append(action)
        action_ids.append(action_id)
    if len(action_ids) != len(set(action_ids)):
        raise SuccessorEvidenceError(f"{context} hostile action ids are not unique")
    if hostile_payload["action_ids"] != action_ids:
        raise SuccessorEvidenceError(f"{context} hostile job/action closure differs")
    if action_ids != required_action_ids:
        raise SuccessorEvidenceError(
            f"{context} name-only hostile requirements lack observed action closure"
        )
    if required_action_ids != SECURITY_REQUIRED_HOSTILE_ACTION_IDS:
        raise SuccessorEvidenceError(f"{context} exact hostile action suite differs")

    hostile_authorization_id = f"authorization:{policy['policy_id']}:default-untrusted"
    hostile_receipt = _security_launcher_receipt_id(
        inputs, hostile_job, hostile_authorization_id
    )
    hostile_closure = _security_action_closure(
        actions,
        launcher_receipt_id=hostile_receipt,
        attempted=True,
        contained=True,
    )
    derived_receipt, hostile_proof, hostile_provenance = _security_launcher_identities(
        inputs,
        hostile_job,
        hostile_authorization_id,
        hostile_closure,
        "contained_failure",
        "sandbox_proof_observed",
        "PRIVATE_SECURITY_PROOF",
    )
    if derived_receipt != hostile_receipt:
        raise SuccessorEvidenceError(f"{context} launcher receipt derivation differs")
    denied_provenance = _security_preflight_provenance_id(
        inputs, trusted_job, str(authorization["authorization_id"])
    )
    expected_rows = [
        [
            trusted_job["job_id"],
            "trusted_local_preflight",
            "denied",
            "CAPABILITY_UNAVAILABLE",
            policy["policy_id"],
            authorization["authorization_id"],
            None,
            None,
            denied_provenance,
            "PUBLIC_DENIAL_ONLY",
            "not_advertised",
            0,
            0,
            [],
            0,
            0,
        ],
        [
            hostile_job["job_id"],
            "untrusted_sandbox_execution",
            "contained_failure",
            None,
            policy["policy_id"],
            hostile_authorization_id,
            hostile_receipt,
            hostile_proof,
            hostile_provenance,
            "PRIVATE_SECURITY_PROOF",
            "sandbox_proof_observed",
            len(hostile_closure),
            len(hostile_closure),
            hostile_closure,
            0,
            0,
        ],
    ]
    if (
        decoded["columns"] != SECURITY_TERMINAL_COLUMNS
        or decoded["rows"] != expected_rows
    ):
        raise SuccessorEvidenceError(
            f"{context} security terminal identity or containment closure differs"
        )


PUBLIC_RESPONSE_FIELDS = {
    "specification",
    "version",
    "semantic_request_id",
    "execution_state",
    "availability_state",
    "completeness_state",
    "freshness_state",
    "limit_state",
    "successful_query_count",
    "failed_query_count",
    "not_executed_dependency_count",
    "snapshot",
    "entities",
    "facts",
    "paths",
    "groups",
    "source_contexts",
    "query_results",
    "errors",
}
TERMINAL_EVENT_KEYS = {
    "header",
    "execution_state",
    "availability_state",
    "freshness_state",
    "limit_state",
    "dependency_state",
    "canonical_response_checksum",
    "canonical_error_record_json",
    "artifact_id",
    "result_row_count",
    "result_byte_count",
    "cleanup_state",
    "semantic_execution_state",
    "completeness_state",
    "truncated",
    "query_statuses",
    "notices",
}
QUERY_EVENT_HEADER_KEYS = {
    "daemon_query_id",
    "sequence",
    "snapshot_id",
    "event_at_unix_ms",
    "event_checksum",
}
QUERY_STATUS_SUMMARY_KEYS = {
    "query_id",
    "execution_state",
    "canonical_error_record_json",
    "notices",
}


def _projected_query_counts(
    statuses: Sequence[Mapping[str, Any]],
) -> tuple[int, int, int]:
    return (
        sum(status.get("execution_state") == "COMPLETE" for status in statuses),
        sum(status.get("execution_state") == "FAILED" for status in statuses),
        sum(
            status.get("execution_state") == "NOT_EXECUTED_DEPENDENCY"
            for status in statuses
        ),
    )


def _daemon_canonical_responses(
    inputs: Mapping[str, Any], context: str
) -> dict[str, tuple[str, Mapping[str, Any]]]:
    source = _mapping(
        inputs["daemon_canonical_response_results"],
        f"{context} daemon canonical response results",
    )
    _strict_keys(
        source,
        {"authority", "canonicalization_profile", "results"},
        f"{context} daemon canonical response results",
    )
    if (
        source["authority"] != "rust_daemon_canonical_semantic_response"
        or source["canonicalization_profile"] != "RFC8785"
    ):
        raise SuccessorEvidenceError(
            f"{context} daemon canonical response authority differs"
        )
    results = _list(source["results"], f"{context} daemon canonical results")
    if not results:
        raise SuccessorEvidenceError(f"{context} has zero daemon canonical results")
    by_checksum: dict[str, tuple[str, Mapping[str, Any]]] = {}
    result_ids: list[str] = []
    for index, result_value in enumerate(results, 1):
        result_context = f"{context} daemon canonical result {index}"
        result = _mapping(result_value, result_context)
        _strict_keys(
            result,
            {"result_id", "canonical_response_checksum", "canonical_json"},
            result_context,
        )
        result_id = _nonempty_string(result["result_id"], f"{result_context} id")
        canonical_json = _nonempty_string(
            result["canonical_json"], f"{result_context} canonical JSON"
        )
        try:
            decoded = json.loads(canonical_json, object_pairs_hook=_reject_duplicates)
        except json.JSONDecodeError as error:
            raise SuccessorEvidenceError(
                f"{result_context} canonical JSON is invalid"
            ) from error
        response = _mapping(decoded, f"{result_context} decoded response")
        if canonical_json != rfc8785.dumps(response).decode("utf-8"):
            raise SuccessorEvidenceError(
                f"{result_context} is not exact RFC8785 canonical JSON"
            )
        checksum = _nonempty_string(
            result["canonical_response_checksum"], f"{result_context} checksum"
        )
        if checksum != _canonical_b3(response):
            raise SuccessorEvidenceError(
                f"{result_context} canonical response checksum differs"
            )
        if checksum in by_checksum or result_id in result_ids:
            raise SuccessorEvidenceError(
                f"{context} duplicates a daemon canonical response identity"
            )
        by_checksum[checksum] = (result_id, response)
        result_ids.append(result_id)
    if result_ids != sorted(result_ids):
        raise SuccessorEvidenceError(
            f"{context} daemon canonical results are not deterministically ordered"
        )
    return by_checksum


def _validate_wire_inputs(
    root: Path, inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    json_contract = _mapping(
        inputs["public_json_contract"], f"{context} public JSON contract"
    )
    proto_contract = _mapping(
        inputs["protobuf_contract"], f"{context} Protobuf contract"
    )
    if (
        json_contract.get("schema_version") != "1.3"
        or proto_contract.get("message") != "TerminalEvent"
        or proto_contract.get("package") != "codefabric.cpgd.v1"
    ):
        raise SuccessorEvidenceError(f"{context} released wire identity differs")
    terminal = _mapping(inputs["internal_terminal"], f"{context} TerminalEvent")
    _strict_keys(terminal, TERMINAL_EVENT_KEYS, f"{context} TerminalEvent")
    header = _mapping(terminal["header"], f"{context} QueryEventHeader")
    _strict_keys(header, QUERY_EVENT_HEADER_KEYS, f"{context} QueryEventHeader")
    statuses = _list(terminal["query_statuses"], f"{context} query statuses")
    if len(statuses) != 1:
        raise SuccessorEvidenceError(f"{context} TerminalEvent status count differs")
    status = _mapping(statuses[0], f"{context} QueryStatusSummary")
    _strict_keys(status, QUERY_STATUS_SUMMARY_KEYS, f"{context} QueryStatusSummary")
    projection = _mapping(
        inputs["public_projection_policy"], f"{context} public projection policy"
    )
    _strict_keys(
        projection,
        {
            "response_authority",
            "projection",
            "terminal_role",
            "terminal_correlation",
            "request_correlation",
            "collection_policy",
            "internal_field_policy",
        },
        f"{context} public projection policy",
    )
    if (
        projection["response_authority"] != "rust_daemon_canonical_semantic_response"
        or projection["projection"] != "allowlisted_released_fields_exact_pass_through"
        or projection["terminal_role"] != "operational_evidence_only"
        or projection["terminal_correlation"]
        != {
            "checksum": "internal.canonical_response_checksum",
            "snapshot_id": "internal.header.snapshot_id",
            "execution_state": "internal.semantic_execution_state",
            "availability_state": "internal.availability_state",
            "completeness_state": "internal.completeness_state",
            "freshness_state": "internal.freshness_state",
            "limit_state": "internal.limit_state",
            "query_statuses": "internal.query_statuses",
        }
        or projection["request_correlation"]
        != {
            "semantic_request_id": "request_context.semantic_request_id",
            "query_id": "request_context.query_id",
            "request": "request_context.request",
            "workspace_id": "request_context.workspace_id",
        }
        or projection["collection_policy"]
        != (
            "emit every required collection from the daemon canonical response; "
            "empty is an explicit empty object or array"
        )
        or projection["internal_field_policy"]
        != (
            "construct an explicit released allowlist; internal_table and "
            "physical_plan are forbidden"
        )
    ):
        raise SuccessorEvidenceError(
            f"{context} public pass-through projection policy differs"
        )
    request_context = _mapping(inputs["request_context"], f"{context} request context")
    _strict_keys(
        request_context,
        {"semantic_request_id", "query_id", "request", "workspace_id"},
        f"{context} request context",
    )
    access_scope = _mapping(inputs["access_scope"], f"{context} access scope")
    _strict_keys(
        access_scope,
        {"workspace", "agent", "query"},
        f"{context} access scope",
    )
    if (
        access_scope["workspace"] != request_context["workspace_id"]
        or access_scope["query"] != request_context["query_id"]
        or not _nonempty_string(access_scope["agent"], f"{context} access agent")
    ):
        raise SuccessorEvidenceError(f"{context} wire workspace correlation differs")
    private = _mapping(inputs["private_diagnostics"], f"{context} private diagnostics")
    _strict_keys(
        private,
        {"internal_table", "physical_plan", "diagnostic_visibility"},
        f"{context} private diagnostics",
    )
    if private["diagnostic_visibility"] != "private_authorized_resource_only":
        raise SuccessorEvidenceError(f"{context} private diagnostic boundary differs")
    redaction = _mapping(inputs["redaction_policy"], f"{context} redaction policy")
    _strict_keys(
        redaction,
        {"physical_names", "physical_plans", "source_bytes"},
        f"{context} redaction policy",
    )
    if redaction != {
        "physical_names": "deny",
        "physical_plans": "deny",
        "source_bytes": "deny",
    }:
        raise SuccessorEvidenceError(f"{context} released redaction policy differs")
    for contract, label in (
        (json_contract, "public response schema"),
        (proto_contract, "query service proto"),
    ):
        path = root / _relative_path(contract["path"], f"{context} {label} path")
        if contract.get("sha256") != _sha256(path):
            raise SuccessorEvidenceError(f"{context} {label} digest differs")
    rows = _decoded_rows(decoded, f"{context} decoded response")
    if decoded["columns"] != ["public_response"] or len(rows) != 1:
        raise SuccessorEvidenceError(f"{context} lacks full decoded public response")
    response = _mapping(rows[0][0], f"{context} public response")
    _strict_keys(response, PUBLIC_RESPONSE_FIELDS, f"{context} public response")
    candidate = _mapping(
        inputs["candidate_released_projection"],
        f"{context} candidate released projection",
    )
    _strict_keys(
        candidate,
        PUBLIC_RESPONSE_FIELDS,
        f"{context} candidate released projection",
    )
    if candidate != response:
        raise SuccessorEvidenceError(
            f"{context} decoded public response differs from its candidate projection"
        )
    response_schema = _load_json(
        root / _relative_path(json_contract["path"], f"{context} response schema path"),
        f"{context} response schema",
    )
    schema_errors = sorted(
        jsonschema.Draft202012Validator(response_schema).iter_errors(response),
        key=lambda error: tuple(str(part) for part in error.absolute_path),
    )
    if schema_errors:
        error = schema_errors[0]
        location = "/".join(str(part) for part in error.absolute_path) or "<root>"
        raise SuccessorEvidenceError(
            f"{context} public response violates released schema at {location}: "
            f"{error.message}"
        )
    daemon_responses = _daemon_canonical_responses(inputs, context)
    for result_id, daemon_response in daemon_responses.values():
        _strict_keys(
            daemon_response,
            PUBLIC_RESPONSE_FIELDS,
            f"{context} {result_id} response",
        )
        schema_errors = sorted(
            jsonschema.Draft202012Validator(response_schema).iter_errors(
                daemon_response
            ),
            key=lambda error: tuple(str(part) for part in error.absolute_path),
        )
        if schema_errors:
            error = schema_errors[0]
            location = "/".join(str(part) for part in error.absolute_path) or "<root>"
            raise SuccessorEvidenceError(
                f"{context} daemon response violates released schema at {location}: "
                f"{error.message}"
            )
        if any(key in daemon_response for key in ("internal_table", "physical_plan")):
            raise SuccessorEvidenceError(
                f"{context} daemon response leaks physical authority"
            )
    selected_result = daemon_responses.get(terminal["canonical_response_checksum"])
    if selected_result is None:
        raise SuccessorEvidenceError(
            f"{context} TerminalEvent checksum differs from every daemon canonical response"
        )
    _, daemon_response = selected_result
    if response != daemon_response:
        raise SuccessorEvidenceError(
            f"{context} released JSON is not an exact daemon response pass-through"
        )
    if (
        response["specification"] != "composable semantic CPG fact query response"
        or response["version"] != "1.3"
        or response["freshness_state"] != "CURRENT"
        or response["limit_state"] != "NOT_APPLIED"
    ):
        raise SuccessorEvidenceError(f"{context} public response values differ")
    query_results = _list(response["query_results"], f"{context} public query results")
    if (
        response["semantic_request_id"] != request_context["semantic_request_id"]
        or response["snapshot"].get("snapshot_id") != header["snapshot_id"]
        or response["snapshot"].get("workspace_id") != access_scope["workspace"]
        or response["execution_state"] != terminal["semantic_execution_state"]
        or response["availability_state"] != terminal["availability_state"]
        or response["completeness_state"] != terminal["completeness_state"]
        or response["freshness_state"] != terminal["freshness_state"]
        or response["limit_state"] != terminal["limit_state"]
        or len(query_results) != 1
        or query_results[0].get("query_id") != request_context["query_id"]
        or query_results[0].get("request") != request_context["request"]
        or query_results[0].get("execution_state") != status["execution_state"]
    ):
        raise SuccessorEvidenceError(f"{context} public projection differs from inputs")
    if terminal["canonical_response_checksum"] != _canonical_b3(daemon_response):
        raise SuccessorEvidenceError(
            f"{context} canonical response checksum differs from daemon response"
        )
    successful, failed, not_executed = _projected_query_counts([status])
    if (
        response["successful_query_count"] != successful
        or response["failed_query_count"] != failed
        or response["not_executed_dependency_count"] != not_executed
    ):
        raise SuccessorEvidenceError(f"{context} public query counters differ")
    coverage = _mapping(
        query_results[0].get("coverage"), f"{context} public query coverage"
    )
    if (
        status["execution_state"] == "COMPLETE"
        and str(coverage.get("state")).upper() != "COMPLETE"
    ):
        raise SuccessorEvidenceError(f"{context} completed query coverage differs")
    if any(
        key in value
        for value in (response, candidate)
        for key in ("internal_table", "physical_plan")
    ):
        raise SuccessorEvidenceError(
            f"{context} public response leaks physical authority"
        )


SOURCE_IMAGE_KEYS = {
    "source_id",
    "workspace_id",
    "module_id",
    "file_id",
    "language",
    "semantic_environment_digest",
    "analysis_context_id",
    "source_generation",
    "bytes_utf8",
    "content_digest",
}


def _source_semantic_rows(image_value: Any, context: str) -> list[list[Any]]:
    image = _mapping(image_value, f"{context} source image")
    _strict_keys(image, SOURCE_IMAGE_KEYS, f"{context} source image")
    source = _nonempty_string(image["bytes_utf8"], f"{context} source bytes")
    if (
        image["language"] != "python"
        or not image["source_id"]
        or re.fullmatch(r"workspace:[0-9a-f]{32}", str(image["workspace_id"])) is None
        or re.fullmatch(r"entity:module:[0-9a-f]{32}", str(image["module_id"])) is None
        or re.fullmatch(r"[0-9a-f]{32}", str(image["file_id"])) is None
        or image["analysis_context_id"]
        != _cbef_analysis_context_id(
            workspace_id=image["workspace_id"],
            language_slug=str(image["language"]),
            environment_digest=image["semantic_environment_digest"],
        )
        or not image["source_generation"]
        or image["content_digest"] != _bytes_b3(source.encode("utf-8"))
    ):
        raise SuccessorEvidenceError(f"{context} source image identity differs")
    try:
        tree = ast.parse(source)
    except SyntaxError as error:
        raise SuccessorEvidenceError(
            f"{context} source image is not valid Python"
        ) from error
    functions = [
        node
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    ]
    if len({function.name for function in functions}) != len(functions):
        raise SuccessorEvidenceError(
            f"{context} duplicates a fixture function identity"
        )

    identity_context = {
        "workspace_id": image["workspace_id"],
        "module_id": image["module_id"],
        "analysis_context_id": image["analysis_context_id"],
        "file_id": image["file_id"],
        "content_digest": image["content_digest"],
    }
    function_ids = {
        function.name: _python_function_id(identity_context, [function.name])
        for function in functions
    }
    rows: set[tuple[str, str, str | None, str | None, str | None, str | None]] = {
        (entity_id, "function", name, None, None, None)
        for name, entity_id in function_ids.items()
    }
    for function in functions:
        caller_id = function_ids[function.name]
        calls = sorted(
            (node for node in ast.walk(function) if isinstance(node, ast.Call)),
            key=lambda node: _node_byte_range(source, node.func, context),
        )
        for owner_ordinal, node in enumerate(calls):
            if not isinstance(node.func, ast.Name):
                raise SuccessorEvidenceError(
                    f"{context} independent fixture oracle lacks a direct call identity"
                )
            callee_id = function_ids.get(node.func.id)
            if callee_id is None:
                raise SuccessorEvidenceError(
                    f"{context} unresolved call cannot be emitted as a semantic entity"
                )
            start_byte, end_byte = _node_byte_range(source, node.func, context)
            call_site_id = _python_call_site_id(
                identity_context,
                owner_id=caller_id,
                owner_relative_role="body.call.callee",
                owner_relative_ordinal=owner_ordinal,
                start_byte=start_byte,
                end_byte=end_byte,
            )
            fact_id = _calls_fact_id(
                identity_context,
                caller_id=caller_id,
                callee_id=callee_id,
                call_site_id=call_site_id,
            )
            rows.add(
                (
                    call_site_id,
                    "call_site",
                    f"{start_byte}:{end_byte}",
                    caller_id,
                    None,
                    call_site_id,
                )
            )
            rows.add((fact_id, "calls", None, caller_id, callee_id, call_site_id))
    if not rows:
        raise SuccessorEvidenceError(
            f"{context} source image produced no fixture facts"
        )
    return [list(row) for row in sorted(rows)]


def _equivalence_source_rows(
    inputs: Mapping[str, Any], context: str
) -> tuple[list[list[str]], list[list[str]], Mapping[str, Any]]:
    images = _mapping(inputs["source_images"], f"{context} source images")
    _strict_keys(images, {"generation_g1", "generation_g2"}, f"{context} source images")
    generation_g1 = _mapping(images["generation_g1"], f"{context} generation g1")
    generation_g2 = _mapping(images["generation_g2"], f"{context} generation g2")
    if (
        generation_g1["source_id"] != generation_g2["source_id"]
        or generation_g1["workspace_id"] != generation_g2["workspace_id"]
        or generation_g1["module_id"] != generation_g2["module_id"]
        or generation_g1["file_id"] != generation_g2["file_id"]
        or generation_g1["analysis_context_id"] != generation_g2["analysis_context_id"]
        or generation_g1["source_generation"] != "g1"
        or generation_g2["source_generation"] != "g2"
    ):
        raise SuccessorEvidenceError(f"{context} source generation closure differs")
    return (
        _source_semantic_rows(generation_g1, f"{context} generation g1"),
        _source_semantic_rows(generation_g2, f"{context} generation g2"),
        generation_g2,
    )


def _derive_equivalence_routes(
    inputs: Mapping[str, Any], context: str
) -> tuple[list[list[Any]], list[list[Any]]]:
    generation_g1, source_rows, _ = _equivalence_source_rows(inputs, context)
    clean = sorted([[*row, "current"] for row in source_rows], key=lambda row: row[0])
    base_input = _mapping(inputs["incremental_base_state"], f"{context} base state")
    _strict_keys(
        base_input,
        {"generation", "source_content_digest", "analysis_context_id", "rows"},
        f"{context} base state",
    )
    generation_g1_image = inputs["source_images"]["generation_g1"]
    expected_base = sorted(
        [[*row, "current"] for row in generation_g1], key=lambda row: row[0]
    )
    if (
        base_input["generation"] != "g1"
        or base_input["source_content_digest"] != generation_g1_image["content_digest"]
        or base_input["analysis_context_id"]
        != generation_g1_image["analysis_context_id"]
        or base_input["rows"] != expected_base
    ):
        raise SuccessorEvidenceError(
            f"{context} incremental base was not derived from generation g1"
        )
    base = {row[0]: row for row in base_input["rows"]}
    wanted = {row[0]: row for row in source_rows}
    incremental_definition = inputs["route_definitions"]["incremental"]
    delete_enabled = next(
        operation["enabled"]
        for operation in incremental_definition["operations"]
        if operation["operator"] == "apply_deletes"
    )
    if delete_enabled:
        for identity in set(base) - set(wanted):
            del base[identity]
    for identity, row in wanted.items():
        base[identity] = [*row, "current"]
    incremental = sorted(base.values(), key=lambda row: row[0])
    return clean, incremental


def _equivalence_provenance(inputs: Mapping[str, Any]) -> dict[str, Any]:
    pins = inputs["policy_proof_pins"]
    image = inputs["source_images"]["generation_g2"]
    derivation = inputs["change_derivation"]
    return {
        "source_generation": inputs["coverage_proof_inputs"]["source_generation"],
        "source_content_digest": image["content_digest"],
        "analysis_context_id": image["analysis_context_id"],
        "provider_program_identity": derivation["provider_program_identity"],
        "transformation_program_identity": derivation[
            "transformation_program_identity"
        ],
        "provider_release_vector": inputs["provider_release_vector"],
        "transformation_analysis_release": inputs["transformation_analysis_release"],
        "table_version_vector": inputs["exact_table_vector"],
        "policy_release": pins["policy_release"],
        "expectation_issuance": pins["expectation_issuance"],
        "proof_set": pins["proof_set"],
        "proof_state": pins["proof_state"],
    }


def _validate_equivalence_inputs(
    inputs: Mapping[str, Any], decoded: Mapping[str, Any], context: str
) -> None:
    routes = _mapping(inputs["route_definitions"], f"{context} route definitions")
    for name in ("clean", "incremental"):
        route = _mapping(routes[name], f"{context} {name} route")
        operations = _list(route.get("operations"), f"{context} {name} operations")
        if not operations or any(
            not isinstance(operation, dict) for operation in operations
        ):
            raise SuccessorEvidenceError(f"{context} {name} route is prose or empty")
        if "rows" in json.dumps(route, sort_keys=True):
            raise SuccessorEvidenceError(
                f"{context} {name} route authors producer output"
            )
    derivation = _mapping(inputs["change_derivation"], f"{context} change derivation")
    if derivation != {
        "authority": "application-owned source difference",
        "provider_program_identity": "independent.python-ast-provider-fixture.v2",
        "transformation_program_identity": "independent.canonical-entity-fixture.v2",
        "identity_key": "canonical_row_identity",
        "operations": ["insert", "delete", "replace"],
        "identity_contract": {
            "format": "CBEF-v1",
            "format_version": 1,
            "magic": "CFID",
            "byte_order": "big-endian",
            "digest": {
                "algorithm": "BLAKE3-256",
                "public_identity": "first 16 digest bytes as lowercase hexadecimal",
                "collision_diagnostic": "retain the full 32-byte digest",
            },
            "analysis_context": {
                "recipe": "ANALYSIS_CONTEXT",
                "domain_code": CBEF_ANALYSIS_CONTEXT_DOMAIN_CODE,
                "fields": [
                    ["workspace_id", "ID"],
                    ["language_slug", "UTF8/ASCII_LOWER"],
                    ["environment_digest", "DIGEST"],
                ],
            },
            "function": {
                "recipe": "ENTITY",
                "domain_code": CBEF_ENTITY_DOMAIN_CODE,
                "kind_code": PYTHON_FUNCTION_KIND_CODE,
                "kind_slug": "function",
                "owner_field": "module_id",
                "semantic_key_encoding": "RFC8785 JCS UTF-8 bytes",
                "semantic_key_fields": [
                    "schema_version",
                    "module_id",
                    "qualified_lexical_path",
                    "kind",
                ],
            },
            "call_site": {
                "recipe": "ENTITY",
                "domain_code": CBEF_ENTITY_DOMAIN_CODE,
                "kind_code": PYTHON_CALL_SITE_KIND_CODE,
                "kind_slug": "call-site",
                "owner_field": "owner_id",
                "semantic_key_encoding": "RFC8785 JCS UTF-8 bytes",
                "semantic_key_fields": [
                    "schema_version",
                    "module_id",
                    "owner_relative_role",
                    "owner_relative_ordinal",
                    "file_id",
                    "content_digest",
                    "source_range",
                    "kind",
                ],
            },
            "calls_fact": {
                "recipe": "RELATION_FACT",
                "domain_code": CBEF_RELATION_FACT_DOMAIN_CODE,
                "relation_kind_code": CALLS_RELATION_KIND_CODE,
                "kind_slug": "calls",
                "subject_field": "caller_id",
                "object_field": "callee_id",
                "role": {
                    "type": "TAGGED_UNION",
                    "variant": 1,
                    "member_type": "UTF8",
                    "encoding": "RFC8785 JCS UTF-8 string",
                    "fields": ["schema_version", "module_id", "call_site_id"],
                },
                "outer_context_fields": [
                    "workspace_id",
                    "analysis_context_id",
                ],
            },
        },
    }:
        raise SuccessorEvidenceError(f"{context} change derivation differs")
    if any(
        "change_derivation.identity_contract"
        not in _string_list(route["typed_inputs"], f"{context} {name} typed inputs")
        for name, route in routes.items()
    ):
        raise SuccessorEvidenceError(
            f"{context} route omits the canonical identity contract"
        )
    clean, incremental = _derive_equivalence_routes(inputs, context)
    if clean != incremental:
        raise SuccessorEvidenceError(f"{context} baseline routes are not equivalent")
    call_site_rows = {row[0] for row in clean if row[1] == "call_site"}
    calls_rows = [row for row in clean if row[1] == "calls"]
    if (
        not calls_rows
        or any(len(row) != 7 for row in clean)
        or any(row[5] not in call_site_rows for row in calls_rows)
    ):
        raise SuccessorEvidenceError(
            f"{context} calls facts do not expose a canonical call_site_id"
        )
    decoded_routes = {row[0]: row for row in decoded["rows"]}
    if set(decoded_routes) != {"clean", "incremental"}:
        raise SuccessorEvidenceError(f"{context} decoded route coverage differs")
    expected_provenance = _equivalence_provenance(inputs)
    for name, expected in (("clean", clean), ("incremental", incremental)):
        row = decoded_routes[name]
        if row[1] != expected or row[2] != expected or row[5] != "advertised":
            raise SuccessorEvidenceError(f"{context} {name} decoded route differs")
        provenance = _mapping(row[6], f"{context} {name} provenance")
        if provenance != expected_provenance:
            raise SuccessorEvidenceError(f"{context} {name} provenance closure differs")


def _validate_family_inputs(
    root: Path,
    family: str,
    inputs: Mapping[str, Any],
    decoded: Mapping[str, Any],
    context: str,
) -> None:
    if family == "exact_provider_facts":
        _validate_provider_inputs(root, inputs, decoded, context)
    elif family == "programmatic_transformations":
        _validate_transformation_inputs(inputs, decoded, context)
    elif family == "derived_analyses":
        _validate_derived_inputs(inputs, decoded, context)
    elif family in EXPECTED_QUERY_FAMILIES:
        _validate_query_inputs(family, inputs, decoded, context)
    elif family == "delta_exact_version_protocol":
        _validate_delta_inputs(inputs, decoded, context)
    elif family == "activation_recovery":
        _validate_activation_inputs(inputs, decoded, context)
    elif family == "authorization":
        access_scope = _mapping(inputs["access_scope"], f"{context} access scope")
        authorization_policy = _mapping(
            inputs["authorization_policy"], f"{context} authorization policy"
        )
        expected_scope_recipe = _authorization_scope_recipe(
            access_scope, authorization_policy
        )
        if (
            access_scope.get("scope_id") != expected_scope_recipe["output_id"]
            or access_scope.get("identity_recipe") != expected_scope_recipe
        ):
            raise SuccessorEvidenceError(
                f"{context} access-scope identity does not bind its exact grants"
            )
        allowed_relations = _list(
            access_scope["allowed_relations"], f"{context} allowed relations"
        )
        allowed_columns = _mapping(
            access_scope["allowed_columns"], f"{context} allowed columns"
        )
        allowed = set(allowed_relations)
        if len(allowed) != len(allowed_relations) or allowed != set(allowed_columns):
            raise SuccessorEvidenceError(
                f"{context} access-scope relation and column grants differ"
            )
        child_bindings = _mapping(
            inputs["child_catalog_bindings"], f"{context} child catalog bindings"
        )
        installed = set(child_bindings["installed_relations"])
        if set(child_bindings["visible_functions"]) != set(
            access_scope["allowed_functions"]
        ) or set(child_bindings["visible_object_stores"]) != set(
            access_scope["allowed_object_stores"]
        ):
            raise SuccessorEvidenceError(
                f"{context} child catalog grant classes differ from access scope"
            )
        epoch_catalog = _mapping(
            inputs["epoch_provider_catalog"], f"{context} epoch provider catalog"
        )
        _strict_keys(
            epoch_catalog,
            {"fabric_epoch_id", "relations"},
            f"{context} epoch provider catalog",
        )
        available_relations = _mapping(
            epoch_catalog["relations"], f"{context} available epoch relations"
        )
        for relation_name, binding_value in available_relations.items():
            binding = _mapping(
                binding_value, f"{context} {relation_name} provider binding"
            )
            _strict_keys(
                binding,
                {"schema_digest", "provider_identity"},
                f"{context} {relation_name} provider binding",
            )
            if re.fullmatch(
                r"b3:[0-9a-f]{64}", str(binding["schema_digest"])
            ) is None or not _nonempty_string(
                binding["provider_identity"],
                f"{context} {relation_name} provider identity",
            ):
                raise SuccessorEvidenceError(
                    f"{context} available provider binding is incomplete"
                )
        columns = _mapping(
            inputs["access_scope"]["allowed_columns"],
            f"{context} authorized columns",
        )
        if (
            allowed != installed
            or allowed != set(columns)
            or not allowed <= set(available_relations)
            or not set(inputs["bound_plan"]["providers"]) <= allowed
        ):
            raise SuccessorEvidenceError(
                f"{context} child catalog is not exactly reduced to authorized providers"
            )
    elif family == "resource_terminals":
        _validate_resource_inputs(inputs, decoded, context)
    elif family == "security_denial":
        _validate_security_inputs(inputs, decoded, context)
    elif family == "released_wire_projection":
        _validate_wire_inputs(root, inputs, decoded, context)
    elif family == "clean_incremental_equivalence":
        _validate_equivalence_inputs(inputs, decoded, context)


def validate_expectations(root: Path = ROOT) -> list[dict[str, Any]]:
    rows = _load_jsonl(root / EXPECTATIONS_PATH, "successor expectations")
    identifiers: set[str] = set()
    families: set[str] = set()
    query_families: set[str] = set()
    for index, row in enumerate(rows, 1):
        context = f"expectation row {index}"
        _strict_keys(row, EXPECTATION_KEYS, context)
        claim_id = _nonempty_string(row["claim_id"], f"{context} claim_id")
        if CLAIM_ID.fullmatch(claim_id) is None or claim_id in identifiers:
            raise SuccessorEvidenceError(f"{context} has invalid or duplicate claim_id")
        identifiers.add(claim_id)

        family = _nonempty_string(row["claim_family"], f"{context} claim_family")
        if family not in EXPECTED_FAMILIES or family in families:
            raise SuccessorEvidenceError(
                f"{context} has unknown or duplicate claim family: {family}"
            )
        families.add(family)
        if family.startswith("query_"):
            query_families.add(family)

        for key in ("subject", "author_id", "source_anchor"):
            _nonempty_string(row[key], f"{context} {key}")
        _validate_source_references(
            root, row["source_anchor"], f"{context} source_anchor"
        )
        governing_clauses = _string_list(
            row["governing_clauses"], f"{context} governing_clauses"
        )
        for clause_index, clause in enumerate(governing_clauses, 1):
            _validate_source_references(
                root, clause, f"{context} governing clause {clause_index}"
            )

        universe = _mapping(
            row["complete_input_universe"], f"{context} complete_input_universe"
        )
        _strict_keys(universe, {"closed", "inputs"}, f"{context} input universe")
        if universe["closed"] is not True:
            raise SuccessorEvidenceError(f"{context} input universe is not closed")
        inputs = _mapping(universe["inputs"], f"{context} input roles")
        expected_roles = INPUT_ROLE_CONTRACT[family]
        if set(inputs) != expected_roles:
            raise SuccessorEvidenceError(
                f"{context} input-role closure differs: "
                f"missing={sorted(expected_roles - set(inputs))}, "
                f"extra={sorted(set(inputs) - expected_roles)}"
            )
        for role, value in inputs.items():
            if value is None or value == "" or value == [] or value == {}:
                raise SuccessorEvidenceError(
                    f"{context} input role {role} has no decoded value"
                )

        pins = _mapping(row["exact_pins"], f"{context} exact_pins")
        if family == "exact_provider_facts":
            expected_pins = PROVIDER_PIN_CONTRACT
        elif family == "released_wire_projection":
            expected_pins = WIRE_PIN_CONTRACT
        else:
            expected_pins = PIN_CONTRACT
        if dict(pins) != expected_pins:
            raise SuccessorEvidenceError(
                f"{context} exact pin set differs from its authoritative family contract"
            )

        decoded = _mapping(row["decoded_expectation"], f"{context} decoded")
        _strict_keys(
            decoded,
            {"terminal", "relation", "columns", "rows", "coverage"},
            f"{context} decoded expectation",
        )
        _nonempty_string(decoded["terminal"], f"{context} terminal")
        _nonempty_string(decoded["relation"], f"{context} relation")
        columns = _string_list(decoded["columns"], f"{context} columns")
        if not isinstance(decoded["rows"], list) or not decoded["rows"]:
            raise SuccessorEvidenceError(f"{context} decoded expectation has zero rows")
        for row_index, decoded_row in enumerate(decoded["rows"], 1):
            if not isinstance(decoded_row, list) or len(decoded_row) != len(columns):
                raise SuccessorEvidenceError(
                    f"{context} decoded row {row_index} does not match columns"
                )
        _nonempty_string(decoded["coverage"], f"{context} coverage")
        _validate_family_inputs(root, family, inputs, decoded, context)

        semantics = _mapping(row["semantics"], f"{context} semantics")
        _strict_keys(
            semantics,
            {"ordering", "nulls", "unknowns", "provenance"},
            f"{context} semantics",
        )
        for key in ("ordering", "nulls", "unknowns", "provenance"):
            _nonempty_string(semantics[key], f"{context} semantics {key}")
        _string_list(row["limitations"], f"{context} limitations")

        consumer = _mapping(row["future_consumer"], f"{context} future_consumer")
        _strict_keys(consumer, {"packet", "oracle"}, f"{context} future_consumer")
        if not re.fullmatch(r"WP(?:3[4-8])", str(consumer["packet"])):
            raise SuccessorEvidenceError(f"{context} has invalid future consumer")
        _nonempty_string(consumer["oracle"], f"{context} future oracle")

        causal = _nonempty_string(
            row["causal_fixture_id"], f"{context} causal_fixture_id"
        )
        negative = _nonempty_string(
            row["negative_fixture_id"], f"{context} negative_fixture_id"
        )
        if FIXTURE_ID.fullmatch(causal) is None or not causal.endswith("-C"):
            raise SuccessorEvidenceError(
                f"{context} causal fixture identity is invalid"
            )
        if FIXTURE_ID.fullmatch(negative) is None or not negative.endswith("-N"):
            raise SuccessorEvidenceError(
                f"{context} negative fixture identity is invalid"
            )

    if families != EXPECTED_FAMILIES:
        raise SuccessorEvidenceError(
            "successor claim-family coverage differs: "
            f"missing={sorted(EXPECTED_FAMILIES - families)}, "
            f"extra={sorted(families - EXPECTED_FAMILIES)}"
        )
    if query_families != EXPECTED_QUERY_FAMILIES:
        raise SuccessorEvidenceError("all eight semantic request forms are required")
    return rows


def _json_pointer_tokens(pointer: object, context: str) -> list[str]:
    text = pointer if isinstance(pointer, str) else None
    if text is None or (text and not text.startswith("/")):
        raise SuccessorEvidenceError(f"{context} is not a valid JSON pointer")
    if text == "":
        return []
    tokens: list[str] = []
    for raw in text[1:].split("/"):
        if re.search(r"~(?![01])", raw):
            raise SuccessorEvidenceError(f"{context} has an invalid escape")
        tokens.append(raw.replace("~1", "/").replace("~0", "~"))
    return tokens


def _pointer_index(token: str, length: int, context: str) -> int:
    if token == "-" or not token.isdigit() or (token.startswith("0") and token != "0"):
        raise SuccessorEvidenceError(f"{context} has an invalid array index")
    index = int(token)
    if index >= length:
        raise SuccessorEvidenceError(f"{context} points outside the input")
    return index


def _resolve_json_pointer(value: object, pointer: object, context: str) -> object:
    current = value
    for token in _json_pointer_tokens(pointer, context):
        if isinstance(current, list):
            current = current[_pointer_index(token, len(current), context)]
        elif isinstance(current, dict):
            if token not in current:
                raise SuccessorEvidenceError(f"{context} references a missing member")
            current = current[token]
        else:
            raise SuccessorEvidenceError(f"{context} traverses a scalar value")
    return current


def _apply_json_pointer(
    value: object, pointer: object, replacement: object, context: str
) -> object:
    result = copy.deepcopy(value)
    tokens = _json_pointer_tokens(pointer, context)
    if not tokens:
        return copy.deepcopy(replacement)
    current = result
    for token in tokens[:-1]:
        if isinstance(current, list):
            current = current[_pointer_index(token, len(current), context)]
        elif isinstance(current, dict):
            if token not in current:
                raise SuccessorEvidenceError(f"{context} references a missing member")
            current = current[token]
        else:
            raise SuccessorEvidenceError(f"{context} traverses a scalar value")
    final = tokens[-1]
    if isinstance(current, list):
        current[_pointer_index(final, len(current), context)] = copy.deepcopy(
            replacement
        )
    elif isinstance(current, dict):
        if final not in current:
            raise SuccessorEvidenceError(f"{context} references a missing member")
        current[final] = copy.deepcopy(replacement)
    else:
        raise SuccessorEvidenceError(f"{context} traverses a scalar value")
    return result


ERROR_RECORD_KEYS = {
    "code",
    "layer",
    "retryable",
    "safe_message",
    "field",
    "semantic_phrase",
    "candidate_interpretations",
    "failed_dependency_query_id",
    "diagnostic_id",
}
QUERY_FIXTURE_STATE_KEYS = {
    "execution_state",
    "availability_state",
    "completeness_state",
    "freshness_state",
    "limit_state",
    "dependency_state",
    "resolved_semantics",
    "query_result",
    "errors",
}


def _validate_query_fixture_decoded(value: object, context: str) -> None:
    state = _mapping(value, f"{context} expected query state")
    _strict_keys(state, QUERY_FIXTURE_STATE_KEYS, f"{context} expected query state")
    _validate_query_states(state, f"{context} expected query state")
    if state["freshness_state"] != "CURRENT":
        raise SuccessorEvidenceError(f"{context} does not preserve pinned freshness")
    result = _mapping(state["query_result"], f"{context} query result")
    for key in ("query_id", "result_role", "coverage", "errors", "notices"):
        if key not in result:
            raise SuccessorEvidenceError(f"{context} query result lacks {key}")
    _mapping(result["coverage"], f"{context} fixture coverage")
    for location, values in (
        ("response", _list(state["errors"], f"{context} errors")),
        ("query result", _list(result["errors"], f"{context} result errors")),
    ):
        for index, error_value in enumerate(values, 1):
            record = _mapping(error_value, f"{context} {location} error {index}")
            _strict_keys(
                record,
                ERROR_RECORD_KEYS,
                f"{context} {location} error {index}",
            )
            _nonempty_string(record["code"], f"{context} error code")
            _nonempty_string(record["safe_message"], f"{context} safe error message")


def _query_fixture_error_codes(expected: Mapping[str, Any]) -> set[str]:
    result = _mapping(expected["query_result"], "fixture query result")
    values = [*_list(expected["errors"], "fixture response errors")]
    values.extend(_list(result["errors"], "fixture query-result errors"))
    return {
        str(record["code"])
        for value in values
        if isinstance(value, dict)
        for record in [value]
    }


def _validate_query_fixture_causality(
    family: str,
    kind: str,
    mutated_inputs: Mapping[str, Any],
    expected: Mapping[str, Any],
    context: str,
) -> None:
    request_wrapper = _mapping(
        mutated_inputs["request_envelope"], f"{context} request wrapper"
    )
    envelope = _mapping(request_wrapper["decoded"], f"{context} request envelope")
    if request_wrapper.get("canonical_json") != rfc8785.dumps(envelope).decode("utf-8"):
        raise SuccessorEvidenceError(
            f"{context} fixture request bytes are not exact canonical JSON"
        )
    queries = _list(envelope["queries"], f"{context} fixture queries")
    if family == "query_combine_results":
        combine_blocks = [
            _mapping(value, f"{context} fixture query block")
            for value in queries
            if isinstance(value, dict) and value.get("request") == "combine result sets"
        ]
        if len(queries) != 3 or len(combine_blocks) != 1:
            raise SuccessorEvidenceError(
                f"{context} fixture combine request does not close its producer DAG"
            )
        block = combine_blocks[0]
    else:
        if len(queries) != 1:
            raise SuccessorEvidenceError(f"{context} fixture request is not singular")
        block = _mapping(queries[0], f"{context} fixture query block")
    result = _mapping(expected["query_result"], f"{context} fixture query result")
    if result.get("query_id") != block.get("query_id"):
        raise SuccessorEvidenceError(f"{context} fixture query identity differs")
    resolved = _mapping(expected["resolved_semantics"], f"{context} resolved semantics")
    codes = _query_fixture_error_codes(expected)

    if family == "query_find_code_entities":
        rows, resolution = _validate_find_entity_authority(
            mutated_inputs, block, context
        )
        if kind == "causal":
            selected, expected_semantics, expected_coverage = (
                _derive_find_entity_selection(mutated_inputs, block, context)
            )
            if (
                block.get("looking_for") != "function"
                or resolved != expected_semantics
                or result.get("entity_ids") != [row["entity_id"] for row in selected]
                or result.get("entities") != selected
                or result.get("coverage") != expected_coverage
                or any(
                    row.get("representation") != "semantic_entity" for row in selected
                )
            ):
                raise SuccessorEvidenceError(f"{context} entity mutation is noncausal")
        elif (
            resolved.get("rejected_phrase") != block.get("looking_for")
            or "NOT_OBJECTIVE_FACT_REQUEST" not in codes
            or block.get("looking_for") in resolution
            or result.get("entity_ids") != []
            or result.get("coverage")
            != {"state": "NOT_APPLICABLE", "reason": "excluded domain"}
            or not rows
        ):
            raise SuccessorEvidenceError(f"{context} excluded-domain fixture differs")
    elif family == "query_retrieve_facts":
        relations = _mapping(
            mutated_inputs["admitted_relations"], f"{context} admitted relations"
        )
        input_set_id = _validate_retrieve_fact_input_set(
            relations, mutated_inputs["pinned_epoch"]["policy_release"], context
        )
        fact_rows = _list(relations["fact_rows"], f"{context} fact rows")
        type_fact = next(
            (
                value
                for value in fact_rows
                if isinstance(value, dict) and value.get("fact_kind") == "type"
            ),
            None,
        )
        if type_fact is None:
            raise SuccessorEvidenceError(f"{context} lacks the known type fact")
        if kind == "causal":
            if resolved.get("type_return") != type_fact.get("statement", {}).get(
                "object", {}
            ).get("return"):
                raise SuccessorEvidenceError(
                    f"{context} type-fact mutation is noncausal"
                )
        else:
            coverage_rows = _list(
                relations["coverage_rows"], f"{context} coverage rows"
            )
            effect_state = next(
                (
                    value.get("state")
                    for value in coverage_rows
                    if isinstance(value, dict) and value.get("family") == "effects"
                ),
                None,
            )
            coverage = _mapping(result["coverage"], f"{context} result coverage")
            if (
                effect_state != "PARTIAL"
                or coverage.get("families", {}).get("effects") != effect_state
                or "unknown_reason" not in coverage
            ):
                raise SuccessorEvidenceError(
                    f"{context} partial coverage fixture differs"
                )
        identity_contract = _mapping(
            result.get("identity_contract"), f"{context} fact identity contract"
        )
        coverage_rows = _list(relations["coverage_rows"], f"{context} coverage rows")
        effects = next(
            row
            for row in coverage_rows
            if isinstance(row, dict) and row.get("family") == "effects"
        )
        source_file_id = _validate_retrieve_source_identity(
            effects, envelope["scope"]["workspace_id"], context
        )
        property_kinds = _property_kind_allocation(relations, context)
        type_code = property_kinds.get("type")
        unknown_code = property_kinds.get("UNKNOWN_EFFECT")
        if (
            type_code is None
            or unknown_code is None
            or type_fact.get("property_kind_code") != type_code
        ):
            raise SuccessorEvidenceError(
                f"{context} fixture property-kind allocation differs"
            )
        type_statement = _mapping(
            type_fact.get("statement"), f"{context} fixture type statement"
        )
        type_scalar = rfc8785.dumps(type_statement["object"]).decode("utf-8")
        type_typed = _cbef_typed_value(2, type_scalar.encode("utf-8"))
        type_tagged = (
            (50).to_bytes(2, "big") + len(type_typed).to_bytes(4, "big") + type_typed
        )
        type_id = _validate_cbef_recipe(
            identity_contract.get("known_type_identity_recipe"),
            domain_code=10,
            domain_name="PROPERTY_FACT",
            output_prefix="fact:type",
            fields=[
                (
                    1,
                    "workspace_id",
                    7,
                    "ID",
                    _public_id_bytes(type_fact["workspace_id"], "workspace"),
                    type_fact["workspace_id"],
                ),
                (
                    2,
                    "analysis_context_id",
                    7,
                    "ID",
                    _analysis_context_id_bytes(type_fact["analysis_context_id"]),
                    type_fact["analysis_context_id"],
                ),
                (
                    3,
                    "property_kind_code",
                    4,
                    "UNSIGNED",
                    type_code.to_bytes(2, "big"),
                    type_code,
                ),
                (
                    4,
                    "subject_entity_id",
                    7,
                    "ID",
                    _public_domain_id_bytes(type_statement["subject"], "entity"),
                    type_statement["subject"],
                ),
                (
                    5,
                    "canonical_value",
                    12,
                    "TAGGED_UNION",
                    type_tagged,
                    {"variant": 50, "member_type": "UTF8", "value": type_scalar},
                ),
            ],
            excluded=[
                "source and producer provenance",
                "input-set and policy identity",
                "diagnostic evidence",
                "mutable coverage counters",
            ],
            context=context,
        )
        if type_fact.get("fact_id") != type_id or type_fact.get(
            "identity_recipe"
        ) != identity_contract.get("known_type_identity_recipe"):
            raise SuccessorEvidenceError(
                f"{context} admitted type row carries a stale property identity"
            )
        unknown_scalar = _nonempty_string(
            effects["family"], f"{context} unknown effect family"
        )
        unknown_typed = _cbef_typed_value(2, unknown_scalar.encode("utf-8"))
        unknown_tagged = (
            (50).to_bytes(2, "big")
            + len(unknown_typed).to_bytes(4, "big")
            + unknown_typed
        )
        unknown_id = _validate_cbef_recipe(
            identity_contract.get("identity_recipe"),
            domain_code=10,
            domain_name="PROPERTY_FACT",
            output_prefix="fact:unknown-effect",
            fields=[
                (
                    1,
                    "workspace_id",
                    7,
                    "ID",
                    _public_id_bytes(envelope["scope"]["workspace_id"], "workspace"),
                    envelope["scope"]["workspace_id"],
                ),
                (
                    2,
                    "analysis_context_id",
                    7,
                    "ID",
                    _analysis_context_id_bytes(type_fact["analysis_context_id"]),
                    type_fact["analysis_context_id"],
                ),
                (
                    3,
                    "property_kind_code",
                    4,
                    "UNSIGNED",
                    unknown_code.to_bytes(2, "big"),
                    unknown_code,
                ),
                (
                    4,
                    "subject_entity_id",
                    7,
                    "ID",
                    _public_domain_id_bytes(block["about"][0], "entity"),
                    block["about"][0],
                ),
                (
                    5,
                    "canonical_value",
                    12,
                    "TAGGED_UNION",
                    unknown_tagged,
                    {
                        "variant": 50,
                        "member_type": "UTF8",
                        "value": unknown_scalar,
                    },
                ),
            ],
            excluded=[
                "coverage state",
                "coverage reason",
                "retryability",
                "source and producer provenance",
                "input-set and policy identity",
                "diagnostic evidence",
                "mutable coverage counters",
            ],
            context=context,
        )
        if (
            identity_contract.get("known_type_fact_id") != type_id
            or identity_contract.get("unknown_fact_id") != unknown_id
            or identity_contract.get("input_set_id") != input_set_id
            or identity_contract.get("input_set_identity_recipe")
            != relations.get("input_set_identity")
            or identity_contract.get("source_file_id") != source_file_id
            or set(result.get("fact_ids", [])) != {type_id, unknown_id}
        ):
            raise SuccessorEvidenceError(f"{context} fact identity binding differs")
    elif family == "query_follow_relationships":
        selected_from_inputs = _derive_follow_edges(mutated_inputs, block, context)
        selected_input_ids = [str(edge["fact_id"]) for edge in selected_from_inputs]
        binding = _mapping(
            mutated_inputs["program_binding"], f"{context} follow program"
        )
        _strict_keys(
            binding,
            {
                "binding_relation_id",
                "distance_policy",
                "fact_identity_required",
                "form",
                "live_capability_state",
                "phrase_resolution_relation_id",
                "producer_closure_id",
                "projection_policy_id",
                "query_program_release",
                "relationship_resolution",
                "result_role",
            },
            f"{context} follow program",
        )
        access = _mapping(mutated_inputs["access_scope"], f"{context} follow access")
        _strict_keys(
            access,
            {"workspace_id", "relationship_families"},
            f"{context} follow access",
        )
        child = _validate_query_child_catalog(mutated_inputs, context)
        limits = _mapping(mutated_inputs["resource_limits"], f"{context} follow limits")
        _strict_keys(
            limits,
            {"deadline_ms", "max_distance", "max_rows"},
            f"{context} follow limits",
        )
        _nonempty_string(
            binding["query_program_release"],
            f"{context} follow query program release",
        )
        _nonempty_string(
            binding["producer_closure_id"],
            f"{context} follow producer closure identity",
        )
        if (
            binding.get("form") != block.get("request")
            or binding.get("live_capability_state") != "available"
            or binding.get("result_role") != "facts"
            or binding.get("binding_relation_id") != "query.binding"
            or binding.get("phrase_resolution_relation_id") != "query.phrase_resolution"
            or binding.get("projection_policy_id") != "projection:fixture-v1"
            or binding.get("distance_policy") != "exactly one step"
            or binding.get("fact_identity_required") is not True
            or binding.get("relationship_resolution")
            != {
                "phrase": block.get("relationship"),
                "family_id": block.get("relationship"),
                "resolution": "exact",
            }
            or access.get("workspace_id") != envelope["scope"]["workspace_id"]
            or set(access.get("relationship_families", []))
            != {block.get("relationship")}
            or set(child.get("visible_relations", []))
            != {"canonical.call_fact", "proof.call_coverage"}
            or child.get("visible_functions") != []
            or child.get("visible_object_stores") != []
            or limits.get("max_distance") != block.get("distance")
            or limits.get("max_rows", 0) < len(selected_input_ids)
            or limits.get("deadline_ms") != envelope["freshness"]["deadline_ms"]
        ):
            raise SuccessorEvidenceError(
                f"{context} follow fixture does not close program/access/catalog/bounds"
            )
        if kind == "causal":
            start = block["starting_from"][0]
            relations = _mapping(
                mutated_inputs["admitted_relations"], f"{context} admitted relations"
            )
            edges = _list(relations["call_edges"], f"{context} call edges")
            dictionary = _mapping(
                relations["entity_dictionary"], f"{context} entity dictionary"
            )
            for edge in edges:
                statement = _mapping(edge.get("statement"), f"{context} call statement")
                if not {statement.get("subject"), statement.get("object")} <= set(
                    dictionary
                ):
                    raise SuccessorEvidenceError(
                        f"{context} follow-edge dictionary closure differs"
                    )
            selected_edges = [
                edge
                for edge in edges
                if isinstance(edge, dict)
                and edge.get("statement", {}).get("subject") == start
            ]
            selected_edges.sort(
                key=lambda edge: (
                    edge.get("statement", {}).get("object", ""),
                    edge.get("fact_id", ""),
                )
            )
            expected_ids = [edge["fact_id"] for edge in selected_edges]
            added_fact = _mapping(result.get("added_fact"), f"{context} added fact")
            added_target = added_fact.get("statement", {}).get("object")
            coverage_proof = _mapping(
                mutated_inputs["producer_coverage"], f"{context} producer coverage"
            )
            _strict_keys(
                coverage_proof,
                {
                    "state",
                    "owner",
                    "analysis_context_id",
                    "family",
                    "covered_fact_ids",
                },
                f"{context} complete follow coverage",
            )
            selected_contexts = {
                str(edge["analysis_context_id"]) for edge in selected_edges
            }
            expected_coverage = {
                "state": coverage_proof.get("state"),
                "owner": coverage_proof.get("owner"),
                "analysis_context_id": coverage_proof.get("analysis_context_id"),
                "distance": block.get("distance"),
                "completed_family": coverage_proof.get("family"),
            }
            if (
                result.get("fact_ids") != expected_ids
                or added_fact not in selected_edges
                or result.get("added_entity") != dictionary.get(added_target)
                or coverage_proof.get("state") != "COMPLETE"
                or coverage_proof.get("owner") != start
                or coverage_proof.get("family") != block.get("relationship")
                or {coverage_proof.get("analysis_context_id")} != selected_contexts
                or set(coverage_proof.get("covered_fact_ids", [])) != set(expected_ids)
                or result.get("coverage") != expected_coverage
                or resolved
                != {
                    "starting_from": block["starting_from"],
                    "relationship": block["relationship"],
                    "direction": block["direction"],
                    "distance": block["distance"],
                }
            ):
                raise SuccessorEvidenceError(f"{context} follow-edge result differs")
        else:
            coverage_proof = _mapping(
                mutated_inputs["producer_coverage"], f"{context} partial coverage"
            )
            _strict_keys(
                coverage_proof,
                {
                    "state",
                    "owner",
                    "analysis_context_id",
                    "family",
                    "covered_fact_ids",
                    "remainders",
                },
                f"{context} partial follow coverage",
            )
            remainders = _list(
                coverage_proof.get("remainders"), f"{context} follow remainders"
            )
            if len(remainders) != 1:
                raise SuccessorEvidenceError(
                    f"{context} partial follow coverage lacks one explicit remainder"
                )
            remainder = _mapping(remainders[0], f"{context} follow remainder")
            expected_remainder = {
                "kind": "unknown_relationship_remainder",
                "owner_id": coverage_proof.get("owner"),
                "analysis_context_id": coverage_proof.get("analysis_context_id"),
                "family": coverage_proof.get("family"),
                "direction": block.get("direction"),
                "distance": block.get("distance"),
                "reason": "PRODUCER_COVERAGE_PARTIAL",
                "retryable": True,
            }
            expected_facts = {
                str(edge["fact_id"]): dict(edge) for edge in selected_from_inputs
            }
            expected_coverage = {
                "state": "PARTIAL",
                "owner": coverage_proof.get("owner"),
                "analysis_context_id": coverage_proof.get("analysis_context_id"),
                "completed_family": coverage_proof.get("family"),
                "distance": block.get("distance"),
                "covered_fact_ids": coverage_proof.get("covered_fact_ids"),
                "remainders": [expected_remainder],
            }
            if (
                coverage_proof.get("state") != "PARTIAL"
                or coverage_proof.get("owner") != block["starting_from"][0]
                or coverage_proof.get("family") != block.get("relationship")
                or {coverage_proof.get("analysis_context_id")}
                != {str(edge["analysis_context_id"]) for edge in selected_from_inputs}
                or set(coverage_proof.get("covered_fact_ids", []))
                != set(selected_input_ids)
                or remainder != expected_remainder
                or expected.get("execution_state") != "COMPLETE"
                or expected.get("availability_state") != "PARTIAL"
                or expected.get("completeness_state") != "PARTIAL"
                or resolved
                != {
                    "starting_from": block["starting_from"],
                    "relationship": block["relationship"],
                    "direction": block["direction"],
                    "distance": block["distance"],
                }
                or result.get("fact_ids") != selected_input_ids
                or result.get("facts") != expected_facts
                or result.get("remainders") != [expected_remainder]
                or result.get("coverage") != expected_coverage
            ):
                raise SuccessorEvidenceError(
                    f"{context} partial follow result does not retain known facts plus a typed unknown remainder"
                )
    elif family == "query_connecting_paths":
        edges, _, coverage_proof = _validate_path_authority(
            mutated_inputs, block, context
        )
        if kind == "causal":
            paths = result.get("paths")
            if not isinstance(paths, list) or len(paths) != 1:
                raise SuccessorEvidenceError(f"{context} causal path is not singular")
            path = _mapping(paths[0], f"{context} causal path")
            entity_ids, fact_ids = _canonical_shortest_query_witness(
                edges,
                start=str(block["from"][0]),
                target=str(block["to"][0]),
                families=set(block["using"]),
                maximum_length=int(
                    mutated_inputs["resource_limits"]["max_path_length"]
                ),
                context=context,
            )
            expected_semantics = {
                "from": block["from"],
                "to": block["to"],
                "relationship_families": block["using"],
                "path_policy": block["path_policy"],
                "maximum_path_length": mutated_inputs["resource_limits"][
                    "max_path_length"
                ],
            }
            if (
                path.get("ordered_fact_ids") != fact_ids
                or path.get("ordered_entity_ids") != entity_ids
                or path.get("length") != len(fact_ids)
                or path.get("path_policy") != block["path_policy"]
                or path.get("certainty_summary") != "exact"
                or resolved != expected_semantics
                or result.get("coverage")
                != {
                    "state": coverage_proof["state"],
                    "searched_fact_count": len(edges),
                }
            ):
                raise SuccessorEvidenceError(
                    f"{context} causal path is not the exact canonical shortest witness"
                )
            identity_contract = _mapping(
                result.get("identity_contract"), f"{context} path identity contract"
            )
            analysis_contexts = {
                edge.get("analysis_context_id")
                for edge in edges
                if isinstance(edge, dict)
            }
            if len(analysis_contexts) != 1:
                raise SuccessorEvidenceError(
                    f"{context} path witness crosses analysis contexts"
                )
            path_id = _validate_path_result_recipe(
                identity_contract.get("identity_recipe"),
                workspace_id=envelope["scope"]["workspace_id"],
                analysis_context_id=analysis_contexts.pop(),
                fabric_epoch_id=mutated_inputs["pinned_epoch"]["fabric_epoch_id"],
                policy_identity=mutated_inputs["pinned_epoch"]["policy_release"],
                ordered_entity_ids=entity_ids,
                ordered_fact_ids=fact_ids,
                context=context,
            )
            if (
                identity_contract.get("witness_bound") is not True
                or identity_contract.get("path_id") != path_id
                or path.get("path_id") != path_id
                or result.get("path_ids") != [path_id]
            ):
                raise SuccessorEvidenceError(
                    f"{context} path identity is detached from its witness"
                )
        elif (
            block.get("path_policy") != "unrestricted all paths"
            or block.get("path_policy")
            == mutated_inputs["program_binding"]["path_policy"]
            or "UNBOUNDED_QUERY" not in codes
            or "identity_contract" in result
            or result.get("path_ids") != []
            or result.get("coverage") != {"state": "NOT_APPLICABLE"}
        ):
            raise SuccessorEvidenceError(f"{context} unbounded-path fixture differs")
    elif family == "query_match_pattern":
        matches, entity_ids, fact_ids, expected_coverage, dictionary = (
            _derive_typed_pattern_matches(mutated_inputs, block, context)
        )
        pattern = block["pattern"]
        expected_semantics = {
            "pattern_id": "pattern:typed-edge-no-outgoing-call-v1",
            "typed_bindings": {
                str(node["binding"]): str(node["semantic_kind"])
                for node in pattern["nodes"]
            },
            "positive_fact_count": len(pattern["facts"]),
            "scoped_negation_universe": mutated_inputs["producer_coverage"][
                "negative_proof_universe_id"
            ],
        }
        if kind == "causal":
            relations = _mapping(
                mutated_inputs["admitted_relations"], f"{context} admitted relations"
            )
            f_node = next(node for node in pattern["nodes"] if node["binding"] == "f")
            f_entity = next(
                value
                for value in relations["entities"]["rows"]
                if value.get("qualified_name", "").rsplit(".", 1)[-1] == f_node["name"]
                and value.get("module_id") == f_node["module_id"]
            )
            has_outgoing = any(
                edge.get("statement", {}).get("subject") == f_entity["entity_id"]
                and edge.get("statement", {}).get("predicate") == "calls"
                for edge in relations["call_edges"]
            )
            if (
                not has_outgoing
                or matches != []
                or result.get("bindings") != []
                or result.get("entity_ids") != []
                or result.get("fact_ids") != []
                or result.get("evaluated_fact_ids")
                != mutated_inputs["producer_coverage"]["covered_fact_ids"]
                or result.get("coverage") != expected_coverage
                or resolved != expected_semantics
            ):
                raise SuccessorEvidenceError(f"{context} scoped-negation delta differs")
        else:
            coverage = _mapping(
                mutated_inputs["producer_coverage"], f"{context} partial coverage"
            )
            remainders = _list(
                coverage.get("remainders"), f"{context} pattern remainders"
            )
            remainder = _mapping(remainders[0], f"{context} pattern remainder")
            expected_remainder = {
                "kind": "unknown_scoped_negation_remainder",
                "owner_scope": coverage["owner_scope"],
                "analysis_context_id": coverage["analysis_context_id"],
                "family": coverage["family"],
                "covered_subject_ids": coverage["covered_subject_ids"],
                "reason": "NEGATIVE_PROOF_UNIVERSE_PARTIAL",
                "retryable": True,
            }
            expected_records = {
                entity_id: dictionary[entity_id] for entity_id in entity_ids
            }
            admitted_edges = mutated_inputs["admitted_relations"]["call_edges"]
            expected_facts = {
                fact_id: next(
                    dict(edge) for edge in admitted_edges if edge["fact_id"] == fact_id
                )
                for fact_id in fact_ids
            }
            if (
                coverage.get("state") != "PARTIAL"
                or remainder != expected_remainder
                or "NEGATIVE_PROOF_INDETERMINATE" not in codes
                or expected.get("execution_state") != "COMPLETE"
                or expected.get("availability_state") != "PARTIAL"
                or expected.get("completeness_state") != "INDETERMINATE"
                or not matches
                or any(
                    match.get("binding_state") != "INDETERMINATE"
                    or any(
                        negation.get("state") != "INDETERMINATE"
                        for negation in match.get("scoped_negation", [])
                    )
                    for match in matches
                )
                or result.get("bindings") != matches
                or result.get("entity_ids") != entity_ids
                or result.get("fact_ids") != fact_ids
                or result.get("entity_records") != expected_records
                or result.get("facts") != expected_facts
                or result.get("remainders") != [expected_remainder]
                or result.get("coverage") != expected_coverage
                or resolved != expected_semantics
            ):
                raise SuccessorEvidenceError(
                    f"{context} indeterminate pattern does not retain known typed bindings/facts and an explicit remainder"
                )
    elif family == "query_combine_results":
        relations = _mapping(
            mutated_inputs["admitted_relations"], f"{context} admitted relations"
        )
        producer_blocks = [
            _mapping(value, f"{context} producer block")
            for value in queries
            if isinstance(value, dict) and value.get("request") == "find code entities"
        ]
        references = [
            str(value.get("results_of"))
            for value in block["inputs"]
            if isinstance(value, dict)
        ]
        if kind == "causal":
            dictionary = _mapping(
                relations["entity_dictionary"], f"{context} entity dictionary"
            )
            producer_results = _derive_combine_producer_results(
                producer_blocks=producer_blocks,
                relations=relations,
                provenance=_query_provenance_from_inputs(mutated_inputs),
                admitted_entities=dictionary,
                context=context,
            )
            intersection = sorted(
                set(producer_results[references[0]]["entity_ids"])
                & set(producer_results[references[1]]["entity_ids"])
            )
            expected_records = {
                entity_id: dictionary[entity_id]
                for entity_id in intersection
                if entity_id in dictionary
            }
            if (
                result.get("entity_ids") != intersection
                or result.get("producer_results") != producer_results
                or result.get("upstream_query_ids") != references
                or len(expected_records) != len(intersection)
                or result.get("entity_records") != expected_records
            ):
                raise SuccessorEvidenceError(f"{context} intersection result differs")
        else:
            if "producer_results" in relations:
                raise SuccessorEvidenceError(
                    f"{context} negative fixture retains circular producer output authority"
                )
            producer_inputs = _mapping(
                relations.get("producer_inputs"), f"{context} producer base inputs"
            )
            missing = [
                reference
                for reference in references
                if reference not in producer_inputs
            ]
            if (
                missing != ["right"]
                or resolved.get("dangling_result_reference") != "right"
                or "DANGLING_RESULT_REFERENCE" not in codes
            ):
                raise SuccessorEvidenceError(
                    f"{context} dangling composition reference differs"
                )
    elif family == "query_summarize_facts":
        if kind == "causal":
            relations = _mapping(
                mutated_inputs["admitted_relations"],
                f"{context} admitted objective relations",
            )
            input_set_id, rows, grouped_facts = _validate_objective_fact_inputs(
                relations,
                str(mutated_inputs["pinned_epoch"]["policy_release"]),
                context,
            )
            groups = [
                _mapping(value, f"{context} causal objective group")
                for value in _list(result.get("groups"), f"{context} causal groups")
            ]
            group_ids = _validate_objective_groups(
                groups,
                input_set_id=input_set_id,
                grouped_facts=grouped_facts,
                context=context,
            )
            identity = _mapping(
                result.get("identity_contract"),
                f"{context} objective identity contract",
            )
            changed_fact = next(
                (
                    row
                    for row in rows
                    if row.get("fact_id") == identity.get("changed_fact_id")
                ),
                None,
            )
            if (
                changed_fact is None
                or identity.get("changed_fact_identity_recipe")
                != changed_fact.get("identity_recipe")
                or identity.get("input_set_id") != input_set_id
                or identity.get("input_set_identity_recipe")
                != relations.get("input_set_identity")
                or identity.get("group_ids_by_native_kind")
                != {
                    str(group["group_key"]["native_kind"]): group["group_id"]
                    for group in groups
                }
                or result.get("group_ids") != group_ids
                or resolved.get("input_set_id") != input_set_id
                or result.get("coverage", {}).get("input_set_id") != input_set_id
            ):
                raise SuccessorEvidenceError(
                    f"{context} objective summary identity chain differs"
                )
        elif (
            block.get("measure") != "high risk"
            or "NOT_OBJECTIVE_FACT_REQUEST" not in codes
        ):
            raise SuccessorEvidenceError(
                f"{context} evaluative-summary fixture differs"
            )
    elif family == "query_source_context":
        if kind == "causal":
            requested_context = _nonempty_string(
                block["context"], f"{context} source context request"
            )
            limit = _positive_int(
                mutated_inputs["resource_limits"]["max_source_bytes"],
                f"{context} source byte limit",
            )
            source = mutated_inputs["admitted_relations"]["source_bytes"]
            contexts = result.get("source_contexts")
            if not isinstance(contexts, list) or len(contexts) != 1:
                raise SuccessorEvidenceError(f"{context} source result is incomplete")
            source_context = contexts[0]
            if (
                resolved.get("context") != requested_context
                or resolved.get("explicit_source_byte_limit") != limit
                or source_context.get("returned_bytes")
                != min(limit, source["byte_length"])
                or source_context.get("omitted_bytes")
                != max(0, source["byte_length"] - limit)
            ):
                raise SuccessorEvidenceError(f"{context} source limit delta differs")
            identity_contract = _mapping(
                result.get("identity_contract"),
                f"{context} source identity contract",
            )
            span = _mapping(
                mutated_inputs["admitted_relations"]["entity_span"],
                f"{context} source span",
            )
            entity = _mapping(
                mutated_inputs["admitted_relations"]["entity_dictionary"][
                    span["entity_id"]
                ],
                f"{context} source entity",
            )
            delivered = source_context["content"]["text"].encode("utf-8")
            source_context_id = _validate_query_source_context_recipe(
                identity_contract.get("identity_recipe"),
                workspace_id=span["workspace_id"],
                analysis_context_id=entity["analysis_context_id"],
                snapshot_id=mutated_inputs["pinned_epoch"]["snapshot_id"],
                entity_id=span["entity_id"],
                source_file_id=span["source_file_id"],
                source_generation=span["source_generation"],
                source_content_digest=span["content_digest"],
                delivered_start_byte=span["start_byte"],
                delivered_end_byte=span["start_byte"] + len(delivered),
                delivered_content_digest=_bytes_b3(delivered),
                disclosure_scope_id=mutated_inputs["access_scope"]["scope_id"],
                policy_identity=mutated_inputs["pinned_epoch"]["policy_release"],
                context_kind=requested_context.lower(),
                context=context,
            )
            if (
                identity_contract.get("delivered_bytes_bound") is not True
                or identity_contract.get("source_context_id") != source_context_id
                or source_context.get("source_context_id") != source_context_id
                or result.get("source_context_ids") != [source_context_id]
            ):
                raise SuccessorEvidenceError(
                    f"{context} source identity is detached from delivered bytes"
                )
        else:
            denied_scope = _mapping(
                mutated_inputs["access_scope"], f"{context} denied source scope"
            )
            denied_recipe = _authorization_scope_recipe(
                denied_scope,
                {"policy_id": mutated_inputs["pinned_epoch"]["policy_release"]},
            )
            if (
                denied_scope.get("source_access") is not False
                or "source.authorized_bytes"
                in set(denied_scope.get("allowed_relations", []))
                or "source.authorized_bytes"
                in set(denied_scope.get("allowed_columns", {}))
                or denied_scope.get("source_file_ids") != []
                or denied_scope.get("authorized_ranges") != []
                or denied_scope.get("scope_id") != denied_recipe["output_id"]
                or denied_scope.get("identity_recipe") != denied_recipe
                or "SOURCE_ACCESS_DENIED" not in codes
                or "identity_contract" in result
            ):
                raise SuccessorEvidenceError(
                    f"{context} source authorization fixture differs"
                )


def _validate_fixture_mutation_semantics(
    expectation: Mapping[str, Any],
    fixture: Mapping[str, Any],
    mutated_inputs: Mapping[str, Any],
    context: str,
) -> None:
    family = str(expectation["claim_family"])
    kind = str(fixture["kind"])
    expected = fixture["expected_decoded"]
    if family in EXPECTED_QUERY_FAMILIES:
        _validate_query_fixture_decoded(expected, context)
        request = _mapping(
            mutated_inputs["request_envelope"], f"{context} mutated request"
        )
        if set(request) != {"decoded", "canonical_json"}:
            raise SuccessorEvidenceError(f"{context} mutated request is not atomic")
        try:
            encoded = json.loads(
                str(request["canonical_json"]), object_pairs_hook=_reject_duplicates
            )
        except json.JSONDecodeError as error:
            raise SuccessorEvidenceError(
                f"{context} mutated canonical request is invalid"
            ) from error
        if encoded != request["decoded"]:
            raise SuccessorEvidenceError(
                f"{context} mutated canonical request differs from decoded value"
            )
        _validate_query_fixture_causality(
            family,
            kind,
            mutated_inputs,
            _mapping(expected, f"{context} query expectation"),
            context,
        )
        return
    if family == "exact_provider_facts":
        if kind == "causal":
            images = _list(mutated_inputs["source_images"], f"{context} source images")
            image = next(
                (
                    value
                    for value in images
                    if isinstance(value, dict) and value.get("source_id") == "py:typed"
                ),
                None,
            )
            if image is None:
                raise SuccessorEvidenceError(f"{context} lacks mutated typed source")
            source = str(image["bytes_utf8"])
            call = re.search(r"return\s+([A-Za-z_]\w*)\(", source)
            digest = image["content_digest"]
            pyrefly_requests = [
                value
                for value in mutated_inputs["provider_requests"]
                if isinstance(value, dict) and value.get("provider_id") == "pyrefly"
            ]
            call_name = call.group(1) if call is not None else ""
            call_start = (
                source.index(call_name, call.start()) if call is not None else -1
            )
            if (
                call is None
                or digest != _bytes_b3(source.encode("utf-8"))
                or len(pyrefly_requests) != 1
                or expected
                != {
                    "provider_id": "pyrefly",
                    "provider_run_id": pyrefly_requests[0]["provider_run_id"],
                    "relation_id": "provider.pyrefly.call_target.v1",
                    "source_digest": digest,
                    "native_fields": {
                        "call_occurrence_ordinal": 0,
                        "start_byte": call_start,
                        "end_byte": call_start + len(call_name),
                        "target_ordinal": 0,
                        "callee_kind": "function",
                        "qualified_target": call_name,
                        "class_name": None,
                        "resolution_state": "resolved",
                    },
                }
            ):
                raise SuccessorEvidenceError(f"{context} provider source delta differs")
        else:
            terminals = _list(
                mutated_inputs["coverage_terminals"], f"{context} coverage terminals"
            )
            open_terminals = [
                value
                for value in terminals
                if isinstance(value, dict) and value.get("state") == "open"
            ]
            if (
                len(open_terminals) != 1
                or open_terminals[0].get("state") != "open"
                or open_terminals[0].get("requested_units") != 1
                or open_terminals[0].get("completed_units") != 0
                or open_terminals[0].get("remainders") != []
                or expected.get("error") != "PROVIDER_REQUESTED_COVERAGE_OPEN"
                or expected.get("relation_id") != open_terminals[0].get("relation_id")
            ):
                raise SuccessorEvidenceError(
                    f"{context} provider coverage fault differs"
                )
    elif family == "programmatic_transformations":
        if kind == "causal":
            _validate_transformation_inputs(
                mutated_inputs,
                _mapping(expected, f"{context} transformation expectation"),
                context,
            )
        else:
            definition = mutated_inputs["transformation_definition"]
            column = definition["plan_building_function"]["operations"][0]["predicate"][
                "left"
            ]["name"]
            if (
                column in mutated_inputs["input_schema"]
                or expected.get("error") != "TRANSFORMATION_INPUT_COLUMN_UNDECLARED"
                or expected.get("column") != column
            ):
                raise SuccessorEvidenceError(
                    f"{context} undeclared-column fault differs"
                )
    elif family == "derived_analyses":
        provider_targets = _mapping(
            mutated_inputs["provider_call_targets"],
            f"{context} mutated provider call targets",
        )
        occurrences = mutated_inputs["canonical_call_occurrences"]
        callables = mutated_inputs["canonical_callable_lookup"]
        if kind == "causal":
            mutation = _mapping(fixture["mutation"], f"{context} mutation")
            rows = _derived_call_graph_rows(
                provider_targets,
                occurrences,
                callables,
                context,
                require_complete=True,
            )
            if (
                mutation.get("input_role") != "provider_call_targets"
                or mutation.get("json_pointer") != "/rows/0/5"
                or mutation.get("before") != "fixture.alpha"
                or mutation.get("after") != "fixture.beta"
                or expected
                != {
                    "relation": "analysis.common_call_graph.v1",
                    "columns": expectation["decoded_expectation"]["columns"],
                    "rows": rows,
                }
                or rows == expectation["decoded_expectation"]["rows"]
            ):
                raise SuccessorEvidenceError(
                    f"{context} concrete call-graph causal delta differs"
                )
        else:
            known_rows = _derived_call_graph_rows(
                provider_targets,
                occurrences,
                callables,
                context,
                require_complete=False,
            )
            terminal = _mapping(
                provider_targets["coverage_terminal"],
                f"{context} partial call-target coverage",
            )
            remainders = _list(
                terminal["remainders"], f"{context} call-target remainders"
            )
            if (
                terminal.get("state") != "partial"
                or terminal.get("requested_call_sites") != 2
                or terminal.get("completed_call_sites") != 1
                or len(remainders) != 1
            ):
                raise SuccessorEvidenceError(
                    f"{context} call-target coverage did not become partial"
                )
            remainder = _mapping(remainders[0], f"{context} call-target remainder")
            provenance = {
                "input_relation": provider_targets["relation"],
                "provider_run_id": provider_targets["provider_run_id"],
                "analysis_context_id": provider_targets["source_image"][
                    "analysis_context_id"
                ],
                "source_content_digest": provider_targets["source_image"][
                    "content_digest"
                ],
                "analysis_semantic_id": "analysis.common.call-graph.candidate-preserving.v1",
            }
            expected_partial = {
                "known_facts": {
                    "relation": "analysis.common_call_graph.v1",
                    "columns": expectation["decoded_expectation"]["columns"],
                    "rows": known_rows,
                },
                "unknown_remainder": {
                    "relation": "analysis.common_unknown.v1",
                    "columns": [
                        "call_site_id",
                        "family_id",
                        "reason",
                        "retryable",
                        "provenance",
                    ],
                    "rows": [
                        [
                            remainder.get("call_site_id"),
                            "common.call_graph",
                            remainder.get("reason"),
                            remainder.get("retryable"),
                            provenance,
                        ]
                    ],
                },
                "published_false_edges": [],
            }
            occurrence_rows = _mapping(
                occurrences, f"{context} canonical call occurrences"
            )["rows"]
            missing = next(
                (
                    row
                    for row in occurrence_rows
                    if isinstance(row, list)
                    and len(row) == 10
                    and row[1] == remainder.get("call_site_id")
                ),
                None,
            )
            if (
                missing is None
                or remainder.get("reason") != "PROVIDER_TARGET_SET_INCOMPLETE"
                or len(known_rows) != 1
                or known_rows[0][3] != "resolved"
                or expected != expected_partial
                or any(
                    isinstance(row, list)
                    and len(row) == 8
                    and (row[0], row[1], row[2]) == (missing[0], missing[7], missing[8])
                    for row in provider_targets["rows"]
                )
            ):
                raise SuccessorEvidenceError(
                    f"{context} explicit call-graph unknown differs"
                )
    elif family == "delta_exact_version_protocol":
        history = _mapping(
            mutated_inputs["delta_table_history"], f"{context} Delta history"
        )
        if set(history) == {"table", "materialization", "schema", "versions"}:
            versions = [
                _mapping(value, f"{context} Delta version")
                for value in _list(history["versions"], f"{context} Delta versions")
            ]
            snapshots: dict[int, list[list[Any]]] = {}
            snapshot: list[list[Any]] = []
            for version in versions:
                snapshot = [*snapshot, *copy.deepcopy(version["input_rows"])]
                snapshots[version["version"]] = copy.deepcopy(snapshot)
            selected = mutated_inputs["selected_version_vector"]["table_versions"][
                "fact.entity"
            ]
            latest = max(snapshots)
            if kind == "causal":
                cdf = mutated_inputs["proof_input"]["cdf_read"]
                cdf_rows = [
                    [*row, "insert", version["version"]]
                    for version in versions
                    if cdf["starting_version"]
                    <= version["version"]
                    <= cdf["ending_version"]
                    for row in version["input_rows"]
                ]
                if expected != {
                    "selected_version": selected,
                    "latest_version": latest,
                    "protocol": versions[selected]["protocol"],
                    "snapshot_rows": snapshots[selected],
                    "cdf_window": {
                        "starting_version": cdf["starting_version"],
                        "ending_version": cdf["ending_version"],
                        "inclusive": True,
                    },
                    "cdf_columns": [
                        "entity_id",
                        "value",
                        "_change_type",
                        "_commit_version",
                    ],
                    "cdf_rows": cdf_rows,
                }:
                    raise SuccessorEvidenceError(
                        f"{context} programmatic Delta version delta differs"
                    )
            else:
                unsupported = [
                    version
                    for version in versions
                    if "rowTracking" in version["protocol"]["writer_features"]
                ]
                expected_fault = (
                    {
                        "error": "DELTA_WRITER_FEATURE_UNSUPPORTED",
                        "feature": "rowTracking",
                        "table_version": unsupported[0]["version"],
                    }
                    if len(unsupported) == 1
                    else None
                )
                if (
                    len(unsupported) != 1
                    or unsupported[0]["protocol"]["min_writer_version"] != 7
                    or "rowTracking"
                    not in mutated_inputs["protocol_support"][
                        "unsupported_writer_features"
                    ]
                    or expected != expected_fault
                ):
                    raise SuccessorEvidenceError(
                        f"{context} unsupported Delta feature fault differs"
                    )
            return
        versions, snapshots = _reconstruct_delta_snapshots(history, context)
        selected = mutated_inputs["selected_version_vector"]["table_versions"][
            "fact.entity"
        ]
        if kind == "causal":
            if selected not in versions:
                raise SuccessorEvidenceError(
                    f"{context} Delta causal version is not reconstructable"
                )
            commit = versions[selected]
            operation = commit["operation"]
            if fixture["expected_decoded"] != {
                "exact_selection": _delta_selection_proof(
                    mutated_inputs, history, selected, commit, context
                ),
                "table_version": selected,
                "relation": f"fact.entity@{selected}",
                "operation": operation["name"],
                "save_mode": operation["mode"],
                "base_version": operation["base_version"],
                "rows": snapshots[selected],
            }:
                raise SuccessorEvidenceError(f"{context} Delta causal outcome differs")
        if kind == "negative":
            write = mutated_inputs["proof_input"]["operations_under_test"]["write"]
            committed_version = write["committed_version"]
            commit = versions.get(committed_version)
            if commit is None:
                raise SuccessorEvidenceError(
                    f"{context} Delta negative write version is not reconstructable"
                )
            protocol = commit["protocol"]
            if (
                write["base_version"] != commit["operation"]["base_version"]
                or "rowTracking" not in protocol["writer_features"]
                or expected
                != {
                    "error": "DELTA_WRITER_FEATURE_UNSUPPORTED",
                    "feature": "rowTracking",
                    "min_writer_version": protocol["min_writer_version"],
                    "table_version": committed_version,
                }
            ):
                raise SuccessorEvidenceError(
                    f"{context} Delta negative is not bound to the exact write under test"
                )
    elif family == "activation_recovery":
        activation_chain = _mapping(
            mutated_inputs["activation_chain"], f"{context} activation chain"
        )
        if set(activation_chain) == PROGRAMMATIC_ACTIVATION_CHAIN_KEYS:
            if kind == "causal":
                events = _validate_programmatic_activation_chain(
                    activation_chain, context
                )
                if expected != _programmatic_activation_outcome(
                    mutated_inputs, events[-1]
                ):
                    raise SuccessorEvidenceError(
                        f"{context} programmatic activation head differs"
                    )
            else:
                try:
                    _validate_programmatic_activation_chain(activation_chain, context)
                except SuccessorEvidenceError:
                    pass
                else:
                    raise SuccessorEvidenceError(
                        f"{context} contradictory activation readback was accepted"
                    )
                if expected != {
                    "error": "ACTIVATION_TRANSACTION_READBACK_MISMATCH",
                    "admission_state": "closed",
                }:
                    raise SuccessorEvidenceError(
                        f"{context} activation rejection terminal differs"
                    )
            return
        if kind == "causal":
            events, readback = _activation_events(
                mutated_inputs["activation_chain"], context
            )
            head = _activation_head(events, context)
            if (
                expected.get("selected_epoch") != head["epoch"]
                or expected.get("predecessor") != head["predecessor"]
                or expected.get("selected_event") != head
                or expected.get("durable_event_identity")
                != head["durable_event_identity"]
                or expected.get("activation_relation_readback") != readback
            ):
                raise SuccessorEvidenceError(
                    f"{context} activation causal head differs"
                )
        else:
            chain = _mapping(
                mutated_inputs["activation_chain"], f"{context} activation chain"
            )
            events = [
                _mapping(value, f"{context} activation event")
                for value in _list(chain["events"], f"{context} activation events")
            ]
            try:
                _activation_head(events, context)
            except SuccessorEvidenceError:
                pass
            else:
                raise SuccessorEvidenceError(
                    f"{context} activation negative still has one head"
                )
    elif family == "authorization":
        scope = _mapping(mutated_inputs["access_scope"], f"{context} access scope")
        policy = _mapping(
            mutated_inputs["authorization_policy"], f"{context} authorization policy"
        )
        allowed = scope["allowed_relations"]
        if kind == "causal":
            available = set(mutated_inputs["epoch_provider_catalog"]["relations"])
            visible = sorted(set(allowed) & available)
            columns = set(mutated_inputs["access_scope"]["allowed_columns"])
            original_inputs = expectation["complete_input_universe"]["inputs"]
            previous_installed = original_inputs["child_catalog_bindings"][
                "installed_relations"
            ]
            derived_recipe = _authorization_scope_recipe(scope, policy)
            if (
                set(allowed) != columns
                or scope.get("scope_id") != derived_recipe["output_id"]
                or scope.get("identity_recipe") != derived_recipe
                or scope.get("scope_id") == original_inputs["access_scope"]["scope_id"]
                or expected.get("scope_id") != scope.get("scope_id")
                or expected.get("previous_scope_id")
                != original_inputs["access_scope"]["scope_id"]
                or not set(allowed) <= available
                or expected.get("visible_relations") != visible
                or expected.get("rebuilt_installed_relations") != visible
                or expected.get("previous_installed_relations") != previous_installed
                or expected.get("bound_plan_providers_unchanged")
                != mutated_inputs["bound_plan"]["providers"]
            ):
                raise SuccessorEvidenceError(f"{context} authorization delta differs")
        else:
            derived_recipe = _authorization_scope_recipe(scope, policy)
            derived_scope_id = derived_recipe["output_id"]
            original_scope = expectation["complete_input_universe"]["inputs"][
                "access_scope"
            ]
            if (
                scope.get("scope_id") == derived_scope_id
                or scope.get("identity_recipe") != original_scope.get("identity_recipe")
                or scope.get("identity_recipe") == derived_recipe
                or expected.get("error") != "ACCESS_SCOPE_IDENTITY_MISMATCH"
                or expected.get("supplied_scope_id") != scope.get("scope_id")
                or expected.get("derived_scope_id") != derived_scope_id
            ):
                raise SuccessorEvidenceError(
                    f"{context} stale authorization-scope identity was not rejected"
                )
    elif family == "resource_terminals":
        batch = _mapping(
            mutated_inputs["actual_output_batch"], f"{context} output batch"
        )
        if "schema_contract" in batch:
            if kind == "causal":
                budget = mutated_inputs["resource_budget"]["rows"]
                actual = batch["row_count"]
                if budget >= actual or expected != _programmatic_resource_terminal(
                    mutated_inputs, "hard_limit_exceeded", False
                ):
                    raise SuccessorEvidenceError(
                        f"{context} programmatic row-budget terminal differs"
                    )
            else:
                cancellation = mutated_inputs["cancellation_state"]
                if cancellation != {
                    "cancelled": True,
                    "cancellation_ordinal": 2,
                } or expected != _programmatic_resource_terminal(
                    mutated_inputs, "cancelled", False
                ):
                    raise SuccessorEvidenceError(
                        f"{context} programmatic cancellation terminal differs"
                    )
            return
        if kind == "causal":
            budget = mutated_inputs["resource_budget"]["rows"]
            actual = mutated_inputs["actual_output_batch"]["row_count"]
            if (
                budget >= actual
                or set(expected)
                != {
                    "query_id",
                    "state",
                    "rows_emitted",
                    "published_resources",
                    "terminal_provenance",
                }
                or expected.get("state") != "limit_exceeded"
                or expected.get("rows_emitted") != budget
                or expected.get("published_resources") != 0
                or expected.get("terminal_provenance")
                != _resource_terminal_provenance(
                    mutated_inputs,
                    "discarded_before_publication",
                    "released",
                    "not_published",
                )
            ):
                raise SuccessorEvidenceError(f"{context} row-budget terminal differs")
        else:
            cancellation = mutated_inputs["cancellation_state"]
            if (
                not cancellation["cancelled"]
                or cancellation["cancellation_ordinal"] is None
                or set(expected)
                != {
                    "query_id",
                    "state",
                    "cancellation_ordinal",
                    "published_resources",
                    "active_leases",
                    "terminal_provenance",
                }
                or expected.get("cancellation_ordinal")
                != cancellation["cancellation_ordinal"]
                or expected.get("active_leases") != 0
                or expected.get("published_resources") != 0
                or expected.get("terminal_provenance")
                != _resource_terminal_provenance(
                    mutated_inputs,
                    "discarded_before_publication",
                    "released",
                    "not_published",
                )
            ):
                raise SuccessorEvidenceError(
                    f"{context} cancellation negative lacks terminal cleanup"
                )
    elif family == "security_denial":
        if (
            mutated_inputs["launcher_evidence_contract"].get("contract_id")
            == "rust-launcher-evidence-v2"
        ):
            mutation = _mapping(fixture["mutation"], f"{context} security mutation")
            if kind == "causal":
                if (
                    mutation["input_role"] != "explicit_authorization"
                    or mutation["json_pointer"] != ""
                ):
                    raise SuccessorEvidenceError(
                        f"{context} authorization fixture must mutate only explicit authorization"
                    )
                authorization = _mapping(
                    mutated_inputs["explicit_authorization"],
                    f"{context} explicit authorization",
                )
                _strict_keys(
                    authorization,
                    {"job_id", "trusted_local", "authorization_id"},
                    f"{context} explicit authorization",
                )
                if (
                    authorization
                    != {
                        "job_id": "trusted-local-request-1",
                        "trusted_local": True,
                        "authorization_id": "authorization:trusted-1",
                    }
                    or mutated_inputs["launcher_constraints"]["untrusted_admission"]
                    != "unavailable"
                    or expected
                    != {
                        "job_id": "trusted-local-request-1",
                        "authorization_state": "accepted",
                        "effective_trust_profile": "trusted_local",
                        "capability_state": "degraded_trusted_local",
                        "launch_admission": "authorized_for_trusted_local_launch",
                        "untrusted_admission": "unavailable",
                        "hostile_actions_attempted": 0,
                    }
                ):
                    raise SuccessorEvidenceError(
                        f"{context} trusted-local authorization preflight differs"
                    )
            else:
                required = mutated_inputs["launcher_constraints"]["required"]
                if (
                    mutation["input_role"] != "launcher_constraints"
                    or mutation["json_pointer"]
                    != "/required/compiled_seccomp_policy_authorized"
                    or required.get("compiled_seccomp_policy_authorized") is not False
                    or expected
                    != {
                        "error": "HOST_CONTAINMENT_POLICY_WEAKENING_REJECTED",
                        "missing_prerequisite": ("compiled_seccomp_policy_authorized"),
                        "untrusted_admission": "unavailable",
                        "hostile_actions_attempted": 0,
                    }
                ):
                    raise SuccessorEvidenceError(
                        f"{context} host-containment weakening fault differs"
                    )
            return
        baseline_inputs = _mapping(
            expectation["complete_input_universe"]["inputs"],
            f"{context} baseline security inputs",
        )
        baseline_jobs = [
            _mapping(value, f"{context} baseline provider job")
            for value in _list(
                baseline_inputs["provider_jobs"], f"{context} baseline provider jobs"
            )
        ]
        hostile_job = _security_job_by_profile(baseline_jobs, "untrusted", context)
        trusted_job = _security_job_by_profile(baseline_jobs, "trusted_local", context)
        baseline_rows = _list(
            expectation["decoded_expectation"]["rows"],
            f"{context} baseline security terminal rows",
        )
        hostile_terminal = next(
            (
                row
                for row in baseline_rows
                if isinstance(row, list) and row and row[0] == hostile_job["job_id"]
            ),
            None,
        )
        if hostile_terminal is None:
            raise SuccessorEvidenceError(
                f"{context} baseline hostile terminal is missing"
            )
        hostile_suite_proof_id = _nonempty_string(
            hostile_terminal[7], f"{context} hostile suite proof id"
        )
        mutation = _mapping(fixture["mutation"], f"{context} security mutation")
        if kind == "causal":
            if (
                mutation["input_role"] != "explicit_authorization"
                or mutation["json_pointer"] != ""
            ):
                raise SuccessorEvidenceError(
                    f"{context} authorization fixture must mutate only explicit authorization"
                )
            for role in (
                "provider_jobs",
                "trust_policy",
                "launcher_evidence_contract",
                "launcher_constraints",
                "hostile_actions",
                "resource_limits",
            ):
                if mutated_inputs[role] != baseline_inputs[role]:
                    raise SuccessorEvidenceError(
                        f"{context} authorization fixture changed fixed launcher/job evidence"
                    )
            authorization = _mapping(
                mutated_inputs["explicit_authorization"],
                f"{context} explicit authorization",
            )
            _strict_keys(
                authorization,
                {"job_id", "trusted_local", "authorization_id"},
                f"{context} explicit authorization",
            )
            authorization_id = _nonempty_string(
                authorization["authorization_id"], f"{context} authorization id"
            )
            if (
                authorization["job_id"] != trusted_job["job_id"]
                or authorization["trusted_local"] is not True
                or authorization_id == "authorization:none"
            ):
                raise SuccessorEvidenceError(
                    f"{context} trusted-local authorization remains incomplete"
                )
            receipt_id, proof_id, provenance_id = _security_launcher_identities(
                mutated_inputs,
                trusted_job,
                authorization_id,
                [],
                "authorized",
                "trusted_local_visible",
                "TRUSTED_LOCAL",
                hostile_suite_proof_id,
            )
            launcher_job_inputs = {
                "provider_job": trusted_job,
                "launcher_evidence_contract": baseline_inputs[
                    "launcher_evidence_contract"
                ],
                "launcher_constraints": baseline_inputs["launcher_constraints"],
                "resource_limits": baseline_inputs["resource_limits"],
            }
            hostile_job_checksum = _canonical_b3(hostile_job)
            hostile_terminal_checksum = _canonical_b3(hostile_terminal)
            expected_causal = {
                "job_id": trusted_job["job_id"],
                "authorization_state": "accepted",
                "effective_trust_profile": "trusted_local",
                "trust_policy_id": baseline_inputs["trust_policy"]["policy_id"],
                "authorization_id": authorization_id,
                "launcher_receipt_id": receipt_id,
                "launcher_proof_id": proof_id,
                "upstream_hostile_suite_proof_id": hostile_suite_proof_id,
                "capability_state": "trusted_local_visible",
                "provenance_id": provenance_id,
                "public_visibility": "TRUSTED_LOCAL",
                "hostile_action_closure": [],
                "trusted_launcher_job_inputs_checksum": _canonical_b3(
                    launcher_job_inputs
                ),
                "hostile_untrusted_job_checksum": hostile_job_checksum,
                "hostile_untrusted_terminal_checksum": hostile_terminal_checksum,
            }
            if (
                expected != expected_causal
                or _canonical_b3(
                    _security_job_by_profile(
                        [
                            _mapping(value, f"{context} mutated provider job")
                            for value in mutated_inputs["provider_jobs"]
                        ],
                        "untrusted",
                        context,
                    )
                )
                != hostile_job_checksum
            ):
                raise SuccessorEvidenceError(
                    f"{context} trusted-local downstream identity or hostile-job closure differs"
                )
        else:
            if (
                mutation["input_role"] != "launcher_constraints"
                or mutation["json_pointer"] != "/process_group_kill"
            ):
                raise SuccessorEvidenceError(
                    f"{context} containment fixture must mutate only process-group kill"
                )
            for role in (
                "provider_jobs",
                "trust_policy",
                "explicit_authorization",
                "launcher_evidence_contract",
                "hostile_actions",
                "resource_limits",
            ):
                if mutated_inputs[role] != baseline_inputs[role]:
                    raise SuccessorEvidenceError(
                        f"{context} containment fixture changed fixed job evidence"
                    )
            expected_constraints = dict(baseline_inputs["launcher_constraints"])
            expected_constraints["process_group_kill"] = False
            if mutated_inputs["launcher_constraints"] != expected_constraints:
                raise SuccessorEvidenceError(
                    f"{context} containment negative changed more than process-group kill"
                )
            policy_id = str(baseline_inputs["trust_policy"]["policy_id"])
            authorization_id = f"authorization:{policy_id}:default-untrusted"
            failed_provenance_payload = {
                "trust_policy_id": policy_id,
                "job_id": hostile_job["job_id"],
                "authorization_id": authorization_id,
                "launcher_receipt_id": None,
                "launcher_proof_id": None,
                "terminal_state": "sandbox_unavailable",
                "capability_state": "not_advertised",
                "public_visibility": "PUBLIC_DENIAL_ONLY",
            }
            actions = [
                _mapping(value, f"{context} hostile action")
                for value in baseline_inputs["hostile_actions"]
            ]
            expected_negative = {
                "job_id": hostile_job["job_id"],
                "trust_policy_id": policy_id,
                "authorization_id": authorization_id,
                "error": "SANDBOX_UNAVAILABLE",
                "launcher_receipt_id": None,
                "launcher_proof_id": None,
                "provenance_id": (
                    f"provenance:{_canonical_b3(failed_provenance_payload)}"
                ),
                "capability_state": "not_advertised",
                "public_visibility": "PUBLIC_DENIAL_ONLY",
                "hostile_action_closure": _security_action_closure(
                    actions,
                    launcher_receipt_id=None,
                    attempted=False,
                    contained="unknown",
                ),
                "public_secret_bytes": 0,
                "surviving_children": "unknown",
                "hostile_untrusted_job_checksum": _canonical_b3(hostile_job),
            }
            if expected != expected_negative:
                raise SuccessorEvidenceError(
                    f"{context} containment negative terminal closure differs"
                )
    elif family == "released_wire_projection":
        if kind == "causal":
            terminal = mutated_inputs["internal_terminal"]
            daemon_responses = _daemon_canonical_responses(mutated_inputs, context)
            selected_result = daemon_responses.get(
                terminal.get("canonical_response_checksum")
            )
            if selected_result is None:
                raise SuccessorEvidenceError(
                    f"{context} TerminalEvent checksum differs from every daemon canonical response"
                )
            _, daemon_response = selected_result
            statuses = [
                _mapping(value, f"{context} query status")
                for value in terminal.get("query_statuses", [])
            ]
            successful, failed, not_executed = _projected_query_counts(statuses)
            query_result = expected.get("query_results", [{}])[0]
            coverage = _mapping(
                query_result.get("coverage"), f"{context} cancelled query coverage"
            )
            if (
                terminal.get("execution_state") != "QUERY_EXECUTION_STATE_CANCELLED"
                or expected != daemon_response
                or set(expected) != PUBLIC_RESPONSE_FIELDS
                or expected.get("execution_state")
                != terminal.get("semantic_execution_state")
                or expected.get("availability_state")
                != terminal.get("availability_state")
                or expected.get("completeness_state")
                != terminal.get("completeness_state")
                or expected.get("freshness_state") != terminal.get("freshness_state")
                or expected.get("limit_state") != terminal.get("limit_state")
                or expected.get("snapshot", {}).get("snapshot_id")
                != terminal.get("header", {}).get("snapshot_id")
                or expected.get("snapshot", {}).get("workspace_id")
                != mutated_inputs["access_scope"].get("workspace")
                or query_result.get("query_id")
                != mutated_inputs["request_context"].get("query_id")
                or query_result.get("request")
                != mutated_inputs["request_context"].get("request")
                or query_result.get("execution_state")
                != terminal.get("query_statuses", [{}])[0].get("execution_state")
                or expected.get("successful_query_count") != successful
                or expected.get("failed_query_count") != failed
                or expected.get("not_executed_dependency_count") != not_executed
                or str(coverage.get("state")).upper() != "PARTIAL"
                or coverage.get("reason") != "CANCELLED"
                or expected.get("semantic_request_id")
                != mutated_inputs["request_context"].get("semantic_request_id")
                or terminal.get("canonical_response_checksum")
                != _canonical_b3(daemon_response)
            ):
                raise SuccessorEvidenceError(f"{context} wire causal terminal differs")
        else:
            mutation = _mapping(fixture["mutation"], f"{context} wire mutation")
            candidate = _mapping(
                mutated_inputs["candidate_released_projection"],
                f"{context} mutated released projection",
            )
            baseline_candidate = _mapping(
                expectation["complete_input_universe"]["inputs"][
                    "candidate_released_projection"
                ],
                f"{context} baseline released projection",
            )
            public_projection = {
                key: value
                for key, value in candidate.items()
                if key != "internal_table"
            }
            if (
                mutation.get("input_role") != "candidate_released_projection"
                or mutation.get("json_pointer") != ""
                or set(candidate) - set(baseline_candidate) != {"internal_table"}
                or public_projection != baseline_candidate
                or candidate.get("internal_table")
                != mutated_inputs["private_diagnostics"].get("internal_table")
                or mutated_inputs["redaction_policy"].get("physical_names") != "deny"
                or expected
                != {
                    "error": "RELEASED_PROJECTION_FORBIDDEN_FIELD",
                    "forbidden_fields": ["internal_table"],
                    "admission_state": "rejected",
                }
            ):
                raise SuccessorEvidenceError(
                    f"{context} candidate released-projection injection differs"
                )
    elif family == "clean_incremental_equivalence":
        clean, incremental = _derive_equivalence_routes(mutated_inputs, context)
        if kind == "causal":
            routes = expected.get("routes")
            provenance = _equivalence_provenance(mutated_inputs)
            if (
                not isinstance(routes, list)
                or {route.get("route") for route in routes} != {"clean", "incremental"}
                or any(
                    route.get("canonical_rows") != clean
                    or route.get("public_rows") != clean
                    or route.get("unknown_rows") != []
                    or route.get("diagnostic_rows") != []
                    or route.get("capability_state") != "advertised"
                    or route.get("provenance_closure") != provenance
                    for route in routes
                )
                or expected.get("policy_release") != provenance["policy_release"]
                or expected.get("expectation_issuance")
                != provenance["expectation_issuance"]
                or expected.get("proof_set") != provenance["proof_set"]
                or clean != incremental
            ):
                raise SuccessorEvidenceError(
                    f"{context} equivalence causal rows differ"
                )
        else:
            clean_ids = {row[0] for row in clean}
            incremental_extra = [row for row in incremental if row[0] not in clean_ids]
            expected_negative = {
                "error": "CLEAN_INCREMENTAL_DIVERGENCE",
                "clean_rows": clean,
                "incremental_extra": incremental_extra,
                "capability_state": "not_advertised",
            }
            calls_extra = [row for row in incremental_extra if row[1] == "calls"]
            call_site_extra = {
                row[0] for row in incremental_extra if row[1] == "call_site"
            }
            if (
                clean == incremental
                or expected != expected_negative
                or not calls_extra
                or any(row[5] not in call_site_extra for row in calls_extra)
            ):
                raise SuccessorEvidenceError(
                    f"{context} equivalence delete-negative rows differ"
                )


def validate_fixtures(
    root: Path = ROOT,
    expectations: Sequence[Mapping[str, Any]] | None = None,
) -> list[dict[str, Any]]:
    expected_rows = list(expectations or validate_expectations(root))
    fixtures = _load_jsonl(root / FIXTURES_PATH, "successor negative fixtures")
    claim_by_id = {str(row["claim_id"]): row for row in expected_rows}
    fixture_by_id: dict[str, Mapping[str, Any]] = {}
    kinds_by_claim: dict[str, set[str]] = {claim_id: set() for claim_id in claim_by_id}

    for index, fixture in enumerate(fixtures, 1):
        context = f"fixture row {index}"
        _strict_keys(fixture, FIXTURE_KEYS, context)
        fixture_id = _nonempty_string(fixture["fixture_id"], f"{context} fixture_id")
        if FIXTURE_ID.fullmatch(fixture_id) is None or fixture_id in fixture_by_id:
            raise SuccessorEvidenceError(
                f"{context} has invalid or duplicate fixture_id"
            )
        fixture_by_id[fixture_id] = fixture
        claim_id = _nonempty_string(fixture["claim_id"], f"{context} claim_id")
        if claim_id not in claim_by_id:
            raise SuccessorEvidenceError(
                f"{context} references unknown claim {claim_id}"
            )
        kind = _nonempty_string(fixture["kind"], f"{context} kind")
        if kind not in {"causal", "negative"} or kind in kinds_by_claim[claim_id]:
            raise SuccessorEvidenceError(
                f"{context} has duplicate or invalid fixture kind for {claim_id}"
            )
        kinds_by_claim[claim_id].add(kind)
        suffix = "-C" if kind == "causal" else "-N"
        if not fixture_id.endswith(suffix):
            raise SuccessorEvidenceError(
                f"{context} fixture suffix disagrees with kind"
            )
        if fixture["author_id"] != claim_by_id[claim_id]["author_id"]:
            raise SuccessorEvidenceError(f"{context} author differs from its claim")
        for key in (
            "source_anchor",
            "authoritative_change",
            "expected_terminal",
            "semantic_basis",
        ):
            _nonempty_string(fixture[key], f"{context} {key}")
        _validate_source_references(
            root, fixture["source_anchor"], f"{context} source_anchor"
        )
        if fixture["fault_dimension"] not in FAULT_DIMENSIONS:
            raise SuccessorEvidenceError(f"{context} has unsupported fault dimension")
        if (
            not isinstance(fixture["expected_decoded"], dict)
            or not fixture["expected_decoded"]
        ):
            raise SuccessorEvidenceError(f"{context} has no decoded expected outcome")
        if fixture["semantic"] is not True or fixture["integrity_only"] is not False:
            raise SuccessorEvidenceError(
                f"{context} must be semantic rather than digest/count/text-only"
            )
        if fixture["imports"] != []:
            imports = fixture["imports"]
            if not isinstance(imports, list):
                raise SuccessorEvidenceError(f"{context} imports must be a list")
            for value in imports:
                path = _relative_path(value, f"{context} import")
                if path.startswith(FORBIDDEN_IMPORT_ROOTS):
                    raise SuccessorEvidenceError(
                        f"{context} imports forbidden target or historical output: {path}"
                    )
            raise SuccessorEvidenceError(
                f"{context} imports external expected-value material"
            )
        if kind == "causal" and fixture["expected_terminal"] != "changed":
            raise SuccessorEvidenceError(
                f"{context} causal fixture is non-discriminating"
            )
        if kind == "negative" and fixture["expected_terminal"] in {"pass", "changed"}:
            raise SuccessorEvidenceError(
                f"{context} negative fixture does not fail closed"
            )

        mutation = _mapping(fixture["mutation"], f"{context} mutation")
        _strict_keys(
            mutation,
            {"input_role", "json_pointer", "before", "after"},
            f"{context} mutation",
        )
        input_role = _nonempty_string(
            mutation["input_role"], f"{context} mutation input_role"
        )
        inputs = _mapping(
            claim_by_id[claim_id]["complete_input_universe"]["inputs"],
            f"{context} claim inputs",
        )
        mutated_inputs = copy.deepcopy(dict(inputs))
        if input_role == "$input_universe":
            if mutation["json_pointer"] != "":
                raise SuccessorEvidenceError(
                    f"{context} atomic input-universe mutation must target its root"
                )
            before_roles = _mapping(
                mutation["before"], f"{context} atomic mutation before"
            )
            after_roles = _mapping(
                mutation["after"], f"{context} atomic mutation after"
            )
            if (
                not before_roles
                or set(before_roles) != set(after_roles)
                or not set(before_roles) <= set(inputs)
                or any(inputs[role] != value for role, value in before_roles.items())
            ):
                raise SuccessorEvidenceError(
                    f"{context} atomic mutation does not bind exact declared input roles"
                )
            if before_roles == after_roles:
                raise SuccessorEvidenceError(f"{context} mutation is a semantic no-op")
            mutated_inputs.update(copy.deepcopy(dict(after_roles)))
        else:
            if input_role not in inputs:
                raise SuccessorEvidenceError(
                    f"{context} mutation targets a non-input role: {input_role}"
                )
            observed_before = _resolve_json_pointer(
                inputs[input_role],
                mutation["json_pointer"],
                f"{context} mutation pointer",
            )
            if observed_before != mutation["before"]:
                raise SuccessorEvidenceError(f"{context} mutation before value differs")
            if mutation["before"] == mutation["after"]:
                raise SuccessorEvidenceError(f"{context} mutation is a semantic no-op")
            mutated_inputs[input_role] = _apply_json_pointer(
                inputs[input_role],
                mutation["json_pointer"],
                mutation["after"],
                f"{context} mutation pointer",
            )
        _validate_fixture_mutation_semantics(
            claim_by_id[claim_id], fixture, mutated_inputs, context
        )

    for claim_id, expectation in claim_by_id.items():
        if kinds_by_claim[claim_id] != {"causal", "negative"}:
            raise SuccessorEvidenceError(
                f"{claim_id} lacks one causal and one negative semantic fixture"
            )
        causal_id = str(expectation["causal_fixture_id"])
        negative_id = str(expectation["negative_fixture_id"])
        if (
            causal_id not in fixture_by_id
            or fixture_by_id[causal_id]["claim_id"] != claim_id
        ):
            raise SuccessorEvidenceError(
                f"{claim_id} causal fixture reference is not closed"
            )
        if (
            negative_id not in fixture_by_id
            or fixture_by_id[negative_id]["claim_id"] != claim_id
        ):
            raise SuccessorEvidenceError(
                f"{claim_id} negative fixture reference is not closed"
            )
    return fixtures


def _artifact_entry(
    issuance: Mapping[str, Any], name: str, expected_path: Path
) -> Mapping[str, Any]:
    artifacts = _mapping(issuance["artifacts"], "issuance artifacts")
    entry = _mapping(artifacts.get(name), f"issuance artifact {name}")
    _strict_keys(entry, {"path", "sha256", "rows"}, f"issuance artifact {name}")
    if _relative_path(entry["path"], f"issuance artifact {name} path") != str(
        expected_path
    ):
        raise SuccessorEvidenceError(f"issuance artifact {name} path differs")
    digest = _nonempty_string(entry["sha256"], f"issuance artifact {name} sha256")
    if SHA256.fullmatch(digest) is None:
        raise SuccessorEvidenceError(f"issuance artifact {name} digest is invalid")
    if not isinstance(entry["rows"], int) or entry["rows"] <= 0:
        raise SuccessorEvidenceError(f"issuance artifact {name} selected zero rows")
    return entry


CLAIM_REVIEW_DIMENSIONS = [
    "decoded_expectation_semantics",
    "causal_fixture_discrimination",
    "negative_fixture_fail_closed_behavior",
    "governing_clause_and_source_authority_conformance",
]


def _expected_claim_review_basis(
    expectation: Mapping[str, Any],
    fixtures: Mapping[str, Mapping[str, Any]],
    context: str,
) -> dict[str, Any]:
    """Derive the immutable review envelope independently from issued rows."""

    claim_id = _nonempty_string(expectation["claim_id"], f"{context} claim id")
    subject = _nonempty_string(expectation["subject"], f"{context} subject")
    source_anchor = _nonempty_string(
        expectation["source_anchor"], f"{context} source authority"
    )
    governing_clauses = _string_list(
        expectation["governing_clauses"], f"{context} governing clauses"
    )
    if not governing_clauses:
        raise SuccessorEvidenceError(f"{context} has no governing clauses")
    if set(fixtures) != {"causal", "negative"}:
        raise SuccessorEvidenceError(f"{context} semantic fixture closure differs")
    payload = {
        "claim_id": claim_id,
        "expectation_sha256": _canonical_sha256(expectation),
        "semantic_fixture_sha256": {
            kind: _canonical_sha256(fixtures[kind]) for kind in ("causal", "negative")
        },
        "subject": subject,
        "source_authority": {
            "source_anchor": source_anchor,
            "governing_clauses": governing_clauses,
        },
        "review_dimensions": CLAIM_REVIEW_DIMENSIONS,
    }
    return {
        **payload,
        "review_binding_id": f"sha256:{_canonical_sha256(payload)}",
    }


def _validate_claim_review_specificity(
    review: Mapping[str, Any],
    expectation: Mapping[str, Any],
    fixtures: Mapping[str, Mapping[str, Any]],
    context: str,
) -> str:
    """Reject review prose or basis that could be copied to another claim."""

    expected_basis = _expected_claim_review_basis(expectation, fixtures, context)
    supplied_basis = _mapping(review["review_basis"], f"{context} review basis")
    if supplied_basis != expected_basis:
        raise SuccessorEvidenceError(
            f"{context} review basis is not bound to its claim, fixtures, and authority"
        )
    rationale = _nonempty_string(review["rationale"], f"{context} review rationale")
    authority = expected_basis["source_authority"]
    required_text = [
        expected_basis["claim_id"],
        expected_basis["review_binding_id"],
        expected_basis["subject"],
        authority["source_anchor"],
        *authority["governing_clauses"],
    ]
    if any(value not in rationale for value in required_text):
        raise SuccessorEvidenceError(
            f"{context} review rationale is generic or copied from another claim"
        )
    lowered = rationale.lower()
    if "accepted" not in lowered or "pending" in lowered or "no acceptance" in lowered:
        raise SuccessorEvidenceError(
            f"{context} accepted review rationale retains a pending disposition"
        )
    return rationale


def _record_unique_review_rationale(rationale: str, seen: set[str]) -> None:
    """Reject one rationale reused as a generic acceptance across claims."""

    if rationale in seen:
        raise SuccessorEvidenceError(
            "claim review rationales are generic repeated text"
        )
    seen.add(rationale)


def validate_issuance(
    root: Path = ROOT,
    expectations: Sequence[Mapping[str, Any]] | None = None,
    fixtures: Sequence[Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    expected_rows = list(expectations or validate_expectations(root))
    fixture_rows = list(fixtures or validate_fixtures(root, expected_rows))
    issuance = _load_json(root / ISSUANCE_PATH, "successor evidence issuance")
    _strict_keys(
        issuance,
        {
            "schema_version",
            "issuance_id",
            "reviewed_content_id",
            "status",
            "suite",
            "author",
            "reviewer",
            "independence",
            "artifacts",
            "claim_reviews",
            "consumer_order",
            "invalidation_policy",
        },
        "evidence issuance",
    )
    issuance_id = _nonempty_string(issuance["issuance_id"], "issuance_id")

    def embedded_issuance_ids(value: object) -> set[str]:
        if isinstance(value, dict):
            return set().union(
                *(embedded_issuance_ids(item) for item in value.values())
            )
        if isinstance(value, list):
            return set().union(*(embedded_issuance_ids(item) for item in value))
        if isinstance(value, str) and re.fullmatch(r"wp33:fixture-r\d+", value):
            return {value}
        return set()

    embedded = embedded_issuance_ids([*expected_rows, *fixture_rows])
    if embedded != {issuance_id}:
        raise SuccessorEvidenceError(
            "embedded expectation/fixture provenance does not bind the issuance id"
        )
    if issuance["schema_version"] != 1 or issuance["status"] != "accepted":
        raise SuccessorEvidenceError(
            "evidence issuance is not accepted schema version 1"
        )
    if issuance["suite"] != PIN_CONTRACT["suite"]:
        raise SuccessorEvidenceError("issuance suite identity differs")
    invalidation_policy = _mapping(
        issuance["invalidation_policy"], "issuance invalidation policy"
    )
    _strict_keys(
        invalidation_policy,
        {
            "policy_version",
            "expectation_change",
            "fixture_change",
            "consumer_effect",
            "review_effect",
        },
        "issuance invalidation policy",
    )
    if invalidation_policy != {
        "policy_version": 1,
        "expectation_change": "invalidate_issuance",
        "fixture_change": "invalidate_issuance",
        "consumer_effect": "reopen_all_affected_consumers",
        "review_effect": "require_new_independent_review_and_issuance",
    }:
        raise SuccessorEvidenceError("issuance invalidation policy differs")

    author = _mapping(issuance["author"], "issuance author")
    reviewer = _mapping(issuance["reviewer"], "issuance reviewer")
    _strict_keys(
        author,
        {"identity", "role", "implementation_owner"},
        "issuance author",
    )
    _strict_keys(
        reviewer,
        {"identity", "role", "implementation_owner", "expectation_author"},
        "issuance reviewer",
    )
    author_id = _nonempty_string(author["identity"], "issuance author identity")
    reviewer_id = _nonempty_string(reviewer["identity"], "issuance reviewer identity")
    _nonempty_string(author["role"], "issuance author role")
    _nonempty_string(reviewer["role"], "issuance reviewer role")
    if author["implementation_owner"] is not False:
        raise SuccessorEvidenceError("expectation author is an implementation owner")
    if (
        reviewer["implementation_owner"] is not False
        or reviewer["expectation_author"] is not False
        or reviewer_id == author_id
    ):
        raise SuccessorEvidenceError("author/reviewer independence is not established")
    if {str(row["author_id"]) for row in expected_rows} != {author_id}:
        raise SuccessorEvidenceError(
            "claim author identities differ from issuance author"
        )

    independence = _mapping(issuance["independence"], "issuance independence")
    _strict_keys(
        independence,
        {
            "target_execution_used",
            "legacy_execution_used",
            "production_expected_value_code_used",
            "required_inputs",
        },
        "issuance independence",
    )
    for key in (
        "target_execution_used",
        "legacy_execution_used",
        "production_expected_value_code_used",
    ):
        if independence[key] is not False:
            raise SuccessorEvidenceError(f"issuance independence violated: {key}")
    required_inputs = _string_list(
        independence["required_inputs"], "issuance required inputs"
    )
    if set(required_inputs) != {
        "authoritative_v2.1_sources",
        "decoded_expectations",
        "semantic_fixtures",
    }:
        raise SuccessorEvidenceError(
            "issuance has a forbidden or incomplete evidence edge"
        )
    lowered_edges = " ".join(required_inputs).lower()
    if any(edge in lowered_edges for edge in FORBIDDEN_ISSUANCE_EDGES):
        raise SuccessorEvidenceError(
            "issuance retains a forbidden legacy evidence edge"
        )

    expectation_entry = _artifact_entry(issuance, "expectations", EXPECTATIONS_PATH)
    fixture_entry = _artifact_entry(issuance, "negative_fixtures", FIXTURES_PATH)
    if expectation_entry["sha256"] != _sha256(root / EXPECTATIONS_PATH):
        raise SuccessorEvidenceError(
            "frozen expectation identity changed; dependent consumers must reopen"
        )
    if fixture_entry["sha256"] != _sha256(root / FIXTURES_PATH):
        raise SuccessorEvidenceError(
            "frozen fixture identity changed; dependent consumers must reopen"
        )
    if expectation_entry["rows"] != len(expected_rows):
        raise SuccessorEvidenceError(
            "expectation row count differs from frozen issuance"
        )
    if fixture_entry["rows"] != len(fixture_rows):
        raise SuccessorEvidenceError("fixture row count differs from frozen issuance")
    content_projection = {
        "expectations_sha256": expectation_entry["sha256"],
        "negative_fixtures_sha256": fixture_entry["sha256"],
    }
    expected_content_id = f"sha256:{_canonical_sha256(content_projection)}"
    if issuance["reviewed_content_id"] != expected_content_id:
        raise SuccessorEvidenceError("reviewed content identity differs")

    expectation_by_id = {str(row["claim_id"]): row for row in expected_rows}
    fixtures_by_claim: dict[str, dict[str, Mapping[str, Any]]] = {}
    for fixture in fixture_rows:
        fixtures_by_claim.setdefault(str(fixture["claim_id"]), {})[
            str(fixture["kind"])
        ] = fixture
    reviews = issuance["claim_reviews"]
    if not isinstance(reviews, list) or len(reviews) != len(expected_rows):
        raise SuccessorEvidenceError("every claim requires exactly one review record")
    reviewed_claims: set[str] = set()
    review_rationales: set[str] = set()
    for index, review_value in enumerate(reviews, 1):
        review = _mapping(review_value, f"claim review {index}")
        _strict_keys(
            review,
            {
                "claim_id",
                "expectation_sha256",
                "fixture_sha256",
                "reviewer_id",
                "disposition",
                "review_basis",
                "rationale",
            },
            f"claim review {index}",
        )
        claim_id = _nonempty_string(review["claim_id"], f"claim review {index} id")
        if claim_id not in expectation_by_id or claim_id in reviewed_claims:
            raise SuccessorEvidenceError(
                "claim review references unknown or duplicate claim"
            )
        reviewed_claims.add(claim_id)
        if review["reviewer_id"] != reviewer_id or review["disposition"] != "accepted":
            raise SuccessorEvidenceError(f"{claim_id} lacks independent acceptance")
        if review["expectation_sha256"] != _canonical_sha256(
            expectation_by_id[claim_id]
        ):
            raise SuccessorEvidenceError(
                f"{claim_id} decoded expected value changed after review"
            )
        fixture_digests = _mapping(
            review["fixture_sha256"], f"{claim_id} fixture digests"
        )
        _strict_keys(
            fixture_digests,
            {"causal", "negative"},
            f"{claim_id} fixture digests",
        )
        for kind in ("causal", "negative"):
            if fixture_digests[kind] != _canonical_sha256(
                fixtures_by_claim[claim_id][kind]
            ):
                raise SuccessorEvidenceError(
                    f"{claim_id} {kind} fixture changed after review"
                )
        rationale = _validate_claim_review_specificity(
            review,
            expectation_by_id[claim_id],
            fixtures_by_claim[claim_id],
            claim_id,
        )
        _record_unique_review_rationale(rationale, review_rationales)
    if reviewed_claims != set(expectation_by_id):
        raise SuccessorEvidenceError("claim review closure is incomplete")
    return issuance


def _dependency_map(plan_path: Path) -> dict[str, set[str]]:
    try:
        text = plan_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise SuccessorEvidenceError(
            f"cannot read successor plan: {plan_path}"
        ) from error
    matches = list(PACKET_HEADING.finditer(text))
    dependencies: dict[str, set[str]] = {}
    for index, match in enumerate(matches):
        packet = match.group(1)
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        block = text[match.start() : end]
        dependency = re.search(
            r"\*\*Dependencies\.\*\*\s*(.*?)(?=\n\n\*\*Target invariants\.\*\*)",
            block,
            re.DOTALL,
        )
        if dependency is None:
            raise SuccessorEvidenceError(f"{packet} has no dependency clause")
        value = dependency.group(1)
        dependencies[packet] = (
            set()
            if value.strip().startswith("None")
            else set(re.findall(r"\bWP\d{2}\b", value))
        )
    if not dependencies:
        raise SuccessorEvidenceError("successor plan selected zero work packets")
    return dependencies


def _ancestors(dependencies: Mapping[str, set[str]], packet: str) -> set[str]:
    if packet not in dependencies:
        raise SuccessorEvidenceError(f"unknown consumer packet {packet}")
    result: set[str] = set()
    pending = list(dependencies[packet])
    while pending:
        predecessor = pending.pop()
        if predecessor not in dependencies:
            raise SuccessorEvidenceError(
                f"{packet} references unknown predecessor {predecessor}"
            )
        if predecessor not in result:
            result.add(predecessor)
            pending.extend(dependencies[predecessor])
    return result


def validate_consumer_order(
    root: Path = ROOT,
    issuance: Mapping[str, Any] | None = None,
    expectations: Sequence[Mapping[str, Any]] | None = None,
) -> None:
    expected_rows = list(expectations or validate_expectations(root))
    transaction = dict(issuance or validate_issuance(root, expected_rows))
    order = _mapping(transaction["consumer_order"], "consumer order")
    _strict_keys(order, {"plan_path", "required_before"}, "consumer order")
    plan_path = _relative_path(order["plan_path"], "consumer order plan_path")
    if plan_path != str(PLAN_PATH):
        raise SuccessorEvidenceError("consumer order references the wrong plan")
    required_before = _mapping(order["required_before"], "required_before")
    expected_by_packet: dict[str, set[str]] = {}
    for row in expected_rows:
        packet = str(_mapping(row["future_consumer"], "future consumer")["packet"])
        expected_by_packet.setdefault(packet, set()).add(str(row["claim_id"]))
    if set(required_before) != set(expected_by_packet):
        raise SuccessorEvidenceError("consumer-order packet coverage differs")
    for packet, claims_value in required_before.items():
        claims = set(_string_list(claims_value, f"{packet} required claims"))
        if claims != expected_by_packet[packet]:
            raise SuccessorEvidenceError(f"{packet} claim dependency coverage differs")

    dependencies = _dependency_map(root / PLAN_PATH)
    for packet in sorted(expected_by_packet):
        if "WP33" not in _ancestors(dependencies, packet):
            raise SuccessorEvidenceError(
                f"{packet} can progress without transitive WP33 issuance"
            )

    plan = (root / PLAN_PATH).read_text(encoding="utf-8")
    headings = list(PACKET_HEADING.finditer(plan))
    blocks: dict[str, str] = {}
    for index, heading in enumerate(headings):
        end = headings[index + 1].start() if index + 1 < len(headings) else len(plan)
        blocks[heading.group(1)] = plan[heading.start() : end]
    for row in expected_rows:
        consumer = _mapping(row["future_consumer"], "future consumer")
        packet = str(consumer["packet"])
        oracle = str(consumer["oracle"])
        if f"`{oracle}`" not in blocks.get(packet, ""):
            raise SuccessorEvidenceError(
                f"{row['claim_id']} names an oracle absent from {packet}: {oracle}"
            )


def _select_claims(
    expectations: Sequence[Mapping[str, Any]], selected: Sequence[str]
) -> list[Mapping[str, Any]]:
    if not selected:
        return list(expectations)
    selected_set = set(selected)
    result = [row for row in expectations if row["claim_id"] in selected_set]
    unknown = selected_set - {str(row["claim_id"]) for row in expectations}
    if unknown or not result:
        raise SuccessorEvidenceError(
            f"claim selector selected zero rows or unknown claims: {sorted(unknown)}"
        )
    return result


def validate_transaction_integrity(root: Path = ROOT) -> int:
    expectations = validate_expectations(root)
    fixtures = validate_fixtures(root, expectations)
    validate_issuance(root, expectations, fixtures)
    return len(expectations)


def validate_expected_behavior_review(root: Path = ROOT) -> int:
    expectations = validate_expectations(root)
    fixtures = validate_fixtures(root, expectations)
    validate_issuance(root, expectations, fixtures)
    return len(expectations)


def validate_negative_fixture_independence(root: Path = ROOT) -> int:
    expectations = validate_expectations(root)
    fixtures = validate_fixtures(root, expectations)
    validate_issuance(root, expectations, fixtures)
    return len(fixtures)


def validate_readiness(root: Path = ROOT, selected_claims: Sequence[str] = ()) -> int:
    expectations = validate_expectations(root)
    selected = _select_claims(expectations, selected_claims)
    fixtures = validate_fixtures(root, expectations)
    issuance = validate_issuance(root, expectations, fixtures)
    validate_consumer_order(root, issuance, expectations)
    return len(selected)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "mode",
        choices=(
            "transaction-integrity",
            "expected-behavior-review",
            "negative-fixture-independence",
            "readiness",
        ),
    )
    parser.add_argument(
        "--claim",
        action="append",
        default=[],
        help="select an exact claim id; a zero selection fails closed",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.mode == "transaction-integrity":
            count = validate_transaction_integrity()
        elif arguments.mode == "expected-behavior-review":
            count = validate_expected_behavior_review()
        elif arguments.mode == "negative-fixture-independence":
            count = validate_negative_fixture_independence()
        else:
            count = validate_readiness(selected_claims=arguments.claim)
    except SuccessorEvidenceError as error:
        print(f"successor evidence issuance failed: {error}", file=sys.stderr)
        return 1
    print(f"successor evidence issuance valid: {count} selected rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
