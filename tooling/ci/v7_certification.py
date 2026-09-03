"""Derived, fail-closed terminal certification for the active relational-fabric v7 plan.

The accepted plan is the command and oracle authority.  This module materializes no
parallel certification contract: it derives the packet selectors, final gate matrix,
and still-live v5 WP43--WP48 expectations on every invocation.  A run records every
child exit under ``target/`` and fails after (not instead of) preserving the complete
result set.
"""

from __future__ import annotations

import argparse
import json
import re
import shlex
import subprocess
import sys
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from tooling.ci import artifact_contracts, plan_assurance

ROOT = Path(__file__).resolve().parents[2]
EXPECTED_PLAN_ID = "codefabric-execution-proved-relational-data-fabric"
EXPECTED_PLAN_VERSION = "v7"
SELF_RECIPE = "relational-fabric-v7-certification"
REPORT_PATH = Path("target/relational-fabric-v7-certification/report.json")
FINAL_MATRIX_START = "## 7. Final gate matrix"
FINAL_MATRIX_END = "## 8. Execution sequence and state discipline"
MATRIX_COMMAND = re.compile(r"^- `(?P<command>just [^`\n]+)`[^\n]*$", re.MULTILINE)
JUST_COMMAND = re.compile(r"`(?P<command>just [^`\n]+)`")
RETAINED_V5_RANGE = re.compile(
    r"\bv5 WP(?P<first>\d+)--WP(?P<last>\d+)\b", re.IGNORECASE
)


class V7CertificationError(ValueError):
    """The live repository cannot satisfy the derived v7 certification contract."""


@dataclass(frozen=True)
class CertificationCommand:
    """One independently observable child of the terminal aggregate."""

    source: str
    argv: tuple[str, ...]


@dataclass(frozen=True)
class CommandResult:
    """The complete status retained for one certification child."""

    source: str
    argv: tuple[str, ...]
    exit_code: int
    elapsed_ms: int


@dataclass(frozen=True)
class CertificationModel:
    """The certification surface derived from current authorities."""

    plan_path: str
    terminal_packet: str
    packet_oracle_count: int
    packet_selectors: tuple[CertificationCommand, ...]
    packet_acceptance: tuple[CertificationCommand, ...]
    final_matrix: tuple[CertificationCommand, ...]
    retained_v5: tuple[CertificationCommand, ...]

    @property
    def executable_commands(self) -> tuple[CertificationCommand, ...]:
        """Return every non-recursive child in dependency order."""

        matrix_children = tuple(
            command
            for command in self.final_matrix
            if command.argv != ("just", SELF_RECIPE)
        )
        return (
            self.packet_selectors
            + self.packet_acceptance
            + self.retained_v5
            + matrix_children
        )


CommandRunner = Callable[[CertificationCommand], int]


