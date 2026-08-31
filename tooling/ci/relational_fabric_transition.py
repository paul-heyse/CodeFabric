"""Prove the v2 authority cutover and its coverage-qualified legacy envelope.

WP01 publishes selectors bound to the exact accepted design and plan plus a freeze receipt
over their live matches. Loaders fail closed when either input is absent or stale, while the
pure validators remain usable against isolated review fixtures.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from tooling.ci.artifact_contracts import ArtifactContractError, parse_frontmatter
from tooling.ci.authoritative_design_conformance import (
    AUTHORITY_ROOT,
    CURRENT_SUITE_ID,
    CURRENT_SUITE_VERSION,
    AuthoritativeDesignError,
    validate_master_directory,
)

ROOT = Path(__file__).resolve().parents[2]
LEGACY_SELECTORS = Path("contracts/governance/relational-fabric-legacy-selectors.json")
LEGACY_FREEZE = Path("contracts/governance/relational-fabric-legacy-freeze.json")
TARGET_DESIGN = Path(
    "docs/designs/"
    "codefabric_execution_proved_relational_data_fabric_design_v2_2026-08-29.md"
)
TARGET_PLAN = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md"
)
TARGET_PRINCIPLES = Path("docs/library_ref/full_data_fabric_design_principles_v2.md")
PREDECESSOR_SUITE_ID = "codefabric-present-state-cpg-v1.3"

CARGO_MANIFESTS = (
    Path("Cargo.toml"),
    Path("rustc-extractor/Cargo.toml"),
    Path("pyrefly-sidecar/Cargo.toml"),
    Path("fuzz/Cargo.toml"),
)
PARSE_SUFFIXES = frozenset({".py", ".rs", ".sh"})
REQUIRED_FILE_PATHS = frozenset(
    {
        ".ignore",
        ".config/nextest.toml",
        "src/lib.rs",
        "src/contracts/mod.rs",
        "fuzz/Cargo.toml",
    }
)
REQUIRED_INVENTORY_SOURCES = frozenset(
    {"git", "filesystem", "ast-grep", "cargo", "installed", "wheel", "sdist"}
)
ALLOWED_DISPOSITIONS = frozenset(
    {"delete", "replace", "reshape", "preserve", "encapsulate-temporarily"}
)
EXPECTED_DECISIONS = frozenset(f"L-{number}" for number in range(20, 56))
MAX_INVENTORY_FILE_BYTES = 128 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 100_000
MAX_ARCHIVE_MEMBER_BYTES = 128 * 1024 * 1024
_SHA256 = re.compile(r"[0-9a-f]{64}")
_GIT_COMMIT = re.compile(r"[0-9a-f]{40}")
_MARKDOWN_LINK = re.compile(r"\]\((?P<target>[^)#?]+\.md)(?:#[^)]+)?\)")
_LEGACY_DECISION_ROW = re.compile(
    r"^\|\s*(?P<decision>L-\d+)\s*\|.*?\|\s*\*\*(?P<disposition>[a-z-]+)\*\*\s*\|"
)
_EXCLUDED_DIRECTORY_NAMES = frozenset(
    {".git", "target", ".venv", "__pycache__", ".pytest_cache", ".ruff_cache"}
)
_EXCLUDED_FILE_NAMES = frozenset({".envrc.local"})


class TransitionGovernanceError(ValueError):
    """The v2 transition is incomplete, ambiguous, stale, or unsafe to inspect."""


@dataclass(frozen=True)
class InventoryIssue:
    """One omission whose contents are never included in a report."""

    source: str
    subject: str
    reason: str


@dataclass(frozen=True)
class InventorySurface:
    """One independently enumerable repository, symbol, build, or package surface."""

    surface_id: str
    path: str
    kind: str
    sources: frozenset[str]
    content_digest: str
    symbol: str | None = None
    signature: str | None = None
    package: str | None = None
    legacy_candidate: bool = False

    def __post_init__(self) -> None:
        _validate_relative_name(self.path, "inventory path")
        if not self.surface_id or not self.kind or not self.sources:
            raise TransitionGovernanceError("inventory surface identity is incomplete")
        if _SHA256.fullmatch(self.content_digest) is None:
            raise TransitionGovernanceError(
                f"{self.surface_id}: content digest is not SHA-256"
            )


@dataclass(frozen=True)
class InventoryReport:
    """The reconciled inventory relation and its enumeration evidence."""

    surfaces: tuple[InventorySurface, ...]
    git_paths: frozenset[str]
    filesystem_paths: frozenset[str]
    parsed_paths: frozenset[str]
    cargo_manifests: frozenset[str]
    excluded: tuple[InventoryIssue, ...]
    skipped: tuple[InventoryIssue, ...]
    unknowns: tuple[InventoryIssue, ...]


@dataclass(frozen=True)
class SurfaceSelector:
    """A compiled candidate or disposition selector over inventory facts."""

    selector_id: str
    path_glob: str
    surface_kinds: frozenset[str]
    symbol_regex: re.Pattern[str] | None
    package_regex: re.Pattern[str] | None
    decision_id: str | None = None
    disposition: str | None = None

    def matches(self, surface: InventorySurface) -> bool:
        if not fnmatch.fnmatchcase(surface.path, self.path_glob):
            return False
        if self.surface_kinds and surface.kind not in self.surface_kinds:
            return False
        if self.symbol_regex is not None:
            searchable_values = tuple(
                value for value in (surface.symbol, surface.signature) if value
            )
            if not searchable_values or not any(
                self.symbol_regex.search(value) is not None
                for value in searchable_values
            ):
                return False
        return not (
            self.package_regex is not None
            and (
                surface.package is None
                or self.package_regex.search(surface.package) is None
            )
        )


@dataclass(frozen=True)
class SelectorProgram:
    """Fresh selector rows compiled from the accepted design and plan."""

    candidates: tuple[SurfaceSelector, ...]
    dispositions: tuple[SurfaceSelector, ...]
    source_digest: str


@dataclass(frozen=True)
class CoverageReport:
    """Relational anti-join, cardinality, mixed-file, and selector evidence."""

    candidate_surfaces: frozenset[str]
    uncovered_surfaces: tuple[str, ...]
    overlapping_surfaces: tuple[str, ...]
    unresolved_mixed_files: tuple[str, ...]
    no_match_selectors: tuple[str, ...]


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _canonical_digest(value: object) -> str:
    payload = json.dumps(
        value, ensure_ascii=True, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return _sha256_bytes(payload)


def _validate_relative_name(value: str, context: str) -> str:
    if not value or "\x00" in value or "\\" in value:
        raise TransitionGovernanceError(f"{context} is not a safe POSIX path")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or str(path) != value:
        raise TransitionGovernanceError(
            f"{context} is not repository-relative: {value}"
        )
    return value


def _safe_repository_path(root: Path, relative: str, context: str) -> Path:
    _validate_relative_name(relative, context)
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as error:
        raise TransitionGovernanceError(
            f"{context} escapes repository: {relative}"
        ) from error
    return candidate


def _load_json(path: Path, context: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise TransitionGovernanceError(
            f"missing or invalid {context}: {path}"
        ) from error
    if not isinstance(value, dict):
        raise TransitionGovernanceError(f"{context} must be a JSON object: {path}")
    return value


def _strict_keys(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    if set(value) != expected:
        raise TransitionGovernanceError(
            f"{context} keys differ: missing={sorted(expected - set(value))}, "
            f"extra={sorted(set(value) - expected)}"
        )


def _git_output(root: Path, *arguments: str, binary: bool = False) -> str | bytes:
    completed = subprocess.run(
        ("git", *arguments),
        cwd=root,
        check=False,
        capture_output=True,
        text=not binary,
    )
    if completed.returncode != 0:
        stderr = (
            completed.stderr.decode("utf-8", "replace") if binary else completed.stderr
        )
        raise TransitionGovernanceError(
            f"git {' '.join(arguments)} failed: {stderr.strip()}"
        )
    return completed.stdout


def _historical_blob_digests(root: Path, commit: str) -> Counter[str]:
    if _GIT_COMMIT.fullmatch(commit) is None:
        raise TransitionGovernanceError("historical baseline commit is invalid")
    listing = _git_output(
        root,
        "ls-tree",
        "-r",
        "-z",
        "--name-only",
        commit,
        "--",
        "docs/authoritative_design",
        binary=True,
    )
    assert isinstance(listing, bytes)
    paths: list[str] = []
    for index, raw_path in enumerate(listing.split(b"\0")):
        if not raw_path:
            continue
        try:
            relative = raw_path.decode("utf-8")
        except UnicodeDecodeError as error:
            raise TransitionGovernanceError(
                f"historical authority path {index} is not UTF-8"
            ) from error
        if relative.endswith(".md"):
            paths.append(relative)
    if len(paths) != 8:
        raise TransitionGovernanceError(
            f"historical baseline must contain eight masters, observed {len(paths)}"
        )
    digests: Counter[str] = Counter()
    for relative in paths:
        payload = _git_output(root, "show", f"{commit}:{relative}", binary=True)
        assert isinstance(payload, bytes)
        digests[_sha256_bytes(payload)] += 1
    return digests


def validate_authority_selection(
    root: Path = ROOT,
) -> dict[str, object]:
    """Prove one metadata-selected v2 suite and byte-identical predecessor history.

    Authority is discovered from the authored master frontmatter.  A copied JSON census
    would be a second static answer and is intentionally neither read nor generated here.
    """
    root = root.resolve()
    if not (root / TARGET_PRINCIPLES).is_file():
        raise TransitionGovernanceError("selected v2 principles document is absent")
    try:
        current = validate_master_directory(root / AUTHORITY_ROOT, root=root)
        plan = parse_frontmatter(root / TARGET_PLAN)
    except (ArtifactContractError, AuthoritativeDesignError) as error:
        raise TransitionGovernanceError(str(error)) from error
    baseline = plan.get("baseline_commit")
    if not isinstance(baseline, str):
        raise TransitionGovernanceError("active v2 plan omits its historical baseline")
    predecessors = {contract.predecessor_path for contract in current.values()}
    observed = Counter(_sha256_file(root / path) for path in predecessors)
    expected = _historical_blob_digests(root, baseline)
    if observed != expected:
        raise TransitionGovernanceError(
            "historical predecessor suite is not byte-identical to its baseline"
        )
    suite_text = (root / current["SUITE"].path).read_text(encoding="utf-8")
    if TARGET_PRINCIPLES.as_posix() not in suite_text:
        raise TransitionGovernanceError(
            "current suite root does not bind the v2 principles"
        )
    if "no generated manifest" not in suite_text.lower():
        raise TransitionGovernanceError(
            "current suite root does not reject generated manifest authority"
        )
    return {
        "current_suite_id": CURRENT_SUITE_ID,
        "current_suite_version": CURRENT_SUITE_VERSION,
        "current_master_count": len(current),
        "historical_suite_count": 1,
        "historical_predecessor_count": int(len(predecessors) == 8),
        "unrouted_master_count": 0,
        "generated_authority_selector_count": 0,
    }


def _excluded_reason(relative: str) -> str | None:
    parts = PurePosixPath(relative).parts
    if any(part in _EXCLUDED_DIRECTORY_NAMES for part in parts):
        matched = next(part for part in parts if part in _EXCLUDED_DIRECTORY_NAMES)
        return f"excluded-directory:{matched}"
    if parts and parts[-1] in _EXCLUDED_FILE_NAMES:
        return f"secret-file:{parts[-1]}"
    return None


def _requires_parser(path: str) -> bool:
    return Path(path).suffix in PARSE_SUFFIXES and not path.startswith(
        "docs/library_ref/"
    )


def _file_digest_without_following(path: Path) -> str:
    if path.is_symlink():
        return _sha256_bytes(os.readlink(path).encode("utf-8", "surrogateescape"))
    if path.stat().st_size > MAX_INVENTORY_FILE_BYTES:
        raise TransitionGovernanceError(
            f"inventory file exceeds {MAX_INVENTORY_FILE_BYTES} bytes"
        )
    return _sha256_file(path)


def enumerate_repository_files(
    root: Path,
) -> tuple[
    tuple[InventorySurface, ...],
    frozenset[str],
    frozenset[str],
    tuple[InventoryIssue, ...],
    tuple[InventoryIssue, ...],
]:
    """Union Git tracked/untracked names with a hidden, no-ignore filesystem walk."""
    root = root.resolve()
    git_raw = _git_output(
        root,
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
        binary=True,
    )
    assert isinstance(git_raw, bytes)
    git_paths: set[str] = set()
    unknowns: list[InventoryIssue] = []
    excluded: list[InventoryIssue] = []
    for index, raw_path in enumerate(git_raw.split(b"\0")):
        if not raw_path:
            continue
        try:
            relative = raw_path.decode("utf-8")
        except UnicodeDecodeError:
            unknowns.append(
                InventoryIssue("git", f"path-index:{index}", "non-UTF-8 path")
            )
            continue
        _validate_relative_name(relative, "Git path")
        reason = _excluded_reason(relative)
        if reason is not None:
            excluded.append(InventoryIssue("git", relative, reason))
            continue
        git_paths.add(relative)

    filesystem_paths: set[str] = set()
    for directory, directory_names, file_names in os.walk(root, followlinks=False):
        directory_path = Path(directory)
        relative_directory = directory_path.relative_to(root)
        kept_directories: list[str] = []
        for name in directory_names:
            relative = (relative_directory / name).as_posix()
            reason = _excluded_reason(relative)
            candidate = directory_path / name
            if reason is None and candidate.is_symlink():
                filesystem_paths.add(relative)
            elif reason is None:
                kept_directories.append(name)
            else:
                excluded.append(InventoryIssue("filesystem", relative, reason))
        directory_names[:] = kept_directories
        for name in file_names:
            relative = (relative_directory / name).as_posix()
            reason = _excluded_reason(relative)
            if reason is not None:
                excluded.append(InventoryIssue("filesystem", relative, reason))
                continue
            _validate_relative_name(relative, "filesystem path")
            filesystem_paths.add(relative)

    surfaces: list[InventorySurface] = []
    skipped: list[InventoryIssue] = []
    for relative in sorted(git_paths | filesystem_paths):
        path = root / relative
        if not path.is_file() and not path.is_symlink():
            skipped.append(InventoryIssue("filesystem", relative, "not a file"))
            continue
        sources = {
            source
            for source, paths in (("git", git_paths), ("filesystem", filesystem_paths))
            if relative in paths
        }
        try:
            digest = _file_digest_without_following(path)
        except (OSError, TransitionGovernanceError) as error:
            skipped.append(InventoryIssue("filesystem", relative, str(error)))
            continue
        surfaces.append(
            InventorySurface(
                surface_id=f"file:{relative}",
                path=relative,
                kind="file",
                sources=frozenset(sources),
                content_digest=digest,
            )
        )
    return (
        tuple(surfaces),
        frozenset(git_paths),
        frozenset(filesystem_paths),
        tuple(excluded),
        tuple(skipped + unknowns),
    )


def collect_outline_surfaces(
    root: Path, file_paths: Iterable[str]
) -> tuple[tuple[InventorySurface, ...], frozenset[str], tuple[InventoryIssue, ...]]:
    """Collect AST-backed symbols/imports/re-exports and account for every code file."""
    expected = sorted(path for path in file_paths if _requires_parser(path))
    observed: set[str] = set()
    surfaces: list[InventorySurface] = []
    issues: list[InventoryIssue] = []
    for offset in range(0, len(expected), 100):
        chunk = expected[offset : offset + 100]
        completed = subprocess.run(
            (
                "ast-grep",
                "outline",
                *chunk,
                "--items",
                "all",
                "--json=compact",
                "--no-ignore",
                "hidden",
                "--no-ignore",
                "dot",
                "--no-ignore",
                "vcs",
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode != 0:
            issues.append(
                InventoryIssue(
                    "ast-grep",
                    f"chunk:{offset // 100}",
                    f"outline exit {completed.returncode}: {completed.stderr.strip()}",
                )
            )
            continue
        try:
            documents = json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            issues.append(
                InventoryIssue("ast-grep", f"chunk:{offset // 100}", str(error))
            )
            continue
        if not isinstance(documents, list):
            issues.append(
                InventoryIssue("ast-grep", f"chunk:{offset // 100}", "non-list output")
            )
            continue
        for document in documents:
            if not isinstance(document, dict) or not isinstance(
                document.get("path"), str
            ):
                issues.append(
                    InventoryIssue(
                        "ast-grep", f"chunk:{offset // 100}", "malformed file row"
                    )
                )
                continue
            path = document["path"]
            _validate_relative_name(path, "ast-grep path")
            if path not in expected:
                issues.append(
                    InventoryIssue("ast-grep", path, "unexpected parsed path")
                )
                continue
            observed.add(path)
            items = document.get("items")
            if not isinstance(items, list):
                issues.append(InventoryIssue("ast-grep", path, "items are absent"))
                continue
            for item in items:
                if not isinstance(item, dict):
                    issues.append(InventoryIssue("ast-grep", path, "malformed item"))
                    continue
                if not (item.get("isImport") or item.get("isExported")):
                    continue
                name = item.get("name")
                symbol_type = item.get("symbolType")
                start = item.get("range", {}).get("start", {})
                line = start.get("line")
                column = start.get("column")
                if (
                    not isinstance(name, str)
                    or not isinstance(symbol_type, str)
                    or not isinstance(line, int)
                    or not isinstance(column, int)
                ):
                    issues.append(
                        InventoryIssue("ast-grep", path, "incomplete item identity")
                    )
                    continue
                kind = (
                    "reexport"
                    if item.get("isImport") and item.get("isExported")
                    else "import"
                    if item.get("isImport")
                    else "symbol"
                )
                identity = f"{symbol_type}:{name}:{line}:{column}"
                surfaces.append(
                    InventorySurface(
                        surface_id=f"{kind}:{path}#{identity}",
                        path=path,
                        kind=kind,
                        sources=frozenset({"ast-grep"}),
                        content_digest=_canonical_digest(item),
                        symbol=name,
                        signature=(
                            item.get("signature")
                            if isinstance(item.get("signature"), str)
                            else None
                        ),
                    )
                )
    for path in sorted(set(expected) - observed):
        issues.append(InventoryIssue("ast-grep", path, "code file was not parsed"))
    return tuple(surfaces), frozenset(observed), tuple(issues)


def collect_cargo_surfaces(
    root: Path, manifests: Sequence[Path] = CARGO_MANIFESTS
) -> tuple[tuple[InventorySurface, ...], frozenset[str], tuple[InventoryIssue, ...]]:
    """Enumerate every package, feature, and build target from each Cargo root."""
    surfaces: list[InventorySurface] = []
    observed_manifests: set[str] = set()
    issues: list[InventoryIssue] = []
    for manifest in manifests:
        relative_manifest = manifest.as_posix()
        if not (root / manifest).is_file():
            issues.append(
                InventoryIssue("cargo", relative_manifest, "manifest is absent")
            )
            continue
        completed = subprocess.run(
            (
                "cargo",
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--no-deps",
                "--manifest-path",
                relative_manifest,
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode != 0:
            issues.append(
                InventoryIssue(
                    "cargo",
                    relative_manifest,
                    f"metadata exit {completed.returncode}: {completed.stderr.strip()}",
                )
            )
            continue
        try:
            metadata = json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            issues.append(InventoryIssue("cargo", relative_manifest, str(error)))
            continue
        packages = metadata.get("packages") if isinstance(metadata, dict) else None
        if not isinstance(packages, list) or not packages:
            issues.append(
                InventoryIssue("cargo", relative_manifest, "packages are absent")
            )
            continue
        matching_packages: list[dict[str, Any]] = []
        for package in packages:
            if not isinstance(package, dict) or not isinstance(
                package.get("manifest_path"), str
            ):
                issues.append(
                    InventoryIssue("cargo", relative_manifest, "malformed package")
                )
                continue
            package_manifest = Path(package["manifest_path"]).resolve()
            try:
                package_relative = package_manifest.relative_to(
                    root.resolve()
                ).as_posix()
            except ValueError:
                continue
            if package_relative == relative_manifest:
                matching_packages.append(package)
        if len(matching_packages) != 1:
            issues.append(
                InventoryIssue(
                    "cargo",
                    relative_manifest,
                    f"expected one owning package, observed {len(matching_packages)}",
                )
            )
            continue
        package = matching_packages[0]
        package_name = package.get("name")
        features = package.get("features")
        targets = package.get("targets")
        if (
            not isinstance(package_name, str)
            or not isinstance(features, dict)
            or not isinstance(targets, list)
        ):
            issues.append(
                InventoryIssue(
                    "cargo", relative_manifest, "package facts are incomplete"
                )
            )
            continue
        observed_manifests.add(relative_manifest)
        surfaces.append(
            InventorySurface(
                surface_id=f"cargo-package:{relative_manifest}#{package_name}",
                path=relative_manifest,
                kind="cargo-package",
                sources=frozenset({"cargo"}),
                content_digest=_canonical_digest(
                    {"name": package_name, "version": package.get("version")}
                ),
                package=package_name,
            )
        )
        for feature_name, members in sorted(features.items()):
            if not isinstance(feature_name, str) or not isinstance(members, list):
                issues.append(
                    InventoryIssue(
                        "cargo", relative_manifest, "feature row is malformed"
                    )
                )
                continue
            surfaces.append(
                InventorySurface(
                    surface_id=f"cargo-feature:{relative_manifest}#{package_name}:{feature_name}",
                    path=relative_manifest,
                    kind="cargo-feature",
                    sources=frozenset({"cargo"}),
                    content_digest=_canonical_digest(sorted(members)),
                    symbol=feature_name,
                    package=package_name,
                )
            )
        for target in targets:
            if not isinstance(target, dict):
                issues.append(
                    InventoryIssue(
                        "cargo", relative_manifest, "target row is malformed"
                    )
                )
                continue
            name = target.get("name")
            kinds = target.get("kind")
            source_path = target.get("src_path")
            if (
                not isinstance(name, str)
                or not isinstance(kinds, list)
                or not isinstance(source_path, str)
            ):
                issues.append(
                    InventoryIssue(
                        "cargo", relative_manifest, "target identity is incomplete"
                    )
                )
                continue
            try:
                source_relative = (
                    Path(source_path).resolve().relative_to(root.resolve()).as_posix()
                )
            except ValueError:
                issues.append(
                    InventoryIssue(
                        "cargo", relative_manifest, "target source escapes repository"
                    )
                )
                continue
            kind = "+".join(sorted(str(value) for value in kinds))
            surfaces.append(
                InventorySurface(
                    surface_id=f"cargo-target:{relative_manifest}#{package_name}:{kind}:{name}",
                    path=source_relative,
                    kind="cargo-target",
                    sources=frozenset({"cargo"}),
                    content_digest=_canonical_digest(target),
                    symbol=name,
                    package=package_name,
                )
            )
    return tuple(surfaces), frozenset(observed_manifests), tuple(issues)


def _safe_archive_member(name: str, context: str) -> str:
    normalized = name.removeprefix("./")
    return _validate_relative_name(normalized, context)


def _wheel_surfaces(wheel: Path) -> list[InventorySurface]:
    surfaces: list[InventorySurface] = []
    with zipfile.ZipFile(wheel) as archive:
        members = archive.infolist()
        if len(members) > MAX_ARCHIVE_MEMBERS:
            raise TransitionGovernanceError("adapter wheel has too many members")
        for member in members:
            if member.is_dir():
                continue
            name = _safe_archive_member(member.filename, "wheel member")
            if member.file_size > MAX_ARCHIVE_MEMBER_BYTES:
                raise TransitionGovernanceError(f"wheel member is too large: {name}")
            digest = _sha256_bytes(archive.read(member))
            surfaces.append(
                InventorySurface(
                    surface_id=f"wheel:{wheel.name}!{name}",
                    path=f"codefabric-cpg-mcp/{name}",
                    kind="wheel",
                    sources=frozenset({"wheel"}),
                    content_digest=digest,
                    package="codefabric-cpg-mcp",
                )
            )
    return surfaces


def _sdist_surfaces(sdist: Path) -> list[InventorySurface]:
    surfaces: list[InventorySurface] = []
    with tarfile.open(sdist, mode="r:gz") as archive:
        members = archive.getmembers()
        if len(members) > MAX_ARCHIVE_MEMBERS:
            raise TransitionGovernanceError("adapter sdist has too many members")
        for member in members:
            if member.isdir():
                continue
            name = _safe_archive_member(member.name, "sdist member")
            if member.size > MAX_ARCHIVE_MEMBER_BYTES:
                raise TransitionGovernanceError(f"sdist member is too large: {name}")
            if member.issym() or member.islnk():
                digest = _sha256_bytes(
                    member.linkname.encode("utf-8", "surrogateescape")
                )
            elif member.isfile():
                source = archive.extractfile(member)
                if source is None:
                    raise TransitionGovernanceError(f"cannot read sdist member: {name}")
                digest = _sha256_bytes(source.read())
            else:
                raise TransitionGovernanceError(
                    f"unsupported special sdist member: {name}"
                )
            surfaces.append(
                InventorySurface(
                    surface_id=f"sdist:{sdist.name}!{name}",
                    path=f"codefabric-cpg-mcp/{name}",
                    kind="sdist",
                    sources=frozenset({"sdist"}),
                    content_digest=digest,
                    package="codefabric-cpg-mcp",
                )
            )
    return surfaces


def collect_python_distribution_surfaces(
    root: Path,
) -> tuple[tuple[InventorySurface, ...], tuple[InventoryIssue, ...]]:
    """Inventory the locked installed adapter and freshly built wheel and sdist."""
    project = root / "codefabric-cpg-mcp"
    python = project / ".venv/bin/python"
    issues: list[InventoryIssue] = []
    surfaces: list[InventorySurface] = []
    if not python.is_file():
        return (), (
            InventoryIssue(
                "installed", python.as_posix(), "adapter environment is absent"
            ),
        )
    installed_probe = """
