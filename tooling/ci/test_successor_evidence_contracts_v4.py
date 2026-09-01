"""Falsification tests for the v4 typed expectation/fixture contracts."""

from __future__ import annotations

import ast
import copy
import json
from pathlib import Path

import pytest

from tooling.ci.successor_evidence_contracts_v4 import (
    _CAUSAL_PATHS,
    _DERIVERS,
    _ENUMS,
    _INPUT_KEYS,
    _NESTED_KEYS,
    EVIDENCE_ROOT,
    EXPECTATIONS_PATH,
    EXPECTED_FAMILIES,
    FIXTURES_PATH,
    FROZEN_SHA256,
    ISSUANCE_PATH,
    ROOT,
    V4ContractError,
    _verify_frozen_release,
    apply_json_merge_patch,
    load_jsonl,
    validate_evidence_contracts,
    validate_expectation_row,
    validate_fixture_row,
)


def _expectations() -> list[dict[str, object]]:
    return load_jsonl(ROOT / EXPECTATIONS_PATH)


def _fixtures() -> list[dict[str, object]]:
    return load_jsonl(ROOT / FIXTURES_PATH)


def _fixture_by_id(fixture_id: str) -> dict[str, object]:
    return next(row for row in _fixtures() if row["fixture_id"] == fixture_id)


def _expectation_by_id(claim_id: str) -> dict[str, object]:
    return next(row for row in _expectations() if row["claim_id"] == claim_id)


def test_int_repository_contract_covers_all_41_families_and_82_fixtures() -> None:
    report = validate_evidence_contracts()
    assert report.families == 41
    assert report.expectations == 41
    assert report.causal_fixtures == 41
    assert report.negative_fixtures == 41


def test_int_every_family_has_a_distinct_typed_deriver_and_input_contract() -> None:
    assert tuple(_DERIVERS) == EXPECTED_FAMILIES
    assert set(_INPUT_KEYS) == set(EXPECTED_FAMILIES)
    assert len({id(derive) for derive in _DERIVERS.values()}) == len(EXPECTED_FAMILIES)


@pytest.mark.parametrize("row", _expectations(), ids=lambda row: str(row["family"]))
def test_neg_every_family_rejects_an_invented_control_knob(
    row: dict[str, object],
) -> None:
    candidate = copy.deepcopy(row)
    controlled = candidate["controlled_input"]
    assert isinstance(controlled, dict)
    controlled["invented_control_knob"] = "not-authority"
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code == "V4_INVENTED_CONTROL_KNOB"


@pytest.mark.parametrize("row", _expectations(), ids=lambda row: str(row["family"]))
def test_neg_every_family_rejects_an_undeduced_decoded_outcome(
    row: dict[str, object],
) -> None:
    candidate = copy.deepcopy(row)
    decoded = candidate["expected_decoded"]
    assert isinstance(decoded, dict)
    decoded["outcome"] = "invented-success"
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code == "V4_TYPED_EXPECTATION_MISMATCH"


@pytest.mark.parametrize(
    ("claim_id", "path"),
    [
        ("RFV4-CLAIM-001", ("normalized_rows", 0, "fact_id")),
        ("RFV4-CLAIM-005", ("rows", 0, "fact_id")),
        ("RFV4-CLAIM-006", ("entity_ids", 0)),
        ("RFV4-CLAIM-011", ("workspace",)),
        ("RFV4-CLAIM-014", ("selected_epoch",)),
        ("RFV4-CLAIM-017", ("selected_head",)),
        ("RFV4-CLAIM-023", ("retained_query",)),
        ("RFV4-CLAIM-025", ("current_epoch",)),
        ("RFV4-CLAIM-026", ("id",)),
        ("RFV4-CLAIM-030", ("query_id",)),
        ("RFV4-CLAIM-032", ("resource_id",)),
    ],
)
def test_neg_output_only_identity_is_rejected(
    claim_id: str, path: tuple[str | int, ...]
) -> None:
    candidate = copy.deepcopy(_expectation_by_id(claim_id))
    cursor: object = candidate["expected_decoded"]
    for step in path[:-1]:
        if isinstance(step, int):
            assert isinstance(cursor, list)
            cursor = cursor[step]
        else:
            assert isinstance(cursor, dict)
            cursor = cursor[step]
    final = path[-1]
    if isinstance(final, int):
        assert isinstance(cursor, list)
        cursor[final] = "invented:identity:ffffffffffffffffffffffffffffffff"
    else:
        assert isinstance(cursor, dict)
        cursor[final] = "invented:identity:ffffffffffffffffffffffffffffffff"
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code == "V4_OUTPUT_ONLY_IDENTITY"


