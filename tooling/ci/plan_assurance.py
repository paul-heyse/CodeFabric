"""Executable assurance for implementation-plan oracles and dependency closure."""

from __future__ import annotations

import argparse
import ast
import re
import subprocess
import sys
from collections import defaultdict, deque
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml

from tooling.ci import artifact_contracts

ROOT = artifact_contracts.ROOT
OVERLAP_DISPOSITIONS = Path("contracts/governance/plan-overlap-dispositions.yaml")
SOURCE_ROOTS = (
    Path("src"),
    Path("tests"),
    Path("tooling"),
    Path("scripts"),
    Path("codefabric-cpg-mcp"),
    Path("rustc-extractor"),
    Path("pyrefly-sidecar"),
)
ORACLE_KINDS = ("BEH", "STR", "NEG", "OPS")


class PlanAssuranceError(ValueError):
    """The active plan lacks executable proof or dependency closure."""


@dataclass(frozen=True)
class OracleDefinition:
    oracle: str
    language: str
    path: str
    function: str


def _active(root: Path = ROOT) -> tuple[Path, Mapping[str, Any], Mapping[str, Any]]:
    path = artifact_contracts.active_plan_path(root)
    # Packet/DAG assurance must remain runnable while an implementation is changing a
    # declared input. Input freshness and trusted completion are independently enforced
    # by artifact_contracts plan-status/artifacts-check.
    plan = artifact_contracts.validate_plan(
        root,
        path,
        verify_declared_inputs=False,
    )
    state = artifact_contracts.validate_state(
        root,
        root / str(plan["state_path"]),
        expected_ids=plan["ids"],
    )
    return path, plan, state


def _dependency_map(plan_path: Path) -> dict[str, set[str]]:
    blocks = artifact_contracts._packet_blocks(plan_path)
    dependencies: dict[str, set[str]] = {}
    for packet, block in blocks.items():
        match = re.search(
            r"\*\*Dependencies\.\*\*\s*(.*?)(?=\n\n\*\*Target invariants\.\*\*)",
            block,
            re.DOTALL,
        )
        if match is None:
            raise PlanAssuranceError(f"{packet} has no Dependencies clause")
        value = match.group(1)
        dependencies[packet] = (
            set()
            if value.strip().startswith("None")
            else set(re.findall(r"\bWP\d+\b", value))
        )
        unknown = dependencies[packet] - blocks.keys()
        if unknown:
            raise PlanAssuranceError(
                f"{packet} has unknown dependencies {sorted(unknown)}"
            )
        if packet in dependencies[packet]:
            raise PlanAssuranceError(f"{packet} depends on itself")
    return dependencies


def _topological_order(dependencies: Mapping[str, set[str]]) -> list[str]:
    followers: dict[str, set[str]] = defaultdict(set)
    indegree = {packet: len(required) for packet, required in dependencies.items()}
    for packet, required in dependencies.items():
        for predecessor in required:
            followers[predecessor].add(packet)
    ready = deque(sorted(packet for packet, count in indegree.items() if count == 0))
    order: list[str] = []
    while ready:
        packet = ready.popleft()
        order.append(packet)
        for follower in sorted(followers[packet]):
            indegree[follower] -= 1
            if indegree[follower] == 0:
                ready.append(follower)
    if len(order) != len(dependencies):
        cycle = sorted(packet for packet, count in indegree.items() if count > 0)
        raise PlanAssuranceError(f"packet dependency graph contains a cycle: {cycle}")
    return order


def _ancestors(dependencies: Mapping[str, set[str]], packet: str) -> set[str]:
    result: set[str] = set()
    pending = list(dependencies[packet])
    while pending:
        predecessor = pending.pop()
        if predecessor not in result:
            result.add(predecessor)
            pending.extend(dependencies[predecessor])
    return result