def _relative(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError as error:
        raise V7CertificationError(f"path escapes repository: {path}") from error


def _section(text: str, start: str, end: str) -> str:
    if text.count(start) != 1 or text.count(end) != 1:
        raise V7CertificationError(
            f"plan must contain exactly one {start!r} and one {end!r}"
        )
    before, remainder = text.split(start, 1)
    del before
    body, _ = remainder.split(end, 1)
    if not body.strip():
        raise V7CertificationError("final gate matrix is empty")
    return body


def _parse_just_command(value: str) -> tuple[str, ...]:
    try:
        argv = tuple(shlex.split(value))
    except ValueError as error:
        raise V7CertificationError(f"invalid final-matrix command {value!r}") from error
    if len(argv) < 2 or argv[0] != "just" or argv[1].startswith("-"):
        raise V7CertificationError(f"unsupported final-matrix command {value!r}")
    if any(token in {";", "&&", "||", "|"} for token in argv):
        raise V7CertificationError(f"compound final-matrix command {value!r}")
    return argv


def derive_final_matrix(
    plan_path: Path,
    recipes: Mapping[str, Any],
) -> tuple[CertificationCommand, ...]:
    """Parse the exact ordered §7 recipe matrix and close it against live Just."""

    text = plan_path.read_text(encoding="utf-8")
    section = _section(text, FINAL_MATRIX_START, FINAL_MATRIX_END)
    commands = tuple(
        CertificationCommand(
            "final-matrix", _parse_just_command(match.group("command"))
        )
        for match in MATRIX_COMMAND.finditer(section)
    )
    if not commands:
        raise V7CertificationError("final gate matrix selects no commands")
    duplicates = sorted(
        {command.argv for command in commands if commands.count(command) > 1}
    )
    if duplicates:
        raise V7CertificationError(
            f"final gate matrix contains duplicates: {duplicates}"
        )
    self_commands = [
        command for command in commands if command.argv == ("just", SELF_RECIPE)
    ]
    if len(self_commands) != 1 or commands[-1] != self_commands[0]:
        raise V7CertificationError(
            f"final gate matrix must end in exactly one `just {SELF_RECIPE}`"
        )
    unknown = sorted({command.argv[1] for command in commands} - recipes.keys())
    if unknown:
        raise V7CertificationError(f"final gate matrix has unknown recipes: {unknown}")
    return commands


def derive_packet_acceptance_commands(
    plan_path: Path,
    final_matrix: Sequence[CertificationCommand],
    recipes: Mapping[str, Any],
) -> tuple[CertificationCommand, ...]:
    """Derive packet acceptance recipes not already explicit §7 children."""

    final_commands = {command.argv for command in final_matrix}
    observed: set[tuple[str, ...]] = set()
    commands: list[CertificationCommand] = []
    dependencies = plan_assurance._dependency_map(plan_path)
    blocks = artifact_contracts._packet_blocks(plan_path)
    for packet in plan_assurance._topological_order(dependencies):
        block = blocks[packet]
        marker = "**Acceptance checks.**"
        if block.count(marker) != 1:
            raise V7CertificationError(f"{packet} lacks one acceptance-check section")
        acceptance = block.split(marker, 1)[1]
        for match in JUST_COMMAND.finditer(acceptance):
            argv = _parse_just_command(match.group("command"))
            if (
                argv == ("just", SELF_RECIPE)
                or argv in final_commands
                or argv in observed
            ):
                continue
            if argv[1] not in recipes:
                raise V7CertificationError(
                    f"{packet} acceptance check has unknown recipe: {argv[1]}"
                )
            observed.add(argv)
            commands.append(CertificationCommand("packet-acceptance", argv))
    return tuple(commands)


def _recipe_dependency_closure(
    roots: Sequence[str], recipes: Mapping[str, Any]
) -> set[str]:
    closure: set[str] = set()
    pending = list(roots)
    while pending:
        recipe = pending.pop()
        if recipe in closure:
            continue
        definition = recipes.get(recipe)
        if not isinstance(definition, Mapping):
            raise V7CertificationError(f"recipe dependency is absent: {recipe}")
        closure.add(recipe)
        dependencies = definition.get("dependencies")
        if not isinstance(dependencies, list):
            raise V7CertificationError(f"recipe dependencies are malformed: {recipe}")
        for dependency in dependencies:
            if not isinstance(dependency, Mapping) or not isinstance(
                dependency.get("recipe"), str
            ):
                raise V7CertificationError(f"recipe dependency is malformed: {recipe}")
            pending.append(str(dependency["recipe"]))
    return closure


def _retained_v5_packets(plan_text: str) -> set[str]:
    ranges = {
        (int(match.group("first")), int(match.group("last")))
        for match in RETAINED_V5_RANGE.finditer(plan_text)
    }
    maximal = {
        candidate
        for candidate in ranges
        if not any(
            other != candidate and other[0] <= candidate[0] and other[1] >= candidate[1]
            for other in ranges
        )
    }
    if len(maximal) != 1:
        raise V7CertificationError(
            "plan must name one maximal retained v5 packet range, "
            f"observed {sorted(ranges)}"
        )
    first, last = maximal.pop()
    if first > last:
        raise V7CertificationError("retained v5 packet range is reversed")
    return {f"WP{number}" for number in range(first, last + 1)}


def derive_retained_v5_commands(
    root: Path,
    plan: Mapping[str, Any],
    plan_path: Path,
    final_matrix: Sequence[CertificationCommand],
    recipes: Mapping[str, Any],
) -> tuple[CertificationCommand, ...]:
    """Select live v5 functional oracles not already reached by the v7 matrix."""

    predecessor_value = plan.get("supersedes_on_activation")
    if not isinstance(predecessor_value, str):
        raise V7CertificationError("v7 plan lacks its superseded-plan authority")
    predecessor = (root / predecessor_value).resolve()
    if not predecessor.is_file():
        raise V7CertificationError(
            f"superseded plan does not exist: {predecessor_value}"
        )
    selected = _retained_v5_packets(plan_path.read_text(encoding="utf-8"))
    try:
        contracts = plan_assurance._oracle_contracts(
            predecessor,
            selected_packets=selected,
            allow_legacy_mapping=True,
        )
    except plan_assurance.PlanAssuranceError as error:
        raise V7CertificationError(str(error)) from error
    if set(contracts) != selected:
        raise V7CertificationError(
            "superseded plan does not define the retained packet range"
        )

    matrix_roots = [
        command.argv[1]
        for command in final_matrix
        if command.argv != ("just", SELF_RECIPE)
    ]
    reached = _recipe_dependency_closure(matrix_roots, recipes)
    retained: list[CertificationCommand] = []
    for packet in sorted(contracts, key=lambda value: int(value.removeprefix("WP"))):
        for oracle, _criterion in contracts[packet]:
            if oracle in recipes and oracle not in reached:
                retained.append(
                    CertificationCommand(
                        "retained-v5-functional-oracle", ("just", oracle)
                    )
                )
    return tuple(retained)


def _terminal_packet(plan_path: Path) -> str:
    dependencies = plan_assurance._dependency_map(plan_path)
    all_packets = set(dependencies)
    terminal = [
        packet
        for packet in dependencies
        if plan_assurance._ancestors(dependencies, packet) == all_packets - {packet}
    ]
    if len(terminal) != 1:
        raise V7CertificationError(
            f"plan must derive one dependency-closed terminal packet: {terminal}"
        )
    return terminal[0]


def _validate_oracle_definitions(
    root: Path,
    contracts: Mapping[str, Sequence[tuple[str, str]]],
) -> int:
    wanted = {oracle for values in contracts.values() for oracle, _criterion in values}
    try:
        plan_assurance._require_exact_definitions(
            wanted,
            plan_assurance.oracle_definitions(root, wanted),
            context="v7 certification oracle universe",
        )
    except plan_assurance.PlanAssuranceError as error:
        raise V7CertificationError(str(error)) from error
    return len(wanted)


def _validate_state(
    root: Path,
    plan_path: Path,
    plan: Mapping[str, Any],
    terminal_packet: str,
) -> None:
    state_path = root / str(plan["state_path"])
    state = artifact_contracts.validate_state(
        root,
        state_path,
        expected_ids=plan["ids"],
    )
    status = artifact_contracts.derive_plan_status(root, plan_path)
    if not status["healthy"]:
        raise V7CertificationError("active v7 plan status is not healthy")

    complete_phase = state["status"] == "complete"
    if complete_phase:
        if state["current_packet"] is not None:
            raise V7CertificationError("complete v7 state retains a current packet")
    elif state["status"] == "executing":
        if state["current_packet"] != terminal_packet:
            raise V7CertificationError("v7 is not executing its terminal packet")
    else:
        raise V7CertificationError(
            f"v7 state has invalid certification phase {state['status']}"
        )

    for packet, entry in state["packets"].items():
        if packet == terminal_packet and not complete_phase:
            if entry["status"] != "in_progress" or entry["proving_commit"] is not None:
                raise V7CertificationError(
                    "terminal packet must be in progress without a proving commit"
                )
            continue
        if entry["status"] != "complete" or not status["packets"][packet]["trusted"]:
            raise V7CertificationError(f"packet is not complete and trusted: {packet}")

    incomplete_milestones: list[str] = []
    for milestone, entry in state["milestones"].items():
        if entry["status"] == "complete":
            if not status["milestones"][milestone]["trusted"]:
                raise V7CertificationError(
                    f"milestone is not complete and trusted: {milestone}"
                )
        else:
            incomplete_milestones.append(milestone)
    if complete_phase:
        if incomplete_milestones:
            raise V7CertificationError(
                f"complete state has open milestones: {incomplete_milestones}"
            )
    elif (
        len(incomplete_milestones) != 1
        or state["milestones"][incomplete_milestones[0]]["status"] != "in_progress"
    ):
        raise V7CertificationError(
            f"certification requires one in-progress milestone: {incomplete_milestones}"
        )
    for batch, entry in state["decommission_batches"].items():
        if (
            entry["status"] != "complete"
            or not status["decommission_batches"][batch]["trusted"]
        ):
            raise V7CertificationError(
                f"decommission batch is not complete and trusted: {batch}"
            )


def _tracked_tree_is_clean(root: Path) -> bool:
    return all(
        subprocess.run(("git", *arguments), cwd=root, check=False).returncode == 0
        for arguments in (("diff", "--quiet"), ("diff", "--cached", "--quiet"))
    )


def _head(root: Path) -> str:
    result = subprocess.run(
        ("git", "rev-parse", "HEAD"),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def derive_model(
    root: Path = ROOT,
    *,
    require_clean: bool = False,
) -> CertificationModel:
    """Derive and validate the complete v7 certification surface."""

    plan_path = artifact_contracts.active_plan_path(root)
    plan = artifact_contracts.validate_plan(
        root,
        plan_path,
        verify_declared_inputs=False,
    )
    if (
        plan.get("plan_id") != EXPECTED_PLAN_ID
        or plan.get("version") != EXPECTED_PLAN_VERSION
    ):
        raise V7CertificationError("the active plan is not relational-fabric v7")
    terminal_packet = _terminal_packet(plan_path)
    _validate_state(root, plan_path, plan, terminal_packet)
    if require_clean and not _tracked_tree_is_clean(root):
        raise V7CertificationError("certification candidate has tracked changes")

    try:
        contracts = plan_assurance._oracle_contracts(plan_path)
    except plan_assurance.PlanAssuranceError as error:
        raise V7CertificationError(str(error)) from error
    packet_oracle_count = _validate_oracle_definitions(root, contracts)
    recipes = artifact_contracts.load_just_recipes(root)
    final_matrix = derive_final_matrix(plan_path, recipes)
    packet_acceptance = derive_packet_acceptance_commands(
        plan_path,
        final_matrix,
        recipes,
    )
    retained_v5 = derive_retained_v5_commands(
        root,
        plan,
        plan_path,
        final_matrix,
        recipes,
    )
    packet_selectors = tuple(
        CertificationCommand("packet-oracle", ("just", "packet-oracle-check", packet))
        for packet in plan_assurance._topological_order(
            plan_assurance._dependency_map(plan_path)
        )
    )
    model = CertificationModel(
        plan_path=_relative(plan_path, root),
        terminal_packet=terminal_packet,
        packet_oracle_count=packet_oracle_count,
        packet_selectors=packet_selectors,
        packet_acceptance=packet_acceptance,
        final_matrix=final_matrix,
        retained_v5=retained_v5,
    )
    if not model.executable_commands:
        raise V7CertificationError("certification derives no executable children")
    if len({command.argv for command in model.executable_commands}) != len(
        model.executable_commands
    ):
        raise V7CertificationError("derived certification children are duplicated")
    return model


def execute_commands(
    commands: Sequence[CertificationCommand],
    runner: CommandRunner,
    *,
    announce: bool = False,
) -> list[CommandResult]:
    """Execute every child and retain each status even after earlier failures."""

    results: list[CommandResult] = []
    total = len(commands)
    for index, command in enumerate(commands, start=1):
        if announce:
            print(
                f"[{index:03d}/{total:03d}] {command.source}: "
                f"{shlex.join(command.argv)}",
                flush=True,
            )
        started = time.monotonic_ns()
        try:
            exit_code = int(runner(command))
        except OSError as error:
            print(
                f"cannot execute {shlex.join(command.argv)}: {error}", file=sys.stderr
            )
            exit_code = 127
        elapsed_ms = (time.monotonic_ns() - started) // 1_000_000
        results.append(
            CommandResult(command.source, command.argv, exit_code, elapsed_ms)
        )
    return results


def _subprocess_runner(root: Path) -> CommandRunner:
    def run(command: CertificationCommand) -> int:
        return subprocess.run(command.argv, cwd=root, check=False).returncode

    return run


def assert_seeded_failure_propagates() -> None:
    """Prove the shared executor preserves and rejects an injected child failure."""

    commands = (
        CertificationCommand("fault-fixture", ("just", "passing-before")),
        CertificationCommand("fault-fixture", ("just", "seeded-failure")),
        CertificationCommand("fault-fixture", ("just", "passing-after")),
    )
    observed: list[tuple[str, ...]] = []

    def runner(command: CertificationCommand) -> int:
        observed.append(command.argv)
        return 73 if command.argv[-1] == "seeded-failure" else 0

    results = execute_commands(commands, runner)
    if observed != [command.argv for command in commands]:
        raise V7CertificationError(
            "seeded failure caused child execution to short-circuit"
        )
    if [result.exit_code for result in results] != [0, 73, 0]:
        raise V7CertificationError("seeded child status was not preserved exactly")
    if not any(result.exit_code != 0 for result in results):
        raise V7CertificationError("seeded child failure was masked")


def _report(
    model: CertificationModel,
    candidate_head: str,
    started_at: str,
    results: Sequence[CommandResult],
    stable: bool,
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "plan_path": model.plan_path,
        "terminal_packet": model.terminal_packet,
        "candidate_head": candidate_head,
        "started_at": started_at,
        "finished_at": datetime.now(UTC).isoformat(),
        "packet_oracle_count": model.packet_oracle_count,
        "logical_child_count": len(model.executable_commands) + 1,
        "executed_child_count": len(results),
        "aggregate_recipe": SELF_RECIPE,
        "seeded_failure_propagation": "passed",
        "candidate_stable": stable,
        "success": stable and all(result.exit_code == 0 for result in results),
        "results": [asdict(result) for result in results],
    }


def _write_report(root: Path, report: Mapping[str, Any]) -> Path:
    path = root / REPORT_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)
    return path


def run_certification(root: Path = ROOT) -> bool:
    """Run every derived child at one immutable tracked candidate HEAD."""

    model = derive_model(root, require_clean=True)
    assert_seeded_failure_propagates()
    candidate_head = _head(root)
    started_at = datetime.now(UTC).isoformat()
    print(
        f"v7 certification candidate {candidate_head}: "
        f"{len(model.executable_commands)} executable children; "
        f"{model.packet_oracle_count} plan-derived oracles",
        flush=True,
    )
    for index, command in enumerate(model.executable_commands, start=1):
        print(
            f"  {index:03d} {command.source}: {shlex.join(command.argv)}",
            flush=True,
        )
    results = execute_commands(
        model.executable_commands,
        _subprocess_runner(root),
        announce=True,
    )
    stable = candidate_head == _head(root) and _tracked_tree_is_clean(root)
    report = _report(model, candidate_head, started_at, results, stable)
    path = _write_report(root, report)
    failures = [result for result in results if result.exit_code != 0]
    print(
        f"v7 certification report: {_relative(path, root)}; "
        f"{len(results) - len(failures)} passed, {len(failures)} failed; "
        f"candidate_stable={str(stable).lower()}",
        flush=True,
    )
    return bool(report["success"])


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("list", "fault-check", "run"))
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "fault-check":
            assert_seeded_failure_propagates()
            print("v7 certification seeded-child propagation: passed")
            return 0
        if args.command == "list":
            model = derive_model()
            for command in model.executable_commands:
                print(f"{command.source}\t{shlex.join(command.argv)}")
            print(f"aggregate\tjust {SELF_RECIPE}")
            return 0
        return 0 if run_certification() else 1
    except (
        V7CertificationError,
        artifact_contracts.ArtifactContractError,
        plan_assurance.PlanAssuranceError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"v7 certification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
