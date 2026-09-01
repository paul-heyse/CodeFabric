"""Validate the discovered v2.1 authoritative suite and successor routing."""

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
CURRENT_SUITE_VERSION = "2.1.0"
REQUIRED_TAGS = frozenset({"SUITE", "ONT", "GEN", "FAB", "QRY", "LIFE", "SRV", "RM"})
SUCCESSOR_PLAN = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md"
)
PREDECESSOR_PLAN = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md"
)
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
    """One current master discovered from its authored identity fields."""

    path: Path
    artifact_id: str
    artifact_tag: str
    artifact_version: str
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


def _historical_chain_paths(
    root: Path,
    directory: Path,
    current: dict[str, MasterContract],
) -> set[Path]:
    """Return the exact recursively linked history for every current role."""
    expected: set[Path] = set()
    for tag, contract in current.items():
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
            if metadata.get("authority_status") != "historical":
                raise AuthoritativeDesignError(
                    f"predecessor remains coequal current authority: {predecessor}"
                )
            if (
                metadata.get("artifact") != "authoritative-design"
                or metadata.get("suite_id") != CURRENT_SUITE_ID
                or metadata.get("artifact_tag") != tag
            ):
                raise AuthoritativeDesignError(
                    f"historical role/identity chain differs for {tag}: {predecessor}"
                )
            if metadata.get("successor_path") != successor.as_posix():
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


def validate_master_directory(
    directory: Path, *, root: Path = ROOT
) -> dict[str, MasterContract]:
    """Discover and validate one current master per stable domain role."""
    if not directory.is_dir():
        raise AuthoritativeDesignError(f"missing authoritative root: {directory}")
    entries = list(directory.iterdir())
    stray = sorted(
        item.name for item in entries if not item.is_file() or item.suffix != ".md"
    )
    if stray:
        raise AuthoritativeDesignError(f"non-master entries in authority root: {stray}")

    current: dict[str, MasterContract] = {}
    current_paths: set[Path] = set()
    artifact_ids: set[str] = set()

    for path in sorted(entries):
        payload = path.read_bytes()
        if len(payload) > MAX_SOURCE_BYTES:
            raise AuthoritativeDesignError(
                f"authoritative master exceeds limit: {path}"
            )
        try:
            text = payload.decode("utf-8")
        except UnicodeDecodeError as error:
            raise AuthoritativeDesignError(
                f"authoritative master is not UTF-8: {path}"
            ) from error
        metadata = _frontmatter(path)
        if metadata is None or metadata.get("authority_status") != "current":
            continue
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
        missing = required - metadata.keys()
        if missing:
            raise AuthoritativeDesignError(
                f"current master missing identity fields {sorted(missing)}: {path}"
            )
        if metadata["artifact"] != "authoritative-design":
            raise AuthoritativeDesignError(f"invalid artifact class: {path}")
        if (
            metadata["suite_id"] != CURRENT_SUITE_ID
            or metadata["suite_version"] != CURRENT_SUITE_VERSION
        ):
            raise AuthoritativeDesignError(f"coequal current suite detected: {path}")
        tag = str(metadata["artifact_tag"])
        artifact_id = str(metadata["artifact_id"])
        version = str(metadata["artifact_version"])
        predecessor = Path(str(metadata["predecessor_path"]))
        if predecessor.is_absolute() or ".." in predecessor.parts:
            raise AuthoritativeDesignError(f"invalid predecessor path: {path}")
        predecessor_absolute = root / predecessor
        if (
            not predecessor_absolute.is_file()
            or predecessor_absolute.parent.resolve() != directory.resolve()
            or predecessor_absolute.resolve() == path.resolve()
        ):
            raise AuthoritativeDesignError(
                f"unresolved or external predecessor for {path}: {predecessor}"
            )
        if tag in current:
            raise AuthoritativeDesignError(f"duplicate current artifact tag {tag}")
        if artifact_id in artifact_ids:
            raise AuthoritativeDesignError(
                f"duplicate current artifact ID {artifact_id}"
            )
        marker = f"{chr(96)}{artifact_id}{chr(96)}"
        if marker not in text:
            raise AuthoritativeDesignError(
                f"artifact identity missing from body: {path}"
            )
        contract = MasterContract(
            path=_relative(path, root),
            artifact_id=artifact_id,
            artifact_tag=tag,
            artifact_version=version,
            predecessor_path=predecessor,
        )
        current[tag] = contract
        current_paths.add(contract.path)
        artifact_ids.add(artifact_id)

    if set(current) != REQUIRED_TAGS:
        raise AuthoritativeDesignError(
            "current authority roles differ: "
            f"missing={sorted(REQUIRED_TAGS - set(current))}, "
            f"extra={sorted(set(current) - REQUIRED_TAGS)}"
        )

    all_paths = {
        _relative(item, root)
        for item in entries
        if item.is_file() and item.suffix == ".md"
    }
    historical_paths = all_paths - current_paths
    predecessor_paths = _historical_chain_paths(root, directory, current)
    if historical_paths != predecessor_paths:
        raise AuthoritativeDesignError(
            "historical predecessor closure differs: "
            f"missing={sorted(predecessor_paths - historical_paths)}, "
            f"extra={sorted(historical_paths - predecessor_paths)}"
        )
    return current


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