@pytest.mark.parametrize(
    "fixture",
    [row for row in _fixtures() if row["fixture_kind"] == "causal"],
    ids=lambda row: str(row["fixture_id"]),
)
def test_beh_every_family_causal_fixture_is_recomputed(
    fixture: dict[str, object],
) -> None:
    claim_id = str(fixture["claim_id"])
    expectation = _expectation_by_id(claim_id)
    derived = validate_fixture_row(fixture, expectation)
    assert derived == fixture["expected_decoded"]


@pytest.mark.parametrize(
    "fixture",
    [row for row in _fixtures() if row["fixture_kind"] == "causal"],
    ids=lambda row: str(row["fixture_id"]),
)
def test_neg_every_causal_fixture_detects_a_stale_decoded_observation(
    fixture: dict[str, object],
) -> None:
    candidate = copy.deepcopy(fixture)
    decoded = candidate["expected_decoded"]
    assert isinstance(decoded, dict)
    decoded["outcome"] = "stale-observation"
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id(str(candidate["claim_id"])))
    assert failure.value.code == "V4_CAUSAL_OBSERVATION_DRIFT"


@pytest.mark.parametrize(
    "fixture",
    [row for row in _fixtures() if row["fixture_kind"] == "negative"],
    ids=lambda row: str(row["fixture_id"]),
)
def test_neg_every_negative_fixture_is_typed_and_fail_closed(
    fixture: dict[str, object],
) -> None:
    decoded = validate_fixture_row(
        fixture, _expectation_by_id(str(fixture["claim_id"]))
    )
    assert decoded == fixture["expected_decoded"]
    assert decoded["outcome"] in {"rejected", "case_matrix", "accounted_partial"}


@pytest.mark.parametrize(
    "claim_number", [19, 20, 21, 22, 24, 28, 39], ids=lambda value: f"claim-{value:03d}"
)
def test_neg_supervisor_and_security_case_loss_fails_closed(claim_number: int) -> None:
    fixture_id = f"RFV4-FIX-{claim_number:03d}-N"
    candidate = copy.deepcopy(_fixture_by_id(fixture_id))
    decoded = candidate["expected_decoded"]
    assert isinstance(decoded, dict)
    cases = decoded["cases"]
    assert isinstance(cases, list)
    cases.pop()
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id(str(candidate["claim_id"])))
    assert failure.value.code == "V4_NEGATIVE_OBSERVATION_DRIFT"


def test_neg_policy_denial_cannot_register_a_grant_or_spawn_an_adapter() -> None:
    candidate = copy.deepcopy(_fixture_by_id("RFV4-FIX-019-N"))
    decoded = candidate["expected_decoded"]
    assert isinstance(decoded, dict)
    decoded["grant_registered"] = True
    decoded["adapter_spawned"] = True
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id("RFV4-CLAIM-019"))
    assert failure.value.code == "V4_NEGATIVE_OBSERVATION_DRIFT"


def test_neg_rejected_rpc_cannot_start_query_or_resubmit_accepted_work() -> None:
    validate_fixture_row(
        _fixture_by_id("RFV4-FIX-023-N"), _expectation_by_id("RFV4-CLAIM-023")
    )
    validate_fixture_row(
        _fixture_by_id("RFV4-FIX-027-N"), _expectation_by_id("RFV4-CLAIM-027")
    )
    for fixture_id, key in (
        ("RFV4-FIX-023-N", "query_resubmitted"),
        ("RFV4-FIX-027-N", "query_started"),
    ):
        candidate = copy.deepcopy(_fixture_by_id(fixture_id))
        decoded = candidate["expected_decoded"]
        assert isinstance(decoded, dict)
        decoded[key] = True
        with pytest.raises(V4ContractError) as failure:
            validate_fixture_row(
                candidate, _expectation_by_id(str(candidate["claim_id"]))
            )
        assert failure.value.code == "V4_NEGATIVE_OBSERVATION_DRIFT"


def test_unit_json_merge_patch_uses_rfc7396_delete_and_whole_array_replace() -> None:
    target = {"nested": {"kept": 1, "deleted": 2}, "records": [1, 2, 3]}
    patch = {"nested": {"deleted": None}, "records": [{"sequence": 1}]}
    assert apply_json_merge_patch(target, patch) == {
        "nested": {"kept": 1},
        "records": [{"sequence": 1}],
    }
    assert target == {"nested": {"kept": 1, "deleted": 2}, "records": [1, 2, 3]}


