"""Independent seeded proof of plan-derived real-time CPG orchestration."""

from __future__ import annotations

import os
import subprocess
import sys
from dataclasses import replace
from pathlib import Path

import pytest

from tooling.ci import artifact_contracts, plan_assurance
from tooling.ci import real_time_cpg_assurance as assurance
from tooling.ci.test_artifact_contracts import (
    _init_repository,
    _write_activation_fixture,
)


def _packet(number: int, gates: str = "`just local-proof`") -> str:
    packet = f"WP{number:02}"
    oracles = "\n\n".join(
        f"Executable oracle: `fixture_{packet.lower()}_{kind.lower()}`\n"
        f"Governed criterion: `PC-{packet}-{kind}`"
        for kind in plan_assurance.ORACLE_KINDS
    )
    return (
        f"### {packet} — Fixture\n\n"
        "**Dependencies.** None.\n\n"
        "**Target invariants.** Independent source expectations.\n\n"
        "**Design and library references.** Fixture contract.\n\n"
        f"{oracles}\n\n"
        f"**Packet-Local Gates.** `just real-time-cpg-packet-check {packet}`; {gates}.\n\n"
        "**Integration Milestone.** M01.\n\n"
    )


def _oracles(root: Path, number: int, body: str | None = None) -> Path:
    source = root / "tooling" / f"test_oracles_{number}.py"
    source.parent.mkdir(exist_ok=True)
    statements = (
        body or "    observed = sorted([3, 1, 2])\n    assert observed == [1, 2, 3]\n"
    )
    source.write_text(
        "\n".join(
            f"def test_fixture_wp{number:02}_{kind.lower()}():\n{statements}"
            for kind in plan_assurance.ORACLE_KINDS
        ),
        encoding="utf-8",
    )
    return source


def _fixture(root: Path, *, future: bool = False) -> assurance.Plan:
    _init_repository(root)
    path = _write_activation_fixture(root, status="draft")
    front = path.read_text(encoding="utf-8").split("## 4. Work packets", 1)[0]
    front = front.replace("plan_id: fixture", f"plan_id: {assurance.PLAN_ID}")
    body = _packet(1) + (_packet(2, "`just future-proof`") if future else "")
    path.write_text(
        front + "## 4. Work packets\n\n" + body + "## 5. Integration milestones\n\n"
        "### M01 — Fixture\n\n"
        "Members: WP01. Execute member oracles plus `just integration-proof`. "
        "Invocation: `just real-time-cpg-milestone-check M01`.\n\n"
        "## 6. Cross-packet decommission batches\n\n"
        "### DB01 — Fixture\n\n"
        "Prerequisites: WP01. Target-positive and negative checks.\n\n"
        "Exit: `just real-time-cpg-decommission-check DB01`, `just deletion-proof`.\n\n"
        "## 7. Layered gate matrix\n\n"
        "### 7.3 Final nonmutating leaf gates\n\n"
        "- `just final-proof`\n\n"
        "### 7.4 Completion evidence\n\n"
        "## 8. Execution sequence\n",
        encoding="utf-8",
    )
    (root / "justfile").write_text(
        "local-proof:\n    @test 4 -eq 4\n\n"
        "integration-proof:\n    @test 5 -eq 5\n\n"
        "deletion-proof:\n    @test 6 -eq 6\n\n"
        "final-proof:\n    @test 7 -eq 7\n",
        encoding="utf-8",
    )
    _oracles(root, 1)
    return assurance.load_plan(root, path)


def _recipe(*dependencies: str, body: str = "test 3 -eq 3") -> dict[str, object]:
    return {
        "attributes": [],
        "dependencies": [{"recipe": value, "arguments": []} for value in dependencies],
        "body": [[body]] if body else [],
        "parameters": [],
    }


def test_inactive_draft_no_state_and_future_oracles_do_not_block_first_packet(
    tmp_path: Path,
) -> None:
    plan = _fixture(tmp_path, future=True)
    commands = assurance.derive_commands(tmp_path, plan, "packet", "WP01")
    assert len(commands) == 5
    assert all("WP02" not in command.source for command in commands)
    assert commands[-1].argv == ("just", "local-proof")
    assert not (tmp_path / "state.json").exists()
    assert not (tmp_path / artifact_contracts.ACTIVE_PLAN_POINTER).exists()


