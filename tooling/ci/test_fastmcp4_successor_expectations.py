"""Falsification tests for the frozen WP43 FastMCP 4 expectation release."""

from __future__ import annotations

import copy
import hashlib
import json
import shutil
from dataclasses import replace
from pathlib import Path

import pytest

from tooling.ci.fastmcp4_successor_expectations import (
    ALLOWED_DESIGN_PATHS,
    ALLOWED_SOURCE_INPUT_PATHS,
    R1_FROZEN_BYTES_SHA256,
    R1_RELEASE_ID,
    R1_RELEASE_PATH,
    R2_FROZEN_BYTES_SHA256,
    R2_RELEASE_ID,
    R2_RELEASE_PATH,
    R3_FROZEN_BYTES_SHA256,
    R3_RELEASE_ID,
    R3_RELEASE_PATH,
    RELEASE_PATH,
    ROOT,
    SUBCOMMANDS,
    ExpectationReleaseError,
    _expectation_index,
    _run,
    apply_merge_patch,
    load_bundle,
    main,
    validate_drift,
    validate_independent_review,
    validate_issuance,
    validate_negative_fixtures,
    validate_observation,
)


def _bundle():
    return load_bundle()


def _r1_bundle():
    return load_bundle(release_path=R1_RELEASE_PATH)


def _r2_bundle():
    return load_bundle(release_path=R2_RELEASE_PATH)


def _copy_root(tmp_path: Path, release_path: Path = RELEASE_PATH) -> Path:
    release = tmp_path / release_path
    release.parent.mkdir(parents=True)
    shutil.copytree(ROOT / release_path, release)
    for relative in ALLOWED_SOURCE_INPUT_PATHS:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, destination)
    return tmp_path


def test_int_public_issuance_api_returns_all_independent_cases() -> None:
    r1 = validate_issuance(release_path=R1_RELEASE_PATH, require_review=True)
    r2 = validate_issuance(release_path=R2_RELEASE_PATH, require_review=True)
    r3 = validate_issuance(release_path=R3_RELEASE_PATH, require_review=False)
    for bundle in (r1, r2, r3):
        assert len(bundle.expectations) == 16
        assert len(bundle.causal) == 16
        assert len(bundle.negative) == 16


def test_int_active_release_is_allowlisted_r3_candidate() -> None:
    bundle = load_bundle()
    assert RELEASE_PATH == R3_RELEASE_PATH
    assert bundle.spec.release_id == R3_RELEASE_ID
    assert bundle.release_dir == ROOT / R3_RELEASE_PATH
    assert bundle.spec.review_status == "pending"


def test_int_all_release_paths_are_allowlisted_and_absolute_form_is_supported() -> None:
    assert load_bundle(release_path=R1_RELEASE_PATH).spec.release_id == R1_RELEASE_ID
    assert load_bundle(release_path=R2_RELEASE_PATH).spec.release_id == R2_RELEASE_ID
    assert load_bundle(release_path=R3_RELEASE_PATH).spec.release_id == R3_RELEASE_ID
    assert (
        load_bundle(release_path=(ROOT / R3_RELEASE_PATH).resolve()).spec.release_id
        == R3_RELEASE_ID
    )


@pytest.mark.parametrize(
    "release_path",
    [Path("contracts/acceptance/relational-fabric-v5-r4"), Path("../outside")],
)
def test_neg_unallowlisted_release_path_is_rejected(release_path: Path) -> None:
    with pytest.raises(ExpectationReleaseError) as failure:
        load_bundle(release_path=release_path)
    assert failure.value.code == "RFV5_RELEASE_NOT_ALLOWLISTED"


def test_neg_absolute_release_path_outside_root_is_rejected(tmp_path: Path) -> None:
    with pytest.raises(ExpectationReleaseError) as failure:
        load_bundle(release_path=tmp_path.resolve())
    assert failure.value.code == "RFV5_RELEASE_NOT_ALLOWLISTED"


def test_ops_r1_frozen_bytes_remain_unchanged() -> None:
    for name, digest in R1_FROZEN_BYTES_SHA256.items():
        assert (
            hashlib.sha256((ROOT / R1_RELEASE_PATH / name).read_bytes()).hexdigest()
            == digest
        )


