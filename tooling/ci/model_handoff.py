"""Validate the sealed model-control-plane to Waves 4-7 handoff."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

from tooling.ci.artifact_contracts import (
    ROOT,
    ArtifactContractError,
    active_plan_path,
    commit_trust,
    load_state,
    validate_plan,
    validate_state,
)

REMEDIATION_PLAN = Path(
    "docs/plans/codefabric_model_driven_artifact_and_assurance_control_plane_implementation_plan_v1_2026-08-22.md"
)
SUCCESSOR_PLAN = Path(
    "docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v5_2026-08-22.md"
)
FROZEN_PLAN = Path(
    "docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v4_2026-08-22.md"
)
FROZEN_STATE = Path("docs/plans/state/codefabric-waves-4-7-core-facts_v4_state.json")
TRUSTED_PACKETS = {
    "WP27": "7d82ec80b8b3e0812e97b668058315da1aa73030",
    "WP28": "fed2917249b1e791346b289e4195c658bb40a8d1",
    "WP29": "0e440ac69ea2d684fbefc50b3508d523c304c4e1",
    "WP30": "0b5cbdbba286bb465126caffe458963cc4d9dc38",
    "WP31": "74befce8d04cc1f5a53c9d2ae728e03f8a929457",
}


def _relative(path: Path, root: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def _state_for(root: Path, plan: dict[str, Any]) -> dict[str, Any]:
    return validate_state(
        root, root / str(plan["state_path"]), expected_ids=plan["ids"]
    )


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ArtifactContractError(message)


def _git_diff_is_quiet(root: Path, baseline: str, *paths: Path) -> bool:
    return (
        subprocess.run(
            (
                "git",
                "diff",
                "--quiet",
                baseline,
                "--",
                *(path.as_posix() for path in paths),
            ),
            cwd=root,
            check=False,
        ).returncode
        == 0
    )


def validate_handoff(root: Path = ROOT) -> dict[str, Any]:
    """Validate both the inactive-candidate and activated H/S handoff states."""
    root = root.resolve()
    remediation_path = root / REMEDIATION_PLAN
    successor_path = root / SUCCESSOR_PLAN

    remediation = validate_plan(root, remediation_path, verify_declared_inputs=False)
    successor = validate_plan(root, successor_path)
    _require(
        successor["status"] == "approved", "Waves successor is not explicitly approved"
    )
    _require(
        successor.get("supersedes_plan_path") == FROZEN_PLAN.as_posix(),
        "Waves successor does not identify the frozen v4 plan",
    )
    _require(
        successor.get("activation_requires")
        == "codefabric-model-driven-artifact-and-assurance-control-plane/M05",
        "Waves successor lacks the sealed remediation-M05 activation prerequisite",
    )

    remediation_state = _state_for(root, remediation)
    successor_state = _state_for(root, successor)
    frozen_state = load_state(root / FROZEN_STATE)
    baseline = str(successor["baseline_commit"])
    _require(
        commit_trust(root, baseline)["ancestor"],
        "successor baseline is not an ancestor",
    )
    _require(
        _git_diff_is_quiet(root, baseline, FROZEN_PLAN, FROZEN_STATE),
        "frozen Waves v4 plan/state changed after the successor baseline",
    )

    for packet, commit in TRUSTED_PACKETS.items():
        candidate = successor_state["packets"][packet]
        historical = frozen_state["packets"][packet]
        _require(
            candidate["status"] == "complete", f"{packet} is not complete in successor"
        )
        _require(
            candidate["proving_commit"] == commit, f"{packet} proving commit changed"
        )
        _require(
            historical["proving_commit"] == commit,
            f"{packet} differs from frozen state",
        )
        _require(
            commit_trust(root, commit)["ancestor"], f"{packet} proof is not an ancestor"
        )

    wp32 = successor_state["packets"]["WP32"]
    _require(wp32["status"] == "in_progress", "successor WP32 must remain in progress")
    _require(wp32["proving_commit"] is None, "successor WP32 must remain unproved")
    for number in range(33, 54):
        packet = f"WP{number}"
        entry = successor_state["packets"][packet]
        _require(
            entry["status"] == "not_started", f"{packet} progressed before handoff seal"
        )
        _require(
            entry["proving_commit"] is None, f"{packet} has an early proving commit"
        )
    for group in ("milestones", "decommission_batches"):
        for identifier, entry in successor_state[group].items():
            _require(
                entry["status"] == "not_started", f"{identifier} progressed before WP32"
            )
            _require(
                entry["proving_commit"] is None, f"{identifier} has an early proof"
            )

    active = active_plan_path(root)
    remediation_relative = _relative(remediation_path, root)
    successor_relative = _relative(successor_path, root)
    mode: str
    if active.resolve() == remediation_path.resolve():
        mode = "approved-inactive-candidate"
        _require(
            remediation_state["status"] == "executing", "remediation is not executing"
        )
        _require(
            remediation_state["current_packet"] in {"WP14", "WP15"},
            "remediation is not at its release/handoff boundary",
        )
        _require(
            successor_state["status"] == "not_started",
            "inactive successor state is active",
        )
        if remediation_state["current_packet"] == "WP15":
            _require(
                remediation_state["packets"]["WP14"]["status"] == "complete"
                and remediation_state["milestones"]["M04"]["status"] == "complete",
                "WP15 was released before WP14/M04 completion",
            )
    elif active.resolve() == successor_path.resolve():
        mode = "handoff-unsealed"
        _require(
            successor_state["status"] == "executing",
            "active successor is not executing",
        )
        _require(
            successor_state["current_packet"] == "WP32",
            "active successor is not at WP32",
        )
        wp15 = remediation_state["packets"]["WP15"]
        _require(
            wp15["status"] in {"in_progress", "complete"},
            "WP15 does not own activation",
        )
        if wp15["status"] == "complete":
            mode = "handoff-sealed"
            proof = wp15["proving_commit"]
            _require(
                commit_trust(root, proof)["ancestor"],
                "WP15 handoff proof is not trusted",
            )
            _require(
                remediation_state["status"] == "complete"
                and remediation_state["milestones"]["M05"]["status"] == "complete"
                and remediation_state["decommission_batches"]["DB06"]["status"]
                == "complete",
                "terminal remediation seal is incomplete",
            )
            _require(
                remediation_state["milestones"]["M05"]["proving_commit"] == proof
                and remediation_state["decommission_batches"]["DB06"]["proving_commit"]
                == proof,
                "terminal seal does not consistently record handoff commit H",
            )
    else:
        raise ArtifactContractError(
            f"active plan is neither remediation nor successor: {_relative(active, root)}"
        )

    return {
        "mode": mode,
        "active_plan": _relative(active, root),
        "remediation_plan": remediation_relative,
        "successor_plan": successor_relative,
        "trusted_historical_packets": sorted(TRUSTED_PACKETS),
        "resume_packet": "WP32",
        "product_packets_released": 0,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        report = validate_handoff(args.root)
    except (
        ArtifactContractError,
        OSError,
        KeyError,
        TypeError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"model handoff error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