def _known_touch_resources(block: str) -> set[str]:
    match = re.search(
        r"\*\*Change surface(?: / Preflight / Known Touch)?\.\*\*"
        r"(.*?)(?=\n\n\*\*Required changes\.\*\*)",
        block,
        re.DOTALL,
    )
    if match is None:
        return set()
    known_touch = match.group(1)
    marker = "Known touch:"
    if marker in known_touch:
        known_touch = known_touch.split(marker, 1)[1]
    resources: set[str] = set()
    for literal in re.findall(r"`([^`]+)`", known_touch):
        candidate = re.sub(r":\d+(?:-\d+)?$", "", literal.strip())
        candidates = [candidate]
        brace = re.fullmatch(r"([^{}]+)\{([^{}]+)\}([^{}]*)", candidate, re.DOTALL)
        if brace is not None:
            prefix, members, suffix = brace.groups()
            candidates = [
                f"{prefix}{member.strip()}{suffix}" for member in members.split(",")
            ]
        for path in candidates:
            path = re.sub(r"\s+", "", path)
            if (
                " " not in path
                and not path.endswith("/")
                and re.search(
                    r"\.(?:rs|py|json|jsonl|ya?ml|md|toml|proto|sql|sh)$",
                    path,
                )
            ):
                resources.add(path)
    resources.update(re.findall(r"\bAC-G-\d+\b", known_touch))
    return resources


ONTOLOGY_FABRIC_REQUIRED_ANCESTORS: dict[str, frozenset[str]] = {
    "WP05": frozenset(("WP03",)),
    "WP06": frozenset(("WP03",)),
    "WP09": frozenset(("WP08",)),
    "WP17": frozenset(("WP09", "WP10", "WP11", "WP12", "WP16")),
    "WP14": frozenset(("WP17",)),
    "WP15": frozenset(("WP17",)),
}


def _validate_ontology_fabric_readiness_states(
    dependencies: Mapping[str, set[str]],
) -> int:
    """Exhaust legal completion states and enforce the plan's release barriers."""
    if set(dependencies) != {f"WP{index:02d}" for index in range(1, 18)}:
        return 0
    ancestors = {packet: _ancestors(dependencies, packet) for packet in dependencies}
    for packet, required in ONTOLOGY_FABRIC_REQUIRED_ANCESTORS.items():
        missing = required - ancestors[packet]
        if missing:
            raise PlanAssuranceError(
                f"{packet} lacks required release-barrier ancestors {sorted(missing)}"
            )

    initial = frozenset()
    pending = [initial]
    visited = {initial}
    while pending:
        complete = pending.pop()
        ready = {
            packet
            for packet, required in dependencies.items()
            if packet not in complete and required <= complete
        }
        for packet in ready:
            required = ONTOLOGY_FABRIC_REQUIRED_ANCESTORS.get(packet, frozenset())
            if not required <= complete:
                raise PlanAssuranceError(
                    f"legal readiness state exposes {packet} before {sorted(required)}"
                )
            successor = frozenset((*complete, packet))
            if successor not in visited:
                visited.add(successor)
                pending.append(successor)
    return len(visited)


def _load_overlap_dispositions(
    root: Path,
) -> dict[tuple[str, frozenset[str]], Mapping[str, Any]]:
    path = root / OVERLAP_DISPOSITIONS
    if not path.is_file():
        return {}
    document = yaml.safe_load(path.read_text(encoding="utf-8"))
    expected_keys = {
        "artifact_id",
        "artifact_kind",
        "version",
        "compatible_suite_major",
        "status",
        "schema_version",
        "records",
    }
    if (
        not isinstance(document, Mapping)
        or document.get("schema_version") != 1
        or set(document) != expected_keys
        or document.get("artifact_id")
        != "codefabric.governance.plan-overlap-dispositions"
        or not isinstance(document.get("records"), list)
    ):
        raise PlanAssuranceError(f"{OVERLAP_DISPOSITIONS} has an invalid root")
    result: dict[tuple[str, frozenset[str]], Mapping[str, Any]] = {}
    for record in document["records"]:
        if not isinstance(record, Mapping):
            raise PlanAssuranceError("overlap disposition must be a mapping")
        packets = record.get("packets")
        resource = record.get("resource")
        if (
            not isinstance(packets, list)
            or len(packets) != 2
            or not all(isinstance(packet, str) for packet in packets)
            or not isinstance(resource, str)
        ):
            raise PlanAssuranceError("overlap disposition key is invalid")
        if not all(
            isinstance(record.get(field), str) and record[field].strip()
            for field in ("left_phase", "right_phase", "rationale")
        ):
            raise PlanAssuranceError(
                f"{resource} overlap lacks disjoint phase evidence"
            )
        key = (resource, frozenset(packets))
        if key in result:
            raise PlanAssuranceError(f"duplicate overlap disposition {key}")
        result[key] = record
    return result


