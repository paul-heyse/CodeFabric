"""Executable oracles for the sealed Waves 4-7 successor handoff."""

from __future__ import annotations

from copy import deepcopy

import pytest

from tooling.ci import model_handoff
from tooling.ci.artifact_contracts import (
    ROOT,
    ArtifactContractError,
    active_plan_path,
    load_state,
)
from tooling.ci.model_handoff import (
    REMEDIATION_PLAN,
    SUCCESSOR_PLAN,
    TRUSTED_PACKETS,
    validate_handoff,
)


def test_model_waves_successor_candidate_preserves_history_and_leaves_wp32_incomplete() -> (
    None
):
    report = validate_handoff()
    assert report["trusted_historical_packets"] == sorted(TRUSTED_PACKETS)
    assert report["resume_packet"] == "WP32"
    state = load_state(
        ROOT / "docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json"
    )
    assert state["packets"]["WP32"]["status"] == "in_progress"
    assert state["packets"]["WP32"]["proving_commit"] is None


def test_model_active_pointer_remains_on_remediation_through_release_certification() -> (
    None
):
    report = validate_handoff()
    if report["mode"] == "approved-inactive-candidate":
        assert active_plan_path(ROOT) == ROOT / REMEDIATION_PLAN
    else:
        assert active_plan_path(ROOT) == ROOT / SUCCESSOR_PLAN


def test_model_handoff_commit_activates_only_approved_successor_at_incomplete_wp32() -> (
    None
):
    report = validate_handoff()
    assert report["product_packets_released"] == 0
    assert report["mode"] in {
        "approved-inactive-candidate",
        "handoff-unsealed",
        "handoff-sealed",
    }


def test_model_handoff_at_h_is_the_complete_executable_outcome() -> None:
    report = validate_handoff()
    assert report["successor_plan"] == SUCCESSOR_PLAN.as_posix()
    assert report["resume_packet"] == "WP32"


def test_model_handoff_rejects_two_active_plans_unapproved_successor_and_early_wp33(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    remediation_plan = model_handoff.validate_plan(
        ROOT, ROOT / REMEDIATION_PLAN, verify_declared_inputs=False
    )
    successor_plan = model_handoff.validate_plan(ROOT, ROOT / SUCCESSOR_PLAN)
    remediation_state = load_state(ROOT / remediation_plan["state_path"])
    successor_state = load_state(ROOT / successor_plan["state_path"])
    frozen_state = load_state(ROOT / model_handoff.FROZEN_STATE)

    def fake_plan(_root: object, path: object, **_kwargs: object) -> object:
        selected = (
            remediation_plan
            if str(path).endswith(REMEDIATION_PLAN.as_posix())
            else successor_plan
        )
        return deepcopy(selected)

    def fake_state(_root: object, plan: object) -> object:
        assert isinstance(plan, dict)
        selected = (
            remediation_state
            if plan["plan_id"] == remediation_plan["plan_id"]
            else successor_state
        )
        return deepcopy(selected)

    monkeypatch.setattr(model_handoff, "validate_plan", fake_plan)
    monkeypatch.setattr(model_handoff, "_state_for", fake_state)
    monkeypatch.setattr(
        model_handoff, "load_state", lambda _path: deepcopy(frozen_state)
    )
    monkeypatch.setattr(
        model_handoff,
        "commit_trust",
        lambda _root, _commit: {"exists": True, "ancestor": True},
    )
    monkeypatch.setattr(model_handoff, "_git_diff_is_quiet", lambda *_args: True)

    successor_plan["status"] = "draft"
    with pytest.raises(ArtifactContractError, match="not explicitly approved"):
        validate_handoff()
    successor_plan["status"] = "approved"

    remediation_state["status"] = "executing"
    remediation_state["current_packet"] = "WP15"
    remediation_state["packets"]["WP15"]["status"] = "in_progress"
    successor_state["status"] = "executing"
    monkeypatch.setattr(
        model_handoff, "active_plan_path", lambda _root: ROOT / REMEDIATION_PLAN
    )
    with pytest.raises(
        ArtifactContractError, match="inactive successor state is active"
    ):
        validate_handoff()

    remediation_state["status"] = "complete"
    remediation_state["current_packet"] = None
    remediation_state["packets"]["WP15"]["status"] = "complete"
    successor_state["packets"]["WP33"]["status"] = "in_progress"
    monkeypatch.setattr(
        model_handoff, "active_plan_path", lambda _root: ROOT / SUCCESSOR_PLAN
    )
    with pytest.raises(
        ArtifactContractError, match="WP33 progressed before handoff seal"
    ):
        validate_handoff()
