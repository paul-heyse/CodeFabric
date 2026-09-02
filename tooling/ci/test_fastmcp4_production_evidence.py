"""Falsification tests for the WP48 production-evidence transaction."""

from __future__ import annotations

import copy
import hashlib
import json
import shutil
import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest

from tooling.ci.fastmcp4_production_evidence import (
    ACTIVE_RELEASE_ID,
    ACTIVE_RELEASE_PATH,
    BEHAVIOR_RUN_SPECS,
    CLAIM_RUN_IDS,
    CLEAN_RUN_SPECS,
    CRITERIA,
    EXPECTED_ENTRY_KINDS,
    FAULT_RUN_IDS,
    FAULT_RUN_SPECS,
    JUSTFILE_PATH,
    LOCAL_OBSERVATION_CLAIMS,
    LOCAL_OBSERVATION_RUN_SPEC,
    MAX_STDERR_BYTES,
    NEGATIVE_FIXTURE_RUN_IDS,
    NORMAL_LOCAL_SOURCE,
    NORMAL_REAL_SOURCE,
    ORACLES,
    REAL_OBSERVATION_RUN_SPEC,
    REAL_OBSERVER_MODULE_PATH,
    REAL_TOPOLOGY_CLAIMS,
    REVIEW_PATH,
    REVIEW_REPORT_PATH,
    REVIEW_SCHEMA,
    REVIEW_SCOPE,
    ROOT,
    RUNNER_PATH,
    RUNNER_TEST_PATH,
    SUITE,
    TRANSACTION_ID,
    TRANSACTION_PATH,
    ProductionEvidenceError,
    RunSpec,
    _append,
    _claim_payload,
    _clean_payload,
    _expected_input_paths,
    _fault_candidate,
    _fault_payload,
    _immutable_source_bindings,
    _limitations_payload,
    _load_jsonl,
    _local_actual_observation,
    _matched_comparison,
    _opened_payload,
    _recipe_body,
    _rejected_comparison,
    _review_payload,
    _run,
    _source_row,
    _validate_capture_candidate,
    _validate_chain,
    _validate_opened,
    _validate_recipe_specs,
    canonical_sha256,
    capture_transaction,
    finalize_review,
    validate_capture,
    validate_transaction,
)
from tooling.ci.fastmcp4_successor_expectations import (
    R1_RELEASE_ID,
    R1_RELEASE_PATH,
    apply_merge_patch,
    load_bundle,
)

TEST_RELEASE_PATH = R1_RELEASE_PATH
TEST_RELEASE_ID = R1_RELEASE_ID


def _run_record(spec: RunSpec) -> dict[str, object]:
    stdout = f"passed:{spec.run_id}\n".encode()
    stderr = b""
    return {
        "run_id": spec.run_id,
        "argv": list(spec.argv),
        "exit_code": 0,
        "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
        "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
        "stdout_bytes": len(stdout),
        "stderr_bytes": len(stderr),
        "selected_count": spec.selected_count,
        "selectors": list(spec.selectors),
        "execution_class": spec.execution_class,
    }


def _review(root: Path, reviewed_tip: str) -> dict[str, object]:
    report_path = root / REVIEW_REPORT_PATH
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        "# Independent WP48 evidence review\n\n"
        f"Reviewed transaction tip: `{reviewed_tip}`\n\n"
        + "\n".join(f"- {scope}" for scope in REVIEW_SCOPE)
        + "\n\nVerdict: ACCEPTED\n",
        encoding="utf-8",
    )
    return {
        "schema": REVIEW_SCHEMA,
        "transaction_id": TRANSACTION_ID,
        "reviewer_identity": "wp48-independent-evidence-reviewer",
        "reviewed_through_entry_sha256": reviewed_tip,
        "review_report_path": str(REVIEW_REPORT_PATH),
        "review_report_sha256": hashlib.sha256(report_path.read_bytes()).hexdigest(),
        "reviewer_is_implementation_owner": False,
        "reviewer_is_expectation_author": False,
        "verdict": "accepted",
        "scope": list(REVIEW_SCOPE),
        "findings": [],
    }