def validate_dependencies(root: Path = ROOT) -> tuple[int, int]:
    plan_path, _, _ = _active(root)
    dependencies = _dependency_map(plan_path)
    _topological_order(dependencies)
    _validate_ontology_fabric_readiness_states(dependencies)
    blocks = artifact_contracts._packet_blocks(plan_path)
    resources = {
        packet: _known_touch_resources(block) for packet, block in blocks.items()
    }
    dispositions = {
        key: value
        for key, value in _load_overlap_dispositions(root).items()
        if key[1] <= blocks.keys()
    }
    required: set[tuple[str, frozenset[str]]] = set()
    packets = sorted(blocks)
    ancestors = {packet: _ancestors(dependencies, packet) for packet in packets}
    for index, left in enumerate(packets):
        for right in packets[index + 1 :]:
            if left in ancestors[right] or right in ancestors[left]:
                continue
            for resource in resources[left] & resources[right]:
                required.add((resource, frozenset((left, right))))
    missing = sorted(
        required - dispositions.keys(), key=lambda item: (item[0], sorted(item[1]))
    )
    stale = sorted(
        dispositions.keys() - required, key=lambda item: (item[0], sorted(item[1]))
    )
    if missing:
        rendered = [
            f"{resource}:{','.join(sorted(pair))}" for resource, pair in missing
        ]
        raise PlanAssuranceError(
            f"unordered known-touch overlaps lack disposition: {rendered}"
        )
    if stale:
        rendered = [f"{resource}:{','.join(sorted(pair))}" for resource, pair in stale]
        raise PlanAssuranceError(f"stale overlap dispositions: {rendered}")
    return len(dependencies), len(required)


def _oracle_contracts(
    plan_path: Path,
    *,
    selected_packets: set[str] | None = None,
    allow_legacy_mapping: bool = False,
) -> dict[str, list[tuple[str, str]]]:
    result: dict[str, list[tuple[str, str]]] = {}
    seen_oracles: set[str] = set()
    seen_criteria: set[str] = set()
    for packet, block in artifact_contracts._packet_blocks(plan_path).items():
        if selected_packets is not None and packet not in selected_packets:
            continue
        current_mapping = re.search(
            r"\*\*Target invariants\.\*\*.+?\*\*Design(?: and library)? references\.\*\*",
            block,
            re.DOTALL,
        )
        legacy_mapping = allow_legacy_mapping and re.search(
            r"\*\*Target invariants\.\*\*.+?Design references:",
            block,
            re.DOTALL,
        )
        if current_mapping is None and legacy_mapping is None:
            raise PlanAssuranceError(f"{packet} lacks target/design mapping")
        pairs = re.findall(
            r"Executable oracle:\s*`([^`]+)`[^\n]*\n\s*"
            r"Governed criterion:\s*`([^`]+)`",
            block,
        )
        if not pairs:
            catalog = re.search(
                r"Oracle catalog:\s*(.*?)(?=\n\n(?:- \*\*Behavioral|\*\*Edit-Local Gates\.\*\*))",
                block,
                re.DOTALL,
            )
            oracles = (
                re.findall(r"Executable oracle:\s*`([^`]+)`", catalog.group(1))
                if catalog is not None
                else []
            )
            pairs = [
                (oracle, f"PC-{packet}-{ORACLE_KINDS[index]}")
                for index, oracle in enumerate(oracles)
            ]
        if len(pairs) != 4:
            raise PlanAssuranceError(
                f"{packet} must map exactly four oracle/criterion pairs"
            )
        for index, (oracle, criterion) in enumerate(pairs):
            expected = f"PC-{packet}-{ORACLE_KINDS[index]}"
            if criterion != expected:
                raise PlanAssuranceError(
                    f"{packet} criterion {criterion} must be {expected}"
                )
            if oracle in seen_oracles or criterion in seen_criteria:
                raise PlanAssuranceError(f"duplicate oracle or criterion in {packet}")
            seen_oracles.add(oracle)
            seen_criteria.add(criterion)
        result[packet] = pairs
    return result