def test_ops_r2_frozen_bytes_remain_unchanged() -> None:
    for name, digest in R2_FROZEN_BYTES_SHA256.items():
        assert (
            hashlib.sha256((ROOT / R2_RELEASE_PATH / name).read_bytes()).hexdigest()
            == digest
        )


def test_ops_r2_and_r3_inherit_performance_method_byte_identically() -> None:
    r1 = (ROOT / R1_RELEASE_PATH / "performance-method.yaml").read_bytes()
    r2 = (ROOT / R2_RELEASE_PATH / "performance-method.yaml").read_bytes()
    r3 = (ROOT / R3_RELEASE_PATH / "performance-method.yaml").read_bytes()
    assert r3 == r2 == r1
    assert (
        hashlib.sha256(r3).hexdigest()
        == R3_FROZEN_BYTES_SHA256["performance-method.yaml"]
    )


@pytest.mark.parametrize("command", sorted(SUBCOMMANDS))
def test_ops_every_r1_cli_selector_reports_nonzero_selection(command: str) -> None:
    report = _run(command, ROOT, R1_RELEASE_PATH)
    assert report["status"] == "passed"
    assert int(report["selected_count"]) > 0
    assert report["oracle"] == SUBCOMMANDS[command][0]
    assert report["criterion"] == SUBCOMMANDS[command][1]


@pytest.mark.parametrize("command", sorted(SUBCOMMANDS))
def test_ops_every_r2_selector_reports_nonzero_selection(command: str) -> None:
    report = _run(command, ROOT, R2_RELEASE_PATH)
    assert report["status"] == "passed"
    assert report["release_id"] == R2_RELEASE_ID
    assert int(report["selected_count"]) > 0


@pytest.mark.parametrize(
    "command",
    sorted(set(SUBCOMMANDS) - {"independent-expectation-review"}),
)
def test_ops_every_review_independent_r3_selector_reports_nonzero_selection(
    command: str,
) -> None:
    report = _run(command, ROOT, R3_RELEASE_PATH)
    assert report["status"] == "passed"
    assert report["release_id"] == R3_RELEASE_ID
    assert int(report["selected_count"]) > 0


def test_beh_r3_independent_review_remains_pending() -> None:
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_issuance(release_path=R3_RELEASE_PATH, require_review=True)
    assert failure.value.code == "RFV5_REVIEW_PENDING"


def test_beh_r3_provenance_separates_gap_discovery_from_expected_value_authority() -> (
    None
):
    bundle = _bundle()
    provenance = bundle.expectations[0]["provenance"]
    authoring = bundle.issuance["authoring_constraints"]
    audit = bundle.issuance["correction_audit"]

    assert provenance["target_execution_revealed_evidence_gap"] is True
    assert provenance["target_execution_used"] is False
    assert provenance["target_output_used_as_expected_value_source"] is False
    assert provenance["static_authority_adjudication_completed"] is True
    assert authoring["target_execution_revealed_evidence_gap"] is True
    assert authoring["target_execution_used_to_author_expected_values"] is False
    assert authoring["target_output_copied_as_expected_values"] is False
    assert authoring["correction_completed_after_target_execution"] is True
    assert authoring["static_authority_adjudication_completed"] is True
    assert audit["target_execution_revealed_gap"] is True
    assert audit["target_output_used_as_authority"] is False
    assert audit["static_authority_adjudication_completed"] is True


def test_beh_r3_pending_handoff_cannot_reuse_r2_acceptance_or_drift_hash() -> None:
    bundle = _bundle()
    review_document = copy.deepcopy(bundle.review)
    review = review_document["review"]
    assert review["status"] == "pending"
    assert review["acceptance_authority"] is False
    assert review["reviewed_claim_ids"] == []
    assert review["dispositions"] == []
    assert review["handoff"]["r2_acceptance_may_be_reused"] is False
    review["candidate_expectations_sha256"] = "0" * 64
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(replace(bundle, review=review_document))
    assert failure.value.code == "RFV5_REVIEW_HANDOFF_INVALID"


