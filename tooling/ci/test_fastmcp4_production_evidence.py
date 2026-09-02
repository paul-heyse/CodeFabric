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
    BEHAVIOR_RUN_SPECS,
    CLAIM_RUN_IDS,
    CLEAN_RUN_SPECS,
    CRITERIA,
    EXPECTED_ENTRY_KINDS,
    EXPECTED_INPUT_PATHS,
    FAULT_RUN_IDS,
    FAULT_RUN_SPECS,
    JUSTFILE_PATH,
    MAX_STDERR_BYTES,
    NEGATIVE_FIXTURE_RUN_IDS,
    ORACLES,
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
    _fault_payload,
    _limitations_payload,
    _load_jsonl,
    _opened_payload,
    _recipe_body,
    _review_payload,
    _run,
    _validate_chain,
    _validate_recipe_specs,
    canonical_sha256,
    capture_transaction,
    finalize_review,
    validate_capture,
    validate_transaction,
)
from tooling.ci.fastmcp4_successor_expectations import load_bundle


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


def _draft(root: Path, *, reviewed: bool = True) -> list[dict[str, Any]]:
    bundle = load_bundle(root)
    entries: list[dict[str, Any]] = []
    opened = _opened_payload(root, "1" * 40, "2" * 40)
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
        _claim_payload(bundle, [_run_record(spec) for spec in BEHAVIOR_RUN_SPECS]),
        recorder="wp48-production-evidence-executor",
    )
    _append(
        entries,
        "causal_fault_map",
        _fault_payload(
            bundle,
            [_run_record(spec) for spec in FAULT_RUN_SPECS],
            [_run_record(spec) for spec in BEHAVIOR_RUN_SPECS],
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
    for path in (*EXPECTED_INPUT_PATHS, RUNNER_PATH, RUNNER_TEST_PATH):
        destination = root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / path, destination)
    destination = root / JUSTFILE_PATH
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / JUSTFILE_PATH, destination)
    entries = _draft(root)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    return root


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


def test_int_recipe_body_parser_stops_before_the_next_recipe() -> None:
    justfile = "target-check:\n    true\n\nnext-check:\n    forbidden-history-edge\n"
    assert _recipe_body(justfile, "target-check") == "    true\n\n"


def test_beh_claims_bind_independent_values_to_executed_runs(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    claim = entries[1]["payload"]["claims"][4]
    claim["expected_observation_sha256"] = "0" * 64
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_CLAIM_CLOSURE"


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


def test_neg_surviving_fault_is_rejected(tmp_path: Path) -> None:
    root = _candidate(tmp_path)
    entries = _draft(root)
    entries[2]["payload"]["faults"][0]["distinguished"] = False
    _rechain(entries)
    _write_jsonl(root / TRANSACTION_PATH, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_transaction(root, check_git=False)
    assert failure.value.code == "RFV5_EVIDENCE_FAULT_SURVIVED"


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
            check_git=False,
        )
        == 16
    )
    entries = _load_jsonl(root / TRANSACTION_PATH)
    _validate_chain(entries, reviewed=False)
    expected_count = (
        len(BEHAVIOR_RUN_SPECS) + len(FAULT_RUN_SPECS) + len(CLEAN_RUN_SPECS)
    )
    assert len(calls) == expected_count
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
