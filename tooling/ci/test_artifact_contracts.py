"""Executable acceptance oracles for the planning-artifact contract."""

from __future__ import annotations

import hashlib
import json
import subprocess
from copy import deepcopy
from pathlib import Path

import pytest

from tooling.ci.artifact_contracts import (
    DEFAULT_PLAN,
    REVIEW_REQUIREMENTS,
    ROOT,
    ArtifactContractError,
    _accepted_gate_substitutions,
    _accepted_input_evolution_paths,
    _successor_evidence_claim_count,
    activate_plan,
    active_plan_path,
    check_tracked_target_zero_state,
    commit_trust,
    declared_inputs,
    derive_plan_status,
    documented_review_artifacts,
    load_just_recipes,
    load_state,
    parse_frontmatter,
    plan_ids,
    validate_artifacts,
    validate_plan,
    validate_state,
)

STATE = ROOT / str(parse_frontmatter(DEFAULT_PLAN)["state_path"])


def test_review_artifact_vocabulary_matches_documented_schema() -> None:
    assert documented_review_artifacts() == set(REVIEW_REQUIREMENTS)


def _git(root: Path, *args: str) -> str:
    return subprocess.run(
        ("git", *args),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def _write_state(path: Path, state: dict[str, object]) -> None:
    path.write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")


def _init_repository(root: Path) -> None:
    _git(root, "init", "-q")
    _git(root, "config", "user.name", "CodeFabric Test")
    _git(root, "config", "user.email", "codefabric-test@example.invalid")
    (root / ".gitignore").write_text(
        "/target/\n/fuzz/target/\n/rustc-extractor/target/\n/pyrefly-sidecar/target/\n",
        encoding="utf-8",
    )
    (root / "README.md").write_text("fixture\n", encoding="utf-8")
    _git(root, "add", ".gitignore", "README.md")
    _git(root, "commit", "-q", "-m", "fixture baseline")


def _force_commit_target(root: Path, relative: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("transient build output\n", encoding="utf-8")
    _git(root, "add", "-f", relative)
    _git(root, "commit", "-q", "-m", "track forbidden target output")


def _write_activation_fixture(root: Path, *, status: str) -> Path:
    design = root / "design.md"
    design.write_text("design\n", encoding="utf-8")
    declared = root / "input.md"
    declared.write_text("input\n", encoding="utf-8")
    digest = hashlib.sha256(declared.read_bytes()).hexdigest()
    baseline = _git(root, "rev-parse", "HEAD")
    plan = root / "plan.md"
    plan.write_text(
        f"""---
artifact: implementation-plan
plan_id: fixture
version: v1
date: 2026-08-25
status: {status}
design_path: design.md
design_version: v1
baseline_commit: {baseline}
state_path: state.json
cutover: true
---

## 1. Outcome and non-goals

## 2. Source design and declared inputs

| path | sha256 |
|---|---|
| input.md | {digest} |

## 3. Global target invariants

## 4. Work packets

### WP01 — Fixture packet

Executable oracle: `fixture_behavioral`
Executable oracle: `fixture_structural`
Executable oracle: `fixture_negative`
Executable oracle: `fixture_operational`

## 5. Integration milestones

### M01 — Fixture milestone

## 6. Cross-packet decommission batches

### DB01 — Fixture decommission
""",
        encoding="utf-8",
    )
    return plan


def test_state_schema_v2_round_trips_judgment_only(tmp_path: Path) -> None:
    state = load_state(STATE)
    state_path = tmp_path / "state.json"
    _write_state(state_path, state)
    assert validate_state(ROOT, state_path) == state


def test_inactive_draft_may_declare_future_state(tmp_path: Path) -> None:
    _init_repository(tmp_path)
    plan = _write_activation_fixture(tmp_path, status="draft")
    assert validate_plan(tmp_path, plan)["status"] == "draft"
    assert not (tmp_path / "state.json").exists()


def test_activation_creates_valid_state_before_pointer_cutover(tmp_path: Path) -> None:
    _init_repository(tmp_path)
    plan = _write_activation_fixture(tmp_path, status="approved")
    with pytest.raises(ArtifactContractError, match="unresolved state_path"):
        validate_plan(tmp_path, plan)

    report = activate_plan(tmp_path, plan)
    assert report["plan"] == "plan.md"
    pointer = json.loads((tmp_path / "docs/plans/active-plan.json").read_text())
    assert pointer == {"schema_version": 1, "plan_path": "plan.md"}
    state = validate_state(
        tmp_path,
        tmp_path / "state.json",
        expected_ids=plan_ids(plan),
    )
    assert state["plan_path"] == "plan.md"


def test_failed_activation_preserves_prior_pointer(tmp_path: Path) -> None:
    _init_repository(tmp_path)
    pointer_path = tmp_path / "docs/plans/active-plan.json"
    pointer_path.parent.mkdir(parents=True)
    prior = {"schema_version": 1, "plan_path": "prior.md"}
    pointer_path.write_text(json.dumps(prior) + "\n", encoding="utf-8")
    (tmp_path / "prior.md").write_text("prior\n", encoding="utf-8")
    plan = _write_activation_fixture(tmp_path, status="draft")

    with pytest.raises(ArtifactContractError, match="only an approved plan"):
        activate_plan(tmp_path, plan)
    assert json.loads(pointer_path.read_text()) == prior
    assert not (tmp_path / "state.json").exists()


def test_packet_trust_requires_ancestor_commit(tmp_path: Path) -> None:
    _init_repository(tmp_path)
    ancestor = _git(tmp_path, "rev-parse", "HEAD")
    _git(tmp_path, "checkout", "-q", "--orphan", "detached-proof")
    (tmp_path / "detached.txt").write_text("unrelated\n", encoding="utf-8")
    _git(tmp_path, "add", "detached.txt")
    _git(tmp_path, "commit", "-q", "-m", "non-ancestor proof")
    non_ancestor = _git(tmp_path, "rev-parse", "HEAD")
    _git(tmp_path, "checkout", "-q", "master")

    assert commit_trust(tmp_path, ancestor) == {"exists": True, "ancestor": True}
    assert commit_trust(tmp_path, non_ancestor) == {
        "exists": True,
        "ancestor": False,
    }
    assert commit_trust(tmp_path, "0" * 40) == {
        "exists": False,
        "ancestor": False,
    }


def test_active_program_behavioral_acceptance() -> None:
    assert active_plan_path(ROOT) == DEFAULT_PLAN
    assert parse_frontmatter(DEFAULT_PLAN)["status"] == "approved"
    state = validate_state(ROOT, STATE)
    assert state["schema_version"] == 2
    assert (
        state["current_packet"] is None or state["current_packet"] in state["packets"]
    )
    assert set(state["packets"]) == set(plan_ids(DEFAULT_PLAN)["packets"])
    assert commit_trust(ROOT, state["baseline_commit"])["ancestor"]


def test_active_program_structural_acceptance() -> None:
    report = validate_artifacts(ROOT, DEFAULT_PLAN)
    assert report["packet_count"] == len(plan_ids(DEFAULT_PLAN)["packets"])
    assert report["declared_input_count"] == len(declared_inputs(DEFAULT_PLAN))


def test_active_v4_artifact_contract_uses_only_v4_evidence(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from types import SimpleNamespace

    from tooling.ci import successor_evidence_issuance, successor_evidence_issuance_v4

    expected_claims = successor_evidence_issuance_v4.EXPECTED_CLAIM_IDS

    def predecessor_must_not_run(_root: Path) -> int:
        raise AssertionError("v3 evidence remained live after v4 activation")

    monkeypatch.setattr(
        successor_evidence_issuance,
        "validate_transaction_integrity",
        predecessor_must_not_run,
    )
    monkeypatch.setattr(
        successor_evidence_issuance_v4,
        "validate_issuance",
        lambda _root, *, require_review: SimpleNamespace(expectations=expected_claims),
    )
    plan = parse_frontmatter(DEFAULT_PLAN)
    assert _successor_evidence_claim_count(ROOT, plan) == len(expected_claims)


def test_non_relational_plan_has_no_implicit_successor_evidence() -> None:
    assert (
        _successor_evidence_claim_count(ROOT, {"plan_id": "fixture", "version": "v1"})
        == 0
    )


def test_planned_input_evolution_requires_completed_ancestor_proof() -> None:
    state = load_state(STATE)
    evolutions = [
        deviation
        for deviation in state["plan_deviations"]
        if deviation.get("kind") == "planned_design_input_evolution"
    ]
    incomplete = deepcopy(state)
    for evolution in evolutions:
        packet = evolution["packet"]
        incomplete["packets"][packet]["status"] = "in_progress"
        incomplete["packets"][packet]["proving_commit"] = None
    assert not _accepted_input_evolution_paths(ROOT, incomplete)

    expected = {
        path
        for evolution in evolutions
        if state["packets"][evolution["packet"]]["status"] == "complete"
        and commit_trust(ROOT, state["packets"][evolution["packet"]]["proving_commit"])[
            "ancestor"
        ]
        for path in evolution["paths"]
    }
    assert _accepted_input_evolution_paths(ROOT, state) == expected


def test_gate_substitution_is_explicit_and_cannot_self_replace() -> None:
    state = load_state(STATE)
    packet, replacement = list(state["packets"])[:2]
    synthetic = deepcopy(state)
    synthetic["plan_deviations"].append(
        {
            "kind": "accepted_gate_substitution",
            "replacement_packet": replacement,
            "superseded_packets": [packet],
            "summary": "Synthetic judgment for the generic state-contract oracle.",
        }
    )
    assert _accepted_gate_substitutions(synthetic)[packet] == replacement

    invalid = deepcopy(synthetic)
    invalid["plan_deviations"][-1]["superseded_packets"] = [replacement]
    with pytest.raises(ArtifactContractError, match="replace a packet with itself"):
        _accepted_gate_substitutions(invalid)


def test_active_program_negative_zero_state(tmp_path: Path) -> None:
    state = load_state(STATE)
    packet = next(iter(state["packets"]))
    derived = deepcopy(state)
    derived["packets"][packet]["checks"] = ["invented"]
    derived_path = tmp_path / "derived.json"
    _write_state(derived_path, derived)
    with pytest.raises(ArtifactContractError, match="expected keys|derived"):
        validate_state(ROOT, derived_path)

    unproved = deepcopy(state)
    unproved["packets"][packet]["status"] = "complete"
    unproved["packets"][packet]["proving_commit"] = None
    unproved_path = tmp_path / "unproved.json"
    _write_state(unproved_path, unproved)
    with pytest.raises(ArtifactContractError, match="requires a proving commit"):
        validate_state(ROOT, unproved_path)

    current_repo = tmp_path / "current"
    current_repo.mkdir()
    _init_repository(current_repo)
    _force_commit_target(current_repo, "rustc-extractor/target/debug/output")
    with pytest.raises(ArtifactContractError, match="tracked="):
        check_tracked_target_zero_state(current_repo)

    history_repo = tmp_path / "history"
    history_repo.mkdir()
    _init_repository(history_repo)
    forbidden = "pyrefly-sidecar/target/debug/output"
    _force_commit_target(history_repo, forbidden)
    _git(history_repo, "rm", "-q", forbidden)
    _git(history_repo, "commit", "-q", "-m", "remove forbidden output")
    with pytest.raises(ArtifactContractError, match="historical="):
        check_tracked_target_zero_state(history_repo)


def test_active_program_operational_acceptance() -> None:
    recipes = load_just_recipes()
    assert {
        "artifacts-check",
        "plan-status",
        "tracked-target-zero-state-check",
    } <= recipes.keys()
    assert check_tracked_target_zero_state(ROOT)["tracked_target_paths"] == 0
    status = derive_plan_status(ROOT, DEFAULT_PLAN)
    assert status["healthy"]
    assert status["untrusted_complete_entries"] == []
    for packet in status["packets"].values():
        for oracle, implemented in packet["named_oracles"].items():
            if packet["status"] == "complete" and oracle.startswith("just "):
                assert implemented is True or packet["assurance_substitute"] is not None