def test_beh_r2_review_is_distinct_claim_specific_and_accepted() -> None:
    bundle = validate_issuance(release_path=R2_RELEASE_PATH, require_review=True)
    review = bundle.review["review"]
    assert review["status"] == "accepted"
    assert review["acceptance_authority"] is True
    assert review["reviewer_identity"] != review["author_identity"]
    assert review["reviewed_candidate_commit"] == (
        "25e10b66453e4d665ffa05e36ec95247691f846f"
    )
    dispositions = review["dispositions"]
    assert len(dispositions) == 16
    assert {row["claim_id"] for row in dispositions} == {
        f"RFV5-FM4-{number:03d}" for number in range(1, 17)
    }
    assert {row["disposition"] for row in dispositions} == {"accepted"}


def test_ops_main_emits_machine_readable_report(
    capsys: pytest.CaptureFixture[str],
) -> None:
    assert (
        main(
            [
                "independent-expectation-review",
                "--release-path",
                str(R1_RELEASE_PATH),
            ]
        )
        == 0
    )
    report = json.loads(capsys.readouterr().out)
    assert report["oracle"] == "fastmcp4-independent-expectation-review-check"
    assert report["selected_count"] == 16


def test_beh_main_reports_active_r3_review_pending(
    capsys: pytest.CaptureFixture[str],
) -> None:
    assert main(["independent-expectation-review"]) == 1
    captured = capsys.readouterr()
    report = json.loads(captured.err)
    assert report["code"] == "RFV5_REVIEW_PENDING"
    assert report["status"] == "failed"


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("generated", True),
        ("imports", ["codefabric_cpg_mcp.server"]),
        ("target_execution_used", True),
        ("predecessor_expected_values_used", True),
        ("expected_value_origin", "generated-observation"),
    ],
)
def test_neg_generated_self_imported_or_target_derived_expectation_is_rejected(
    field: str, value: object
) -> None:
    bundle = _bundle()
    expectations = copy.deepcopy(bundle.expectations)
    provenance = expectations[0]["provenance"]
    assert isinstance(provenance, dict)
    provenance[field] = value
    candidate = replace(bundle, expectations=expectations)
    with pytest.raises(ExpectationReleaseError) as failure:
        _expectation_index(candidate)
    assert failure.value.code == "RFV5_EXPECTATION_NOT_INDEPENDENT"


def test_neg_production_source_basis_is_rejected() -> None:
    bundle = _bundle()
    expectations = copy.deepcopy(bundle.expectations)
    basis = expectations[0]["design_basis"]
    assert isinstance(basis, list)
    basis[0]["path"] = "codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py"
    with pytest.raises(ExpectationReleaseError) as failure:
        _expectation_index(replace(bundle, expectations=expectations))
    assert failure.value.code == "RFV5_FORBIDDEN_EXPECTATION_SOURCE"


def test_beh_all_causal_fixtures_change_controlled_input_and_expected_observation() -> (
    None
):
    assert validate_independent_review(_r1_bundle()) == 16
    assert validate_independent_review(_r2_bundle()) == 16
    bundle = validate_issuance(release_path=R3_RELEASE_PATH, require_review=False)
    assert len(bundle.causal) == 16


def test_beh_independent_review_hash_binding_drift_is_rejected() -> None:
    bundle = _r2_bundle()
    review_document = copy.deepcopy(bundle.review)
    review = review_document["review"]
    assert isinstance(review, dict)
    review["reviewed_expectations_sha256"] = "0" * 64
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_independent_review(replace(bundle, review=review_document))
    assert failure.value.code == "RFV5_REVIEW_HASH_BINDING_DRIFT"


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("reviewer_identity", "codex-wp43-r2-forward-expectation-author"),
        ("reviewed_candidate_commit", "0" * 40),
        ("target_execution_used", True),
    ],
)
def test_beh_r2_non_independent_acceptance_is_rejected(
    field: str, value: object
) -> None:
    bundle = _r2_bundle()
    review_document = copy.deepcopy(bundle.review)
    review = review_document["review"]
    assert isinstance(review, dict)
    review[field] = value
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_independent_review(replace(bundle, review=review_document))
    assert failure.value.code == "RFV5_REVIEW_NOT_INDEPENDENT"


