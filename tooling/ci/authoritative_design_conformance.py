"""Validate the terminal authoritative suite and active relational plan."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from tooling.ci.artifact_contracts import (
    ArtifactContractError,
    active_plan_path,
    parse_frontmatter,
    validate_plan,
)

ROOT = Path(__file__).resolve().parents[2]
AUTHORITY_ROOT = Path("docs/authoritative_design")
CURRENT_SUITE_ID = "codefabric-relational-data-fabric"
REQUIRED_TAGS = frozenset({"SUITE", "ONT", "GEN", "FAB", "QRY", "LIFE", "SRV", "RM"})
LEGACY_ROOT = b"docs/" + b"upfront_design"
LEGACY_OWNER = b"upfront" + b"-design"
MAX_SOURCE_BYTES = 16 * 1024 * 1024

HISTORICAL_PREFIXES = (
    "docs/designs/",
    "docs/plans/",
    "docs/reviews/",
    "docs/library_ref/",
)

NAVIGATION_SURFACES = (
    Path("AGENTS.md"),
    Path("docs/spec_index/README.md"),
)


class AuthoritativeDesignError(ValueError):
    """The authoritative suite or one of its authority routes is inconsistent."""


@dataclass(frozen=True)
class MasterContract:
    """One master in the unique synchronized terminal suite."""

    path: Path
    artifact_id: str
    artifact_tag: str
    artifact_version: str
    suite_version: str
    predecessor_path: Path


def _git_paths(root: Path) -> list[str]:
    completed = subprocess.run(
        ("git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"),
        cwd=root,
        check=True,
        capture_output=True,
    )
    return [item.decode("utf-8") for item in completed.stdout.split(b"\0") if item]


def _tracked_masters(root: Path) -> set[Path]:
    completed = subprocess.run(
        (
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            f"{AUTHORITY_ROOT.as_posix()}/*.md",
        ),
        cwd=root,
        check=True,
        capture_output=True,
    )
    return {
        Path(item.decode("utf-8")) for item in completed.stdout.split(b"\0") if item
    }


def _frontmatter(path: Path) -> dict[str, Any] | None:
    if not path.read_bytes().startswith(b"---\n"):
        return None
    try:
        return parse_frontmatter(path)
    except ArtifactContractError as error:
        raise AuthoritativeDesignError(str(error)) from error


def _relative(path: Path, root: Path) -> Path:
    try:
        return path.resolve().relative_to(root.resolve())
    except ValueError as error:
        raise AuthoritativeDesignError(f"path escapes repository: {path}") from error


def _safe_predecessor(
    path: Path, metadata: dict[str, Any], directory: Path, root: Path
) -> Path:
    predecessor = Path(str(metadata["predecessor_path"]))
    if predecessor.is_absolute() or ".." in predecessor.parts:
        raise AuthoritativeDesignError(f"invalid predecessor path: {path}")
    absolute = root / predecessor
    if (
        not absolute.is_file()
        or absolute.parent.resolve() != directory.resolve()
        or absolute.resolve() == path.resolve()
    ):
        raise AuthoritativeDesignError(
            f"unresolved or external predecessor for {path}: {predecessor}"
        )
    return predecessor


def _historical_chain_paths(
    root: Path,
    directory: Path,
    terminal: dict[str, MasterContract],
) -> set[Path]:
    """Return every exact predecessor reachable from the terminal suite."""
    expected: set[Path] = set()
    for tag, contract in terminal.items():
        successor = contract.path
        predecessor = contract.predecessor_path
        seen = {successor}
        while True:
            if predecessor in seen:
                raise AuthoritativeDesignError(
                    f"historical predecessor cycle for {tag}: {predecessor}"
                )
            seen.add(predecessor)
            absolute = root / predecessor
            if (
                not absolute.is_file()
                or absolute.parent.resolve() != directory.resolve()
            ):
                raise AuthoritativeDesignError(
                    f"unresolved or external historical predecessor for {tag}: "
                    f"{predecessor}"
                )
            if predecessor in expected:
                raise AuthoritativeDesignError(
                    f"historical predecessor shared across roles: {predecessor}"
                )
            expected.add(predecessor)
            metadata = _frontmatter(absolute)
            if metadata is None:
                break
            if (
                metadata.get("artifact") != "authoritative-design"
                or metadata.get("suite_id") != CURRENT_SUITE_ID
                or metadata.get("artifact_tag") != tag
            ):
                raise AuthoritativeDesignError(
                    f"historical role/identity chain differs for {tag}: {predecessor}"
                )
            declared_successor = metadata.get("successor_path")
            if (
                declared_successor is not None
                and declared_successor != successor.as_posix()
            ):
                raise AuthoritativeDesignError(
                    f"historical successor link differs for {tag}: {predecessor}"
                )
            next_predecessor = metadata.get("predecessor_path")
            if next_predecessor is None:
                break
            candidate = Path(str(next_predecessor))
            if candidate.is_absolute() or ".." in candidate.parts:
                raise AuthoritativeDesignError(
                    f"invalid historical predecessor path: {predecessor}"
                )
            successor = predecessor
            predecessor = candidate
    return expected


def validate_master_directory(
    directory: Path, *, root: Path = ROOT
) -> dict[str, MasterContract]:
    """Discover the unique synchronized terminal suite from predecessor edges."""
    if not directory.is_dir():
        raise AuthoritativeDesignError(f"missing authoritative root: {directory}")
    entries = list(directory.iterdir())
    stray = sorted(
        item.name for item in entries if not item.is_file() or item.suffix != ".md"
    )
    if stray:
        raise AuthoritativeDesignError(f"non-master entries in authority root: {stray}")

    all_paths: set[Path] = set()
    metadata_by_path: dict[Path, dict[str, Any]] = {}
    required = {
        "artifact",
        "artifact_id",
        "suite_id",
        "suite_version",
        "artifact_tag",
        "artifact_version",
        "authority_status",
        "predecessor_path",
    }
    referenced_predecessors: set[Path] = set()

    for path in sorted(entries):
        payload = path.read_bytes()
        if len(payload) > MAX_SOURCE_BYTES:
            raise AuthoritativeDesignError(
                f"authoritative master exceeds limit: {path}"
            )
        try:
            payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise AuthoritativeDesignError(
                f"authoritative master is not UTF-8: {path}"
            ) from error
        relative = _relative(path, root)
        all_paths.add(relative)
        metadata = _frontmatter(path)
        if metadata is None:
            continue
        missing = required - metadata.keys()
        if missing:
            raise AuthoritativeDesignError(
                f"master missing identity fields {sorted(missing)}: {path}"
            )
        if metadata["artifact"] != "authoritative-design":
            raise AuthoritativeDesignError(f"invalid artifact class: {path}")
        if metadata["suite_id"] != CURRENT_SUITE_ID:
            raise AuthoritativeDesignError(f"coequal suite detected: {path}")
        predecessor = _safe_predecessor(path, metadata, directory, root)
        if predecessor in referenced_predecessors:
            raise AuthoritativeDesignError(
                f"historical predecessor has multiple successors: {predecessor}"
            )
        referenced_predecessors.add(predecessor)
        metadata_by_path[relative] = metadata

    terminal_paths = set(metadata_by_path) - referenced_predecessors
    terminal: dict[str, MasterContract] = {}
    terminal_ids: set[str] = set()
    terminal_versions: set[str] = set()
    for relative in sorted(terminal_paths):
        metadata = metadata_by_path[relative]
        if metadata.get("authority_status") != "current":
            raise AuthoritativeDesignError(
                f"terminal master is not current at issuance: {relative}"
            )
        tag = str(metadata["artifact_tag"])
        artifact_id = str(metadata["artifact_id"])
        if tag in terminal:
            raise AuthoritativeDesignError(f"duplicate terminal artifact tag {tag}")
        if artifact_id in terminal_ids:
            raise AuthoritativeDesignError(
                f"duplicate terminal artifact ID {artifact_id}"
            )
        text = (root / relative).read_text(encoding="utf-8")
        marker = f"{chr(96)}{artifact_id}{chr(96)}"
        if marker not in text:
            raise AuthoritativeDesignError(
                f"artifact identity missing from body: {relative}"
            )
        suite_version = str(metadata["suite_version"])
        terminal_versions.add(suite_version)
        terminal_ids.add(artifact_id)
        terminal[tag] = MasterContract(
            path=relative,
            artifact_id=artifact_id,
            artifact_tag=tag,
            artifact_version=str(metadata["artifact_version"]),
            suite_version=suite_version,
            predecessor_path=Path(str(metadata["predecessor_path"])),
        )

    if set(terminal) != REQUIRED_TAGS:
        raise AuthoritativeDesignError(
            "terminal authority roles differ: "
            f"missing={sorted(REQUIRED_TAGS - set(terminal))}, "
            f"extra={sorted(set(terminal) - REQUIRED_TAGS)}"
        )
    if len(terminal_versions) != 1:
        raise AuthoritativeDesignError(
            f"terminal suite versions differ: {sorted(terminal_versions)}"
        )

    historical_paths = all_paths - {contract.path for contract in terminal.values()}
    predecessor_paths = _historical_chain_paths(root, directory, terminal)
    if historical_paths != predecessor_paths:
        raise AuthoritativeDesignError(
            "historical predecessor closure differs: "
            f"missing={sorted(predecessor_paths - historical_paths)}, "
            f"extra={sorted(historical_paths - predecessor_paths)}"
        )
    return terminal


def _legacy_hits(root: Path) -> tuple[list[str], list[str]]:
    live: list[str] = []
    historical: list[str] = []
    for relative in _git_paths(root):
        if relative.startswith((".git/", "target/")):
            continue
        candidate = root / relative
        if not candidate.is_file() or candidate.stat().st_size > MAX_SOURCE_BYTES:
            continue
        payload = candidate.read_bytes()
        if LEGACY_ROOT not in payload and LEGACY_OWNER not in payload:
            continue
        if relative.startswith(HISTORICAL_PREFIXES):
            historical.append(relative)
        else:
            live.append(relative)
    return live, historical


def _validate_navigation(root: Path, terminal: dict[str, MasterContract]) -> None:
    for navigation in NAVIGATION_SURFACES:
        text = (root / navigation).read_text(encoding="utf-8")
        missing = [
            contract.path.name
            for contract in terminal.values()
            if contract.path.name not in text
        ]
        if missing:
            raise AuthoritativeDesignError(
                f"{navigation} omits terminal authority paths: {sorted(missing)}"
            )
        if CURRENT_SUITE_ID not in text:
            raise AuthoritativeDesignError(
                f"{navigation} omits current suite identity {CURRENT_SUITE_ID}"
            )


def _validate_active_relational_plan(root: Path) -> str:
    active = _relative(active_plan_path(root), root)
    try:
        plan = validate_plan(root, root / active)
    except ArtifactContractError as error:
        raise AuthoritativeDesignError(str(error)) from error
    if (
        plan.get("status") != "approved"
        or plan.get("plan_id") != "codefabric-execution-proved-relational-data-fabric"
    ):
        raise AuthoritativeDesignError(
            f"active plan is not the approved relational successor: {active}"
        )
    design = parse_frontmatter(root / str(plan.get("design_path", "")))
    if (
        design.get("principles_path")
        != "docs/library_ref/full_data_fabric_design_principles_v2.md"
        or design.get("target_status") != "accepted"
    ):
        raise AuthoritativeDesignError(
            "active relational design does not select the accepted v2 doctrine target"
        )
    return f"active-{plan.get('version')}"


def validate_authoritative_design(root: Path = ROOT) -> dict[str, Any]:
    """Validate terminal masters, history, navigation, and active selection."""
    terminal = validate_master_directory(root / AUTHORITY_ROOT, root=root)
    tracked = _tracked_masters(root)
    historical = _historical_chain_paths(root, root / AUTHORITY_ROOT, terminal)
    expected_tracked = {contract.path for contract in terminal.values()} | historical
    if tracked != expected_tracked:
        raise AuthoritativeDesignError(
            "tracked authority closure differs: "
            f"missing={sorted(expected_tracked - tracked)}, "
            f"extra={sorted(tracked - expected_tracked)}"
        )

    outlined = subprocess.run(
        ("just", "spec-outline"),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if outlined.returncode != 0:
        raise AuthoritativeDesignError("default spec-outline failed")
    missing_outlines = [
        contract.path.name
        for contract in terminal.values()
        if contract.path.name not in outlined.stdout
    ]
    if missing_outlines:
        raise AuthoritativeDesignError(
            f"spec-outline omitted terminal masters: {sorted(missing_outlines)}"
        )

    _validate_navigation(root, terminal)
    plan_selection = _validate_active_relational_plan(root)
    live_hits, historical_hits = _legacy_hits(root)
    if live_hits:
        raise AuthoritativeDesignError(f"legacy live authority remains: {live_hits}")
    suite_versions = {contract.suite_version for contract in terminal.values()}
    suite_version = next(iter(suite_versions))
    return {
        "current_master_count": len(terminal),
        "historical_master_count": len(historical),
        "generated_manifest_authority_count": 0,
        "historical_exclusion_count": len(historical_hits),
        "suite_id": CURRENT_SUITE_ID,
        "suite_version": suite_version,
        "authority_root": AUTHORITY_ROOT.as_posix(),
        "plan_selection": plan_selection,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        report = validate_authoritative_design(args.root.resolve())
    except (
        ArtifactContractError,
        AuthoritativeDesignError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"authoritative design conformance error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