def test_missing_future_packet_oracles_fail_selection(tmp_path: Path) -> None:
    plan = _fixture(tmp_path, future=True)
    with pytest.raises(plan_assurance.PlanAssuranceError, match="lacks definitions"):
        assurance.derive_commands(tmp_path, plan, "packet", "WP02")


@pytest.mark.parametrize(
    "body", ["    pass\n", "    return True\n", '    "not a test"\n']
)
def test_no_op_oracles_do_not_satisfy_selection(tmp_path: Path, body: str) -> None:
    plan = _fixture(tmp_path)
    _oracles(tmp_path, 1, body)
    with pytest.raises(plan_assurance.PlanAssuranceError, match="lacks definitions"):
        assurance.derive_commands(tmp_path, plan, "packet", "WP01")


def test_duplicate_and_single_call_alias_oracles_fail(tmp_path: Path) -> None:
    plan = _fixture(tmp_path)
    source = _oracles(tmp_path, 1)
    duplicate = source.with_name("test_duplicate.py")
    duplicate.write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
    with pytest.raises(
        plan_assurance.PlanAssuranceError, match="duplicate definitions"
    ):
        assurance.derive_commands(tmp_path, plan, "packet", "WP01")
    duplicate.write_text("", encoding="utf-8")
    _oracles(tmp_path, 1, "    another_test()\n")
    with pytest.raises(plan_assurance.PlanAssuranceError, match="single-call"):
        assurance.derive_commands(tmp_path, plan, "packet", "WP01")


def test_milestone_and_decommission_derive_actual_members_and_leaf_gates(
    tmp_path: Path,
) -> None:
    plan = _fixture(tmp_path)
    milestone = assurance.derive_commands(tmp_path, plan, "milestone", "M01")
    batch = assurance.derive_commands(tmp_path, plan, "decommission", "DB01")
    assert len(milestone) == len(batch) == 6
    assert milestone[-1].argv == ("just", "integration-proof")
    assert batch[-1].argv == ("just", "deletion-proof")
    changed = dict(plan.blocks)
    changed["M01"] = changed["M01"].replace("WP01", "WP99")
    with pytest.raises(assurance.AssuranceError, match="unknown packet members"):
        assurance.derive_commands(
            tmp_path, replace(plan, blocks=changed), "milestone", "M01"
        )


def test_changed_milestone_member_is_executed_not_cached(tmp_path: Path) -> None:
    plan = _fixture(tmp_path, future=True)
    _oracles(tmp_path, 2)
    blocks = dict(plan.blocks)
    blocks["M01"] = blocks["M01"].replace("Members: WP01.", "Members: WP01, WP02.")
    recipes = artifact_contracts.load_just_recipes(tmp_path)
    recipes["future-proof"] = _recipe()
    commands = assurance.derive_commands(
        tmp_path, replace(plan, blocks=blocks), "milestone", "M01", recipes
    )
    assert (
        len([command for command in commands if command.source.startswith("WP02:")])
        == 4
    )
    assert ("just", "future-proof") in [command.argv for command in commands]


def test_recipe_dependency_and_body_cycles_are_rejected() -> None:
    for recipes in (
        {"first": _recipe("second"), "second": _recipe("first")},
        {"first": _recipe(body="just second"), "second": _recipe(body="just first")},
    ):
        with pytest.raises(assurance.AssuranceError, match="recursive recipe graph"):
            assurance._check_recipe_graph([("just", "first")], recipes)


@pytest.mark.parametrize("recipe", list(assurance.DISPATCHERS))
def test_indirect_dispatcher_reentry_is_rejected(recipe: str) -> None:
    with pytest.raises(assurance.AssuranceError, match="recursive dispatcher"):
        assurance._check_recipe_graph([("just", "first")], {"first": _recipe(recipe)})


def test_unknown_mutating_and_predecessor_commands_fail_closed() -> None:
    with pytest.raises(assurance.AssuranceError, match="absent"):
        assurance._check_recipe_graph([("just", "undefined")], {})
    mutating = _recipe()
    mutating["attributes"] = [{"group": "mutating"}]
    with pytest.raises(assurance.AssuranceError, match="mutating"):
        assurance._check_recipe_graph([("just", "repair")], {"repair": mutating})
    with pytest.raises(assurance.AssuranceError, match="mutating"):
        assurance._check_recipe_graph(
            [("just", "sample-capture")], {"sample-capture": _recipe()}
        )
    with pytest.raises(assurance.AssuranceError, match="predecessor"):
        assurance._check_recipe_graph(
            [("just", "relational-fabric-v7-certification")],
            {"relational-fabric-v7-certification": _recipe()},
        )