def _write_json(path: Path, value: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")


def _write_jsonl(path: Path, entries: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(
            json.dumps(entry, separators=(",", ":"), sort_keys=True) + "\n"
            for entry in entries
        ),
        encoding="utf-8",
    )


def _synthetic_observations(bundle: Any) -> list[dict[str, object]]:
    """Build isolated validator fixtures; production capture never uses this source."""

    expectations = {str(row["claim_id"]): row for row in bundle.expectations}
    negatives = {str(row["claim_id"]): row for row in bundle.negative}
    rows: list[dict[str, object]] = []
    for claim_id in CLAIM_RUN_IDS:
        expected = copy.deepcopy(expectations[claim_id]["expected_observation"])
        fault = apply_merge_patch(expected, negatives[claim_id]["fault_patch"])
        real = claim_id in REAL_TOPOLOGY_CLAIMS
        rows.append(
            _source_row(
                claim_id=claim_id,
                mode="normal",
                actual=expected,
                source_kind=NORMAL_REAL_SOURCE if real else NORMAL_LOCAL_SOURCE,
            )
        )
        rows.append(
            _source_row(
                claim_id=claim_id,
                mode="fault",
                actual=fault,
                source_kind=(
                    f"{NORMAL_REAL_SOURCE}-fault-mode"
                    if real
                    else f"{NORMAL_LOCAL_SOURCE}-fault-mode"
                ),
            )
        )
    return rows


def _semantic_run_records() -> dict[str, Mapping[str, object]]:
    return {
        REAL_OBSERVATION_RUN_SPEC.run_id: _run_record(REAL_OBSERVATION_RUN_SPEC),
        LOCAL_OBSERVATION_RUN_SPEC.run_id: _run_record(LOCAL_OBSERVATION_RUN_SPEC),
    }


def _test_observation_collector(
    root: Path,
    _candidate_commit: str,
    _candidate_tree: str,
    release_path: Path,
    _environment: Mapping[str, str],
    _executor: Any,
) -> tuple[list[Mapping[str, Any]], Mapping[str, Mapping[str, object]]]:
    return _synthetic_observations(
        load_bundle(root, release_path)
    ), _semantic_run_records()


def _draft(
    root: Path,
    *,
    reviewed: bool = True,
    candidate_commit: str = "1" * 40,
    candidate_tree: str = "2" * 40,
    snapshot_candidate: bool = False,
) -> list[dict[str, Any]]:
    bundle = load_bundle(root, TEST_RELEASE_PATH)
    observations = _synthetic_observations(bundle)
    entries: list[dict[str, Any]] = []
    opened = _opened_payload(
        root,
        candidate_commit,
        candidate_tree,
        snapshot_candidate=snapshot_candidate,
        release_path=TEST_RELEASE_PATH,
        release_id=TEST_RELEASE_ID,
    )
    opened["captured_at_utc"] = "2026-09-02T00:00:00+00:00"
    _append(
        entries,
        "transaction_opened",
        opened,
        recorder="wp48-production-evidence-executor",
    )
    _append(
        entries,
        "claim_observation_map",
        _claim_payload(
            bundle,
            [_run_record(spec) for spec in BEHAVIOR_RUN_SPECS],
            observations,
        ),
        recorder="wp48-production-evidence-executor",
    )
    _append(
        entries,
        "causal_fault_map",
        _fault_payload(
            bundle,
            [_run_record(spec) for spec in FAULT_RUN_SPECS],
            [_run_record(spec) for spec in BEHAVIOR_RUN_SPECS],
            observations,
        ),
        recorder="wp48-production-evidence-executor",
    )
    _append(
        entries,
        "clean_reconstruction_contract",
        _clean_payload([_run_record(spec) for spec in CLEAN_RUN_SPECS]),
        recorder="wp48-production-evidence-executor",
    )
    _append(
        entries,
        "limitations_recorded",
        _limitations_payload(),
        recorder="wp48-production-evidence-executor",
    )
    if reviewed:
        document = _review(root, str(entries[-1]["entry_sha256"]))
        review_path = root / REVIEW_PATH
        _write_json(review_path, document)
        _append(
            entries,
            "review_accepted",
            _review_payload(
                document,
                REVIEW_PATH,
                hashlib.sha256(review_path.read_bytes()).hexdigest(),
            ),
            recorder=str(document["reviewer_identity"]),
        )
    return entries


def _rechain(entries: list[dict[str, Any]]) -> None:
    previous: str | None = None
    for sequence, entry in enumerate(entries, 1):
        entry["sequence"] = sequence
        entry["previous_entry_sha256"] = previous
        unsigned = {key: value for key, value in entry.items() if key != "entry_sha256"}
        entry["entry_sha256"] = canonical_sha256(unsigned)
        previous = entry["entry_sha256"]


def _candidate(tmp_path: Path) -> Path:
    root = tmp_path / "repo"
    for path in (
        *_expected_input_paths(ROOT, TEST_RELEASE_PATH),
        RUNNER_PATH,
        RUNNER_TEST_PATH,
    ):
        destination = root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / path, destination)
    destination = root / JUSTFILE_PATH
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / JUSTFILE_PATH, destination)
    entries = _draft(root)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    return root


