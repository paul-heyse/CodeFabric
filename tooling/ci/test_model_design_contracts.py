"""Executable WP01 oracles for model-control design adoption."""

from __future__ import annotations

import subprocess
from copy import deepcopy
from pathlib import Path

import pytest

from tooling.ci.artifact_contracts import (
    ROOT,
    ArtifactContractError,
    _accepted_input_evolution_paths,
    _sha256,
    active_plan_path,
    declared_inputs,
    load_state,
    parse_frontmatter,
)
from tooling.ci.model_design_contracts import (
    CONTROL_PLAN,
    EVOLVED_DESIGN_INPUTS,
    FORBIDDEN_DESIGN_PHRASES,
    validate_model_design_contract,
)


def test_model_active_program_is_unique() -> None:
    report = validate_model_design_contract()
    assert report["control_plan"] == CONTROL_PLAN.as_posix()
    assert (
        active_plan_path(ROOT).resolve().relative_to(ROOT.resolve()).as_posix()
        == report["active_plan"]
    )
    assert report["active_plan"] != report["suspended_plan"]
    assert report["active_plan"].startswith("docs/plans/")


def test_model_active_program_rejects_an_unlisted_plan(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    rogue_plan = tmp_path / "rogue-plan.md"
    rogue_plan.write_text("---\nstatus: approved\n---\n", encoding="utf-8")
    monkeypatch.setattr(
        "tooling.ci.model_design_contracts.active_plan_path",
        lambda _root: rogue_plan,
    )
    with pytest.raises(
        ArtifactContractError,
        match="outside the governed model-control handoff",
    ):
        validate_model_design_contract()


def test_model_design_rejects_routine_acceptance_writes() -> None:
    validate_model_design_contract()
    suite = (
        ROOT
        / "docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md"
    ).read_text(encoding="utf-8")
    assert "Routine synchronization SHALL NOT edit bundle" in suite
    combined = "\n".join(
        (ROOT / path).read_text(encoding="utf-8")
        for path in sorted(EVOLVED_DESIGN_INPUTS)
    )
    assert not [phrase for phrase in FORBIDDEN_DESIGN_PHRASES if phrase in combined]


def test_model_wp01_planned_input_evolution_names_exactly_five_accepted_paths() -> None:
    plan = parse_frontmatter(ROOT / CONTROL_PLAN)
    state = load_state(ROOT / str(plan["state_path"]))
    evolutions = [
        deviation
        for deviation in state["plan_deviations"]
        if deviation.get("kind") == "planned_design_input_evolution"
        and deviation.get("packet") == "WP01"
    ]
    assert len(evolutions) == 1
    assert evolutions[0]["packet"] == "WP01"
    assert set(evolutions[0]["paths"]) == set(EVOLVED_DESIGN_INPUTS)


def test_model_wp01_state_transition_enables_post_judgment_artifact_freshness() -> None:
    plan_path = ROOT / CONTROL_PLAN
    plan = parse_frontmatter(plan_path)
    state = deepcopy(load_state(ROOT / str(plan["state_path"])))
    head = subprocess.run(
        ("git", "rev-parse", "HEAD"),
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    state["packets"]["WP01"]["status"] = "complete"
    state["packets"]["WP01"]["proving_commit"] = head
    accepted = _accepted_input_evolution_paths(ROOT, state)
    assert set(EVOLVED_DESIGN_INPUTS) <= accepted

    drifted = {
        declared.path
        for declared in declared_inputs(plan_path)
        if _sha256(ROOT / declared.path) != declared.digest
    }
    assert set(EVOLVED_DESIGN_INPUTS) <= drifted
    assert drifted <= accepted