def test_child_failure_and_zero_work_cannot_be_reported_green() -> None:
    calls = []
    commands = tuple(
        assurance.Command("fixture", ("just", name))
        for name in ("before", "failure", "after")
    )

    def run(command: assurance.Command) -> int:
        calls.append(command.argv[-1])
        return 29 if command.argv[-1] == "failure" else 0

    assert assurance.execute_commands(commands, run) == 29
    assert calls == ["before", "failure"]
    with pytest.raises(assurance.AssuranceError, match="zero"):
        assurance.execute_commands([], run)


def test_no_op_recipe_and_raw_module_recursion_are_rejected() -> None:
    for body in ("true", "@:", "exit 0"):
        with pytest.raises(assurance.AssuranceError, match="no-op"):
            assurance._check_recipe_graph(
                [("just", "noop")], {"noop": _recipe(body=body)}
            )
    for body in (
        "python tooling/ci/real_time_cpg_assurance.py certification",
        "python -m tooling.ci.real_time_cpg_assurance certification",
    ):
        with pytest.raises(
            assurance.AssuranceError, match="recursive assurance module"
        ):
            assurance._check_recipe_graph(
                [("just", "indirect")], {"indirect": _recipe(body=body)}
            )
    assurance._check_recipe_graph(
        [("just", "tests")],
        {
            "tests": _recipe(
                body="python -m pytest tooling/ci/test_real_time_cpg_assurance.py"
            )
        },
    )


def test_nested_recipe_flags_cannot_hide_a_mutating_capture() -> None:
    recipes = {"indirect": _recipe(body="just --yes real-time-cpg-performance-capture")}
    with pytest.raises(assurance.AssuranceError, match="unresolved nested Just"):
        assurance._check_recipe_graph([("just", "indirect")], recipes)


