"""Focused authoring and falsification tests for the WP38 transaction utility."""

from __future__ import annotations

import json
import re
import shutil
from pathlib import Path
from typing import Any

import pytest
import rfc8785

from tooling.ci import production_evidence, reissue_wp38_transaction
from tooling.ci.production_evidence import ProductionEvidenceError
from tooling.ci.reissue_wp38_transaction import (
    EXECUTOR_IDENTITY,
    LIMITATION_EVIDENCE,
    append_accepted_review,
    draft_transaction,
    load_transaction,
    write_validated_transaction,
)
from tooling.ci.successor_evidence_issuance import (
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ISSUANCE_PATH,
)


def _write_json(path: Path, value: object) -> None:
    path.write_text(
        json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def _accepted_candidate_root(
    destination: Path, monkeypatch: pytest.MonkeyPatch
) -> Path:
    for path in (
        EXPECTATIONS_PATH,
        FIXTURES_PATH,
        ISSUANCE_PATH,
        production_evidence.JUSTFILE_PATH,
    ):
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(production_evidence.ROOT / path, target)
    issuance_path = destination / ISSUANCE_PATH
    issuance = json.loads(issuance_path.read_text(encoding="utf-8"))
    issuance["status"] = "accepted"
    _write_json(issuance_path, issuance)

    # The fixture isolates WP38 authoring from an in-progress shared WP33 review.
    # The utility's explicit accepted-status check and the real WP38 validator still run.
    monkeypatch.setattr(
        reissue_wp38_transaction,
        "validate_transaction_integrity",
        lambda _root: 18,
    )
    return destination


def _issuance(root: Path) -> dict[str, Any]:
    value = json.loads((root / ISSUANCE_PATH).read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def test_int_draft_is_reproducible_and_uses_live_wp38_constants(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _accepted_candidate_root(tmp_path / "repo", monkeypatch)
    first = draft_transaction(root)
    second = draft_transaction(root)
    assert first == second
    assert len(first) == 4
    assert tuple(entry["entry_kind"] for entry in first) == (
        production_evidence.EXPECTED_ENTRY_KINDS
    )
    assert {entry["recorded_by"] for entry in first} == {EXECUTOR_IDENTITY}
    assert first[0]["payload"]["oracles"] == list(production_evidence.ORACLES)
    assert first[2]["payload"]["recipe_dependencies"] == {
        oracle: list(dependencies)
        for oracle, dependencies in production_evidence.EXPECTED_RECIPE_DEPENDENCIES.items()
    }
    assert len(first[1]["payload"]["claims"]) == 18
    production_evidence._validate_chain(first)


def test_int_pending_wp33_issuance_fails_closed_before_drafting(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _accepted_candidate_root(tmp_path / "repo", monkeypatch)
    issuance_path = root / ISSUANCE_PATH
    issuance = _issuance(root)
    issuance["status"] = "pending_independent_review"
    _write_json(issuance_path, issuance)

    with pytest.raises(ProductionEvidenceError) as failure:
        draft_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_WP33_ISSUANCE_INVALID"


def test_ops_limitations_are_behavioral_and_launcher_version_is_diagnostic_only(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _accepted_candidate_root(tmp_path / "repo", monkeypatch)
    entries = draft_transaction(root)
    limitations = {row["id"]: row for row in entries[3]["payload"]["limitations"]}
    host = " ".join(limitations["HOST-UNTRUSTED-CONTAINMENT"]["evidence"])
    assert "cgroup-v2" in host
    assert "compiled seccomp authorization is absent" in host
    assert "untrusted execution is unavailable" in host
    assert "launcher version are diagnostic-only metadata" in host
    assert "admission or acceptance gates" in host
    assert re.search(r"\b\d+\.\d+\.\d+\b", host) is None
    assert tuple(limitations) == tuple(LIMITATION_EVIDENCE)


def test_ops_independent_review_binds_tip_and_preserves_execution_entries(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _accepted_candidate_root(tmp_path / "repo", monkeypatch)
    draft = draft_transaction(root)
    draft_snapshot = json.loads(json.dumps(draft))
    reviewer = "wp38-independent-transaction-reviewer-test"
    reviewed = append_accepted_review(draft, reviewer_identity=reviewer, root=root)
    assert list(draft) == draft_snapshot
    assert len(reviewed) == 5
    assert reviewed[-1]["entry_kind"] == "review_accepted"
    assert reviewed[-1]["recorded_by"] == reviewer
    assert reviewed[-1]["payload"]["reviewer_identity"] == reviewer
    assert reviewed[-1]["payload"]["reviewed_through_entry_b3"] == draft[-1]["entry_b3"]
    assert (
        production_evidence.validate_append_only_transaction(
            _write_candidate_root(root, tmp_path / "reviewed", reviewed)
        )
        == 18
    )


def _write_candidate_root(
    source: Path, destination: Path, entries: tuple[dict[str, Any], ...]
) -> Path:
    for path in (
        EXPECTATIONS_PATH,
        FIXTURES_PATH,
        ISSUANCE_PATH,
        production_evidence.JUSTFILE_PATH,
    ):
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / path, target)
    transaction_path = destination / production_evidence.TRANSACTION_PATH
    transaction_path.parent.mkdir(parents=True, exist_ok=True)
    transaction_path.write_bytes(
        b"".join(rfc8785.dumps(entry) + b"\n" for entry in entries)
    )
    return destination


@pytest.mark.parametrize("identity_kind", ["executor", "expectation_author"])
def test_neg_executor_and_expectation_author_cannot_accept_transaction(
    identity_kind: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _accepted_candidate_root(tmp_path / identity_kind, monkeypatch)
    draft = draft_transaction(root)
    reviewer = (
        EXECUTOR_IDENTITY
        if identity_kind == "executor"
        else str(_issuance(root)["author"]["identity"])
    )
    with pytest.raises(ProductionEvidenceError) as failure:
        append_accepted_review(draft, reviewer_identity=reviewer, root=root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID"


def test_int_writer_runs_production_validation_before_destination_write(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _accepted_candidate_root(tmp_path / "repo", monkeypatch)
    draft = draft_transaction(root)
    output = tmp_path / "candidate.jsonl"
    output.write_bytes(b"preserve-on-validation-failure\n")
    calls: list[tuple[Path, bool]] = []

    def reject(candidate_root: Path, *, require_review: bool = True) -> int:
        calls.append((candidate_root, require_review))
        assert (candidate_root / production_evidence.TRANSACTION_PATH).is_file()
        raise ProductionEvidenceError(
            "PRODUCTION_EVIDENCE_TEST_REJECTION", "injected validator rejection"
        )

    monkeypatch.setattr(production_evidence, "validate_append_only_transaction", reject)
    with pytest.raises(ProductionEvidenceError) as failure:
        write_validated_transaction(output, draft, root=root, require_review=False)
    assert failure.value.code == "PRODUCTION_EVIDENCE_TEST_REJECTION"
    assert calls and calls[0][1] is False
    assert output.read_bytes() == b"preserve-on-validation-failure\n"


def test_int_validated_writer_emits_canonical_jsonl_and_strict_loader_round_trips(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = _accepted_candidate_root(tmp_path / "repo", monkeypatch)
    draft = draft_transaction(root)
    output = tmp_path / "draft.jsonl"
    assert (
        write_validated_transaction(output, draft, root=root, require_review=False)
        == 18
    )
    lines = output.read_bytes().splitlines()
    assert len(lines) == 4
    assert all(line == rfc8785.dumps(json.loads(line)) for line in lines)
    assert load_transaction(output) == draft


def test_neg_strict_loader_rejects_duplicate_json_members(tmp_path: Path) -> None:
    path = tmp_path / "duplicate.jsonl"
    path.write_text('{"schema_version":1,"schema_version":1}\n', encoding="utf-8")
    with pytest.raises(ProductionEvidenceError) as failure:
        load_transaction(path)
    assert failure.value.code == "PRODUCTION_EVIDENCE_DUPLICATE_JSON_MEMBER"
