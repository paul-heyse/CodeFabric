"""Fail-closed execution of the selected real-time CPG plan's proof graph.

Markdown owns membership and gate obligations; the live Just graph owns recipes.
No completion ledger, test-name inventory, or predecessor terminal selector lives
here. Packet mode deliberately does not require future packet implementations.
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import subprocess
import sys
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from tooling.ci import artifact_contracts, plan_assurance, v7_certification

ROOT = artifact_contracts.ROOT
PLAN_ID = "codefabric-real-time-cpg"
PREFIX = "real-time-cpg-"
DISPATCHERS = {
    f"{PREFIX}packet-check": "packet",
    f"{PREFIX}milestone-check": "milestone",
    f"{PREFIX}decommission-check": "decommission",
    f"{PREFIX}certification": "certification",
}
STACK_ENV = "CODEFABRIC_RT_ASSURANCE_STACK"
COMMAND = re.compile(r"`(just [^`\n]+)`")
HEADING = re.compile(r"^### ((?:WP|M|DB)\d+) —[^\n]*\n", re.MULTILINE)


class AssuranceError(ValueError):
    """The selected obligation cannot be executed faithfully."""


@dataclass(frozen=True)
class Command:
    """One executable command with its source obligation, not a pass claim."""

    source: str
    argv: tuple[str, ...]


@dataclass(frozen=True)
class Plan:
    path: Path
    values: Mapping[str, Any]
    blocks: Mapping[str, str]
    contracts: Mapping[str, list[tuple[str, str]]]
    dependencies: Mapping[str, set[str]]
    final: tuple[tuple[str, ...], ...]


def _section(text: str, start: str, end: str) -> str:
    if text.count(start) != 1 or text.count(end) != 1:
        raise AssuranceError(f"missing or ambiguous plan section: {start}")
    return text.split(start, 1)[1].split(end, 1)[0]


def _commands(text: str) -> tuple[tuple[str, ...], ...]:
    result = []
    for match in COMMAND.finditer(text):
        argv = tuple(shlex.split(match.group(1)))
        if (
            len(argv) < 2
            or not re.fullmatch(r"[a-z][a-z0-9_-]*", argv[1])
            or any(re.search(r"[<>;|&$`\n]", value) for value in argv)
        ):
            raise AssuranceError(f"nonexecutable gate declaration: {match.group(1)}")
        result.append(argv)
    return tuple(dict.fromkeys(result))


def load_plan(root: Path, explicit: Path | None = None) -> Plan:
    """Permit inactive draft tests without state, never implicit predecessor use."""
    path = explicit or artifact_contracts.active_plan_path(root)
    path = (root / path).resolve()
    if not path.is_relative_to(root.resolve()):
        raise AssuranceError("selected plan escapes the repository")
    values = artifact_contracts.validate_plan(root, path, verify_declared_inputs=False)
    if values["plan_id"] != PLAN_ID:
        raise AssuranceError("selected plan is not the real-time CPG plan")
    if explicit is None:
        artifact_contracts.validate_state(
            root, root / values["state_path"], expected_ids=values["ids"]
        )
    text = path.read_text(encoding="utf-8")
    blocks = {}
    for match in HEADING.finditer(text):
        tail = text[match.end() :]
        end = re.search(r"^#{2,3} ", tail, re.MULTILINE)
        blocks[match.group(1)] = tail[: end.start()] if end else tail
    final = _commands(
        _section(text, "### 7.3 Final nonmutating leaf gates", "### 7.4 ")
    )
    if not final:
        raise AssuranceError("final gate matrix selects zero commands")
    contracts = plan_assurance._oracle_contracts(path)
    dependencies = plan_assurance._dependency_map(path)
    plan_assurance._topological_order(dependencies)
    return Plan(path, values, blocks, contracts, dependencies, final)


def _packet_gates(block: str) -> tuple[tuple[str, ...], ...]:
    return _commands(
        _section(block, "**Packet-Local Gates.**", "**Integration Milestone.**")
    )


def _members(plan: Plan, identifier: str) -> tuple[str, ...]:
    block = plan.blocks[identifier]
    clause = re.search(r"^(?:Members?|Prerequisites):\s*([^.;]+)", block, re.MULTILINE)
    if clause is None:
        raise AssuranceError(f"{identifier} has no member/prerequisite clause")
    members = tuple(re.findall(r"\bWP\d+\b", clause.group(1)))
    if not members or len(set(members)) != len(members):
        raise AssuranceError(f"{identifier} has empty/duplicate packet membership")
    unknown = set(members) - plan.contracts.keys()
    if unknown:
        raise AssuranceError(f"{identifier} has unknown packet members: {unknown}")
    return members


def _check_recipe_graph(
    roots: Sequence[tuple[str, ...]], recipes: Mapping[str, Any]
) -> None:
    """Reject cycles, mutating children and indirect dispatcher re-entry."""
    done: set[str] = set()
    visiting: list[str] = []

    def visit(name: str) -> None:
        if name in visiting:
            raise AssuranceError(f"recursive recipe graph: {visiting + [name]}")
        if name in done:
            return
        if name in DISPATCHERS:
            raise AssuranceError(f"recursive dispatcher child: {name}")
        definition = recipes.get(name)
        if not isinstance(definition, Mapping):
            raise AssuranceError(f"required recipe is absent: {name}")
        attributes = definition.get("attributes", [])
        if any(
            isinstance(value, Mapping)
            and (value.get("group") == "mutating" or "confirm" in value)
            for value in attributes
        ) or name.endswith("-capture"):
            raise AssuranceError(f"mutating recipe is not a gate: {name}")
        if "v7-certification" in name or name in {
            "compiled_release_resource_performance_envelope",
            "compiled-release-resource-performance-check",
        }:
            raise AssuranceError(f"predecessor terminal/performance selector: {name}")
        visiting.append(name)
        dependencies = definition.get("dependencies")
        if not isinstance(dependencies, list):
            raise AssuranceError(f"malformed recipe dependencies: {name}")
        for dependency in dependencies:
            if not isinstance(dependency, Mapping) or not isinstance(
                dependency.get("recipe"), str
            ):
                raise AssuranceError(f"malformed recipe dependency: {name}")
            visit(dependency["recipe"])
        body = plan_assurance._just_fragment_text(definition.get("body"))
        if re.search(r"\bjust\s+(?:-|[\"'$`])", body):
            raise AssuranceError(f"unresolved nested Just invocation: {name}")
        for match in re.finditer(r"\bjust\s+([a-z][a-z0-9_-]*)", body):
            visit(match.group(1))
        if re.search(
            r"\bpython(?:\d(?:\.\d+)?)?\s+(?:-m\s+tooling\.ci\.real_time_cpg_assurance"
            r"|(?:[^\s]+/)?real_time_cpg_assurance\.py)(?:\s|$)",
            body,
        ):
            raise AssuranceError(f"recursive assurance module invocation: {name}")
        if not dependencies and re.fullmatch(r"\s*@?(?:true|:|exit\s+0)\s*;?\s*", body):
            raise AssuranceError(f"no-op recipe is not a gate: {name}")
        visiting.pop()
        done.add(name)

    for argv in roots:
        visit(argv[1])


def _definition_commands(root: Path, plan: Plan, packet: str) -> tuple[Command, ...]:
    wanted = {oracle for oracle, _criterion in plan.contracts[packet]}
    definitions = plan_assurance._require_exact_definitions(
        wanted,
        plan_assurance.oracle_definitions(root, wanted),
        context=f"{packet} selector",
    )
    commands = []
    for oracle, _criterion in plan.contracts[packet]:
        definition = definitions[oracle]
        if definition.language == "python":
            argv = (
                "env",
                "-u",
                "VIRTUAL_ENV",
                "-u",
                "UV_PROJECT_ENVIRONMENT",
                "PYTHONPATH=.",
                "uv",
                "run",
                "--frozen",
                "--project",
                "codefabric-cpg-mcp",
                "pytest",
                "-p",
                "tooling.ci.real_time_cpg_assurance",
                f"{definition.path}::{definition.function}",
            )
        elif definition.language == "rust":
            args = ["cargo", "nextest", "run", "--locked", "--no-fail-fast"]
            for domain in ("rustc-extractor", "pyrefly-sidecar"):
                if definition.path.startswith(f"{domain}/"):
                    args.extend(("--manifest-path", f"{domain}/Cargo.toml"))
            args.extend(
                ("-E", f"test(/(^|::){re.escape(oracle)}$/)", "--no-tests=fail")
            )
            argv = tuple(args)
        else:
            argv = ("just", definition.function)
        commands.append(Command(f"{packet}:{oracle}", argv))
    return tuple(commands)


def _flatten_aggregates(
    commands: Sequence[Command], recipes: Mapping[str, Any]
) -> tuple[Command, ...]:
    """Execute each dependency once, preserving bodies of mixed Just aggregates."""
    result: list[Command] = []
    seen: set[tuple[str, ...]] = set()

    def expand(command: Command) -> None:
        if command.argv in seen:
            return
        seen.add(command.argv)
        if command.argv[0] != "just":
            result.append(command)
            return
        definition = recipes[command.argv[1]]
        dependencies = definition["dependencies"]
        for dependency in dependencies:
            # Today's registry has no parameterized dependency edges. Refuse an
            # unevaluated Just expression rather than run a different contract.
            if dependency.get("arguments") or dependency.get("star"):
                raise AssuranceError(
                    "parameterized Just dependency requires explicit expansion"
                )
            expand(Command(command.source, ("just", dependency["recipe"])))
        body = plan_assurance._just_fragment_text(definition.get("body")).strip()
        if body:
            argv = command.argv
            if dependencies:
                argv = ("just", "--no-deps", *argv[1:])
            result.append(Command(command.source, argv))
        elif not dependencies:
            raise AssuranceError(f"empty recipe is not a gate: {command.argv[1]}")

    for command in commands:
        expand(command)
    if not result:
        raise AssuranceError("aggregate expansion selects zero children")
    return tuple(result)


def retained_commands(
    root: Path, plan: Plan, recipes: Mapping[str, Any]
) -> tuple[Command, ...]:
    """Carry forward actual predecessor proof, excluding only replaced contracts.

    V7's measurement and terminal *contracts*, not a numeric packet range, are
    superseded by the current plan. Every other v7 oracle runs from its original
    declaration. Valid leaf gates (including v5 functional carryforward) remain
    required. No historical state or completion claim is imported.
    """
    values = plan.values
    seen: set[Path] = set()
    while True:
        predecessor = values.get("supersedes_on_activation")
        if not isinstance(predecessor, str):
            raise AssuranceError("missing retained predecessor authority")
        path = (root / predecessor).resolve()
        if not path.is_relative_to(root.resolve()) or path in seen:
            raise AssuranceError("escaped or recursive predecessor chain")
        seen.add(path)
        values = artifact_contracts.parse_frontmatter(path)
        if values.get("plan_id") != PLAN_ID:
            break
    if (values.get("plan_id"), values.get("version")) != (
        "codefabric-execution-proved-relational-data-fabric",
        "v7",
    ):
        raise AssuranceError("retained predecessor is not the accepted v7 authority")
    blocks = artifact_contracts._packet_blocks(path)
    replaced_recipes = {
        "compiled-release-resource-performance-check",
        v7_certification.SELF_RECIPE,
    }
    superseded: set[str] = set()
    for recipe in replaced_recipes:
        owners = {
            packet
            for packet, block in blocks.items()
            if ("just", recipe) in _packet_gates(block)
        }
        if len(owners) != 1:
            raise AssuranceError(f"ambiguous superseded v7 obligation: {recipe}")
        superseded.update(owners)
    if len(superseded) != len(replaced_recipes):
        raise AssuranceError("v7 terminal and performance owners are not distinct")
    contracts = plan_assurance._oracle_contracts(path)
    inherited = Plan(path, values, blocks, contracts, {}, ())
    result = []
    for packet in plan_assurance._topological_order(
        plan_assurance._dependency_map(path)
    ):
        if packet not in superseded:
            result.extend(_definition_commands(root, inherited, packet))
        for argv in _packet_gates(blocks[packet]):
            if argv[1] not in replaced_recipes:
                result.append(Command(f"retained-v7:{packet}", argv))
    matrix = v7_certification.derive_final_matrix(path, recipes)
    for command in matrix:
        if command.argv[1] not in replaced_recipes:
            result.append(Command("retained-v7:final", command.argv))
    for command in v7_certification.derive_retained_v5_commands(
        root, values, path, matrix, recipes
    ):
        result.append(Command(command.source, command.argv))
    return tuple(result)


def derive_commands(
    root: Path,
    plan: Plan,
    mode: str,
    identifier: str | None,
    recipes: Mapping[str, Any] | None = None,
) -> tuple[Command, ...]:
    """Expand only the selected proof closure, with deterministic de-duplication."""
    recipes = (
        recipes if recipes is not None else artifact_contracts.load_just_recipes(root)
    )
    result: dict[tuple[str, ...], Command] = {}
    done: set[str] = set()
    visiting: list[str] = []
    gate_roots: list[tuple[str, ...]] = []

    def gate(argv: tuple[str, ...], source: str) -> None:
        if argv[1] in DISPATCHERS:
            kind = DISPATCHERS[argv[1]]
            if kind == "certification":
                # This is the terminal declaration, not an executable child.
                # Its complete expansion is owned by the top-level certifier.
                if (
                    source not in plan.contracts
                    and source != "final"
                    and "all §7 leaf gates" not in plan.blocks.get(source, "")
                ):
                    raise AssuranceError(f"recursive terminal declaration in {source}")
                return
            if len(argv) != 3:
                raise AssuranceError(f"invalid dispatcher arguments: {argv}")
            if argv[2] == source:
                return  # The block's own documented invocation is not a child.
            expand(argv[2])
            return
        gate_roots.append(argv)
        result.setdefault(argv, Command(source, argv))

    def expand(selected: str) -> None:
        if selected in visiting:
            raise AssuranceError(f"recursive obligation graph: {visiting + [selected]}")
        if selected in done:
            return
        if selected not in plan.blocks:
            raise AssuranceError(f"unknown obligation: {selected}")
        visiting.append(selected)
        if selected in plan.contracts:
            for command in _definition_commands(root, plan, selected):
                result.setdefault(command.argv, command)
                if command.argv[0] == "just":
                    gate_roots.append(command.argv)
            for argv in _packet_gates(plan.blocks[selected]):
                gate(argv, selected)
        else:
            for packet in _members(plan, selected):
                expand(packet)
            block = plan.blocks[selected]
            # References before the prose's first executable command identify
            # prerequisite milestones without mistaking Invocation for recursion.
            prefix = block.split("Execute", 1)[0]
            for prerequisite in re.findall(r"\bM\d+\b", prefix):
                expand(prerequisite)
            if "all §7 leaf gates" in block:
                for batch in plan.values["ids"]["decommission_batches"]:
                    expand(batch)
                for argv in plan.final:
                    gate(argv, "final")
            for argv in _commands(block):
                gate(argv, selected)
        visiting.pop()
        done.add(selected)

    if mode == "certification":
        for packet in plan_assurance._topological_order(plan.dependencies):
            expand(packet)
        for group in ("milestones", "decommission_batches"):
            for selected in plan.values["ids"][group]:
                expand(selected)
        for argv in plan.final:
            gate(argv, "final")
        for command in retained_commands(root, plan, recipes):
            result.setdefault(command.argv, command)
            if command.argv[0] == "just":
                gate_roots.append(command.argv)
    else:
        expected = {"packet": "WP", "milestone": "M", "decommission": "DB"}
        if (
            mode not in expected
            or identifier is None
            or not re.fullmatch(rf"{expected[mode]}\d+", identifier)
        ):
            raise AssuranceError(f"invalid {mode} selector: {identifier}")
        expand(identifier)
    if not result:
        raise AssuranceError("selected obligation has no executable children")
    _check_recipe_graph(gate_roots, recipes)
    return _flatten_aggregates(tuple(result.values()), recipes)


def execute_commands(
    commands: Sequence[Command], runner: Callable[[Command], int]
) -> int:
    """Preserve the first failing child's status; never turn no work into green."""
    if not commands:
        raise AssuranceError("zero executable children")
    for command in commands:
        status = runner(command)
        if status:
            return status if status > 0 else 128 - status
    return 0