def test_process_recursion_guard_fails_before_any_child(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv(assurance.STACK_ENV, "certification:")
    assert assurance.main(["packet", "WP01"]) == 1


def test_candidate_requires_trusted_prior_batches_and_clean_tree(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    plan = _fixture(tmp_path)
    blocks = dict(plan.blocks)
    blocks["WP01"] = blocks["WP01"].replace(
        "`just local-proof`", "`just real-time-cpg-certification`"
    )
    plan = replace(plan, blocks=blocks)
    state = {
        "status": "executing",
        "current_packet": "WP01",
        "packets": {"WP01": {"status": "in_progress"}},
        "milestones": {"M01": {"status": "in_progress"}},
        "decommission_batches": {"DB01": {"status": "complete"}},
    }
    status = {"healthy": True, "decommission_batches": {"DB01": {"trusted": False}}}
    monkeypatch.setattr(artifact_contracts, "active_plan_path", lambda _root: plan.path)
    monkeypatch.setattr(artifact_contracts, "validate_plan", lambda *_args: plan.values)
    monkeypatch.setattr(
        artifact_contracts, "validate_state", lambda *_args, **_kwargs: state
    )
    monkeypatch.setattr(artifact_contracts, "derive_plan_status", lambda *_args: status)
    with pytest.raises(
        assurance.AssuranceError, match="DB01 is not complete and trusted"
    ):
        assurance._validate_candidate(tmp_path, plan)
    status["decommission_batches"]["DB01"]["trusted"] = True
    assurance._validate_candidate(tmp_path, plan)
    commands = [assurance.Command("fixture", ("just", "proof"))]
    assert (
        assurance.execute_certification(tmp_path, plan, commands, lambda _command: 0)
        == 0
    )

    def mutate_source(_command: assurance.Command) -> int:
        (tmp_path / "README.md").write_text("seeded child mutation\n", encoding="utf-8")
        return 0

    with pytest.raises(assurance.AssuranceError, match="tracked changes"):
        assurance.execute_certification(tmp_path, plan, commands, mutate_source)
    subprocess.run(("git", "add", "README.md"), cwd=tmp_path, check=True)
    subprocess.run(
        ("git", "commit", "-q", "-m", "seeded change"), cwd=tmp_path, check=True
    )

    def advance_head(_command: assurance.Command) -> int:
        subprocess.run(
            ("git", "commit", "-q", "--allow-empty", "-m", "seeded concurrent commit"),
            cwd=tmp_path,
            check=True,
        )
        return 0

    with pytest.raises(assurance.AssuranceError, match="HEAD changed"):
        assurance.execute_certification(tmp_path, plan, commands, advance_head)
    assert (
        assurance.execute_certification(tmp_path, plan, commands, lambda _command: 29)
        == 29
    )
    subprocess.run(("git", "add", "plan.md"), cwd=tmp_path, check=True)
    with pytest.raises(assurance.AssuranceError, match="tracked changes"):
        assurance._validate_candidate(tmp_path, plan)


def test_packet_ids_and_oracle_contracts_are_not_fuzzy_matches(tmp_path: Path) -> None:
    plan = _fixture(tmp_path)
    for identifier in ("WP", "WP010", "M01", "WP01;true"):
        with pytest.raises(assurance.AssuranceError, match="invalid|unknown"):
            assurance.derive_commands(tmp_path, plan, "packet", identifier)
    changed = plan.path.read_text(encoding="utf-8").replace(
        "PC-WP01-OPS", "PC-WP01-INT"
    )
    plan.path.write_text(changed, encoding="utf-8")
    with pytest.raises(plan_assurance.PlanAssuranceError, match="criterion"):
        assurance.load_plan(tmp_path, plan.path)


def test_rust_selector_anchors_each_oracle_and_fails_zero_matches(
    tmp_path: Path,
) -> None:
    plan = _fixture(tmp_path)
    _oracles(tmp_path, 1, "    pass\n")
    source = tmp_path / "src" / "lib.rs"
    source.parent.mkdir()
    source.write_text(
        "\n".join(
            f"fn {oracle}() {{ let value = 3; assert_eq!(value, 3); }}"
            for oracle, _criterion in plan.contracts["WP01"]
        ),
        encoding="utf-8",
    )
    commands = assurance._definition_commands(tmp_path, plan, "WP01")
    assert len(commands) == 4
    for command, (oracle, _criterion) in zip(
        commands, plan.contracts["WP01"], strict=True
    ):
        assert command.argv[-1] == "--no-tests=fail"
        assert command.argv[-2] == f"test(/(^|::){oracle}$/)"


def test_source_plan_milestone_dependencies_do_not_form_hidden_cycles(
    tmp_path: Path,
) -> None:
    plan = _fixture(tmp_path)
    blocks = dict(plan.blocks)
    blocks["M01"] = "Members: WP01; prerequisite M02. Execute member oracles."
    blocks["M02"] = "Members: WP01; prerequisite M01. Execute member oracles."
    with pytest.raises(assurance.AssuranceError, match="recursive obligation"):
        assurance.derive_commands(
            tmp_path, replace(plan, blocks=blocks), "milestone", "M01"
        )


def test_aggregate_expansion_preserves_bodies_and_executes_shared_dependency_once() -> (
    None
):
    recipes = {
        "shared": _recipe(),
        "pure": _recipe("shared", body=""),
        "mixed": _recipe("pure", body="test 7 -eq 7"),
    }
    commands = [
        assurance.Command("fixture", ("just", name))
        for name in ("pure", "mixed", "shared")
    ]
    assurance._check_recipe_graph([command.argv for command in commands], recipes)
    result = assurance._flatten_aggregates(commands, recipes)
    assert [command.argv for command in result] == [
        ("just", "shared"),
        ("just", "--no-deps", "mixed"),
    ]
    recipes["shared"]["body"] = []
    with pytest.raises(assurance.AssuranceError, match="empty recipe"):
        assurance._flatten_aggregates(commands, recipes)


def test_terminal_expansion_retains_every_packet_milestone_db_and_final_leaf(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    plan = _fixture(tmp_path)
    blocks = dict(plan.blocks)
    blocks["M02"] = (
        "Member: WP01; prerequisite M01. Execute member oracles, the decommission checks, "
        "all §7 leaf gates. Invocation: `just real-time-cpg-milestone-check M02`. "
        "The terminal `just real-time-cpg-certification` orchestrates this graph."
    )
    values = {
        **plan.values,
        "ids": {**plan.values["ids"], "milestones": ["M01", "M02"]},
    }
    monkeypatch.setattr(assurance, "retained_commands", lambda *_args: ())
    commands = assurance.derive_commands(
        tmp_path, replace(plan, blocks=blocks, values=values), "certification", None
    )
    expected = {
        ("just", value)
        for value in (
            "local-proof",
            "integration-proof",
            "deletion-proof",
            "final-proof",
        )
    }
    assert expected <= {command.argv for command in commands}
    assert len(commands) == 8
    assert not any(
        command.argv[1] in assurance.DISPATCHERS
        for command in commands
        if command.argv[0] == "just"
    )


def test_exact_predecessor_authority_retains_real_oracles_but_not_old_terminal_or_performance() -> (
    None
):
    path = (
        assurance.ROOT
        / "docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v7_2026-09-02.md"
    )
    plan = assurance.Plan(
        assurance.ROOT / "unused",
        {"supersedes_on_activation": path.relative_to(assurance.ROOT).as_posix()},
        {},
        {},
        {},
        (),
    )
    recipes = artifact_contracts.load_just_recipes(assurance.ROOT)
    commands = assurance.retained_commands(assurance.ROOT, plan, recipes)
    contracts = plan_assurance._oracle_contracts(path)
    blocks = artifact_contracts._packet_blocks(path)
    replaced = {
        "compiled-release-resource-performance-check",
        "relational-fabric-v7-certification",
    }
    owners = {
        packet
        for packet, block in blocks.items()
        if any(
            ("just", recipe) in assurance._packet_gates(block) for recipe in replaced
        )
    }
    expected = {
        f"{packet}:{oracle}"
        for packet, pairs in contracts.items()
        if packet not in owners
        for oracle, _criterion in pairs
    }
    assert expected == {
        command.source for command in commands if command.source.startswith("WP")
    }
    assert len(expected) == 48
    assert not any(
        command.argv[1] in replaced for command in commands if command.argv[0] == "just"
    )
    assert any(
        command.source == "retained-v5-functional-oracle" for command in commands
    )
    assurance._check_recipe_graph(
        [command.argv for command in commands if command.argv[0] == "just"], recipes
    )
    expanded = assurance._flatten_aggregates(commands, recipes)
    assert expected <= {command.source for command in expanded}


def test_rt_cpg_wp77_operations(tmp_path: Path) -> None:
    """Fresh-shell isolated graph plus real fail/pass/missing selector execution."""
    plan = _fixture(tmp_path, future=True)
    commands = assurance.derive_commands(tmp_path, plan, "packet", "WP01")
    selected = commands[0].argv[-1]
    environment = dict(os.environ)
    environment.pop(assurance.STACK_ENV, None)
    environment["VIRTUAL_ENV"] = "/deliberately/foreign/environment"
    # Exact executable is intentionally passed through the fresh repository shell;
    # no uv resolution or fixture package installation is needed for these tests.
    shell = assurance.ROOT / "scripts/repo-shell.sh"
    graph = subprocess.run(
        (
            str(shell),
            "-c",
            'PYTHONPATH=. "$1" -c "$2"',
            "rt-cpg-operations",
            sys.executable,
            (
                "from tooling.ci.feature_architecture import validate; "
                "result = validate('provider-contracts'); "
                "assert result['scope'] == 'provider-contracts'; "
                "assert 'daemon' not in result['root_features']"
            ),
        ),
        cwd=assurance.ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    assert graph.returncode == 0, graph.stdout + graph.stderr

    def execute_fixture(selector: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            (
                str(shell),
                "-c",
                'PYTHONPATH="$3" "$1" -m pytest -q -p tooling.ci.real_time_cpg_assurance "$2"',
                "rt-cpg-operations",
                sys.executable,
                selector,
                str(assurance.ROOT),
            ),
            cwd=tmp_path,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )

    passed = execute_fixture(selected)
    assert passed.returncode == 0, passed.stdout + passed.stderr
    _oracles(tmp_path, 1, "    actual = 1\n    assert actual == 2\n")
    failed = execute_fixture(selected)
    assert failed.returncode == 1, failed.stdout + failed.stderr
    _oracles(
        tmp_path,
        1,
        "    import pytest\n    pytest.skip('seeded unavailable implementation')\n",
    )
    skipped = execute_fixture(selected)
    assert skipped.returncode == 1, skipped.stdout + skipped.stderr
    missing = execute_fixture(selected + "_not_present")
    assert missing.returncode != 0
    with pytest.raises(plan_assurance.PlanAssuranceError, match="lacks definitions"):
        assurance.derive_commands(tmp_path, plan, "packet", "WP02")
    assert not (tmp_path / "state.json").exists()