def test_beh_r2_incomplete_claim_dispositions_are_rejected() -> None:
    bundle = _r2_bundle()
    review_document = copy.deepcopy(bundle.review)
    review = review_document["review"]
    assert isinstance(review, dict)
    review["dispositions"] = review["dispositions"][:-1]
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_independent_review(replace(bundle, review=review_document))
    assert failure.value.code == "RFV5_REVIEW_NOT_INDEPENDENT"


def test_neg_all_fault_fixtures_are_discriminating_and_caught() -> None:
    assert validate_negative_fixtures(_bundle()) == 16


def test_neg_one_committed_fault_fails_exact_observation() -> None:
    bundle = _bundle()
    expectation = bundle.expectations[0]
    fixture = bundle.negative[0]
    expected = expectation["expected_observation"]
    assert isinstance(expected, dict)
    faulty = apply_merge_patch(expected, fixture["fault_patch"])
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_observation(expectation, faulty)
    assert failure.value.code == "RFV5_OBSERVATION_DRIFT"


def test_neg_a_fault_that_does_not_change_the_observation_is_rejected() -> None:
    bundle = _bundle()
    fixtures = copy.deepcopy(bundle.negative)
    fixtures[0]["fault_patch"] = {}
    fixtures[0]["expected_mismatch_paths"] = []
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_negative_fixtures(replace(bundle, negative=fixtures))
    assert failure.value.code == "RFV5_FAULT_NOT_DISCRIMINATING"


@pytest.mark.parametrize(
    ("fixture_kind", "path_field"),
    [("negative", "expected_mismatch_paths"), ("causal", "changed_output_paths")],
)
def test_neg_duplicate_declared_json_pointer_is_rejected(
    fixture_kind: str, path_field: str
) -> None:
    bundle = _bundle()
    fixtures = copy.deepcopy(
        bundle.negative if fixture_kind == "negative" else bundle.causal
    )
    paths = fixtures[0][path_field]
    assert isinstance(paths, list) and paths
    paths.append(paths[0])
    candidate = replace(
        bundle,
        negative=fixtures if fixture_kind == "negative" else bundle.negative,
        causal=fixtures if fixture_kind == "causal" else bundle.causal,
    )
    with pytest.raises(ExpectationReleaseError) as failure:
        if fixture_kind == "negative":
            validate_negative_fixtures(candidate)
        else:
            validate_independent_review(candidate)
    assert failure.value.code == "RFV5_POINTER_PATH_DUPLICATE"


