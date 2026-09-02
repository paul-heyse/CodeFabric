"""Falsification tests for the frozen WP43 FastMCP 4 expectation release."""

from __future__ import annotations

import copy
import json
import shutil
from dataclasses import replace
from pathlib import Path

import pytest

from tooling.ci.fastmcp4_successor_expectations import (
    ALLOWED_DESIGN_PATHS,
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


def _copy_root(tmp_path: Path) -> Path:
    release = tmp_path / RELEASE_PATH
    release.parent.mkdir(parents=True)
    shutil.copytree(ROOT / RELEASE_PATH, release)
    for relative in ALLOWED_DESIGN_PATHS:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, destination)
    return tmp_path


def test_int_public_issuance_api_returns_all_independent_cases() -> None:
    bundle = validate_issuance(require_review=True)
    assert len(bundle.expectations) == 16
    assert len(bundle.causal) == 16
    assert len(bundle.negative) == 16


@pytest.mark.parametrize("command", sorted(SUBCOMMANDS))
def test_ops_every_cli_selector_reports_nonzero_selection(command: str) -> None:
    report = _run(command, ROOT, RELEASE_PATH)
    assert report["status"] == "passed"
    assert int(report["selected_count"]) > 0
    assert report["oracle"] == SUBCOMMANDS[command][0]
    assert report["criterion"] == SUBCOMMANDS[command][1]


def test_ops_main_emits_machine_readable_report(
    capsys: pytest.CaptureFixture[str],
) -> None:
    assert main(["independent-expectation-review"]) == 0
    report = json.loads(capsys.readouterr().out)
    assert report["oracle"] == "fastmcp4-independent-expectation-review-check"
    assert report["selected_count"] == 16


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
    assert validate_independent_review(_bundle()) == 16


def test_beh_independent_review_hash_binding_drift_is_rejected() -> None:
    bundle = _bundle()
    review_document = copy.deepcopy(bundle.review)
    review = review_document["review"]
    assert isinstance(review, dict)
    review["reviewed_expectations_sha256"] = "0" * 64
    with pytest.raises(ExpectationReleaseError) as failure:
        validate_independent_review(replace(bundle, review=review_document))
    assert failure.value.code == "RFV5_REVIEW_HASH_BINDING_DRIFT"


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