def pytest_sessionfinish(session: Any, exitstatus: int) -> None:
    """Pytest plugin: selected proof cannot pass through skip/xfail/zero calls."""
    if exitstatus:
        return
    reporter = session.config.pluginmanager.get_plugin("terminalreporter")
    reports = (
        []
        if reporter is None
        else [
            report
            for values in reporter.stats.values()
            for report in values
            if getattr(report, "when", None) in {"setup", "call", "teardown"}
        ]
    )
    called = [report for report in reports if report.when == "call"]
    if not called or any(
        report.skipped or report.failed or hasattr(report, "wasxfail")
        for report in reports
    ):
        session.exitstatus = 1


def _validate_candidate(root: Path, plan: Plan) -> None:
    if artifact_contracts.active_plan_path(root).resolve() != plan.path:
        raise AssuranceError("certification requires the active plan")
    artifact_contracts.validate_plan(root, plan.path)
    status = artifact_contracts.derive_plan_status(root, plan.path)
    if not status["healthy"]:
        raise AssuranceError("active plan status is not healthy")
    terminal = [
        packet
        for packet, block in plan.blocks.items()
        if packet in plan.contracts
        and ("just", f"{PREFIX}certification") in _packet_gates(block)
    ]
    if len(terminal) != 1:
        raise AssuranceError("plan must declare exactly one terminal packet")
    state = artifact_contracts.validate_state(
        root, root / plan.values["state_path"], expected_ids=plan.values["ids"]
    )
    complete_phase = state["status"] == "complete"
    if not complete_phase and (
        state["status"] != "executing" or state["current_packet"] != terminal[0]
    ):
        raise AssuranceError("certification requires the terminal execution phase")
    if complete_phase and state["current_packet"] is not None:
        raise AssuranceError("complete state retains a current packet")
    for packet, entry in state["packets"].items():
        if packet == terminal[0] and state["current_packet"] == packet:
            if entry["status"] != "in_progress":
                raise AssuranceError("terminal packet is not in progress")
        elif entry["status"] != "complete" or not status["packets"][packet]["trusted"]:
            raise AssuranceError(f"packet is not complete and trusted: {packet}")
    terminal_milestones = {
        milestone
        for milestone in plan.values["ids"]["milestones"]
        if terminal[0] in _members(plan, milestone)
    }
    if len(terminal_milestones) != 1:
        raise AssuranceError("terminal packet must belong to exactly one milestone")
    for group in ("milestones", "decommission_batches"):
        for identifier, entry in state[group].items():
            if identifier in terminal_milestones and not complete_phase:
                if entry["status"] != "in_progress":
                    raise AssuranceError("terminal milestone is not in progress")
            elif (
                entry["status"] != "complete"
                or not status[group][identifier]["trusted"]
            ):
                raise AssuranceError(f"{identifier} is not complete and trusted")
    for arguments in (("diff", "--quiet"), ("diff", "--cached", "--quiet")):
        if subprocess.run(("git", *arguments), cwd=root, check=False).returncode:
            raise AssuranceError("certification candidate has tracked changes")