def test_beh_r2_corrected_claim_and_fault_shapes_are_relational() -> None:
    bundle = _r2_bundle()
    expectations = {str(row["claim_id"]): row for row in bundle.expectations}
    causal = {str(row["claim_id"]): row for row in bundle.causal}
    negative = {str(row["claim_id"]): row for row in bundle.negative}

    catalog_fault = negative["RFV5-FM4-003"]
    assert catalog_fault["fault_patch"]["application_extensions"] == [
        "dev.codefabric/custom"
    ]
    assert (
        "/framework_extensions/dev.codefabric~1custom"
        in catalog_fault["expected_mismatch_paths"]
    )

    schema = expectations["RFV5-FM4-004"]["expected_observation"]
    assert schema["tools"]["query_code_graph"]["fields"]["request"] == {
        "type": "object"
    }
    assert (
        schema["tools"]["get_code_graph_reference"]["fields"]["version"]["default"]
        is None
    )
    assert schema["semantic_request_authority"] == "rust-daemon"
    assert schema["adapter_semantic_request_rewrites"] is False
    schema_fault = negative["RFV5-FM4-004"]
    assert schema_fault["fault_patch"]["tools"]["query_code_graph"]["fields"] == {
        "freshness": {"type": "string"},
        "daemon_port": {"type": "string"},
    }
    assert {
        "/tools/query_code_graph/fields/freshness",
        "/tools/query_code_graph/fields/daemon_port",
        "/semantic_request_authority",
        "/adapter_semantic_request_rewrites",
    } <= set(schema_fault["expected_mismatch_paths"])

    guard = expectations["RFV5-FM4-005"]
    guard_serialized = json.dumps(
        [guard, causal["RFV5-FM4-005"], negative["RFV5-FM4-005"]],
        sort_keys=True,
    )
    assert all(
        invented not in guard_serialized
        for invented in ("traversal_direction", "outgoing", "incoming")
    )
    assert guard["controlled_input"]["answer_selection"] == "first-authorized-choice"
    assert guard["expected_observation"]["challenge"] == {
        "semantic_field_count": 1,
        "semantic_field_ids_stable": True,
        "semantic_field_owner": "rust-daemon",
        "presentation_keys_safe": True,
        "input_kind": "enum",
        "authorized_choices_present": True,
        "authorized_choice_owner": "rust-daemon",
        "adapter_authored_defaults": False,
    }
    assert causal["RFV5-FM4-005"]["changed_output_paths"] == [
        "/valid_second_leg/selected_authorized_choice_ordinal"
    ]

    recovery = expectations["RFV5-FM4-009"]
    recovery_serialized = json.dumps(
        [recovery, causal["RFV5-FM4-009"], negative["RFV5-FM4-009"]],
        sort_keys=True,
    )
    assert "query:aaaaaaaa" not in recovery_serialized
    assert recovery["expected_observation"]["accepted_query_identity"] == {
        "present": True,
        "owner": "rust-daemon",
    }
    assert (
        recovery["expected_observation"]["reconnect"][
            "resumed_query_identity_matches_accepted"
        ]
        is True
    )
    assert (
        "/reconnect/resumed_query_identity_matches_accepted"
        in negative["RFV5-FM4-009"]["expected_mismatch_paths"]
    )


def test_beh_r3_resource_denials_are_split_and_typed() -> None:
    bundle = _bundle()
    expectations = {str(row["claim_id"]): row for row in bundle.expectations}
    negative = {str(row["claim_id"]): row for row in bundle.negative}
    claim = expectations["RFV5-FM4-011"]

    assert claim["controlled_input"]["oversized_input_requirement"] == {
        "semantic_request_bytes_relation": (
            "greater_than_released_semantic_request_limit"
        ),
        "grpc_encoded_message_bytes_relation": (
            "less_than_grpc_transport_decode_limit"
        ),
    }
    assert claim["controlled_input"]["resource_read_denials"] == {
        "invalid_resource_maximum_bytes": {
            "offset": 0,
            "maximum_bytes": (1 << 64) - 1,
            "bound_relation": "exceeds_effective_maximum_resource_chunk_bytes",
        },
        "resource_offset_past_end": {
            "offset_relation": "resource_byte_length_plus_one",
            "maximum_bytes": 1,
        },
    }
    denied = claim["expected_observation"]["denied_cases"]
    assert denied["oversized_input_requirement"] == "RESOURCE_EXHAUSTED"
    assert denied["invalid_resource_maximum_bytes"] == {
        "grpc_status": "INVALID_ARGUMENT",
        "safe_error_code": "SAFE_ERROR_CODE_INVALID_REQUEST",
    }
    assert denied["resource_offset_past_end"] == {
        "grpc_status": "OUT_OF_RANGE",
        "safe_error_code": "SAFE_ERROR_CODE_RANGE_NOT_SATISFIABLE",
    }
    assert "oversized_resource_range" not in denied
    assert claim["expected_observation"]["denied_before_bytes"] is True
    mismatch_paths = set(negative["RFV5-FM4-011"]["expected_mismatch_paths"])
    assert {
        "/denied_cases/invalid_resource_maximum_bytes/grpc_status",
        "/denied_cases/invalid_resource_maximum_bytes/safe_error_code",
        "/denied_cases/resource_offset_past_end/grpc_status",
        "/denied_cases/resource_offset_past_end/safe_error_code",
        "/denied_before_bytes",
    } <= mismatch_paths