def test_int_contract_module_imports_only_the_python_standard_library() -> None:
    path = Path(__file__).with_name("successor_evidence_contracts_v4.py")
    tree = ast.parse(path.read_text(encoding="utf-8"))
    imports = {
        node.names[0].name.split(".", 1)[0]
        for node in ast.walk(tree)
        if isinstance(node, ast.Import)
    }
    imports.update(
        node.module.split(".", 1)[0]
        for node in ast.walk(tree)
        if isinstance(node, ast.ImportFrom) and node.module
    )
    assert imports <= {
        "__future__",
        "base64",
        "collections",
        "copy",
        "dataclasses",
        "hashlib",
        "json",
        "pathlib",
        "re",
        "typing",
    }


def _first_nested_mapping(value: object) -> dict[str, object] | None:
    if isinstance(value, dict):
        for child in value.values():
            if isinstance(child, dict):
                return child
            if isinstance(child, list):
                for item in child:
                    if isinstance(item, dict):
                        return item
            found = _first_nested_mapping(child)
            if found is not None:
                return found
    elif isinstance(value, list):
        for child in value:
            found = _first_nested_mapping(child)
            if found is not None:
                return found
    return None


def _mutate_first_scalar(value: object) -> bool:
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "case_id" or child is None:
                continue
            if isinstance(child, bool):
                value[key] = "not-a-bool"
                return True
            if isinstance(child, int):
                value[key] = "not-an-int"
                return True
            if isinstance(child, str):
                value[key] = 17
                return True
            if _mutate_first_scalar(child):
                return True
    elif isinstance(value, list):
        for index, child in enumerate(value):
            if isinstance(child, bool):
                value[index] = "not-a-bool"
                return True
            if isinstance(child, int):
                value[index] = "not-an-int"
                return True
            if isinstance(child, str):
                value[index] = 17
                return True
            if _mutate_first_scalar(child):
                return True
    return False


@pytest.mark.parametrize(
    "fixture",
    [row for row in _fixtures() if row["fixture_kind"] == "negative"],
    ids=lambda row: f"invalid-change-{row['fixture_id']}",
)
def test_neg_every_invalid_change_is_interpreted_not_merely_present(
    fixture: dict[str, object],
) -> None:
    candidate = copy.deepcopy(fixture)
    fixture_input = candidate["fixture_input"]
    assert isinstance(fixture_input, dict)
    invalid = fixture_input["invalid_change"]
    if isinstance(invalid, dict):
        invalid["invented_invalid_control"] = True
    else:
        assert isinstance(invalid, str)
        fixture_input["invalid_change"] = f"{invalid} with arbitrary suffix"
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id(str(candidate["claim_id"])))
    assert failure.value.code in {
        "V4_INVENTED_CONTROL_KNOB",
        "V4_NEGATIVE_INPUT_INVALID",
    }
    assert str(candidate["fixture_id"]) in str(failure.value)
    assert str(candidate["claim_id"]) in str(failure.value)


@pytest.mark.parametrize(
    "fixture",
    [row for row in _fixtures() if row["fixture_kind"] == "negative"],
    ids=lambda row: f"output-closure-{row['fixture_id']}",
)
def test_neg_every_negative_rejects_arbitrary_or_extra_output(
    fixture: dict[str, object],
) -> None:
    candidate = copy.deepcopy(fixture)
    decoded = candidate["expected_decoded"]
    assert isinstance(decoded, dict)
    decoded["invented_output_authority"] = True
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id(str(candidate["claim_id"])))
    assert failure.value.code == "V4_NEGATIVE_OBSERVATION_DRIFT"
    assert str(candidate["fixture_id"]) in str(failure.value)
    assert str(candidate["claim_id"]) in str(failure.value)


@pytest.mark.parametrize(
    "fixture",
    [row for row in _fixtures() if row["fixture_kind"] == "causal"],
    ids=lambda row: f"semantic-path-{row['fixture_id']}",
)
def test_neg_every_causal_patch_rejects_an_extra_semantic_path(
    fixture: dict[str, object],
) -> None:
    candidate = copy.deepcopy(fixture)
    fixture_input = candidate["fixture_input"]
    assert isinstance(fixture_input, dict)
    patch = fixture_input["merge_patch"]
    assert isinstance(patch, dict)
    patch["case_id"] = "case-id-only-is-not-semantic"
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id(str(candidate["claim_id"])))
    assert failure.value.code == "V4_CAUSAL_PATCH_PATH_INVALID"
    assert str(candidate["fixture_id"]) in str(failure.value)
    assert str(candidate["claim_id"]) in str(failure.value)