def _git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


@pytest.mark.skipif(
    not (ROOT / TRANSACTION_PATH).is_file(),
    reason="WP48 capture awaits the committed WP44-WP47 candidate",
)
def test_int_repository_transaction_is_complete_and_reviewed() -> None:
    assert validate_transaction(ROOT) == len(CLAIM_RUN_IDS) == 16


def test_int_current_recipe_bindings_are_substantive_and_predecessor_free() -> None:
    _validate_recipe_specs(
        ROOT, (*BEHAVIOR_RUN_SPECS, *FAULT_RUN_SPECS, *CLEAN_RUN_SPECS)
    )


def test_int_closed_claim_fault_and_entry_censuses() -> None:
    assert len(CLAIM_RUN_IDS) == 16
    assert len(FAULT_RUN_IDS) == 11
    assert EXPECTED_ENTRY_KINDS[-1] == "review_accepted"


def test_int_repository_transaction_is_closed_in_a_candidate_root(
    tmp_path: Path,
) -> None:
    root = _candidate(tmp_path)
    assert validate_transaction(root, check_git=False) == 16


def test_int_tampered_entry_without_rehash_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    entries[0]["payload"]["packet"] = "WP99"
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_CHAIN_INVALID"


def test_int_bound_input_drift_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    (root / "Cargo.lock").write_text("drift\n", encoding="utf-8")
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_INPUT_DRIFT"


def test_int_runner_or_selector_drift_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    with (root / RUNNER_PATH).open("a", encoding="utf-8") as runner:
        runner.write("\n# drift\n")
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_RUNNER_DRIFT"


def test_int_transitive_recipe_dependency_drift_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    justfile = root / JUSTFILE_PATH
    text = justfile.read_text(encoding="utf-8")
    marker = "_wp45-generated-client-uds-interop:\n"
    assert marker in text
    justfile.write_text(
        text.replace(marker, f"{marker}    # drift\n", 1), encoding="utf-8"
    )
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_RUNNER_DRIFT"


def test_int_candidate_snapshot_survives_descendant_input_and_recipe_replacement(
    tmp_path: Path,
) -> None:
    root = _candidate(tmp_path)
    (root / TRANSACTION_PATH).unlink()
    (root / REVIEW_PATH).unlink()
    (root / REVIEW_REPORT_PATH).unlink()
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "WP48 Test")
    _git(root, "config", "user.email", "wp48-test@example.invalid")
    _git(root, "add", ".")
    _git(root, "commit", "-qm", "candidate")
    candidate = _git(root, "rev-parse", "HEAD")
    candidate_tree = _git(root, "rev-parse", "HEAD^{tree}")
    entries = _draft(
        root,
        candidate_commit=candidate,
        candidate_tree=candidate_tree,
        snapshot_candidate=True,
    )
    _write_jsonl(root / TRANSACTION_PATH, entries)
    assert validate_transaction(root, check_git=True) == 16

    (root / "Cargo.lock").write_text("descendant replacement\n", encoding="utf-8")
    (root / JUSTFILE_PATH).write_text("descendant-check:\n    true\n", encoding="utf-8")
    (root / RUNNER_PATH).write_text("# descendant replacement\n", encoding="utf-8")
    _git(root, "add", ".")
    _git(root, "commit", "-qm", "descendant removes live WP48 selectors")
    assert validate_transaction(root, check_git=True) == 16

    opened = entries[0]["payload"]
    bad_tree = copy.deepcopy(opened)
    bad_tree["candidate_tree"] = "0" * 40
    with pytest.raises(ProductionEvidenceError) as failure:
        _validate_opened(bad_tree, root, check_git=True)
    assert failure.value.code == "RFV5_EVIDENCE_CANDIDATE_INVALID"

    bad_binding = copy.deepcopy(opened)
    bad_binding["input_bindings"][0]["sha256"] = "0" * 64
    with pytest.raises(ProductionEvidenceError) as failure:
        _validate_opened(bad_binding, root, check_git=True)
    assert failure.value.code == "RFV5_EVIDENCE_INPUT_DRIFT"

    bad_candidate = copy.deepcopy(opened)
    bad_candidate["candidate_commit"] = _git(root, "rev-parse", "HEAD")
    with pytest.raises(ProductionEvidenceError) as failure:
        _validate_opened(bad_candidate, root, check_git=True)
    assert failure.value.code == "RFV5_EVIDENCE_CANDIDATE_INVALID"


