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

    assert assurance.validate_oracle_substance() == (92, 8)
    requirements = set(artifact_contracts.REVIEW_REQUIREMENTS)
    documented = artifact_contracts.documented_review_artifacts()
    assert documented == requirements


def test_wp54_structural_acceptance() -> None:
    packets, _ = assurance.validate_dependencies()
    assert packets == 23
    plan_path = artifact_contracts.active_plan_path()
    contracts = assurance._oracle_contracts(plan_path)
    assert len(contracts) == 23
    assert all(len(pairs) == 4 for pairs in contracts.values())


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
    assert assurance.validate_oracle_substance()[1] == 8
    assert assurance.validate_dependencies()[0] == 23
