"""Executable oracles for the sealed Waves 4-7 successor handoff."""

from __future__ import annotations

from tooling.ci.artifact_contracts import ROOT, active_plan_path, load_state
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