@pytest.mark.parametrize(
    "path",
    (RUNNER_PATH, JUSTFILE_PATH, REAL_OBSERVER_MODULE_PATH),
    ids=("runner", "justfile", "observer"),
)
def test_int_capture_rejects_uncommitted_evidence_producer_mutation(
    tmp_path: Path, path: Path
) -> None:
    root = _candidate(tmp_path)
    (root / TRANSACTION_PATH).unlink()
    (root / REVIEW_PATH).unlink()
    (root / REVIEW_REPORT_PATH).unlink()
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "WP48 Test")
    _git(root, "config", "user.email", "wp48-test@example.invalid")
    _git(root, "add", ".")
    _git(root, "commit", "-qm", "candidate")
    candidate = _git(root, "rev-parse", "HEAD")

    target = root / path
    target.write_text(
        target.read_text(encoding="utf-8")
        + "\n# uncommitted evidence-producer mutation\n",
        encoding="utf-8",
    )

    with pytest.raises(ProductionEvidenceError) as failure:
        _validate_capture_candidate(root, candidate)
    assert failure.value.code == "RFV5_EVIDENCE_CANDIDATE_DIRTY"


def test_int_capture_allows_only_unrelated_untitled_scratch_file(
    tmp_path: Path,
) -> None:
    root = _candidate(tmp_path)
    (root / TRANSACTION_PATH).unlink()
    (root / REVIEW_PATH).unlink()
    (root / REVIEW_REPORT_PATH).unlink()
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "WP48 Test")
    _git(root, "config", "user.email", "wp48-test@example.invalid")
    _git(root, "add", ".")
    _git(root, "commit", "-qm", "candidate")
    candidate = _git(root, "rev-parse", "HEAD")
    candidate_tree = _git(root, "rev-parse", "HEAD^{tree}")

    (root / "Untitled").write_text("user-owned scratch\n", encoding="utf-8")

    assert _validate_capture_candidate(root, candidate) == candidate_tree


def test_int_recipe_body_parser_stops_before_the_next_recipe() -> None:
    justfile = "target-check:\n    true\n\nnext-check:\n    forbidden-history-edge\n"
    assert _recipe_body(justfile, "target-check") == "    true\n\n"


def test_beh_fabricated_actual_value_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    claim = entries[1]["payload"]["claims"][0]
    claim["actual_observation"]["protocol_version"] = "fabricated-version"
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_OBSERVATION_MISMATCH"


def test_beh_local_executable_sources_match_the_active_release() -> None:
    bundle = load_bundle(ROOT, ACTIVE_RELEASE_PATH)
    assert bundle.spec.release_id == ACTIVE_RELEASE_ID
    expectations = {str(row["claim_id"]): row for row in bundle.expectations}
    fixtures = {str(row["claim_id"]): row for row in bundle.negative}
    for claim_id in LOCAL_OBSERVATION_CLAIMS:
        normal = _local_actual_observation(
            ROOT, claim_id, "normal", ACTIVE_RELEASE_PATH
        )
        fault = _local_actual_observation(ROOT, claim_id, "fault", ACTIVE_RELEASE_PATH)
        assert (
            _matched_comparison(expectations[claim_id], normal)["status"] == "matched"
        )
        comparison = _rejected_comparison(
            expectations[claim_id], fixtures[claim_id], normal, fault
        )
        assert comparison["status"] == "rejected"
        assert comparison["typed_error"] == fixtures[claim_id]["expected_error"]
        assert comparison["mismatch_paths"] == sorted(
            fixtures[claim_id]["expected_mismatch_paths"]
        )


