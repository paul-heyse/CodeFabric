"""Focused positive and falsification tests for the ancestral WP40 release record."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

import pytest

from tooling.ci.production_evidence import TRANSACTION_PATH
from tooling.ci.relational_fabric_release import (
    JUSTFILE_PATH,
    PERFORMANCE_DISCLOSURE_ID,
    RECORD_PATH,
    REQUIRED_FROZEN_INPUTS,
    ROOT,
    ReleaseEvidenceError,
    _content_id,
    _sha256,
    refresh_release_record,
    validate_clean_incremental_recovery_performance,
    validate_matrix_v3,
    validate_record_integrity,
    validate_security_resource_rejection,
)


def _load(root: Path) -> dict[str, object]:
    value = json.loads((root / RECORD_PATH).read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def _write(root: Path, record: dict[str, object], *, refresh: bool = True) -> None:
    if refresh:
        record["content_id"] = _content_id(record)
    (root / RECORD_PATH).write_text(
        json.dumps(record, indent=2) + "\n", encoding="utf-8"
    )


def _copy_candidate(
    destination: Path,
    *,
    accept_wp38: bool = True,
    refresh: bool = True,
) -> Path:
    for relative in (RECORD_PATH, JUSTFILE_PATH, *sorted(REQUIRED_FROZEN_INPUTS)):
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, target)
    justfile_path = destination / JUSTFILE_PATH
    justfile = justfile_path.read_text(encoding="utf-8")
    for recipe in (
        "production-evidence-input-integrity-check",
        "first-principles-production-behavior-check",
        "causal-fault-discrimination-check",
        "production-evidence-recovery-operations-check",
    ):
        if f"{recipe}:" not in justfile:
            justfile += f"\n{recipe}:\n    true\n"
    justfile_path.write_text(justfile, encoding="utf-8")
    record = _load(destination)
    selected: set[str] = set()
    for field in ("matrix", "rejection_matrix", "operations_matrix"):
        entries = record[field]
        assert isinstance(entries, list)
        for entry in entries:
            assert isinstance(entry, dict)
            names = entry["selected_tests"]
            assert isinstance(names, list)
            selected.update(str(name) for name in names)
    inventory = destination / "tooling/ci/test_selected_inventory.py"
    inventory.parent.mkdir(parents=True, exist_ok=True)
    inventory.write_text(
        "\n\n".join(f"def {name}():\n    pass" for name in sorted(selected)) + "\n",
        encoding="utf-8",
    )
    if not accept_wp38:
        transaction = destination / TRANSACTION_PATH
        pending = transaction.read_text(encoding="utf-8").splitlines()[:4]
        transaction.write_text("\n".join(pending) + "\n", encoding="utf-8")
    if refresh:
        refresh_release_record(
            destination,
            source=RECORD_PATH,
            output=RECORD_PATH,
        )
    return destination


def test_int_current_record_is_immutable_and_development_only() -> None:
    assert validate_record_integrity(ROOT) == len(REQUIRED_FROZEN_INPUTS)


def test_int_content_identity_corruption_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    record["record_id"] = "mutated"
    _write(root, record, refresh=False)
    with pytest.raises(ReleaseEvidenceError, match="immutable content identity"):
        validate_record_integrity(root)


def test_int_frozen_input_digest_substitution_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    frozen = record["frozen_inputs"]
    assert isinstance(frozen, list) and isinstance(frozen[0], dict)
    frozen[0]["sha256"] = "0" * 64
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="frozen input digest differs"):
        validate_record_integrity(root)


def test_int_wp38_reviewed_transaction_is_a_required_frozen_input(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    frozen = record["frozen_inputs"]
    assert isinstance(frozen, list)
    frozen[:] = [
        item
        for item in frozen
        if isinstance(item, dict)
        and item.get("path")
        != "contracts/acceptance/relational-fabric-v3/production-evidence-transaction.jsonl"
    ]
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="frozen release input set differs"):
        validate_record_integrity(root)


def test_int_refresh_requires_reviewed_wp38_before_writing(tmp_path: Path) -> None:
    root = _copy_candidate(
        tmp_path / "repo",
        accept_wp38=False,
        refresh=False,
    )
    output = Path("target/wp40-candidate.json")
    with pytest.raises(ReleaseEvidenceError, match="not independently reviewed"):
        refresh_release_record(root, source=RECORD_PATH, output=output)
    assert not (root / output).exists()


def test_int_refresh_is_byte_idempotent_and_rebuilds_exact_inputs(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    frozen = record["frozen_inputs"]
    assert isinstance(frozen, list)
    frozen[:] = [
        item
        for item in frozen
        if isinstance(item, dict) and item.get("path") != str(TRANSACTION_PATH)
    ]
    _write(root, record)

    first = Path("target/wp40-first.json")
    second = Path("target/wp40-second.json")
    assert refresh_release_record(root, source=RECORD_PATH, output=first) == len(
        REQUIRED_FROZEN_INPUTS
    )
    assert refresh_release_record(root, source=RECORD_PATH, output=second) == len(
        REQUIRED_FROZEN_INPUTS
    )
    assert (root / first).read_bytes() == (root / second).read_bytes()

    refreshed = json.loads((root / first).read_text(encoding="utf-8"))
    transaction_rows = [
        item
        for item in refreshed["frozen_inputs"]
        if item["path"] == str(TRANSACTION_PATH)
    ]
    assert transaction_rows == [
        {
            "path": str(TRANSACTION_PATH),
            "sha256": _sha256(root / TRANSACTION_PATH),
            "role": "independently reviewed WP38 production evidence transaction",
        }
    ]
    assert refreshed["content_id"] == _content_id(refreshed)


def test_int_embedded_fake_pass_record_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    observations = record["development_observations"]
    assert isinstance(observations, list) and isinstance(observations[0], dict)
    observations[0]["status"] = "passed"
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="embedded pass records"):
        validate_record_integrity(root)


def test_int_dirty_base_cannot_be_promoted_to_proving_commit(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    proving = record["proving_state"]
    assert isinstance(proving, dict)
    proving["release_certification"] = "certified"
    proving["head_role"] = "proving_commit"
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="proving-state posture is invalid"):
        validate_record_integrity(root)


def test_int_clean_evidence_base_remains_valid_at_a_descendant_commit(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    subprocess.run(("git", "init", "-q"), cwd=root, check=True)
    subprocess.run(("git", "config", "user.name", "WP40 Test"), cwd=root, check=True)
    subprocess.run(
        ("git", "config", "user.email", "wp40-test@example.invalid"),
        cwd=root,
        check=True,
    )
    subprocess.run(("git", "add", "."), cwd=root, check=True)
    subprocess.run(("git", "commit", "-qm", "evidence base"), cwd=root, check=True)
    base = subprocess.run(
        ("git", "rev-parse", "HEAD"),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()

    record = _load(root)
    record["proving_state"] = {
        "base_head": base,
        "head_role": "ancestral_evidence_base",
        "worktree_state": "clean_evidence_base",
        "release_certification": "deferred_to_wp42",
        "reason": (
            "WP40 is frozen at an ancestral clean evidence base; final release authority "
            "belongs to WP42 and its independently reviewed certification."
        ),
    }
    limitations = record["limitations"]
    assert isinstance(limitations, list)
    limitations[:] = [
        item
        for item in limitations
        if isinstance(item, dict)
        and item.get("limitation_id")
        not in {"WORKTREE-NOT-PROVING-COMMIT", "EXECUTION-STATE-UNAVAILABLE"}
    ]
    _write(root, record)
    subprocess.run(("git", "add", str(RECORD_PATH)), cwd=root, check=True)
    subprocess.run(
        ("git", "commit", "-qm", "record evidence base"), cwd=root, check=True
    )

    assert validate_record_integrity(root) == len(REQUIRED_FROZEN_INPUTS)


def test_int_absent_live_recipe_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    justfile = (root / JUSTFILE_PATH).read_text(encoding="utf-8")
    (root / JUSTFILE_PATH).write_text(
        justfile.replace("exact-provider-batch-check:", "removed-provider-gate:", 1),
        encoding="utf-8",
    )
    with pytest.raises(ReleaseEvidenceError, match="absent live recipes"):
        validate_record_integrity(root)


def test_int_nonexistent_selected_test_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    matrix = record["matrix"]
    assert isinstance(matrix, list) and isinstance(matrix[0], dict)
    matrix[0]["selected_tests"] = ["invented_green_test"]
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="non-existent selected tests"):
        validate_record_integrity(root)


def test_beh_complete_successor_matrix_is_live_and_closed() -> None:
    assert validate_matrix_v3(ROOT) == 15


def test_beh_missing_dimension_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    matrix = record["matrix"]
    assert isinstance(matrix, list)
    matrix.pop()
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="matrix dimensions differ"):
        validate_matrix_v3(root)


def test_beh_retired_authority_recipe_cannot_enter_matrix(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    matrix = record["matrix"]
    assert isinstance(matrix, list) and isinstance(matrix[0], dict)
    matrix[0]["live_recipes"] = ["model-zero-state-check"]
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="retired authority recipe"):
        validate_matrix_v3(root)


def test_beh_missing_or_cyclic_provenance_rejects_release_evidence(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    graph = record["provenance_graph"]
    assert isinstance(graph, dict)
    edges = graph["edges"]
    assert isinstance(edges, list)
    edges.append({"subject": "source_image", "depends_on": "served_row"})
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="contains a cycle"):
        validate_matrix_v3(root)


def test_neg_security_resource_matrix_is_closed_and_fail_closed() -> None:
    assert validate_security_resource_rejection(ROOT) == 14


def test_neg_missing_fault_class_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    matrix = record["rejection_matrix"]
    assert isinstance(matrix, list)
    matrix.pop()
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="rejection matrix differs"):
        validate_security_resource_rejection(root)


def test_neg_unsupported_host_cannot_claim_untrusted_execution(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    environment = record["environment"]
    assert isinstance(environment, dict)
    containment = environment["host_containment"]
    assert isinstance(containment, dict)
    containment["untrusted_execution"] = "supported"
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="cannot claim untrusted execution"):
        validate_security_resource_rejection(root)


def test_neg_launcher_version_is_diagnostic_not_admission_authority(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    environment = record["environment"]
    assert isinstance(environment, dict)
    containment = environment["host_containment"]
    assert isinstance(containment, dict)
    launcher = containment["launcher"]
    assert isinstance(launcher, dict)
    launcher["observed_version"] = "bubblewrap 999.0"
    _write(root, record)
    assert validate_security_resource_rejection(root) == 14


def test_neg_launcher_implementation_is_diagnostic_not_admission_authority(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    environment = record["environment"]
    assert isinstance(environment, dict)
    containment = environment["host_containment"]
    assert isinstance(containment, dict)
    launcher = containment["launcher"]
    assert isinstance(launcher, dict)
    launcher.update(
        {
            "kind": "application-owned-launcher",
            "path": "/opt/codefabric/bin/provider-launcher",
            "observed_version": "provider-launcher 1",
            "observed_root_owned": False,
            "observed_mode": "0700",
            "observed_setuid": True,
        }
    )
    _write(root, record)
    assert validate_security_resource_rejection(root) == 14


def test_neg_launcher_diagnostic_cannot_promote_itself_to_admission_authority(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    environment = record["environment"]
    assert isinstance(environment, dict)
    containment = environment["host_containment"]
    assert isinstance(containment, dict)
    launcher = containment["launcher"]
    assert isinstance(launcher, dict)
    launcher["version_role"] = "admission_authority"
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="diagnostics are malformed"):
        validate_security_resource_rejection(root)


def test_neg_launcher_identity_cannot_replace_capability_remainder(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    environment = record["environment"]
    assert isinstance(environment, dict)
    containment = environment["host_containment"]
    assert isinstance(containment, dict)
    containment["unmet_requirements"] = ["compiled-seccomp-policy-authorized"]
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="capability remainders"):
        validate_security_resource_rejection(root)


def test_neg_blocking_host_limitation_cannot_be_removed(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    limitations = record["limitations"]
    assert isinstance(limitations, list)
    limitations[:] = [
        item
        for item in limitations
        if isinstance(item, dict)
        and item.get("limitation_id") != "UNTRUSTED-PROVIDER-PROFILE-UNAVAILABLE"
    ]
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="fail-closed limitation differs"):
        validate_security_resource_rejection(root)


def test_ops_clean_incremental_recovery_matrix_is_closed_without_certification() -> (
    None
):
    assert validate_clean_incremental_recovery_performance(ROOT) == 8


def test_ops_missing_operational_variation_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    matrix = record["operations_matrix"]
    assert isinstance(matrix, list)
    matrix.pop()
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="variation matrix differs"):
        validate_clean_incremental_recovery_performance(root)


def test_ops_unmeasured_performance_cannot_be_promoted(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    performance = record["performance"]
    assert isinstance(performance, dict)
    performance["claim_status"] = "certified"
    performance["benchmark_comparison"] = "inferred_from_one_run"
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="cannot become a passing claim"):
        validate_clean_incremental_recovery_performance(root)


def test_ops_performance_nonclaim_is_disclosed_but_not_blocking(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    limitations = record["limitations"]
    assert isinstance(limitations, list)
    disclosure = next(
        item
        for item in limitations
        if isinstance(item, dict)
        and item.get("limitation_id") == PERFORMANCE_DISCLOSURE_ID
    )
    assert disclosure == {
        "limitation_id": PERFORMANCE_DISCLOSURE_ID,
        "severity": "informational",
        "waivability": "not_applicable",
        "detail": (
            "No representative production workload or regression baseline is accepted; "
            "WP40 makes no performance or regression claim."
        ),
    }
    assert validate_clean_incremental_recovery_performance(root) == 8


def test_ops_retired_performance_baseline_blocker_is_rejected(
    tmp_path: Path,
) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    limitations = record["limitations"]
    assert isinstance(limitations, list)
    for item in limitations:
        if isinstance(item, dict) and item.get("limitation_id") == (
            PERFORMANCE_DISCLOSURE_ID
        ):
            item.update(
                {
                    "limitation_id": "PERFORMANCE-BASELINE-UNAVAILABLE",
                    "severity": "blocking",
                    "waivability": "non_waivable",
                }
            )
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="retired performance-baseline"):
        validate_clean_incremental_recovery_performance(root)


def test_ops_retired_performance_baseline_field_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate(tmp_path / "repo")
    record = _load(root)
    performance = record["performance"]
    assert isinstance(performance, dict)
    performance["benchmark_baseline"] = "not_recorded"
    _write(root, record)
    with pytest.raises(ReleaseEvidenceError, match="non-claim fields differ"):
        validate_clean_incremental_recovery_performance(root)