def _python_body(node: ast.FunctionDef | ast.AsyncFunctionDef) -> list[ast.stmt]:
    body = list(node.body)
    if (
        body
        and isinstance(body[0], ast.Expr)
        and isinstance(body[0].value, ast.Constant)
        and isinstance(body[0].value.value, str)
    ):
        body.pop(0)
    return body


def _python_alias(body: Sequence[ast.stmt]) -> bool:
    if len(body) != 1:
        return False
    statement = body[0]
    return (
        isinstance(statement, ast.Expr) and isinstance(statement.value, ast.Call)
    ) or (isinstance(statement, ast.Return) and isinstance(statement.value, ast.Call))


def _python_substantive(body: Sequence[ast.stmt]) -> bool:
    if not body:
        return False
    for statement in body:
        if isinstance(statement, ast.Pass):
            continue
        if isinstance(statement, ast.Expr) and isinstance(
            statement.value, ast.Constant
        ):
            continue
        if isinstance(statement, ast.Return) and (
            statement.value is None or isinstance(statement.value, ast.Constant)
        ):
            continue
        return True
    return False


def _python_definitions(root: Path, wanted: set[str]) -> list[OracleDefinition]:
    definitions: list[OracleDefinition] = []
    for source_root in SOURCE_ROOTS:
        base = root / source_root
        if not base.exists():
            continue
        for path in base.rglob("*.py"):
            if any(part in {".venv", "target", "generated"} for part in path.parts):
                continue
            try:
                tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            except (OSError, SyntaxError, UnicodeDecodeError):
                continue
            for node in ast.walk(tree):
                if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    continue
                oracle = node.name.removeprefix("test_")
                if oracle not in wanted:
                    continue
                body = _python_body(node)
                if _python_alias(body):
                    raise PlanAssuranceError(
                        f"{oracle} is a single-call Python alias in {path}"
                    )
                if not _python_substantive(body):
                    continue
                definitions.append(
                    OracleDefinition(
                        oracle, "python", path.relative_to(root).as_posix(), node.name
                    )
                )
    return definitions


def _strip_rust_noncode(source: str) -> str:
    """Blank comments and literals while preserving byte positions and newlines."""
    output = list(source)
    index = 0
    block_depth = 0
    state = "code"
    raw_hashes = 0
    while index < len(source):
        pair = source[index : index + 2]
        if state == "code" and pair == "//":
            state = "line_comment"
            output[index : index + 2] = "  "
            index += 2
            continue
        if state == "line_comment":
            if source[index] == "\n":
                state = "code"
            else:
                output[index] = " "
            index += 1
            continue
        if state == "code" and pair == "/*":
            state = "block_comment"
            block_depth = 1
            output[index : index + 2] = "  "
            index += 2
            continue
        if state == "block_comment":
            if pair == "/*":
                block_depth += 1
                output[index : index + 2] = "  "
                index += 2
            elif pair == "*/":
                block_depth -= 1
                output[index : index + 2] = "  "
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                if source[index] != "\n":
                    output[index] = " "
                index += 1
            continue
        if state == "code" and pair == 'b"':
            state = "string"
            output[index : index + 2] = "  "
            index += 2
            continue
        if state == "code" and source[index] == '"':
            state = "string"
            output[index] = " "
            index += 1
            continue
        if state == "string":
            if source[index] == "\\":
                output[index] = " "
                if index + 1 < len(source):
                    output[index + 1] = " "
                index += 2
            else:
                if source[index] == '"':
                    state = "code"
                if source[index] != "\n":
                    output[index] = " "
                index += 1
            continue
        if state == "code" and source[index] in {"b", "r"}:
            raw = re.match(r'(?:b)?r(#{0,255})"', source[index:])
            if raw is not None:
                raw_hashes = len(raw.group(1))
                width = raw.end()
                output[index : index + width] = " " * width
                index += width
                state = "raw_string"
                continue
        if state == "raw_string":
            terminator = '"' + ("#" * raw_hashes)
            if source.startswith(terminator, index):
                output[index : index + len(terminator)] = " " * len(terminator)
                index += len(terminator)
                state = "code"
            else:
                if source[index] != "\n":
                    output[index] = " "
                index += 1
            continue
        index += 1
    return "".join(output)