def test_beh_active_release_immutable_source_classes_are_copied_and_bound() -> None:
    bindings = _immutable_source_bindings(ROOT, ACTIVE_RELEASE_PATH)
    source_paths = {path for path, _digest in bindings}
    bound_paths = _expected_input_paths(ROOT, ACTIVE_RELEASE_PATH)
    assert source_paths <= bound_paths
    assert any(path.parts[:2] == ("contracts", "rpc") for path in source_paths)
    assert any(path.parts[:2] == ("docs", "library_ref") for path in source_paths)

    candidate, temporary = _fault_candidate(ROOT, "RFV5-FM4-016", ACTIVE_RELEASE_PATH)
    try:
        assert all((candidate / path).is_file() for path in source_paths)
        verified = sum(
            hashlib.sha256((candidate / path).read_bytes()).hexdigest() == digest
            for path, digest in bindings
        )
        assert verified == len(bindings) - 1
    finally:
        temporary.cleanup()


@pytest.mark.parametrize(
    "declared_paths",
    (("/absolute/source.md",), ("../escaped.md",), ("source.md", "source.md")),
    ids=("absolute", "parent-traversal", "duplicate"),
)
def test_beh_immutable_source_path_closure_rejects_unsafe_or_duplicate_paths(
    tmp_path: Path, declared_paths: tuple[str, ...]
) -> None:
    release_path = Path("contracts/acceptance/test-release")
    release = tmp_path / release_path
    release.mkdir(parents=True)
    rows = "".join(
        f"  - path: {json.dumps(path)}\n    sha256: {'0' * 64}\n"
        for path in declared_paths
    )
    (release / "issuance.yaml").write_text(
        "immutable_source_inputs:\n" + rows,
        encoding="utf-8",
    )

    with pytest.raises(ProductionEvidenceError) as failure:
        _immutable_source_bindings(tmp_path, release_path, verify_hashes=False)
    assert failure.value.code == "RFV5_EVIDENCE_EXPECTATION_RELEASE_INVALID"


@pytest.mark.parametrize("mutation", ["omitted", "extra"])
def test_beh_actual_leaf_closure_is_exact(tmp_path: Path, mutation: str) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    actual = entries[1]["payload"]["claims"][0]["actual_observation"]
    if mutation == "omitted":
        actual.pop("protocol_version")
    else:
        actual["fabricated_leaf"] = True
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_OBSERVATION_LEAF_CLOSURE"


def test_beh_field_source_omission_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    sources = entries[1]["payload"]["claims"][0]["field_sources"]
    pointer = next(iter(sources))
    sources.pop(pointer)
    entries[2]["payload"]["negative_fixture_production_map"][0][
        "normal_field_sources"
    ].pop(pointer)
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FIELD_SOURCE_CLOSURE"


def test_beh_fabricated_source_class_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    sources = entries[1]["payload"]["claims"][0]["field_sources"]
    pointer = next(iter(sources))
    for descriptor in (
        sources[pointer],
        entries[2]["payload"]["negative_fixture_production_map"][0][
            "normal_field_sources"
        ][pointer],
    ):
        descriptor["probe_kind"] = "fabricated-observer"
        descriptor["execution_class"] = "assertion-binding"
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FIELD_SOURCE_INVALID"


def test_beh_fabricated_source_probe_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    sources = entries[1]["payload"]["claims"][0]["field_sources"]
    pointer = next(iter(sources))
    sources[pointer]["probe_id"] = "fabricated-probe"
    entries[2]["payload"]["negative_fixture_production_map"][0]["normal_field_sources"][
        pointer
    ]["probe_id"] = "fabricated-probe"
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FIELD_SOURCE_INVALID"


@pytest.mark.parametrize("mutation", ("probe", "class"))
def test_beh_real_source_must_match_the_claim_local_executed_seam(
    tmp_path: Path, mutation: str
) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    claim_id = "RFV5-FM4-002"
    claim = next(
        row for row in entries[1]["payload"]["claims"] if row["claim_id"] == claim_id
    )
    fault = next(
        row
        for row in entries[2]["payload"]["negative_fixture_production_map"]
        if row["claim_id"] == claim_id
    )
    pointer = next(iter(claim["field_sources"]))
    descriptors = (
        claim["field_sources"][pointer],
        fault["normal_field_sources"][pointer],
        fault["field_sources"][pointer],
    )
    for descriptor in descriptors:
        if mutation == "probe":
            descriptor["probe_id"] = "installed-fastmcp-reference-completion"
        else:
            descriptor["probe_kind"] = "generated-tonic-client"
            descriptor["execution_class"] = "production-tonic-authority"
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)

    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FIELD_SOURCE_INVALID"