def test_neg_case_id_only_causal_patch_is_rejected_before_derivation() -> None:
    candidate = copy.deepcopy(_fixture_by_id("RFV4-FIX-001-C"))
    fixture_input = candidate["fixture_input"]
    assert isinstance(fixture_input, dict)
    fixture_input["merge_patch"] = {"case_id": "renamed-only"}
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id("RFV4-CLAIM-001"))
    assert failure.value.code == "V4_CAUSAL_NON_SEMANTIC_PATCH"
    assert "RFV4-FIX-001-C/RFV4-CLAIM-001" in str(failure.value)


@pytest.mark.parametrize(
    "row",
    [
        row
        for row in _expectations()
        if _first_nested_mapping(row["controlled_input"]) is not None
    ],
    ids=lambda row: f"nested-{row['family']}",
)
def test_neg_recursive_input_objects_reject_invented_members(
    row: dict[str, object],
) -> None:
    candidate = copy.deepcopy(row)
    controlled = candidate["controlled_input"]
    nested = _first_nested_mapping(controlled)
    assert nested is not None
    nested["invented_nested_authority"] = "forbidden"
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert str(candidate["claim_id"]) in str(failure.value)


@pytest.mark.parametrize(
    "row", _expectations(), ids=lambda row: f"type-{row['family']}"
)
def test_neg_every_family_rejects_a_scalar_type_drift(
    row: dict[str, object],
) -> None:
    candidate = copy.deepcopy(row)
    controlled = candidate["controlled_input"]
    assert _mutate_first_scalar(controlled)
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert str(candidate["claim_id"]) in str(failure.value)


@pytest.mark.parametrize(
    ("family", "path"),
    sorted({key for key in _ENUMS}),
    ids=lambda item: str(item),
)
def test_neg_every_declared_enum_is_closed(family: str, path: str) -> None:
    row = next(item for item in _expectations() if item["family"] == family)
    candidate = copy.deepcopy(row)
    cursor = candidate["controlled_input"]
    assert isinstance(cursor, dict)
    parts = path.split(".")
    for part in parts[:-1]:
        cursor = cursor[part]
        assert isinstance(cursor, dict)
    cursor[parts[-1]] = "outside-released-enum"
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code == "V4_TYPED_ENUM_INVALID"
    assert str(candidate["claim_id"]) in str(failure.value)


def test_neg_identity_anchor_fact_id_conflict_is_rejected() -> None:
    candidate = copy.deepcopy(_expectation_by_id("RFV4-CLAIM-001"))
    controlled = candidate["controlled_input"]
    assert isinstance(controlled, dict)
    anchors = controlled["identity_anchors"]
    assert isinstance(anchors, list)
    conflicting = copy.deepcopy(anchors[0])
    conflicting["identity_inputs"] = copy.deepcopy(anchors[1]["identity_inputs"])
    anchors.append(conflicting)
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code == "V4_IDENTITY_ANCHOR_CONFLICT"
    assert "RFV4-CLAIM-001" in str(failure.value)


def test_neg_unused_identity_anchor_is_rejected() -> None:
    candidate = copy.deepcopy(_expectation_by_id("RFV4-CLAIM-001"))
    controlled = candidate["controlled_input"]
    assert isinstance(controlled, dict)
    anchors = controlled["identity_anchors"]
    assert isinstance(anchors, list)
    unused = copy.deepcopy(anchors[0])
    unused["fact_id"] = "fact:normalized-syntax:dddddddddddddddddddddddddddddddd"
    identity_inputs = unused["identity_inputs"]
    assert isinstance(identity_inputs, dict)
    identity_inputs["input_fact_id"] = (
        "fact:syntax-observation:dddddddddddddddddddddddddddddddd"
    )
    anchors.append(unused)
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code == "V4_UNUSED_IDENTITY_ANCHOR"
    assert "RFV4-CLAIM-001" in str(failure.value)


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("non_public", False),
        ("query_id", "fedcba9876543210fedcba9876543210"),
        ("principal_session_class", "agent-b"),
        ("daemon_generation", 5),
        ("profile", "cpg.other"),
        ("next_sequence", 4),
        ("preceding_event_content_sha256", "3" * 64),
        ("expires_at_unix_ms", 62001),
    ],
)
def test_neg_cursor_kat_binds_every_oracle_authority_field(
    field: str, replacement: object
) -> None:
    candidate = copy.deepcopy(_expectation_by_id("RFV4-CLAIM-029"))
    controlled = candidate["controlled_input"]
    assert isinstance(controlled, dict)
    oracle = controlled["cursor_fixture_oracle"]
    assert isinstance(oracle, dict)
    oracle[field] = replacement
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code in {
        "V4_CURSOR_ORACLE_INVALID",
        "V4_CURSOR_ORACLE_BINDING_DRIFT",
    }
    assert "RFV4-CLAIM-029" in str(failure.value)


