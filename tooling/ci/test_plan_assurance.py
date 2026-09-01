from __future__ import annotations

import ast
import json
from pathlib import Path

import pytest

from tooling.ci import artifact_contracts
from tooling.ci import plan_assurance as assurance
from tooling.ci.test_artifact_contracts import (
    _init_repository,
    _write_activation_fixture,
)


def test_cycle_is_rejected() -> None:
    with pytest.raises(assurance.PlanAssuranceError, match="cycle"):
        assurance._topological_order({"WP01": {"WP02"}, "WP02": {"WP01"}})


def test_first_packet_narrative_does_not_override_self_dependency(
    tmp_path: Path,
) -> None:
    plan = tmp_path / "plan.md"
    plan.write_text(
        "### WP01 — First\n\n"
        "**Dependencies.** WP01 is the first packet in this DAG.\n\n"
        "**Target invariants.** I-01.\n",
        encoding="utf-8",
    )
    with pytest.raises(assurance.PlanAssuranceError, match="depends on itself"):
        assurance._dependency_map(plan)


def test_actual_self_dependency_is_rejected(tmp_path: Path) -> None:
    plan = tmp_path / "plan.md"
    plan.write_text(
        "### WP01 — Invalid\n\n"
        "**Dependencies.** WP01.\n\n"
        "**Target invariants.** I-01.\n",
        encoding="utf-8",
    )
    with pytest.raises(assurance.PlanAssuranceError, match="depends on itself"):
        assurance._dependency_map(plan)


def test_overlap_dispositions_are_plan_qualified() -> None:
    dispositions = assurance._load_overlap_dispositions(assurance.ROOT)
    assert dispositions
    assert all(plan_id for plan_id, _, _ in dispositions)
    assert all(
        plan_id != "codefabric-execution-proved-relational-data-fabric"
        for plan_id, _, _ in dispositions
    )


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
    assert plan["plan_id"] == "codefabric-execution-proved-relational-data-fabric"
    assert state["status"] in {"executing", "complete"}


def test_packet_oracle_can_select_an_immutable_inactive_plan(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    plan = tmp_path / "docs" / "plans" / "inactive.md"
    plan.parent.mkdir(parents=True)
    plan.write_text("inactive plan\n", encoding="utf-8")
    observed: list[Path] = []

    def contracts(
        selected: Path,
        **options: object,
    ) -> dict[str, list[tuple[str, str]]]:
        observed.append(selected)
        assert options == {
            "selected_packets": {"WP05"},
            "allow_legacy_mapping": True,
        }
        return {
            "WP05": [
                ("one", "PC-WP05-INT"),
                ("two", "PC-WP05-BEH"),
                ("three", "PC-WP05-NEG"),
                ("four", "PC-WP05-OPS"),
            ]
        }

    monkeypatch.setattr(assurance, "_oracle_contracts", contracts)
    monkeypatch.setattr(assurance, "oracle_definitions", lambda *_args: [])
    monkeypatch.setattr(
        assurance,
        "_require_exact_definitions",
        lambda *_args, **_kwargs: {},
    )

    assurance.run_packet_oracles(
        "WP05",
        root=tmp_path,
        plan_path=Path("docs/plans/inactive.md"),
    )

    assert observed == [plan.resolve()]


def test_dependency_check_can_select_the_active_plan_by_path() -> None:
    plan_path = artifact_contracts.active_plan_path()
    relative = plan_path.relative_to(assurance.ROOT)
    assert assurance.validate_dependencies(plan_path=relative) == (
        assurance.validate_dependencies()
    )
    assert assurance.main(["dependency-check", str(relative)]) == 0


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


def test_hyphenated_oracle_is_discovered_as_a_substantive_just_recipe(
    tmp_path: Path,
) -> None:
    (tmp_path / "justfile").write_text(
        "substantive-oracle:\n    @printf 'proof\\n' >/dev/null\n",
        encoding="utf-8",
    )
    assert assurance.oracle_definitions(tmp_path, {"substantive-oracle"}) == [
        assurance.OracleDefinition(
            "substantive-oracle",
            "just",
            "justfile",
            "substantive-oracle",
        )
    ]


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
        ("example_behavior", "PC-WP01-INT"),
        ("example_structure", "PC-WP01-BEH"),
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
        ("example_behavior", "PC-WP01-INT"),
        ("example_structure", "PC-WP01-BEH"),
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


def test_wp54_operational_acceptance() -> None:
    implemented = assurance.validate_oracle_substance()[1]
    state = assurance._active()[2]
    required_packets = sum(
        entry["status"] in {"in_progress", "complete"}
        for entry in state["packets"].values()
    )
    assert implemented == required_packets * 4
    assert assurance.validate_dependencies()[0] == len(state["packets"])