def _validate_navigation(root: Path, current: dict[str, MasterContract]) -> None:
    for navigation in NAVIGATION_SURFACES:
        text = (root / navigation).read_text(encoding="utf-8")
        missing = [
            contract.path.name
            for contract in current.values()
            if contract.path.name not in text
        ]
        if missing:
            raise AuthoritativeDesignError(
                f"{navigation} omits current authority paths: {sorted(missing)}"
            )
        if CURRENT_SUITE_ID not in text:
            raise AuthoritativeDesignError(
                f"{navigation} omits current suite identity {CURRENT_SUITE_ID}"
            )


def _validate_successor_plan(root: Path) -> str:
    successor_path = root / SUCCESSOR_PLAN
    try:
        plan = validate_plan(
            root,
            successor_path,
            _allow_missing_state=True,
        )
    except ArtifactContractError as error:
        raise AuthoritativeDesignError(str(error)) from error
    if plan.get("status") != "approved" or plan.get("version") != "v3":
        raise AuthoritativeDesignError("successor relational plan is not approved v3")
    design_path = root / str(plan.get("design_path", ""))
    design = parse_frontmatter(design_path)
    if design.get("design_id") != "codefabric-execution-proved-relational-data-fabric":
        raise AuthoritativeDesignError(
            "successor plan does not select the relational design"
        )
    if design.get("version") != "v3" or design.get("status") != "accepted":
        raise AuthoritativeDesignError("successor relational design is not accepted v3")
    if (
        design.get("doctrine_path")
        != "docs/library_ref/full_data_fabric_design_principles_v2.md"
    ):
        raise AuthoritativeDesignError("successor design does not select v2 doctrine")

    active = _relative(active_plan_path(root), root)
    if active == SUCCESSOR_PLAN:
        try:
            validate_plan(root, successor_path)
        except ArtifactContractError as error:
            raise AuthoritativeDesignError(str(error)) from error
        return "active-v3"
    if active != PREDECESSOR_PLAN:
        raise AuthoritativeDesignError(
            f"unexpected active plan while v3 is inactive: {active}"
        )
    predecessor = parse_frontmatter(root / PREDECESSOR_PLAN)
    state_path = root / str(predecessor.get("state_path", ""))
    try:
        state = json.loads(state_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise AuthoritativeDesignError("invalid predecessor execution state") from error
    if state.get("status") != "invalidated" or state.get("current_packet") is not None:
        raise AuthoritativeDesignError(
            "inactive successor requires invalidated predecessor state with no current packet"
        )
    if (root / str(plan["state_path"])).exists():
        raise AuthoritativeDesignError(
            "inactive successor state exists before the activation transaction"
        )
    return "approved-v3-activation-pending"


def validate_authoritative_design(root: Path = ROOT) -> dict[str, Any]:
    """Validate discovered masters, navigation, history, and active selection."""
    current = validate_master_directory(root / AUTHORITY_ROOT, root=root)
    tracked = _tracked_masters(root)
    historical = _historical_chain_paths(root, root / AUTHORITY_ROOT, current)
    expected_tracked = {contract.path for contract in current.values()} | historical
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
        for contract in current.values()
        if contract.path.name not in outlined.stdout
    ]
    if missing_outlines:
        raise AuthoritativeDesignError(
            f"spec-outline omitted current masters: {sorted(missing_outlines)}"
        )

    _validate_navigation(root, current)
    plan_selection = _validate_successor_plan(root)
    live_hits, historical_hits = _legacy_hits(root)
    if live_hits:
        raise AuthoritativeDesignError(f"legacy live authority remains: {live_hits}")
    return {
        "current_master_count": len(current),
        "historical_master_count": len(historical),
        "generated_manifest_authority_count": 0,
        "historical_exclusion_count": len(historical_hits),
        "suite_id": CURRENT_SUITE_ID,
        "suite_version": CURRENT_SUITE_VERSION,
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