def test_neg_cursor_kat_rejects_unallocated_opaque_bytes() -> None:
    candidate = copy.deepcopy(_expectation_by_id("RFV4-CLAIM-029"))
    controlled = candidate["controlled_input"]
    assert isinstance(controlled, dict)
    controlled["resume_cursor"] = "AQIDBAUGBwgJCgsMDQ4PEg=="
    with pytest.raises(V4ContractError) as failure:
        validate_expectation_row(candidate)
    assert failure.value.code == "V4_CURSOR_ORACLE_BINDING_DRIFT"
    assert "RFV4-CLAIM-029" in str(failure.value)


def test_beh_fd3_matrix_contains_only_the_lawful_conditional_fallback() -> None:
    fixture = _fixture_by_id("RFV4-FIX-022-N")
    decoded = validate_fixture_row(fixture, _expectation_by_id("RFV4-CLAIM-022"))
    cases = decoded["cases"]
    assert isinstance(cases, list)
    fallback = next(
        case
        for case in cases
        if case["reason"] == "fixed_fd3_unavailable_safe_fallback"
    )
    assert fallback == {
        "reason": "fixed_fd3_unavailable_safe_fallback",
        "outcome": "delivered_via_fallback",
        "selected_transport": "one_shot_file",
        "fallback_used": True,
        "fd3_inherited": False,
        "inherited_extra_fds": [],
        "direct_host_stdio": True,
        "capability_read_count": 1,
        "second_read_bytes": 0,
        "opened_identity": {"device": 42, "inode": 9100},
        "unlinked_immediately_after_open": True,
        "path_visible_after_open": False,
        "capability_logged": False,
        "cleanup_complete": True,
        "adapter_spawned": True,
    }


def test_beh_read_resource_reauthorizes_every_chunk_with_exact_authority() -> None:
    expectation = _expectation_by_id("RFV4-CLAIM-031")
    positive = validate_expectation_row(expectation)
    assert positive["authorization_checks"] == 4
    assert positive["bytes_delivered"] == 512
    assert positive["final_cursor_offset"] == 768
    assert positive["authorized_operation"] == "ReadResource"
    assert positive["authorized_workspace"] == (
        "workspace:11111111111111111111111111111111"
    )
    assert positive["artifact_lease"] == {
        "lease_id": "lease:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "active": True,
        "not_before_unix_ms": 50000,
        "expires_at_unix_ms": 62000,
    }
    assert len(positive["per_read_authorization"]) == 4
    assert all(
        observation["cursor"]["content_bound"] is True
        and observation["source_disclosure"]["authorized"] is True
        and observation["descriptor"]["kind"] == "OPAQUE_RESULT_RESOURCE"
        for observation in positive["per_read_authorization"]
    )
    causal = validate_fixture_row(_fixture_by_id("RFV4-FIX-031-C"), expectation)
    assert causal["outcome"] == "stream"
    assert causal["authorization_checks"] == 2
    assert causal["bytes_delivered"] == 256
    assert causal["final_cursor_offset"] == 384
    assert causal["operation_grant_id"] == "operation-grant:read-resource-b"
    assert causal["authorized_workspace"] == (
        "workspace:22222222222222222222222222222222"
    )
    assert causal["artifact_lease"]["lease_id"] == (
        "lease:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    )
    assert causal["source_disclosure_policy"] == (
        "source-disclosure:workspace-authorized:v2"
    )
    assert causal["descriptor_policy"]["descriptor_policy_revision"] == 3


def test_beh_read_resource_complete_reauthorization_matrix_fails_before_bytes() -> None:
    decoded = validate_fixture_row(
        _fixture_by_id("RFV4-FIX-031-N"),
        _expectation_by_id("RFV4-CLAIM-031"),
    )
    assert len(decoded["cases"]) == 18
    assert decoded["cases"][0] == {
        "reason": "wrong_workspace",
        "grpc_status": "PERMISSION_DENIED",
        "typed_code": "WORKSPACE_NOT_AUTHORIZED",
        "bytes_delivered": 0,
    }
    assert all(case["bytes_delivered"] == 0 for case in decoded["cases"])
    assert [case["reason"] for case in decoded["cases"]] == [
        "wrong_workspace",
        "unauthorized_operation",
        "inactive_lease",
        "expired_lease",
        "source_disclosure_denied",
        "wrong_owner",
        "wrong_principal",
        "wrong_session",
        "wrong_daemon_generation",
        "wrong_supervisor_generation",
        "wrong_resource",
        "wrong_cursor_resource",
        "wrong_cursor_content",
        "wrong_cursor_range",
        "range_out_of_bounds",
        "unsafe_filesystem_descriptor",
        "unsafe_object_store_descriptor",
        "unsafe_path_descriptor",
    ]
    assert decoded["filesystem_location_exposed"] is False
    assert decoded["object_store_location_exposed"] is False
    assert decoded["path_descriptor_exposed"] is False


def test_neg_read_resource_causal_source_denial_cannot_remain_authorized() -> None:
    candidate = copy.deepcopy(_fixture_by_id("RFV4-FIX-031-C"))
    observations = candidate["fixture_input"]["merge_patch"][
        "authorization_observations"
    ]
    observations[1]["source_disclosure"]["authorized"] = False
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id("RFV4-CLAIM-031"))
    assert failure.value.code == "V4_TYPED_DERIVATION_FAILED"
    assert "RFV4-FIX-031-C/RFV4-CLAIM-031" in str(failure.value)