def _balanced_body(source: str, opening: int) -> str | None:
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : index]
    return None


def _rust_definitions(root: Path, wanted: set[str]) -> list[OracleDefinition]:
    definitions: list[OracleDefinition] = []
    if not wanted:
        return definitions
    expression = re.compile(
        r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
        r"(?:unsafe\s+)?fn\s+("
        + "|".join(map(re.escape, sorted(wanted)))
        + r")\s*\([^)]*\)[^{;]*\{"
    )
    for source_root in SOURCE_ROOTS:
        base = root / source_root
        if not base.exists():
            continue
        for path in base.rglob("*.rs"):
            if any(part in {"target", "generated"} for part in path.parts):
                continue
            try:
                source = path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue
            code = _strip_rust_noncode(source)
            for match in expression.finditer(code):
                body = _balanced_body(code, match.end() - 1)
                if body is None:
                    continue
                normalized = re.sub(r"\s+", " ", body).strip()
                if not normalized or re.fullmatch(
                    r"(?:return\s+)?(?:true|false|None|\(\)|[0-9_]+);?",
                    normalized,
                ):
                    continue
                if re.fullmatch(
                    r"(?:return\s+)?(?:[A-Za-z_][A-Za-z0-9_]*::)*"
                    r"[A-Za-z_][A-Za-z0-9_]*\([^;{}]*\)\??;?",
                    normalized,
                ):
                    raise PlanAssuranceError(
                        f"{match.group(1)} is a single-call Rust alias in {path}"
                    )
                definitions.append(
                    OracleDefinition(
                        match.group(1),
                        "rust",
                        path.relative_to(root).as_posix(),
                        match.group(1),
                    )
                )
    return definitions


def oracle_definitions(root: Path, wanted: set[str]) -> list[OracleDefinition]:
    return _python_definitions(root, wanted) + _rust_definitions(root, wanted)


def _require_exact_definitions(
    wanted: set[str],
    definitions: Sequence[OracleDefinition],
    *,
    context: str,
) -> dict[str, OracleDefinition]:
    by_oracle: dict[str, list[OracleDefinition]] = defaultdict(list)
    for definition in definitions:
        by_oracle[definition.oracle].append(definition)
    missing = sorted(wanted - by_oracle.keys())
    duplicated = sorted(
        oracle for oracle, values in by_oracle.items() if len(values) != 1
    )
    unexpected = sorted(by_oracle.keys() - wanted)
    if missing:
        raise PlanAssuranceError(f"{context} lacks definitions: {missing}")
    if duplicated:
        raise PlanAssuranceError(f"{context} has duplicate definitions: {duplicated}")
    if unexpected:
        raise PlanAssuranceError(f"{context} has unexpected definitions: {unexpected}")
    return {oracle: values[0] for oracle, values in by_oracle.items()}


def validate_oracle_substance(root: Path = ROOT) -> tuple[int, int]:
    plan_path, _, state = _active(root)
    contracts = _oracle_contracts(plan_path)
    required_packets = {
        packet
        for packet, entry in state["packets"].items()
        if entry["status"] in {"in_progress", "complete"}
    }
    wanted = {oracle for packet in required_packets for oracle, _ in contracts[packet]}
    definitions = oracle_definitions(root, wanted)
    _require_exact_definitions(
        wanted,
        definitions,
        context="active/complete oracles",
    )
    recipes = artifact_contracts.load_just_recipes(root)
    if "packet-oracle-check" not in recipes:
        raise PlanAssuranceError("packet-oracle-check selector is absent")
    return sum(len(values) for values in contracts.values()), len(definitions)


