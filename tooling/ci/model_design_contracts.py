"""Validate the adopted model-control ownership and active-program contract."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path

from tooling.ci.artifact_contracts import (
    ROOT,
    ArtifactContractError,
    active_plan_path,
    load_state,
    parse_frontmatter,
    validate_state,
)

CONTROL_PLAN = Path(
    "docs/plans/codefabric_model_driven_artifact_and_assurance_control_plane_implementation_plan_v1_2026-08-22.md"
)
SUCCESSOR_PLAN = Path(
    "docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v5_2026-08-22.md"
)
DATA_FABRIC_MIGRATION_PLAN = Path(
    "docs/plans/codefabric_data_fabric_datafusion55_arrow59_delta43a0cf10_implementation_plan_v1_2026-08-23.md"
)
POST_CONTROL_PLANS = frozenset({SUCCESSOR_PLAN, DATA_FABRIC_MIGRATION_PLAN})


@dataclass(frozen=True)
class TextRule:
    """One stable design ownership assertion over a normative owner."""

    rule_id: str
    path: str
    required: tuple[str, ...]


EVOLVED_DESIGN_INPUTS = frozenset(
    {
        "docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md",
        "docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md",
        "docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md",
        "docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md",
        "docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md",
    }
)

RULES = (
    TextRule(
        "detached-identities-and-distributed-trace",
        "docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md",
        (
            "computed identities are not authored fields",
            (
                "requirements.jsonl` and `traceability.jsonl` from\n"
                "the distributed declarations"
            ),
            "generated compatibility/provenance view",
            "owner-accepted release census",
            "Bundle membership, member identities, payload ordering",
        ),
    ),
    TextRule(
        "portable-external-driver-boundary",
        "docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md",
        (
            "protocol declares no network\ncapability",
            "source fence detects any repository write outside the declared\nstaging outputs",
            "defense in depth, not a cross-platform\nassumption",
        ),
    ),
    TextRule(
        "model-assurance-and-sealed-handoff",
        "docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md",
        (
            "Exactly one implementation plan and one schema-current execution state",
            "compiled assurance graph",
            "Packet-named mutation\ncampaigns are not completion evidence",
        ),
    ),
    TextRule(
        "recipe-aware-cbef",
        "docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md",
        (
            "generate one recipe-aware builder, validator, and typed field\nview per domain",
            "released `ENTITY` recipe remains exactly five fields",
            "released `RELATION_FACT` recipe remains exactly six fields",
        ),
    ),
    TextRule(
        "governed-occurrence-identity",
        "docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md",
        (
            "generated\nrecipe-aware CBEF builders",
            "Module-local numeric codes, bit masks, and\noverloaded flag bits",
        ),
    ),
    TextRule(
        "governed-provider-flags",
        "docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md",
        (
            "`provider_node_flags` is a persisted governed bitset",
            "Table row encoders accept the generated\nflag type rather than a raw integer",
        ),
    ),
)

FORBIDDEN_DESIGN_PHRASES = (
    "`contracts/manifests/suite-manifest.json` is the sole compiler bootstrap",
    "Machine sources may embed\n`canonical_digest`",
    "compare a machine-readable proof manifest before and after",
    "mutants-wp",
)


def _relative(path: Path, root: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def _active_state(root: Path, plan_path: Path) -> dict[str, object]:
    plan = parse_frontmatter(plan_path)
    state_path = root / str(plan["state_path"])
    return validate_state(root, state_path)


def validate_model_design_contract(
    root: Path = ROOT, plan_path: Path = CONTROL_PLAN
) -> dict[str, object]:
    """Validate the accepted WP01 ownership decisions and the sealed active-program handoff."""
    plan_path = plan_path if plan_path.is_absolute() else root / plan_path
    active = active_plan_path(root)
    post_control_paths = {root / path for path in POST_CONTROL_PLANS}
    allowed_active_paths = {plan_path.resolve()} | {
        path.resolve() for path in post_control_paths
    }
    if active.resolve() not in allowed_active_paths:
        raise ArtifactContractError(
            "active plan is outside the sealed model-control handoff"
        )
    plan = parse_frontmatter(plan_path)
    if plan.get("status") != "approved":
        raise ArtifactContractError("active model control plan is not approved")
    design_path = root / str(plan["design_path"])
    if parse_frontmatter(design_path).get("status") != "accepted":
        raise ArtifactContractError("model control design is not accepted")

    state = _active_state(root, plan_path)
    if state["plan_path"] != _relative(plan_path, root):
        raise ArtifactContractError("active state does not identify the active plan")
    if state["design_path"] != _relative(design_path, root):
        raise ArtifactContractError(
            "active state does not identify the accepted design"
        )
    if active.resolve() != plan_path.resolve():
        active_program = parse_frontmatter(active)
        if active_program.get("status") != "approved":
            raise ArtifactContractError("active post-control program is not approved")
        if (
            state.get("status") != "complete"
            or state.get("current_packet") is not None
            or state["packets"]["WP15"]["status"] != "complete"
            or state["milestones"]["M05"]["status"] != "complete"
            or state["decommission_batches"]["DB06"]["status"] != "complete"
        ):
            raise ArtifactContractError("model-control handoff seal is incomplete")

    evolutions = [
        deviation
        for deviation in state["plan_deviations"]
        if deviation.get("kind") == "planned_design_input_evolution"
        and deviation.get("packet") == "WP01"
    ]
    if len(evolutions) != 1 or set(evolutions[0].get("paths", [])) != set(
        EVOLVED_DESIGN_INPUTS
    ):
        raise ArtifactContractError(
            "WP01 planned input evolution must name exactly the five accepted owners"
        )

    suspended_value = plan.get("suspends_plan_path")
    if not isinstance(suspended_value, str):
        raise ArtifactContractError("active overlay must identify its suspended plan")
    suspended_plan = root / suspended_value
    if suspended_plan.resolve() == plan_path.resolve():
        raise ArtifactContractError("active plan cannot suspend itself")
    suspended_frontmatter = parse_frontmatter(suspended_plan)
    suspended_state = load_state(root / str(suspended_frontmatter["state_path"]))
    current = suspended_state.get("current_packet")
    if not isinstance(current, str):
        raise ArtifactContractError("suspended plan must retain its incomplete packet")
    entry = suspended_state.get("packets", {}).get(current)
    if not isinstance(entry, dict) or entry.get("proving_commit") is not None:
        raise ArtifactContractError("suspended current packet must remain unproved")

    checked_paths: set[str] = set()
    for rule in RULES:
        text = (root / rule.path).read_text(encoding="utf-8")
        missing = [phrase for phrase in rule.required if phrase not in text]
        if missing:
            raise ArtifactContractError(
                f"{rule.rule_id} is absent from {rule.path}: {missing!r}"
            )
        checked_paths.add(rule.path)
    combined = "\n".join(
        (root / path).read_text(encoding="utf-8")
        for path in sorted(EVOLVED_DESIGN_INPUTS)
    )
    present = [phrase for phrase in FORBIDDEN_DESIGN_PHRASES if phrase in combined]
    if present:
        raise ArtifactContractError(
            f"superseded control-plane doctrine remains present: {present!r}"
        )

    return {
        "active_plan": _relative(active, root),
        "control_plan": _relative(plan_path, root),
        "suspended_plan": _relative(suspended_plan, root),
        "evolved_design_inputs": sorted(EVOLVED_DESIGN_INPUTS),
        "rule_ids": [rule.rule_id for rule in RULES],
        "checked_paths": sorted(checked_paths),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--plan", type=Path, default=CONTROL_PLAN)
    args = parser.parse_args()
    try:
        report = validate_model_design_contract(args.root, args.plan)
    except (ArtifactContractError, OSError, KeyError, TypeError) as error:
        print(f"model design contract error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