def test_beh_mcp_progress_is_an_exact_daemon_event_projection() -> None:
    expectation = _expectation_by_id("RFV4-CLAIM-035")
    positive = validate_expectation_row(expectation)
    assert positive["progress_report_count"] == 1
    assert positive["progress_observations"] == [
        {
            "current_query_id": "0123456789abcdef0123456789abcdef",
            "phase": "executing_query_blocks",
            "completed_units": 1,
            "total_units": 2,
            "safe_message": "executing query blocks",
        }
    ]
    causal = validate_fixture_row(_fixture_by_id("RFV4-FIX-035-C"), expectation)
    assert causal["progress_report_count"] == 0
    assert causal["progress_observations"] == []
    assert causal["python_progress_synthesis"] is False


def test_beh_mcp_progress_negative_matrix_closes_synthesis_and_rewriting() -> None:
    decoded = validate_fixture_row(
        _fixture_by_id("RFV4-FIX-035-N"),
        _expectation_by_id("RFV4-CLAIM-035"),
    )
    assert decoded["cases"] == [
        {
            "reason": "python_rewrites_semantics",
            "code": "PYTHON_SEMANTIC_AUTHORITY_FORBIDDEN",
        },
        {
            "reason": "python_synthesizes_progress",
            "code": "PYTHON_PROGRESS_AUTHORITY_FORBIDDEN",
            "progress_report_count": 0,
        },
    ]


def test_beh_zero_state_requires_all_thirteen_coverage_dimensions() -> None:
    expectation = _expectation_by_id("RFV4-CLAIM-041")
    positive = validate_expectation_row(expectation)
    assert positive["coverage_complete"] is True
    assert positive["coverage_totals"] == {
        "candidate_count": 19,
        "classified_count": 19,
        "retain_target_count": 12,
        "retain_history_count": 6,
        "exclude_non_authority_count": 1,
        "parse_error_count": 0,
        "skipped_count": 0,
        "unreadable_count": 0,
        "unparsed_count": 0,
        "overlapping_count": 0,
        "count_incoherent_count": 0,
        "unknown_count": 0,
        "unmatched_count": 0,
    }
    assert len(positive["candidate_dispositions"]) == 19
    assert positive["retained_history_count"] == 6
    assert all(value == 0 for value in positive["zero_outcomes"].values())
    causal = validate_fixture_row(_fixture_by_id("RFV4-FIX-041-C"), expectation)
    assert causal["outcome"] == "incomplete_census"
    assert causal["coverage_complete"] is False
    assert causal["missing_dimensions"] == ["installed_artifact"]
    assert causal["observed_dimension_count"] == 12
    assert causal["uncovered_candidate_ids"] == ["census:installed_artifact:target"]
    assert len(causal["covered_candidate_ids"]) == 18
    assert causal["zero_outcomes_issued"] is False
    assert causal["sole_target_authority"] is False