def test_int_r3_immutable_source_count_matches_release_claim() -> None:
    bundle = _bundle()
    sources = bundle.issuance["immutable_source_inputs"]
    claim = next(
        row for row in bundle.expectations if row["claim_id"] == "RFV5-FM4-016"
    )
    assert len(sources) == 8
    assert claim["expected_observation"]["source_input_hashes_verified"] == len(sources)


@pytest.mark.parametrize("name", sorted(R3_FROZEN_BYTES_SHA256))
def test_neg_duplicate_yaml_mapping_key_is_rejected(tmp_path: Path, name: str) -> None:
    root = _copy_root(tmp_path)
    path = root / R3_RELEASE_PATH / name
    release_id = R1_RELEASE_ID if name == "performance-method.yaml" else R3_RELEASE_ID
    path.write_text(
        path.read_text(encoding="utf-8") + f"release_id: {release_id}\n",
        encoding="utf-8",
    )
    with pytest.raises(ExpectationReleaseError) as failure:
        load_bundle(root)
    assert failure.value.code == "RFV5_YAML_INVALID"


def test_ops_expectation_byte_drift_is_rejected(tmp_path: Path) -> None:
    root = _copy_root(tmp_path)
    path = root / RELEASE_PATH / "expectations.yaml"
    path.write_text(path.read_text(encoding="utf-8") + "\n", encoding="utf-8")
    bundle = load_bundle(root)
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(bundle)
    assert failure.value.code == "RFV5_ARTIFACT_HASH_DRIFT"


def test_ops_issuance_hash_binding_drift_is_rejected() -> None:
    bundle = _bundle()
    issuance = copy.deepcopy(bundle.issuance)
    hashes = issuance["artifact_sha256"]
    assert isinstance(hashes, dict)
    hashes["expectations.yaml"] = "0" * 64
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(replace(bundle, issuance=issuance))
    assert failure.value.code == "RFV5_ISSUANCE_HASH_BINDING_DRIFT"


def test_ops_extra_issuance_hash_binding_is_rejected() -> None:
    bundle = _bundle()
    issuance = copy.deepcopy(bundle.issuance)
    hashes = issuance["artifact_sha256"]
    assert isinstance(hashes, dict)
    hashes["unregistered.yaml"] = "0" * 64
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(replace(bundle, issuance=issuance))
    assert failure.value.code == "RFV5_ISSUANCE_HASH_BINDING_DRIFT"


def test_ops_selector_binding_drift_is_rejected() -> None:
    bundle = _bundle()
    issuance = copy.deepcopy(bundle.issuance)
    selectors = issuance["selectors"]
    assert isinstance(selectors, dict)
    selectors["expectation-drift"]["criterion"] = "PC-WP43-WRONG"
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(replace(bundle, issuance=issuance))
    assert failure.value.code == "RFV5_SELECTOR_DRIFT"


def test_ops_duplicate_source_path_cannot_mask_missing_hash_coverage() -> None:
    bundle = _bundle()
    issuance = copy.deepcopy(bundle.issuance)
    sources = issuance["immutable_source_inputs"]
    assert isinstance(sources, list) and len(sources) == 8
    sources[-1] = copy.deepcopy(sources[0])
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(replace(bundle, issuance=issuance))
    assert failure.value.code == "RFV5_SOURCE_HASH_COVERAGE"


def test_ops_frozen_design_input_drift_is_rejected(tmp_path: Path) -> None:
    root = _copy_root(tmp_path)
    source = root / next(iter(sorted(ALLOWED_DESIGN_PATHS)))
    source.write_text(
        source.read_text(encoding="utf-8") + "\ndrift\n", encoding="utf-8"
    )
    bundle = load_bundle(root)
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(bundle)
    assert failure.value.code == "RFV5_SOURCE_INPUT_DRIFT"


def test_ops_unregistered_release_file_is_rejected(tmp_path: Path) -> None:
    root = _copy_root(tmp_path)
    (root / RELEASE_PATH / "generated-observation.yaml").write_text("generated: true\n")
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_drift(load_bundle(root))
    assert failure.value.code == "RFV5_RELEASE_FILESET_DRIFT"