import hashlib
import importlib.metadata
import json
import sys
from pathlib import Path

distribution = importlib.metadata.distribution("codefabric-cpg-mcp")
prefix = Path(sys.prefix).resolve()
rows = []
for member in distribution.files or ():
    name = str(member)
    if "__pycache__" in name or name.endswith(".pyc"):
        continue
    located = Path(distribution.locate_file(member))
    resolved = located.resolve()
    if not resolved.is_relative_to(prefix) or not located.is_file():
        raise RuntimeError(f"installed member is outside the adapter environment: {name}")
    digest = hashlib.sha256(located.read_bytes()).hexdigest()
    rows.append([name, digest])
print(json.dumps(sorted(rows)))
"""
    script = f"exec({installed_probe!r})"
    installed = subprocess.run(
        (str(python), "-c", script),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if installed.returncode != 0:
        issues.append(
            InventoryIssue(
                "installed",
                "codefabric-cpg-mcp",
                f"metadata exit {installed.returncode}: {installed.stderr.strip()}",
            )
        )
    else:
        try:
            installed_rows = json.loads(installed.stdout)
        except json.JSONDecodeError as error:
            issues.append(InventoryIssue("installed", "codefabric-cpg-mcp", str(error)))
            installed_rows = []
        if not isinstance(installed_rows, list) or not installed_rows:
            issues.append(
                InventoryIssue("installed", "codefabric-cpg-mcp", "manifest is empty")
            )
        else:
            for row in installed_rows:
                if (
                    not isinstance(row, list)
                    or len(row) != 2
                    or not isinstance(row[0], str)
                    or not isinstance(row[1], str)
                    or _SHA256.fullmatch(row[1]) is None
                ):
                    issues.append(
                        InventoryIssue(
                            "installed", "codefabric-cpg-mcp", "member row is invalid"
                        )
                    )
                    continue
                raw_name, digest = row
                try:
                    name = _safe_archive_member(raw_name, "installed member")
                except TransitionGovernanceError as error:
                    issues.append(
                        InventoryIssue("installed", "codefabric-cpg-mcp", str(error))
                    )
                    continue
                surfaces.append(
                    InventorySurface(
                        surface_id=f"installed:codefabric-cpg-mcp!{name}",
                        path=f"codefabric-cpg-mcp/{name}",
                        kind="installed",
                        sources=frozenset({"installed"}),
                        content_digest=digest,
                        package="codefabric-cpg-mcp",
                    )
                )
    with tempfile.TemporaryDirectory(
        prefix="codefabric-transition-package-"
    ) as temporary:
        output = Path(temporary) / "dist"
        built = subprocess.run(
            (
                "uv",
                "build",
                "--project",
                str(project),
                "--out-dir",
                str(output),
                "--no-build-logs",
                "--no-progress",
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if built.returncode != 0:
            issues.append(
                InventoryIssue(
                    "package-build",
                    "codefabric-cpg-mcp",
                    f"uv build exit {built.returncode}: {built.stderr.strip()}",
                )
            )
        else:
            wheels = list(output.glob("*.whl"))
            sdists = list(output.glob("*.tar.gz"))
            if len(wheels) != 1 or len(sdists) != 1:
                issues.append(
                    InventoryIssue(
                        "package-build",
                        "codefabric-cpg-mcp",
                        f"expected one wheel and sdist, observed {len(wheels)} and {len(sdists)}",
                    )
                )
            else:
                try:
                    surfaces.extend(_wheel_surfaces(wheels[0]))
                    surfaces.extend(_sdist_surfaces(sdists[0]))
                except (OSError, tarfile.TarError, zipfile.BadZipFile) as error:
                    issues.append(
                        InventoryIssue(
                            "package-build", "codefabric-cpg-mcp", str(error)
                        )
                    )
    return tuple(surfaces), tuple(issues)


def merge_inventory_surfaces(
    collections: Iterable[Iterable[InventorySurface]],
) -> tuple[InventorySurface, ...]:
    """Union independently observed facts without silently resolving conflicts."""
    merged: dict[str, InventorySurface] = {}
    for collection in collections:
        for surface in collection:
            previous = merged.get(surface.surface_id)
            if previous is None:
                merged[surface.surface_id] = surface
                continue
            identity = (
                surface.path,
                surface.kind,
                surface.content_digest,
                surface.symbol,
                surface.signature,
                surface.package,
            )
            previous_identity = (
                previous.path,
                previous.kind,
                previous.content_digest,
                previous.symbol,
                previous.signature,
                previous.package,
            )
            if identity != previous_identity:
                raise TransitionGovernanceError(
                    f"conflicting inventory facts for {surface.surface_id}"
                )
            merged[surface.surface_id] = InventorySurface(
                surface_id=surface.surface_id,
                path=surface.path,
                kind=surface.kind,
                sources=previous.sources | surface.sources,
                content_digest=surface.content_digest,
                symbol=surface.symbol,
                signature=surface.signature,
                package=surface.package,
                legacy_candidate=previous.legacy_candidate or surface.legacy_candidate,
            )
    return tuple(sorted(merged.values(), key=lambda value: value.surface_id))


def collect_inventory(root: Path = ROOT) -> InventoryReport:
    """Build the exact multi-source legacy inventory universe from the current tree."""
    root = root.resolve()
    files, git_paths, filesystem_paths, excluded, file_issues = (
        enumerate_repository_files(root)
    )
    outline, parsed_paths, outline_issues = collect_outline_surfaces(
        root, git_paths | filesystem_paths
    )
    cargo, cargo_manifests, cargo_issues = collect_cargo_surfaces(root)
    packages, package_issues = collect_python_distribution_surfaces(root)
    report = InventoryReport(
        surfaces=merge_inventory_surfaces((files, outline, cargo, packages)),
        git_paths=git_paths,
        filesystem_paths=filesystem_paths,
        parsed_paths=parsed_paths,
        cargo_manifests=cargo_manifests,
        excluded=excluded,
        skipped=tuple(file_issues + outline_issues + cargo_issues + package_issues),
        unknowns=(),
    )
    validate_inventory_universe(report)
    return report


def validate_inventory_universe(report: InventoryReport) -> dict[str, object]:
    """Fail on source omissions, skipped facts, secret leakage, or missing sentinels."""
    if report.skipped or report.unknowns:
        issues = [
            f"{issue.source}:{issue.subject}:{issue.reason}"
            for issue in (*report.skipped, *report.unknowns)
        ]
        raise TransitionGovernanceError(
            f"inventory has skipped or unknown inputs: {issues[:12]}"
        )
    git_only = report.git_paths - report.filesystem_paths
    if git_only:
        raise TransitionGovernanceError(
            f"Git paths are absent from filesystem inventory: {sorted(git_only)[:12]}"
        )
    sources = frozenset(
        source for surface in report.surfaces for source in surface.sources
    )
    missing_sources = REQUIRED_INVENTORY_SOURCES - sources
    if missing_sources:
        raise TransitionGovernanceError(
            f"inventory sources are absent: {sorted(missing_sources)}"
        )
    file_paths = {surface.path for surface in report.surfaces if surface.kind == "file"}
    missing_files = REQUIRED_FILE_PATHS - file_paths
    if missing_files:
        raise TransitionGovernanceError(
            f"required mixed/hidden inventory paths are absent: {sorted(missing_files)}"
        )
    required_fuzz = {
        path
        for path in report.filesystem_paths
        if path.startswith(("fuzz/fuzz_targets/", "fuzz/corpus/"))
    }
    if not required_fuzz or not required_fuzz <= file_paths:
        raise TransitionGovernanceError("fuzz target/corpus inventory is incomplete")
    expected_parse = {
        path
        for path in report.git_paths | report.filesystem_paths
        if _requires_parser(path)
    }
    unparsed = expected_parse - report.parsed_paths
    if unparsed:
        raise TransitionGovernanceError(
            f"parser coverage is incomplete: {sorted(unparsed)[:12]}"
        )
    if report.cargo_manifests != frozenset(path.as_posix() for path in CARGO_MANIFESTS):
        raise TransitionGovernanceError(
            f"Cargo root coverage differs: observed={sorted(report.cargo_manifests)}"
        )
    excluded_subjects = {issue.subject for issue in report.excluded}
    for surface in report.surfaces:
        if (
            _excluded_reason(surface.path) is not None
            or surface.path in excluded_subjects
        ):
            raise TransitionGovernanceError(
                f"excluded or secret path leaked into inventory: {surface.path}"
            )
    return {
        "surface_count": len(report.surfaces),
        "git_path_count": len(report.git_paths),
        "filesystem_path_count": len(report.filesystem_paths),
        "parsed_path_count": len(report.parsed_paths),
        "cargo_manifest_count": len(report.cargo_manifests),
        "inventory_sources": sorted(sources),
        "excluded_count": len(report.excluded),
        "skipped_count": 0,
        "unknown_count": 0,
    }


def _compile_regex(value: object, context: str) -> re.Pattern[str] | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value:
        raise TransitionGovernanceError(f"{context} must be null or non-empty text")
    try:
        return re.compile(value)
    except re.error as error:
        raise TransitionGovernanceError(f"{context} is invalid: {error}") from error


def design_legacy_dispositions(design_path: Path) -> dict[str, str]:
    """Read the immutable L-20 through L-55 judgments from the accepted design."""
    dispositions: dict[str, str] = {}
    try:
        lines = design_path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise TransitionGovernanceError(
            f"cannot read legacy disposition matrix: {design_path}"
        ) from error
    for line in lines:
        match = _LEGACY_DECISION_ROW.match(line)
        if match is None:
            continue
        decision = match.group("decision")
        disposition = match.group("disposition")
        if decision in dispositions:
            raise TransitionGovernanceError(
                f"duplicate legacy decision in accepted design: {decision}"
            )
        if disposition not in ALLOWED_DISPOSITIONS:
            raise TransitionGovernanceError(
                f"{decision}: unsupported accepted disposition {disposition}"
            )
        dispositions[decision] = disposition
    if set(dispositions) != EXPECTED_DECISIONS:
        raise TransitionGovernanceError(
            "accepted legacy matrix is incomplete: "
            f"missing={sorted(EXPECTED_DECISIONS - set(dispositions))}, "
            f"extra={sorted(set(dispositions) - EXPECTED_DECISIONS)}"
        )
    return dispositions


def _load_selector_row(row: object, *, disposition: bool) -> SurfaceSelector:
    if not isinstance(row, dict):
        raise TransitionGovernanceError("selector row must be an object")
    common = {
        "selector_id",
        "path_glob",
        "surface_kinds",
        "symbol_regex",
        "package_regex",
    }
    expected = common | ({"decision_id", "disposition"} if disposition else set())
    _strict_keys(row, expected, "selector row")
    selector_id = row["selector_id"]
    path_glob = row["path_glob"]
    kinds = row["surface_kinds"]
    if (
        not isinstance(selector_id, str)
        or not selector_id
        or not isinstance(path_glob, str)
        or not path_glob
        or path_glob.startswith("/")
        or ".." in PurePosixPath(path_glob).parts
        or not isinstance(kinds, list)
        or any(not isinstance(kind, str) or not kind for kind in kinds)
    ):
        raise TransitionGovernanceError("selector identity, glob, or kinds are invalid")
    decision_id = row.get("decision_id")
    selected_disposition = row.get("disposition")
    if disposition and (
        decision_id not in EXPECTED_DECISIONS
        or selected_disposition not in ALLOWED_DISPOSITIONS
    ):
        raise TransitionGovernanceError(
            f"{selector_id}: decision or disposition is invalid"
        )
    return SurfaceSelector(
        selector_id=selector_id,
        path_glob=path_glob,
        surface_kinds=frozenset(kinds),
        symbol_regex=_compile_regex(row["symbol_regex"], f"{selector_id} symbol regex"),
        package_regex=_compile_regex(
            row["package_regex"], f"{selector_id} package regex"
        ),
        decision_id=decision_id,
        disposition=selected_disposition,
    )


def load_selector_program(
    root: Path = ROOT, path: Path = LEGACY_SELECTORS
) -> SelectorProgram:
    """Load selector rows only when both immutable source digests are exact."""
    document = _load_json(root / path, "compiled legacy selectors")
    _strict_keys(
        document,
        {
            "schema_version",
            "design_path",
            "design_sha256",
            "plan_path",
            "plan_sha256",
            "candidate_selectors",
            "disposition_selectors",
        },
        "legacy selector program",
    )
    if document["schema_version"] != 1:
        raise TransitionGovernanceError("legacy selector schema_version must be 1")
    source_pairs = (
        (document["design_path"], document["design_sha256"], TARGET_DESIGN),
        (document["plan_path"], document["plan_sha256"], TARGET_PLAN),
    )
    for source_path, expected_digest, required_path in source_pairs:
        if source_path != required_path.as_posix() or not isinstance(
            expected_digest, str
        ):
            raise TransitionGovernanceError(
                "selector source path does not bind the v2 artifact"
            )
        observed_digest = _sha256_file(root / required_path)
        if expected_digest != observed_digest:
            raise TransitionGovernanceError(
                f"compiled selectors are stale for {required_path.as_posix()}"
            )
    candidate_rows = document["candidate_selectors"]
    disposition_rows = document["disposition_selectors"]
    if not isinstance(candidate_rows, list) or not isinstance(disposition_rows, list):
        raise TransitionGovernanceError("selector collections must be arrays")
    candidates = tuple(
        _load_selector_row(row, disposition=False) for row in candidate_rows
    )
    dispositions = tuple(
        _load_selector_row(row, disposition=True) for row in disposition_rows
    )
    selector_ids = [row.selector_id for row in (*candidates, *dispositions)]
    if not candidates or len(selector_ids) != len(set(selector_ids)):
        raise TransitionGovernanceError(
            "selector IDs must be non-empty and globally unique"
        )
    decisions = {row.decision_id for row in dispositions}
    if decisions != EXPECTED_DECISIONS:
        raise TransitionGovernanceError(
            "legacy decision closure differs: "
            f"missing={sorted(EXPECTED_DECISIONS - decisions)}, "
            f"extra={sorted(decisions - EXPECTED_DECISIONS)}"
        )
    accepted_dispositions = design_legacy_dispositions(root / TARGET_DESIGN)
    conflicting = sorted(
        selector.selector_id
        for selector in dispositions
        if selector.disposition != accepted_dispositions[selector.decision_id]
    )
    if conflicting:
        raise TransitionGovernanceError(
            f"selectors contradict accepted design dispositions: {conflicting}"
        )
    return SelectorProgram(
        candidates=candidates,
        dispositions=dispositions,
        source_digest=_sha256_file(root / path),
    )


def evaluate_disposition_coverage(
    report: InventoryReport, program: SelectorProgram
) -> CoverageReport:
    """Evaluate anti-join, exact-cardinality, mixed-file, and no-match relations."""
    validate_inventory_universe(report)
    candidate_matches: dict[str, set[str]] = defaultdict(set)
    disposition_matches: dict[str, list[SurfaceSelector]] = defaultdict(list)
    matched_candidate_selectors: set[str] = set()
    matched_disposition_selectors: set[str] = set()
    surfaces_by_id = {surface.surface_id: surface for surface in report.surfaces}
    for surface in report.surfaces:
        for selector in program.candidates:
            if selector.matches(surface):
                candidate_matches[surface.surface_id].add(selector.selector_id)
                matched_candidate_selectors.add(selector.selector_id)
        if surface.legacy_candidate:
            candidate_matches[surface.surface_id].add("inventory:legacy-candidate")
        for selector in program.dispositions:
            if surface.surface_id in candidate_matches and selector.matches(surface):
                disposition_matches[surface.surface_id].append(selector)
                matched_disposition_selectors.add(selector.selector_id)

    candidate_ids = set(candidate_matches)
    uncovered = sorted(
        surface_id
        for surface_id in candidate_ids
        if len(disposition_matches[surface_id]) == 0
    )
    overlapping = sorted(
        surface_id
        for surface_id in candidate_ids
        if len(disposition_matches[surface_id]) > 1
    )
    candidate_selector_ids = {selector.selector_id for selector in program.candidates}
    disposition_selector_ids = {
        selector.selector_id for selector in program.dispositions
    }
    no_match = sorted(
        (candidate_selector_ids - matched_candidate_selectors)
        | (disposition_selector_ids - matched_disposition_selectors)
    )

    candidate_surfaces_by_path: dict[str, list[InventorySurface]] = defaultdict(list)
    for surface_id in candidate_ids:
        surface = surfaces_by_id[surface_id]
        candidate_surfaces_by_path[surface.path].append(surface)
    unresolved_mixed: list[str] = []
    for path, surfaces in candidate_surfaces_by_path.items():
        by_kind: dict[str, list[InventorySurface]] = defaultdict(list)
        for surface in surfaces:
            by_kind[surface.kind].append(surface)
        for same_kind in by_kind.values():
            selectors = [
                selector
                for surface in same_kind
                for selector in disposition_matches[surface.surface_id]
            ]
            decisions = {selector.decision_id for selector in selectors}
            if len(decisions) <= 1:
                continue
            if any(surface.symbol is None for surface in same_kind) or any(
                selector.symbol_regex is None for selector in selectors
            ):
                unresolved_mixed.append(path)
                break

    return CoverageReport(
        candidate_surfaces=frozenset(candidate_ids),
        uncovered_surfaces=tuple(uncovered),
        overlapping_surfaces=tuple(overlapping),
        unresolved_mixed_files=tuple(sorted(set(unresolved_mixed))),
        no_match_selectors=tuple(no_match),
    )


def validate_disposition_coverage(
    report: InventoryReport, program: SelectorProgram
) -> dict[str, object]:
    """Fail unless every current legacy surface has one live disposition selector."""
    coverage = evaluate_disposition_coverage(report, program)
    failures = {
        "uncovered": coverage.uncovered_surfaces,
        "overlapping": coverage.overlapping_surfaces,
        "unresolved_mixed_files": coverage.unresolved_mixed_files,
        "no_match_selectors": coverage.no_match_selectors,
    }
    if any(failures.values()):
        raise TransitionGovernanceError(
            "legacy disposition coverage failed: "
            + "; ".join(
                f"{name}={list(values[:12])}"
                for name, values in failures.items()
                if values
            )
        )
    return {
        "candidate_surface_count": len(coverage.candidate_surfaces),
        "disposition_selector_count": len(program.dispositions),
        "decision_count": len(EXPECTED_DECISIONS),
        "uncovered_count": 0,
        "overlap_count": 0,
        "unresolved_mixed_file_count": 0,
        "no_match_selector_count": 0,
    }


def validate_legacy_authority_freeze(
    report: InventoryReport,
    program: SelectorProgram,
    freeze: Mapping[str, Any],
    *,
    root: Path | None = None,
) -> dict[str, object]:
    """Reject new or modified legacy authorities/consumers while allowing deletion."""
    validate_inventory_universe(report)
    _strict_keys(
        freeze,
        {"schema_version", "selector_sha256", "frozen_at_commit", "surfaces"},
        "legacy authority freeze",
    )
    if (
        freeze["schema_version"] != 1
        or freeze["selector_sha256"] != program.source_digest
    ):
        raise TransitionGovernanceError(
            "legacy authority freeze is stale for the selectors"
        )
    commit = freeze["frozen_at_commit"]
    if not isinstance(commit, str) or _GIT_COMMIT.fullmatch(commit) is None:
        raise TransitionGovernanceError("legacy authority freeze commit is invalid")
    if root is not None:
        exists = subprocess.run(
            ("git", "cat-file", "-e", f"{commit}^{{commit}}"),
            cwd=root,
            check=False,
            capture_output=True,
        )
        ancestor = subprocess.run(
            ("git", "merge-base", "--is-ancestor", commit, "HEAD"),
            cwd=root,
            check=False,
            capture_output=True,
        )
        if exists.returncode != 0 or ancestor.returncode != 0:
            raise TransitionGovernanceError(
                "legacy authority freeze commit is not an ancestor of HEAD"
            )
    frozen_rows = freeze["surfaces"]
    if not isinstance(frozen_rows, list):
        raise TransitionGovernanceError(
            "legacy authority freeze surfaces must be an array"
        )
    frozen: dict[str, str] = {}
    for row in frozen_rows:
        if not isinstance(row, dict):
            raise TransitionGovernanceError("legacy authority freeze row is malformed")
        _strict_keys(row, {"surface_id", "content_digest"}, "freeze surface")
        surface_id = row["surface_id"]
        digest = row["content_digest"]
        if (
            not isinstance(surface_id, str)
            or not isinstance(digest, str)
            or _SHA256.fullmatch(digest) is None
            or surface_id in frozen
        ):
            raise TransitionGovernanceError(
                "legacy authority freeze row is invalid or duplicate"
            )
        frozen[surface_id] = digest
    coverage = evaluate_disposition_coverage(report, program)
    if (
        coverage.uncovered_surfaces
        or coverage.overlapping_surfaces
        or coverage.unresolved_mixed_files
        or coverage.no_match_selectors
    ):
        raise TransitionGovernanceError(
            "legacy authority freeze requires complete disposition coverage"
        )
    current = {
        surface.surface_id: surface.content_digest
        for surface in report.surfaces
        if surface.surface_id in coverage.candidate_surfaces
    }
    introduced = sorted(set(current) - set(frozen))
    changed = sorted(
        surface_id
        for surface_id in set(current) & set(frozen)
        if current[surface_id] != frozen[surface_id]
    )
    if introduced or changed:
        raise TransitionGovernanceError(
            f"legacy authority freeze violated: introduced={introduced[:12]}, "
            f"changed={changed[:12]}"
        )
    return {
        "frozen_surface_count": len(frozen),
        "current_surface_count": len(current),
        "deleted_surface_count": len(set(frozen) - set(current)),
        "introduced_surface_count": 0,
        "changed_surface_count": 0,
        "frozen_at_commit": commit,
    }


def _inventory_summary(report: InventoryReport) -> dict[str, object]:
    return validate_inventory_universe(report)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("authority-cutover-check")
    commands.add_parser("legacy-suite-current-authority-zero-state-check")
    commands.add_parser("legacy-inventory-universe-check")
    commands.add_parser("legacy-disposition-coverage-check")
    commands.add_parser("legacy-authority-freeze-check")
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    root = arguments.root.resolve()
    try:
        if arguments.command in {
            "authority-cutover-check",
            "legacy-suite-current-authority-zero-state-check",
        }:
            report = validate_authority_selection(root)
        else:
            inventory = collect_inventory(root)
            if arguments.command == "legacy-inventory-universe-check":
                report = _inventory_summary(inventory)
            else:
                program = load_selector_program(root)
                if arguments.command == "legacy-disposition-coverage-check":
                    report = validate_disposition_coverage(inventory, program)
                else:
                    freeze = _load_json(root / LEGACY_FREEZE, "legacy authority freeze")
                    report = validate_legacy_authority_freeze(
                        inventory, program, freeze, root=root
                    )
    except (OSError, TransitionGovernanceError) as error:
        print(f"relational fabric transition error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