def test_beh_zero_state_negative_matrix_covers_every_incomplete_census_class() -> None:
    decoded = validate_fixture_row(
        _fixture_by_id("RFV4-FIX-041-N"),
        _expectation_by_id("RFV4-CLAIM-041"),
    )
    assert [case["code"] for case in decoded["cases"]] == [
        "ZERO_STATE_COVERAGE_INCOMPLETE",
        "ZERO_STATE_COVERAGE_INCOMPLETE",
        "ZERO_STATE_COVERAGE_INCOMPLETE",
        "ZERO_STATE_COVERAGE_INCOMPLETE",
        "ZERO_STATE_COVERAGE_OVERLAP",
        "ZERO_STATE_COVERAGE_UNCLASSIFIED",
        "ZERO_STATE_COVERAGE_UNCLASSIFIED",
        "ZERO_STATE_COVERAGE_COUNT_MISMATCH",
        "ZERO_STATE_CANDIDATE_UNCLASSIFIED",
        "ZERO_STATE_DISPOSITION_COUNT_MISMATCH",
        "PREDECESSOR_RUNTIME_REACHABLE",
        "PREDECESSOR_RUNTIME_REACHABLE",
        "PREDECESSOR_RUNTIME_REACHABLE",
        "PREDECESSOR_RUNTIME_REACHABLE",
        "PREDECESSOR_RUNTIME_REACHABLE",
        "PREDECESSOR_RUNTIME_UNCLASSIFIED",
        "HISTORICAL_ARTIFACT_CLASSIFICATION_INVALID",
    ]
    assert all(case["sole_target_authority"] is False for case in decoded["cases"])


def test_neg_lawful_fd3_fallback_cannot_leak_cleanup() -> None:
    candidate = copy.deepcopy(_fixture_by_id("RFV4-FIX-022-N"))
    fixture_input = candidate["fixture_input"]
    assert isinstance(fixture_input, dict)
    invalid = fixture_input["invalid_change"]
    assert isinstance(invalid, dict)
    cases = invalid["cases"]
    assert isinstance(cases, list)
    fallback = next(
        case
        for case in cases
        if case["reason"] == "fixed_fd3_unavailable_safe_fallback"
    )
    fallback["one_shot_fallback"]["cleanup_complete"] = False
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id("RFV4-CLAIM-022"))
    assert failure.value.code == "V4_NEGATIVE_INPUT_INVALID"
    assert "RFV4-FIX-022-N/RFV4-CLAIM-022" in str(failure.value)


@pytest.mark.parametrize(
    ("fixture_id", "path", "replacement"),
    [
        ("RFV4-FIX-025-N", ("lifecycle_authority",), "FAILED"),
        ("RFV4-FIX-030-N", ("session_owner",), "principal:arbitrary"),
        ("RFV4-FIX-033-N", ("body_authority_fields",), ["principal"]),
        ("RFV4-FIX-034-N", ("python_branch_on",), "arbitrary"),
        ("RFV4-FIX-035-N", ("cases", 0, "python_action"), "arbitrary"),
        ("RFV4-FIX-038-N", ("source",), "arbitrary"),
        ("RFV4-FIX-040-N", ("journal_policy",), "arbitrary"),
        ("RFV4-FIX-041-N", ("cases", 10, "authority_class"), "arbitrary"),
        (
            "RFV4-FIX-039-N",
            ("cases", 1, "resource", "selector", "kind"),
            "ARBITRARY",
        ),
    ],
)
def test_neg_typed_negative_inputs_reject_arbitrary_same_shape_values(
    fixture_id: str, path: tuple[str | int, ...], replacement: object
) -> None:
    candidate = copy.deepcopy(_fixture_by_id(fixture_id))
    cursor: object = candidate["fixture_input"]["invalid_change"]
    for step in path[:-1]:
        if isinstance(step, int):
            assert isinstance(cursor, list)
            cursor = cursor[step]
        else:
            assert isinstance(cursor, dict)
            cursor = cursor[step]
    final = path[-1]
    if isinstance(final, int):
        assert isinstance(cursor, list)
        cursor[final] = replacement
    else:
        assert isinstance(cursor, dict)
        cursor[final] = replacement
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id(str(candidate["claim_id"])))
    assert failure.value.code == "V4_NEGATIVE_INPUT_INVALID"
    assert f"{fixture_id}/{candidate['claim_id']}" in str(failure.value)


