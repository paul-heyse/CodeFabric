"""Validate and execute the design-principle alignment authorities.

The YAML files are the reviewable source authorities.  This module derives
traceability, current detector observations, and dirty-tree ownership from
them so a prose review cannot become an independent status ledger.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[2]
PRINCIPLE_REGISTRY = Path("contracts/registry/design-principle-registry.yaml")
DETECTOR_REGISTRY = Path("contracts/registry/design-principle-detector-registry.yaml")
BASELINE_REGISTRY = Path("contracts/governance/design-principle-baseline.yaml")
ACTIVE_PLAN_POINTER = Path("docs/plans/active-plan.json")
PRINCIPLE_IDS = {f"P{number}" for number in range(1, 26)}
DETECTOR_IDS = {f"DP-{number:03d}" for number in range(1, 125)}
DISPOSITIONS = {"open", "partial", "closed", "invalid", "changed"}
SEVERITIES = {"blocker", "major", "minor", "observation"}
DIRTY_DISPOSITIONS = {
    "plan_owned",
    "execution_state",
    "owner_work_preserve",
    "planned_integration_preserve",
}
REVIEW_EXCLUSIONS = ("docs/reviews/**", "docs/library_ref/**", ".git/**", "target/**")


class AlignmentContractError(ValueError):
    """An alignment authority or observation violates its closed contract."""


@dataclass(frozen=True)
class ProbeObservation:
    detector_id: str
    matched_files: tuple[str, ...]
    match_count: int
    disposition: str


def _load_yaml(path: Path, root: Path = ROOT) -> Mapping[str, Any]:
    try:
        value = yaml.safe_load((root / path).read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError) as error:
        raise AlignmentContractError(f"cannot load {path}: {error}") from error
    if not isinstance(value, Mapping):
        raise AlignmentContractError(f"{path} root must be a mapping")
    return value


def _records(document: Mapping[str, Any], path: Path) -> list[Mapping[str, Any]]:
    value = document.get("records")
    if not isinstance(value, list) or not all(
        isinstance(item, Mapping) for item in value
    ):
        raise AlignmentContractError(f"{path} records must be a list of mappings")
    return list(value)


def _exact_ids(
    records: Sequence[Mapping[str, Any]], key: str, expected: set[str]
) -> None:
    ids = [record.get(key) for record in records]
    if not all(isinstance(value, str) for value in ids):
        raise AlignmentContractError(f"every {key} must be a string")
    duplicates = sorted({value for value in ids if ids.count(value) > 1})
    if duplicates:
        raise AlignmentContractError(f"duplicate {key} values: {', '.join(duplicates)}")
    actual = set(ids)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise AlignmentContractError(
            f"{key} census mismatch; missing={missing}, extra={extra}"
        )


def _active_plan(root: Path) -> tuple[Path, str]:
    try:
        pointer = json.loads((root / ACTIVE_PLAN_POINTER).read_text(encoding="utf-8"))
        plan_path = Path(pointer["plan_path"])
        return plan_path, (root / plan_path).read_text(encoding="utf-8")
    except (OSError, KeyError, json.JSONDecodeError, TypeError) as error:
        raise AlignmentContractError(f"cannot resolve active plan: {error}") from error


def _just_recipes(root: Path) -> set[str]:
    completed = subprocess.run(
        ("just", "--dump", "--dump-format", "json"),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return set(json.loads(completed.stdout)["recipes"])


def validate_traceability(root: Path = ROOT) -> tuple[int, int]:
    """Validate the P1-P25 and DP-001-DP-124 authority closure."""
    principle_records = _records(
        _load_yaml(PRINCIPLE_REGISTRY, root), PRINCIPLE_REGISTRY
    )
    detector_records = _records(_load_yaml(DETECTOR_REGISTRY, root), DETECTOR_REGISTRY)
    _exact_ids(principle_records, "principle_id", PRINCIPLE_IDS)
    _exact_ids(detector_records, "detector_id", DETECTOR_IDS)
    recipes = _just_recipes(root)
    plan_path, plan_text = _active_plan(root)

    finding_owners: dict[str, set[str]] = {}
    for principle in principle_records:
        principle_id = str(principle["principle_id"])
        owner = principle.get("owner")
        authorities = principle.get("normative_authorities")
        proof_recipes = principle.get("proof_recipes")
        findings = principle.get("findings", [])
        if not isinstance(owner, str) or not owner:
            raise AlignmentContractError(f"{principle_id} has no owner")
        if not isinstance(authorities, list) or not authorities:
            raise AlignmentContractError(f"{principle_id} has no normative authority")
        for authority in authorities:
            if not isinstance(authority, Mapping):
                raise AlignmentContractError(
                    f"{principle_id} authority must be a mapping"
                )
            path_value = authority.get("path")
            anchor = authority.get("anchor")
            if (
                not isinstance(path_value, str)
                or not isinstance(anchor, str)
                or not anchor
            ):
                raise AlignmentContractError(
                    f"{principle_id} authority path/anchor is incomplete"
                )
            try:
                source = (root / path_value).read_text(encoding="utf-8")
            except OSError as error:
                raise AlignmentContractError(
                    f"{principle_id} authority unreadable: {error}"
                ) from error
            if anchor not in source:
                raise AlignmentContractError(
                    f"{principle_id} authority anchor {anchor!r} is absent from {path_value}"
                )
        if not isinstance(proof_recipes, list) or not proof_recipes:
            raise AlignmentContractError(f"{principle_id} has no proof recipe")
        unknown_recipes = sorted(set(proof_recipes) - recipes)
        if unknown_recipes:
            raise AlignmentContractError(
                f"{principle_id} names unavailable recipes: {', '.join(unknown_recipes)}"
            )
        if not isinstance(findings, list) or not all(
            item in DETECTOR_IDS for item in findings
        ):
            raise AlignmentContractError(
                f"{principle_id} has invalid finding references"
            )
        for finding in findings:
            finding_owners.setdefault(str(finding), set()).add(principle_id)

    for detector in detector_records:
        detector_id = str(detector["detector_id"])
        principles = detector.get("principles")
        packets = detector.get("owning_packets")
        command = detector.get("command")
        coverage = detector.get("coverage")
        if (
            not isinstance(principles, list)
            or not principles
            or not set(principles) <= PRINCIPLE_IDS
        ):
            raise AlignmentContractError(f"{detector_id} has invalid principles")
        if finding_owners.get(detector_id, set()) != set(principles):
            raise AlignmentContractError(
                f"{detector_id} principle mapping disagrees with the principle registry"
            )
        if detector.get("disposition") not in DISPOSITIONS:
            raise AlignmentContractError(f"{detector_id} has invalid disposition")
        if detector.get("severity") not in SEVERITIES:
            raise AlignmentContractError(f"{detector_id} has invalid severity")
        if not isinstance(packets, list) or not packets:
            raise AlignmentContractError(f"{detector_id} has no owning packet")
        for packet in packets:
            if not isinstance(packet, str) or not re.fullmatch(r"WP\d+", packet):
                raise AlignmentContractError(
                    f"{detector_id} has invalid packet {packet!r}"
                )
            if not re.search(rf"^### {re.escape(packet)}\s+—", plan_text, re.MULTILINE):
                raise AlignmentContractError(
                    f"{detector_id} packet {packet} is absent from active plan {plan_path}"
                )
        expected_command = f"just alignment-detector-check {detector_id}"
        if command != expected_command:
            raise AlignmentContractError(
                f"{detector_id} command must be {expected_command!r}, got {command!r}"
            )
        if not isinstance(coverage, Mapping):
            raise AlignmentContractError(f"{detector_id} coverage must be a mapping")
        exclusions = coverage.get("exclude")
        if exclusions != list(REVIEW_EXCLUSIONS):
            raise AlignmentContractError(
                f"{detector_id} must use the standing non-self-matching exclusions"
            )
        _validate_probe(detector_id, detector.get("probe"))
    return len(principle_records), len(detector_records)


def _validate_probe(detector_id: str, value: Any) -> None:
    if not isinstance(value, Mapping):
        raise AlignmentContractError(f"{detector_id} probe must be a mapping")
    if value.get("kind") not in {"contains", "path_count"}:
        raise AlignmentContractError(f"{detector_id} probe kind is invalid")
    paths = value.get("paths")
    if (
        not isinstance(paths, list)
        or not paths
        or not all(isinstance(path, str) for path in paths)
    ):
        raise AlignmentContractError(f"{detector_id} probe paths are invalid")
    minimum = value.get("min_matches")
    maximum = value.get("max_matches")
    if not isinstance(minimum, int) or minimum < 0:
        raise AlignmentContractError(f"{detector_id} min_matches is invalid")
    if maximum is not None and (not isinstance(maximum, int) or maximum < minimum):
        raise AlignmentContractError(f"{detector_id} max_matches is invalid")
    if minimum == 0 and maximum is None:
        raise AlignmentContractError(f"{detector_id} probe is vacuous")
    if value["kind"] == "contains":
        pattern = value.get("pattern")
        if not isinstance(pattern, str) or len(pattern) < 3 or detector_id in pattern:
            raise AlignmentContractError(f"{detector_id} probe pattern is vacuous")


def _candidate_files(
    root: Path, patterns: Sequence[str], exclusions: Sequence[str]
) -> list[Path]:
    files: set[Path] = set()
    for pattern in patterns:
        if any(character in pattern for character in "*?["):
            files.update(path for path in root.glob(pattern) if path.is_file())
        else:
            path = root / pattern
            if path.is_file():
                files.add(path)
    return sorted(
        path
        for path in files
        if not any(
            fnmatch.fnmatch(path.relative_to(root).as_posix(), excluded)
            for excluded in exclusions
        )
    )


def execute_detector(record: Mapping[str, Any], root: Path = ROOT) -> ProbeObservation:
    detector_id = str(record["detector_id"])
    probe = record["probe"]
    coverage = record["coverage"]
    files = _candidate_files(root, probe["paths"], coverage["exclude"])
    if probe["kind"] == "path_count":
        count = len(files)
        matched = files
    else:
        expression = re.compile(str(probe["pattern"]), re.MULTILINE)
        matched = []
        count = 0
        for path in files:
            try:
                source = path.read_text(encoding="utf-8")
            except UnicodeDecodeError as error:
                raise AlignmentContractError(
                    f"{detector_id} attempted a text probe over binary file {path}"
                ) from error
            matches = expression.findall(source)
            if matches:
                matched.append(path)
                count += len(matches)
    minimum = int(probe["min_matches"])
    maximum = probe.get("max_matches")
    if count < minimum or (maximum is not None and count > int(maximum)):
        raise AlignmentContractError(
            f"{detector_id} observed {count} matches outside [{minimum}, {maximum}] "
            f"over {[path.relative_to(root).as_posix() for path in files]}"
        )
    return ProbeObservation(
        detector_id=detector_id,
        matched_files=tuple(path.relative_to(root).as_posix() for path in matched),
        match_count=count,
        disposition=str(record["disposition"]),
    )


def execute_detectors(
    detector_id: str | None = None, root: Path = ROOT
) -> list[ProbeObservation]:
    validate_traceability(root)
    records = _records(_load_yaml(DETECTOR_REGISTRY, root), DETECTOR_REGISTRY)
    selected = [
        record
        for record in records
        if detector_id is None or record["detector_id"] == detector_id
    ]
    if detector_id is not None and not selected:
        raise AlignmentContractError(f"unknown detector {detector_id}")
    return [execute_detector(record, root) for record in selected]


def _git_dirty_paths(root: Path) -> dict[str, str]:
    completed = subprocess.run(
        ("git", "status", "--porcelain=v1", "--untracked-files=all", "-z"),
        cwd=root,
        check=True,
        capture_output=True,
    )
    fields = completed.stdout.split(b"\0")
    result: dict[str, str] = {}
    index = 0
    while index < len(fields) and fields[index]:
        field = fields[index]
        status = field[:2].decode("ascii")
        path = field[3:].decode("utf-8", "surrogateescape")
        result[path] = status
        index += 1
        if status[0] in {"R", "C"}:
            index += 1
    return result


def validate_baseline(root: Path = ROOT) -> tuple[int, int]:
    document = _load_yaml(BASELINE_REGISTRY, root)
    records = _records(document, BASELINE_REGISTRY)
    baseline_commit = document.get("baseline_commit")
    if not isinstance(baseline_commit, str):
        raise AlignmentContractError("baseline_commit is absent")
    subprocess.run(
        ("git", "cat-file", "-e", f"{baseline_commit}^{{commit}}"), cwd=root, check=True
    )
    paths: set[str] = set()
    for record in records:
        path = record.get("path")
        disposition = record.get("disposition")
        owner = record.get("owner")
        if not isinstance(path, str) or path in paths:
            raise AlignmentContractError(f"duplicate or invalid baseline path {path!r}")
        paths.add(path)
        if (
            disposition not in DIRTY_DISPOSITIONS
            or not isinstance(owner, str)
            or not owner
        ):
            raise AlignmentContractError(f"{path} has no valid owner disposition")
        if disposition == "owner_work_preserve" and not record.get("recovery"):
            raise AlignmentContractError(f"{path} owner work has no recovery statement")
    current = _git_dirty_paths(root)
    unattributed = sorted(set(current) - paths)
    if unattributed:
        raise AlignmentContractError(
            f"unattributed dirty paths: {', '.join(unattributed)}"
        )
    return len(current), len(records)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("traceability-check")
    detector = subparsers.add_parser("detector-check")
    detector.add_argument("detector_id", nargs="?")
    subparsers.add_parser("baseline-check")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "traceability-check":
            principles, detectors = validate_traceability()
            print(
                f"design-principle traceability: {principles} principles, {detectors} detectors"
            )
        elif args.command == "detector-check":
            observations = execute_detectors(args.detector_id or None)
            for observation in observations:
                print(json.dumps(observation.__dict__, sort_keys=True))
            print(f"alignment detectors: {len(observations)} executed")
        else:
            dirty, attributed = validate_baseline()
            print(
                f"audit baseline: {dirty} current dirty paths, {attributed} attributed paths"
            )
    except (AlignmentContractError, subprocess.CalledProcessError) as error:
        print(f"design-principle alignment check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