def test_beh_fabricated_exit_or_command_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    run = entries[1]["payload"]["runs"][0]
    run["exit_code"] = 1
    run["argv"] = ["true"]
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_RUN_DRIFT"


def test_beh_output_metadata_must_be_bounded_and_digest_shaped(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    entries[1]["payload"]["runs"][0]["stdout_sha256"] = "not-a-digest"
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_RUN_OUTPUT_INVALID"


def test_beh_output_metadata_cannot_claim_uncaptured_bytes(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    entries[1]["payload"]["runs"][0]["stderr_bytes"] = MAX_STDERR_BYTES + 1
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_RUN_OUTPUT_INVALID"


def test_neg_every_required_layer_has_one_executed_fault(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    faults = entries[2]["payload"]["faults"]
    assert len(faults) == len(FAULT_RUN_IDS) == 11
    assert (
        len(entries[2]["payload"]["negative_fixture_production_map"])
        == len(NEGATIVE_FIXTURE_RUN_IDS)
        == 16
    )
    assert validate_transaction(root, check_git=False) == 16


def test_neg_non_discriminating_fault_actual_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    fault = entries[2]["payload"]["negative_fixture_production_map"][0]
    fault["actual_observation"] = copy.deepcopy(fault["normal_observation"])
    fault["field_sources"] = copy.deepcopy(fault["normal_field_sources"])
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FAULT_NOT_DISCRIMINATING"


def test_neg_incomplete_fault_diff_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    fault = entries[2]["payload"]["negative_fixture_production_map"][0]
    fault["actual_observation"]["selected_suite"]["suite_version"] = fault[
        "normal_observation"
    ]["selected_suite"]["suite_version"]
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FAULT_MISMATCH"


def test_neg_fault_comparison_typed_error_cannot_be_fabricated(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    comparison = entries[2]["payload"]["negative_fixture_production_map"][0][
        "comparison"
    ]
    comparison["typed_error"] = "FABRICATED_ERROR"
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FIXTURE_DRIFT"


def test_neg_independent_fixture_execution_cannot_be_omitted(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    entries[2]["payload"]["independent_fixture_count"] = 0
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FIXTURE_DRIFT"


def test_ops_clean_reconstruction_rejects_source_tree_or_cached_state(
    tmp_path: Path,
) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    payload = entries[3]["payload"]
    payload["installed_wheel_only"] = False
    payload["cached_epoch_present"] = True
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_CLEAN_RECONSTRUCTION_INVALID"


def test_ops_capture_executes_every_closed_run_and_uses_a_fresh_target(
    tmp_path: Path,
) -> None:
    root = _candidate(tmp_path)
    (root / TRANSACTION_PATH).unlink()
    (root / REVIEW_PATH).unlink()
    calls: list[tuple[str, str | None]] = []

    def executor(
        spec: RunSpec, _root: Path, environment: Mapping[str, str]
    ) -> subprocess.CompletedProcess[bytes]:
        target = environment.get("CARGO_TARGET_DIR")
        calls.append((spec.run_id, target))
        if spec in CLEAN_RUN_SPECS:
            assert target is not None
            assert environment["SCCACHE_RECACHE"] == "1"
            Path(target).mkdir(parents=True, exist_ok=True)
        return subprocess.CompletedProcess(spec.argv, 0, b"passed\n", b"")

    assert (
        capture_transaction(
            root,
            "1" * 40,
            TRANSACTION_PATH,
            executor=executor,
            observation_collector=_test_observation_collector,
            release_path=TEST_RELEASE_PATH,
            check_git=False,
        )
        == 16
    )
    entries = _load_jsonl(root / TRANSACTION_PATH)
    _validate_chain(entries, reviewed=False)
    expected_count = (
        len(BEHAVIOR_RUN_SPECS) - 2 + len(FAULT_RUN_SPECS) + len(CLEAN_RUN_SPECS)
    )
    assert len(calls) == expected_count
    clean_topology = next(
        spec for spec in CLEAN_RUN_SPECS if spec.run_id == "clean-real-topology"
    )
    assert "--test-threads=1" in clean_topology.argv
    clean_targets = [target for run_id, target in calls if run_id.startswith("clean-")]
    assert clean_targets and len(set(clean_targets)) == 1
    assert not Path(str(clean_targets[0])).parent.exists()


def test_ops_failed_capture_never_writes_a_success_transaction(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    (root / TRANSACTION_PATH).unlink()
    (root / REVIEW_PATH).unlink()

    def executor(
        spec: RunSpec, _root: Path, _environment: Mapping[str, str]
    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.CompletedProcess(
            spec.argv,
            1 if spec.run_id == "provider-batches" else 0,
            b"",
            b"failed",
        )

    with pytest.raises(ProductionEvidenceError) as failure:
        capture_transaction(
            root,
            "1" * 40,
            TRANSACTION_PATH,
            executor=executor,
            observation_collector=_test_observation_collector,
            release_path=TEST_RELEASE_PATH,
            check_git=False,
        )
    assert failure.value.code == "RFV5_EVIDENCE_COMMAND_FAILED"
    assert not (root / TRANSACTION_PATH).exists()
    failed = list(
        (root / "contracts/evidence/relational-fabric-v5").glob(
            "wp48-production-evidence-failed-*.jsonl"
        )
    )
    assert len(failed) == 1
    failed_entries = _load_jsonl(failed[0])
    assert [entry["entry_kind"] for entry in failed_entries] == [
        "transaction_opened",
        "attempt_failed",
    ]
    assert failed_entries[-1]["payload"]["accepted_as_success"] is False


def test_ops_capture_rejects_unbounded_command_output(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    (root / TRANSACTION_PATH).unlink()
    (root / REVIEW_PATH).unlink()

    def executor(
        spec: RunSpec, _root: Path, _environment: Mapping[str, str]
    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.CompletedProcess(
            spec.argv, 0, b"", b"x" * (MAX_STDERR_BYTES + 1)
        )

    with pytest.raises(ProductionEvidenceError) as failure:
        capture_transaction(
            root,
            "1" * 40,
            TRANSACTION_PATH,
            executor=executor,
            observation_collector=_test_observation_collector,
            release_path=TEST_RELEASE_PATH,
            check_git=False,
        )
    assert failure.value.code == "RFV5_EVIDENCE_COMMAND_OUTPUT_UNBOUNDED"
    assert not (root / TRANSACTION_PATH).exists()


def test_ops_independent_review_is_appended_once_and_bound_by_digest(
    tmp_path: Path,
) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root, reviewed=False)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    assert validate_capture(root, check_git=False) == 16
    document = _review(root, str(entries[-1]["entry_sha256"]))
    _write_json(root / REVIEW_PATH, document)
    assert finalize_review(root, TRANSACTION_PATH, REVIEW_PATH, check_git=False) == 16
    assert validate_transaction(root, check_git=False) == 16
    with pytest.raises(ProductionEvidenceError) as failure:
        finalize_review(root, TRANSACTION_PATH, REVIEW_PATH, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_ENTRY_CLOSURE"


def test_ops_review_artifact_drift_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    document = json.loads((root / REVIEW_PATH).read_text(encoding="utf-8"))
    document["reviewer_identity"] = "changed-reviewer"
    _write_json(root / REVIEW_PATH, document)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_REVIEW_DRIFT"


def test_ops_every_cli_selector_reports_nonzero_selection(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    for command, (oracle, criterion) in {
        "integrity": (ORACLES[0], CRITERIA[0]),
        "behavior": (ORACLES[1], CRITERIA[1]),
        "causal-faults": (ORACLES[2], CRITERIA[2]),
        "clean-reconstruction": (ORACLES[3], CRITERIA[3]),
    }.items():
        report = _run(command, root, check_git=False)
        assert report["oracle"] == oracle
        assert report["criterion"] == criterion
        assert int(report["selected_count"]) > 0


def test_ops_review_must_bind_the_exact_preceding_tip(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = copy.deepcopy(_draft(root))
    entries[-1]["payload"]["reviewed_through_entry_sha256"] = "0" * 64
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_REVIEW_DRIFT"


def test_ops_chain_entry_order_is_closed(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    entries[2], entries[3] = entries[3], entries[2]
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_ENTRY_CLOSURE"


def test_ops_review_schema_is_not_a_prose_attestation(tmp_path: Path) -> None:
    document = _review(tmp_path, "0" * 64)
    assert document["schema"] == REVIEW_SCHEMA
    assert tuple(document["scope"]) == REVIEW_SCOPE
    assert document["findings"] == []
    assert SUITE == "codefabric-relational-data-fabric@2.3.0"