@pytest.mark.parametrize(
    ("claim_number", "expected_count"),
    [(19, 22), (20, 12), (21, 14), (22, 21), (23, 8), (24, 7), (28, 2)],
)
def test_beh_supervisor_and_start_query_matrices_are_closed(
    claim_number: int, expected_count: int
) -> None:
    fixture = _fixture_by_id(f"RFV4-FIX-{claim_number:03d}-N")
    decoded = validate_fixture_row(
        fixture, _expectation_by_id(f"RFV4-CLAIM-{claim_number:03d}")
    )
    assert len(decoded["cases"]) == expected_count


@pytest.mark.parametrize(
    ("fixture_id", "claim_id", "kind"),
    [
        ("RFV4-FIX-002-C", "RFV4-CLAIM-001", "causal"),
        ("RFV4-FIX-001-N", "RFV4-CLAIM-001", "causal"),
        ("RFV4-FIX-001-C", "RFV4-CLAIM-001", "negative"),
    ],
)
def test_neg_fixture_id_claim_kind_triple_is_exact(
    fixture_id: str, claim_id: str, kind: str
) -> None:
    candidate = copy.deepcopy(_fixture_by_id("RFV4-FIX-001-C"))
    candidate["fixture_id"] = fixture_id
    candidate["claim_id"] = claim_id
    candidate["fixture_kind"] = kind
    with pytest.raises(V4ContractError) as failure:
        validate_fixture_row(candidate, _expectation_by_id("RFV4-CLAIM-001"))
    assert failure.value.code in {
        "V4_TYPED_FIXTURE_TRIPLE_INVALID",
        "V4_TYPED_SCHEMA_INVALID",
    }
    assert fixture_id in str(failure.value)
    assert claim_id in str(failure.value)


def test_int_r6_freeze_pins_only_authored_expectations_and_fixtures() -> None:
    assert FROZEN_SHA256 == {
        EXPECTATIONS_PATH: "d9cd74a9cbd4a78f43117b191ef14ddc86957445fc9eb3016db63cf3f5608e7f",
        FIXTURES_PATH: "cce359c558a988ffa104ce4ca463617a79dc08774dc630eec8c8d66613d02d29",
    }
    _verify_frozen_release(ROOT)


def test_beh_independent_acceptance_may_change_only_the_issuance_projection(
    tmp_path: Path,
) -> None:
    target = tmp_path / EVIDENCE_ROOT
    target.mkdir(parents=True)
    for relative in (EXPECTATIONS_PATH, FIXTURES_PATH):
        (tmp_path / relative).write_bytes((ROOT / relative).read_bytes())
    issuance = json.loads((ROOT / ISSUANCE_PATH).read_text(encoding="utf-8"))
    issuance["status"] = "accepted"
    review = issuance["independent_review"]
    assert isinstance(review, dict)
    review["status"] = "accepted"
    review["reviewer"] = "codex-independent-reviewer"
    review["reviewed_at"] = "2026-09-01T12:00:00Z"
    (tmp_path / ISSUANCE_PATH).write_text(
        json.dumps(issuance, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    before = {
        relative: (tmp_path / relative).read_bytes()
        for relative in (EXPECTATIONS_PATH, FIXTURES_PATH)
    }
    _verify_frozen_release(tmp_path)
    assert before == {
        relative: (tmp_path / relative).read_bytes()
        for relative in (EXPECTATIONS_PATH, FIXTURES_PATH)
    }


def test_neg_frozen_authored_bytes_cannot_change(tmp_path: Path) -> None:
    target = tmp_path / EVIDENCE_ROOT
    target.mkdir(parents=True)
    for relative in (EXPECTATIONS_PATH, FIXTURES_PATH, ISSUANCE_PATH):
        (tmp_path / relative).write_bytes((ROOT / relative).read_bytes())
    with (tmp_path / EXPECTATIONS_PATH).open("ab") as stream:
        stream.write(b"\n")
    with pytest.raises(V4ContractError) as failure:
        _verify_frozen_release(tmp_path)
    assert failure.value.code == "V4_R4_FREEZE_DRIFT"


def test_int_causal_path_allocation_is_exact_for_all_claims() -> None:
    assert set(_CAUSAL_PATHS) == {f"RFV4-CLAIM-{number:03d}" for number in range(1, 42)}
    assert all(paths for paths in _CAUSAL_PATHS.values())


def test_int_nested_schema_registry_covers_every_structured_family() -> None:
    structured = {
        row["family"]
        for row in _expectations()
        if _first_nested_mapping(row["controlled_input"]) is not None
    }
    assert structured <= set(_NESTED_KEYS) | {
        "query_retrieve_facts",
    }