def _rust_selector_command(domain: str, oracles: Sequence[str]) -> list[str]:
    command = ["cargo", "nextest", "run", "--locked", "--no-fail-fast"]
    if domain == "root-model-compiler":
        command.extend(
            (
                "--no-default-features",
                "--features",
                "model-compiler",
                "--bin",
                "codefabric-model",
            )
        )
    elif domain != "root":
        command.extend(("--manifest-path", domain))
    expression = "test(/(" + "|".join(sorted(map(re.escape, oracles))) + ")/)"
    command.extend(("-E", expression, "--no-tests=fail"))
    return command


def run_packet_oracles(
    packet: str,
    root: Path = ROOT,
    plan_path: Path | None = None,
) -> None:
    explicit_plan = plan_path is not None
    if plan_path is None:
        plan_path = _active(root)[0]
    elif not plan_path.is_absolute():
        plan_path = root / plan_path
    plan_path = plan_path.resolve()
    try:
        plan_path.relative_to(root.resolve())
    except ValueError as error:
        raise PlanAssuranceError(
            f"packet oracle plan escapes the repository: {plan_path}"
        ) from error
    if not plan_path.is_file():
        raise PlanAssuranceError(f"packet oracle plan does not exist: {plan_path}")
    contracts = _oracle_contracts(
        plan_path,
        selected_packets={packet} if explicit_plan else None,
        allow_legacy_mapping=explicit_plan,
    )
    if packet not in contracts:
        raise PlanAssuranceError(f"unknown packet {packet}")
    wanted = {oracle for oracle, _ in contracts[packet]}
    definitions = oracle_definitions(root, wanted)
    by_oracle = _require_exact_definitions(
        wanted,
        definitions,
        context=f"{packet} selector",
    )
    python_nodes = sorted(
        f"{value.path}::{value.function}"
        for value in by_oracle.values()
        if value.language == "python"
    )
    rust_definitions = [
        value for value in by_oracle.values() if value.language == "rust"
    ]
    if python_nodes:
        subprocess.run(
            (
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
                *python_nodes,
            ),
            cwd=root,
            check=True,
        )
    if rust_definitions:
        roots: dict[str, list[str]] = defaultdict(list)
        for definition in rust_definitions:
            domain = "root"
            if definition.path.startswith("src/bin/codefabric_model/"):
                domain = "root-model-compiler"
            elif definition.path.startswith("rustc-extractor/"):
                domain = "rustc-extractor/Cargo.toml"
            elif definition.path.startswith("pyrefly-sidecar/"):
                domain = "pyrefly-sidecar/Cargo.toml"
            roots[domain].append(definition.oracle)
        for domain, oracles in roots.items():
            command = _rust_selector_command(domain, oracles)
            subprocess.run(command, cwd=root, check=True)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("oracle-substance-check")
    subparsers.add_parser("dependency-check")
    subparsers.add_parser("current-packet-oracle-check")
    packet = subparsers.add_parser("packet-oracle-check")
    packet.add_argument("packet")
    packet.add_argument(
        "--plan",
        type=Path,
        help="select an immutable inactive plan instead of the active plan",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "oracle-substance-check":
            declared, implemented = validate_oracle_substance()
            print(
                f"oracle substance: {declared} declared, {implemented} currently required definitions"
            )
        elif args.command == "dependency-check":
            packets, overlaps = validate_dependencies()
            print(
                f"plan dependency closure: {packets} packets, {overlaps} disjoint-phase overlaps"
            )
        elif args.command == "packet-oracle-check":
            run_packet_oracles(args.packet, plan_path=args.plan)
            print(f"packet oracle selector: {args.packet} passed exactly four oracles")
        else:
            current = _active()[2]["current_packet"]
            if current is None:
                print("packet oracle selector: active plan has no current packet")
            else:
                run_packet_oracles(current)
                print(
                    f"packet oracle selector: current packet {current} passed exactly four oracles"
                )
    except (PlanAssuranceError, subprocess.CalledProcessError) as error:
        print(f"plan assurance failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
