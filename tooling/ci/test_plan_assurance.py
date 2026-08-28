from __future__ import annotations

import ast
import json
import subprocess
from pathlib import Path

import pytest
import yaml

from tooling.ci import artifact_contracts
from tooling.ci import design_principle_alignment as alignment
from tooling.ci import plan_assurance as assurance
from tooling.ci.test_artifact_contracts import (
    _init_repository,
    _write_activation_fixture,
)


def test_cycle_is_rejected() -> None:
    with pytest.raises(assurance.PlanAssuranceError, match="cycle"):
        assurance._topological_order({"WP01": {"WP02"}, "WP02": {"WP01"}})


def test_known_touch_parser_ignores_fenced_preflight_and_reads_owned_paths() -> None:
    block = (
        "**Change surface / Preflight / Known Touch.** Run:\n\n"
        "```bash\nrg -n 'symbol' src/irrelevant.rs\n```\n\n"
        "Known touch: `src/owned.rs`, `contracts/example.yaml`.\n\n"
        "**Required changes.**\n"
    )
    assert assurance._known_touch_resources(block) == {
        "src/owned.rs",
        "contracts/example.yaml",
    }


def test_ontology_fabric_release_barrier_is_structural() -> None:
    plan = artifact_contracts.active_plan_path()
    dependencies = assurance._dependency_map(plan)
    assert assurance._validate_ontology_fabric_readiness_states(dependencies) == 60
    dependencies["WP17"].remove("WP16")
    with pytest.raises(assurance.PlanAssuranceError, match="release-barrier"):
        assurance._validate_ontology_fabric_readiness_states(dependencies)


