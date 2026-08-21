"""Executable acceptance oracles for the planning-artifact contract."""

from __future__ import annotations

import json
import subprocess
from copy import deepcopy
from pathlib import Path

import pytest

from tooling.ci.artifact_contracts import (
    DEFAULT_PLAN,
    ROOT,
    ArtifactContractError,
    check_tracked_target_zero_state,
    commit_trust,
    derive_plan_status,
    load_state,
    validate_artifacts,
    validate_state,
)
from tooling.ci.proof_coverage import load_just_recipes

STATE = ROOT / "docs/plans/state/codefabric-waves-0-3-foundation_v5_state.json"


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


def test_state_schema_v2_round_trips_judgment_only(tmp_path: Path) -> None:
    state = load_state(STATE)
    state_path = tmp_path / "state.json"
    _write_state(state_path, state)
    assert validate_state(ROOT, state_path) == state


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


def test_wp00_behavioral_acceptance() -> None:
    state = validate_state(ROOT, STATE)
    assert state["schema_version"] == 2
    assert state["current_packet"] == "WP00"
    assert commit_trust(ROOT, state["baseline_commit"])["ancestor"]


def test_wp00_structural_acceptance() -> None:
    report = validate_artifacts(ROOT, DEFAULT_PLAN)
    assert report["packet_count"] == 29
    assert report["declared_input_count"] == 18


def test_wp00_negative_zero_state(tmp_path: Path) -> None:
    state = load_state(STATE)
    derived = deepcopy(state)
    derived["packets"]["WP00"]["checks"] = ["invented"]
    derived_path = tmp_path / "derived.json"
    _write_state(derived_path, derived)
    with pytest.raises(ArtifactContractError, match="expected keys|derived"):
        validate_state(ROOT, derived_path)

    unproved = deepcopy(state)
    unproved["packets"]["WP00"]["status"] = "complete"
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


def test_wp00_operational_acceptance() -> None:
    recipes = load_just_recipes()
    assert {
        "artifacts-check",
        "plan-status",
        "tracked-target-zero-state-check",
    } <= recipes.keys()
    assert check_tracked_target_zero_state(ROOT)["tracked_target_paths"] == 0
    assert derive_plan_status(ROOT, DEFAULT_PLAN)["healthy"]