def execute_certification(
    root: Path,
    plan: Plan,
    commands: Sequence[Command],
    runner: Callable[[Command], int],
) -> int:
    """Certify one stable candidate, including authority/HEAD checks after children."""

    def head() -> str:
        return subprocess.run(
            ("git", "rev-parse", "HEAD"),
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    _validate_candidate(root, plan)
    before = head()
    status = execute_commands(commands, runner)
    if status:
        return status
    _validate_candidate(root, plan)
    if head() != before:
        raise AssuranceError(
            "certification candidate HEAD changed during child execution"
        )
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "mode", choices=("packet", "milestone", "decommission", "certification")
    )
    parser.add_argument("identifier", nargs="?")
    parser.add_argument(
        "--plan",
        type=Path,
        help="explicit inactive plan for local proof; no activation",
    )
    options = parser.parse_args(argv)
    try:
        if os.environ.get(STACK_ENV):
            raise AssuranceError("recursive real-time CPG assurance invocation")
        plan = load_plan(ROOT, options.plan)
        if options.mode == "certification":
            if options.identifier is not None:
                raise AssuranceError("certification takes no selector")
            _validate_candidate(ROOT, plan)
        commands = derive_commands(ROOT, plan, options.mode, options.identifier)
        environment = {
            **os.environ,
            STACK_ENV: f"{options.mode}:{options.identifier or ''}",
        }

        def run(command: Command) -> int:
            print(f"{command.source}: {shlex.join(command.argv)}", flush=True)
            return subprocess.run(
                command.argv, cwd=ROOT, env=environment, check=False
            ).returncode

        if options.mode == "certification":
            return execute_certification(ROOT, plan, commands, run)
        return execute_commands(commands, run)
    except (
        AssuranceError,
        artifact_contracts.ArtifactContractError,
        plan_assurance.PlanAssuranceError,
        v7_certification.V7CertificationError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"real-time CPG assurance: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