def test_packet_assurance_remains_runnable_during_declared_input_evolution(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def forbidden_freshness_check(*_args: object, **_kwargs: object) -> None:
        raise AssertionError("packet assurance invoked the completion-freshness gate")

    monkeypatch.setattr(
        artifact_contracts,
        "validate_artifacts",
        forbidden_freshness_check,
    )
    plan_path, plan, state = assurance._active()
    assert plan_path == artifact_contracts.active_plan_path()
    assert plan["plan_id"] == "codefabric-ontology-compiled-data-fabric"
    assert state["status"] == "executing"


def test_single_call_python_alias_is_rejected(tmp_path: Path) -> None:
    source = tmp_path / "tooling" / "test_alias.py"
    source.parent.mkdir()
    source.write_text(
        "def test_wp54_behavioral_acceptance():\n    another_test()\n",
        encoding="utf-8",
    )
    with pytest.raises(assurance.PlanAssuranceError, match="single-call Python alias"):
        assurance._python_definitions(tmp_path, {"wp54_behavioral_acceptance"})


def test_literal_only_oracle_is_not_a_definition(tmp_path: Path) -> None:
    source = tmp_path / "tooling" / "literal.py"
    source.parent.mkdir()
    source.write_text('VALUE = "wp54_behavioral_acceptance"\n', encoding="utf-8")
    assert assurance.oracle_definitions(tmp_path, {"wp54_behavioral_acceptance"}) == []


def test_literal_only_function_is_not_a_definition(tmp_path: Path) -> None:
    source = tmp_path / "tooling" / "literal_function.py"
    source.parent.mkdir()
    source.write_text(
        'def test_wp54_behavioral_acceptance():\n    "placeholder only"\n    pass\n',
        encoding="utf-8",
    )
    assert assurance.oracle_definitions(tmp_path, {"wp54_behavioral_acceptance"}) == []


def test_vacuous_future_state_does_not_satisfy_oracle_definition(
    tmp_path: Path,
) -> None:
    source = tmp_path / "tests" / "placeholder.rs"
    source.parent.mkdir()
    source.write_text("// wp54_structural_acceptance\n", encoding="utf-8")
    assert assurance.oracle_definitions(tmp_path, {"wp54_structural_acceptance"}) == []


def test_commented_rust_function_is_not_a_definition(tmp_path: Path) -> None:
    source = tmp_path / "tests" / "placeholder.rs"
    source.parent.mkdir()
    source.write_text(
        "// fn wp54_structural_acceptance() { assert!(true); }\n",
        encoding="utf-8",
    )
    assert assurance.oracle_definitions(tmp_path, {"wp54_structural_acceptance"}) == []


def test_wp54_behavioral_acceptance(tmp_path: Path) -> None:
    success = tmp_path / "success"
    success.mkdir()
    _init_repository(success)
    approved = _write_activation_fixture(success, status="approved")
    report = artifact_contracts.activate_plan(success, approved)
    assert report["plan"] == "plan.md"
    assert artifact_contracts.active_plan_path(success) == approved
    artifact_contracts.validate_state(
        success,
        success / "state.json",
        expected_ids=artifact_contracts.plan_ids(approved),
    )

    failure = tmp_path / "failure"
    failure.mkdir()
    _init_repository(failure)
    pointer = failure / artifact_contracts.ACTIVE_PLAN_POINTER
    pointer.parent.mkdir(parents=True)
    prior = {"schema_version": 1, "plan_path": "prior.md"}
    pointer.write_text(json.dumps(prior) + "\n", encoding="utf-8")
    (failure / "prior.md").write_text("prior\n", encoding="utf-8")
    draft = _write_activation_fixture(failure, status="draft")
    with pytest.raises(artifact_contracts.ArtifactContractError):
        artifact_contracts.activate_plan(failure, draft)
    assert json.loads(pointer.read_text(encoding="utf-8")) == prior
    assert not (failure / "state.json").exists()

    declared, implemented = assurance.validate_oracle_substance()
    state = assurance._active()[2]
    required_packets = sum(
        entry["status"] in {"in_progress", "complete"}
        for entry in state["packets"].values()
    )
    assert declared == len(state["packets"]) * 4
    assert implemented == required_packets * 4
    requirements = set(artifact_contracts.REVIEW_REQUIREMENTS)
    documented = artifact_contracts.documented_review_artifacts()
    assert documented == requirements


def test_wp54_structural_acceptance() -> None:
    packets, _ = assurance.validate_dependencies()
    plan_path = artifact_contracts.active_plan_path()
    contracts = assurance._oracle_contracts(plan_path)
    assert packets == len(contracts)
    assert all(len(pairs) == 4 for pairs in contracts.values())


def test_ordered_oracle_catalog_gets_stable_criterion_ids(tmp_path: Path) -> None:
    plan = tmp_path / "plan.md"
    plan.write_text(
        "### WP01 — Example\n\n"
        "**Target invariants.** GI-01.\n\n"
        "**Design and library references.** QRY §1.\n\n"
        "**Acceptance Checks.**\n\n"
        "Oracle catalog: Executable oracle: `example_behavior`; "
        "Executable oracle: `example_structure`; Executable oracle: `example_negative`; "
        "Executable oracle: `example_operation`.\n\n"
        "- **Behavioral — Executable oracle:** example.\n",
        encoding="utf-8",
    )
    assert assurance._oracle_contracts(plan)["WP01"] == [
        ("example_behavior", "PC-WP01-BEH"),
        ("example_structure", "PC-WP01-STR"),
        ("example_negative", "PC-WP01-NEG"),
        ("example_operation", "PC-WP01-OPS"),
    ]


def test_multiline_oracle_catalog_stops_at_edit_local_gates(tmp_path: Path) -> None:
    plan = tmp_path / "plan.md"
    plan.write_text(
        "### WP01 — Example\n\n"
        "**Target invariants.** GI-01.\n\n"
        "**Design and library references.** QRY §1.\n\n"
        "**Acceptance Checks.**\n\n"
        "Oracle catalog:\n\n"
        "- Executable oracle: `example_behavior`\n"
        "- Executable oracle: `example_structure`\n"
        "- Executable oracle: `example_negative`\n"
        "- Executable oracle: `example_operation`\n\n"
        "**Edit-Local Gates.** Focused tests.\n",
        encoding="utf-8",
    )
    assert assurance._oracle_contracts(plan)["WP01"] == [
        ("example_behavior", "PC-WP01-BEH"),
        ("example_structure", "PC-WP01-STR"),
        ("example_negative", "PC-WP01-NEG"),
        ("example_operation", "PC-WP01-OPS"),
    ]


def test_wp54_negative_zero_state(tmp_path: Path) -> None:
    tree = ast.parse("def test_wp54_negative_zero_state():\n    alias()\n")
    function = tree.body[0]
    assert isinstance(function, ast.FunctionDef)
    assert assurance._python_alias(assurance._python_body(function))
    with pytest.raises(assurance.PlanAssuranceError, match="cycle"):
        assurance._topological_order({"WP54": {"WP55"}, "WP55": {"WP54"}})
    with pytest.raises(assurance.PlanAssuranceError, match="lacks definitions"):
        assurance._require_exact_definitions(
            {"wp54_negative_zero_state"},
            [],
            context="zero-match selector",
        )
    rust_selector = assurance._rust_selector_command(
        "root", ["wp54_negative_zero_state"]
    )
    assert rust_selector[-1] == "--no-tests=fail"

    repository = tmp_path / "unowned-deletion"
    repository.mkdir()
    _init_repository(repository)
    skill = repository / "skills" / "example" / "SKILL.md"
    skill.parent.mkdir(parents=True)
    skill.write_text("owned skill\n", encoding="utf-8")
    subprocess.run(("git", "add", "skills"), cwd=repository, check=True)
    subprocess.run(
        (
            "git",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "add skill",
        ),
        cwd=repository,
        check=True,
        capture_output=True,
    )
    baseline = subprocess.run(
        ("git", "rev-parse", "HEAD"),
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    skill.unlink()
    registry = repository / alignment.BASELINE_REGISTRY
    registry.parent.mkdir(parents=True, exist_ok=True)
    registry.write_text(
        yaml.safe_dump({"baseline_commit": baseline, "records": []}),
        encoding="utf-8",
    )
    with pytest.raises(
        alignment.AlignmentContractError,
        match="unattributed dirty paths",
    ):
        alignment.validate_baseline(repository)


def test_wp54_operational_acceptance() -> None:
    implemented = assurance.validate_oracle_substance()[1]
    state = assurance._active()[2]
    required_packets = sum(
        entry["status"] in {"in_progress", "complete"}
        for entry in state["packets"].values()
    )
    assert implemented == required_packets * 4
    assert assurance.validate_dependencies()[0] == len(state["packets"])
