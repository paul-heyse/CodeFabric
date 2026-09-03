"""Acceptance and falsification tests for derived v7 terminal certification."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from tooling.ci import artifact_contracts, plan_assurance
from tooling.ci.v7_certification import (
    FINAL_MATRIX_END,
    FINAL_MATRIX_START,
    ROOT,
    SELF_RECIPE,
    CertificationCommand,
    V7CertificationError,
    _report,
    assert_seeded_failure_propagates,
    derive_final_matrix,
    derive_model,
    derive_packet_acceptance_commands,
    execute_commands,
)


def test_v7_certification_artifact_and_ancestry_integrity() -> None:
    model = derive_model(ROOT)
    plan_path = ROOT / model.plan_path
    contracts = plan_assurance._oracle_contracts(plan_path)
    state = artifact_contracts.load_state(
        ROOT / str(artifact_contracts.parse_frontmatter(plan_path)["state_path"])
    )

    assert set(contracts) == set(state["packets"])
    assert all(
        len(values) == len(plan_assurance.ORACLE_KINDS) for values in contracts.values()
    )
    assert model.packet_oracle_count == sum(
        len(values) for values in contracts.values()
    )
    assert model.terminal_packet in contracts
    assert all(
        artifact_contracts.commit_trust(ROOT, entry["proving_commit"])["ancestor"]
        for packet, entry in state["packets"].items()
        if packet != model.terminal_packet or entry["status"] == "complete"
    )


def test_v7_complete_successor_behavior_matrix() -> None:
    model = derive_model(ROOT)
    matrix = [command.argv for command in model.final_matrix]
    executable = [command.argv for command in model.executable_commands]
    retained = [command.argv[1] for command in model.retained_v5]

    assert matrix[-1] == ("just", SELF_RECIPE)
    assert matrix.count(("just", SELF_RECIPE)) == 1
    assert all(command in executable for command in matrix[:-1])
    assert len(model.packet_selectors) == len(
        artifact_contracts._packet_blocks(ROOT / model.plan_path)
    )
    assert len(retained) == len(set(retained))
    recipes = artifact_contracts.load_just_recipes(ROOT)
    assert all(name in recipes for name in retained)
    assert all(
        command.source == "retained-v5-functional-oracle"
        for command in model.retained_v5
    )
    packet_acceptance = derive_packet_acceptance_commands(
        ROOT / model.plan_path,
        model.final_matrix,
        recipes,
    )
    assert packet_acceptance == model.packet_acceptance
    assert all(command.argv in executable for command in packet_acceptance)
    assert not {command.argv for command in packet_acceptance} & set(matrix)


def test_v7_legacy_and_failure_masking_zero_state() -> None:
    model = derive_model(ROOT)
    matrix_recipes = {command.argv[1] for command in model.final_matrix}
    commands = (
        CertificationCommand("fixture", ("just", "before")),
        CertificationCommand("fixture", ("just", "legacy-fault")),
        CertificationCommand("fixture", ("just", "after")),
    )
    observed: list[tuple[str, ...]] = []

    def runner(command: CertificationCommand) -> int:
        observed.append(command.argv)
        return 29 if command.argv[-1] == "legacy-fault" else 0

    results = execute_commands(commands, runner)
    assert observed == [command.argv for command in commands]
    assert [result.exit_code for result in results] == [0, 29, 0]
    assert not all(result.exit_code == 0 for result in results)
    assert "compiled-release-legacy-zero-state-check" in matrix_recipes
    assert_seeded_failure_propagates()


def test_v7_terminal_release_certification() -> None:
    model = derive_model(ROOT)
    results = execute_commands(model.executable_commands, lambda _command: 0)
    report = _report(
        model,
        "0" * 40,
        "2026-09-03T00:00:00+00:00",
        results,
        True,
    )

    assert report["success"] is True
    assert report["executed_child_count"] == len(model.executable_commands)
    assert report["logical_child_count"] == len(model.executable_commands) + 1
    assert report["packet_oracle_count"] == len(model.packet_selectors) * len(
        plan_assurance.ORACLE_KINDS
    )
    assert all(result["exit_code"] == 0 for result in report["results"])


def test_final_matrix_rejects_an_empty_selector(tmp_path: Path) -> None:
    plan = tmp_path / "plan.md"
    plan.write_text(
        f"{FINAL_MATRIX_START}\n\nNo commands.\n\n{FINAL_MATRIX_END}\n",
        encoding="utf-8",
    )
    with pytest.raises(V7CertificationError, match="selects no commands"):
        derive_final_matrix(plan, {})


def test_final_matrix_rejects_an_unknown_recipe(tmp_path: Path) -> None:
    plan = tmp_path / "plan.md"
    plan.write_text(
        f"{FINAL_MATRIX_START}\n\n"
        "- `just missing-child`\n"
        f"- `just {SELF_RECIPE}`\n\n"
        f"{FINAL_MATRIX_END}\n",
        encoding="utf-8",
    )
    recipes = {SELF_RECIPE: {"dependencies": []}}
    with pytest.raises(V7CertificationError, match="unknown recipes"):
        derive_final_matrix(plan, recipes)


def test_failed_report_retains_every_child_status() -> None:
    model = derive_model(ROOT)
    target = model.executable_commands[1]
    results = execute_commands(
        model.executable_commands,
        lambda command: 41 if command == target else 0,
    )
    report = _report(
        model,
        "0" * 40,
        "2026-09-03T00:00:00+00:00",
        results,
        True,
    )

    assert report["success"] is False
    assert report["executed_child_count"] == len(model.executable_commands)
    assert [result["exit_code"] for result in report["results"]].count(41) == 1
    assert json.loads(json.dumps(report))["success"] is False
