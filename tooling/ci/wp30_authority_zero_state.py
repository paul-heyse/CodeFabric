"""Prove the retired bootstrap/model/ontology selection authority is not live.

The inventory is derived from the current filesystem, Cargo metadata, Just recipe graph,
Python source distribution, workflows, services, rules, fixtures, and ignored source files.
Historical documents and oracle definitions are exact exclusions and are counted rather than
silently hidden. A controlled non-history fixture proves that the live detector can fail.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from collections import Counter
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
MAX_TEXT_BYTES = 16 * 1024 * 1024

HISTORY_EXCLUSIONS = (
    "docs/designs/**",
    "docs/plans/**",
    "docs/reviews/**",
)
RELEASED_EVIDENCE_EXCLUSIONS = (
    "contracts/acceptance/relational-fabric-v3/**",
    "contracts/acceptance/released-artifact-census-v1.json",
)
NON_IMPORTABLE_REFERENCE_EXCLUSIONS = (
    "docs/library_ref/**",
    "docs/authoritative_design/**",
)
ORACLE_EXCLUSIONS = (
    "scripts/model_zero_state_check.sh",
    "scripts/adapter_wheel_test.sh",
    "tooling/ci/remaining_legacy_zero_state.py",
    "tooling/ci/test_remaining_legacy_zero_state.py",
    "tooling/ci/wp30_authority_zero_state.py",
    "tooling/ci/test_wp30_authority_zero_state.py",
    "tooling/ci/fixtures/wp30-live-legacy-route.rs",
)
ALL_EXCLUSIONS = (
    *HISTORY_EXCLUSIONS,
    *RELEASED_EVIDENCE_EXCLUSIONS,
    *NON_IMPORTABLE_REFERENCE_EXCLUSIONS,
    *ORACLE_EXCLUSIONS,
)

LIVE_TEXT_ROOTS = (
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    "Cargo.toml",
    "justfile",
    ".cargo",
    ".claude",
    ".codex",
    ".agents",
    ".config",
    ".github",
    "src",
    "tests",
    "scripts",
    "tooling",
    "rules",
    "rule-tests",
    "contracts",
    "fuzz",
    "rustc-extractor",
    "pyrefly-sidecar",
    "codefabric-cpg-mcp",
    "docs/spec_index",
)

FORBIDDEN_PATH_GLOBS = (
    "src/ontology_*",
    "src/ontology_*/**",
    "src/bin/codefabric_model/**",
    "src/generated/model*",
    "src/generated/model*/**",
    "src/relational_model/**",
    "contracts/generated/model/**",
    "tooling/model/**",
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_registries.py",
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/query_forms.py",
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/schemas.py",
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/fingerprints.py",
)
FORBIDDEN_DIRECTORIES = (
    "src/bin/codefabric_model",
    "src/relational_model",
    "contracts/generated/model",
    "tooling/model",
)
FORBIDDEN_TEXT = (
    "serve_programmatic",
    "ProgrammaticWorkspaceConstruction",
    "ProgrammaticWorkspaceRuntimeFactory",
    "ProgrammaticDaemonComposition",
    "ProgrammaticDaemonCompositionShutdownError",
    "TestQueryBackend",
    "programmatic_public_bundle_versions",
    "serve_with_programmatic_query_backend",
    "bootstrap-model-consumer-cutover-check",
    "bootstrap-model-dual-authority-zero-state-check",
    "production_serve_requires_programmatic_composition",
)
FORBIDDEN_RECIPE_NAMES = frozenset(
    {
        "bootstrap-model-consumer-cutover-check",
        "bootstrap-model-dual-authority-zero-state-check",
    }
)
FORBIDDEN_CARGO_FEATURES = frozenset({"model-compiler", "provider-inventory-tooling"})
FORBIDDEN_CARGO_TARGETS = frozenset(
    {
        "codefabric-model",
        "codefabric-model-schema-consumer",
        "codefabric-gate-b-candidate",
        "codefabric-provider-inventory",
    }
)
FORBIDDEN_PACKAGE_MEMBERS = (
    "adapter-fingerprints.json",
    "adapter-package-data.json",
    "adapter-schemas.json",
    "fingerprints.py",
    "model_artifact_index.json",
    "model_registries.py",
    "query-form-contract.json",
    "query_forms.py",
    "schemas.py",
)
FORBIDDEN_IGNORED_NAMES = (
    "model_registries",
    "query_forms",
    "ontology_candidate",
    "ontology_program",
    "relational_fabric_transition",
    "test_ontology_compiled_data_fabric",
    "error_registry_closure",
)
CARGO_MANIFESTS = (
    "Cargo.toml",
    "rustc-extractor/Cargo.toml",
    "pyrefly-sidecar/Cargo.toml",
    "fuzz/Cargo.toml",
)
STRUCTURAL_QUERIES = (
    ("rust", "serve_programmatic($$$A)"),
    ("rust", "ProgrammaticWorkspaceConstruction { $$$F }"),
    ("rust", "ProgrammaticDaemonComposition { $$$F }"),
    ("rust", "TestQueryBackend"),
    ("python", "from codefabric_cpg_mcp.contracts.model_registries import $$$N"),
    ("python", "from codefabric_cpg_mcp.contracts.query_forms import $$$N"),
)
STRUCTURAL_ROOTS = {
    "rust": ("src", "tests", "rustc-extractor/src", "pyrefly-sidecar/src"),
    "python": ("tooling", "codefabric-cpg-mcp/src", "codefabric-cpg-mcp/tests"),
}
NEGATIVE_FIXTURE = "tooling/ci/fixtures/wp30-live-legacy-route.rs"
ACTIVATION_RESIDUE_MANIFEST = "tooling/ci/wp30_activation_residue.json"
EXPECTED_ACTIVATION_RESIDUE = frozenset(
    {
        "ActivatedEpochReceipt",
        "ActivationCacheOutcome",
        "ActivationCacheReceipt",
        "ActivationReconciliationReceiptCache",
        "DeltaActivationRuntimeAuthority",
        "ProgrammaticDeltaRuntime",
        "ProgrammaticDeltaRuntimePorts",
        "ProgrammaticWorkspaceReleasePins",
        "ProgrammaticWorkspaceRuntime",
        "ProgrammaticWorkspaceStartupObservation",
        "WorkspaceEpochQueryAuthorityRegistry",
    }
)


class Wp30ZeroStateError(ValueError):
    """The current inventory contains retired authority or incomplete coverage."""


def _matches(path: str, globs: Iterable[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in globs)


def _git_paths(root: Path, *arguments: str) -> list[str]:
    completed = subprocess.run(
        ("git", "ls-files", "-z", *arguments),
        cwd=root,
        check=True,
        capture_output=True,
    )
    return sorted(item.decode() for item in completed.stdout.split(b"\0") if item)


def current_files(root: Path) -> list[str]:
    paths = _git_paths(root, "--cached", "--others", "--exclude-standard")
    return [path for path in paths if (root / path).is_file()]


def classify_files(paths: Iterable[str]) -> tuple[list[str], list[str]]:
    """Return live candidates and exact exclusions without a maintained file census."""
    live: list[str] = []
    excluded: list[str] = []
    for path in sorted(set(paths)):
        (excluded if _matches(path, ALL_EXCLUSIONS) else live).append(path)
    return live, excluded


def _under_roots(path: str, roots: Iterable[str]) -> bool:
    return any(path == root or path.startswith(f"{root}/") for root in roots)


def path_inventory(root: Path, live: Sequence[str]) -> tuple[int, list[str]]:
    violations = [
        f"path: retired live authority {path}"
        for path in live
        if _matches(path, FORBIDDEN_PATH_GLOBS)
    ]
    for relative in FORBIDDEN_DIRECTORIES:
        if (root / relative).exists():
            violations.append(f"path: retired live directory {relative}")
    return len(live) + len(FORBIDDEN_DIRECTORIES), violations


def _text_candidates(root: Path, live: Sequence[str]) -> tuple[list[str], int]:
    candidates: list[str] = []
    skipped = 0
    for path in live:
        absolute = root / path
        if (
            not _under_roots(path, LIVE_TEXT_ROOTS)
            or absolute.stat().st_size > MAX_TEXT_BYTES
        ):
            skipped += 1
            continue
        candidates.append(path)
    return candidates, skipped


def retired_text_violations(
    path: str, text: str, *, dimension: str = "text"
) -> list[str]:
    """Return the forbidden-token findings shared by live and control scans."""
    return [
        f"{dimension}: {path} contains retired token {token!r}"
        for token in FORBIDDEN_TEXT
        if token in text
    ]


def text_inventory(root: Path, live: Sequence[str]) -> tuple[int, int, list[str]]:
    candidates, skipped = _text_candidates(root, live)
    violations: list[str] = []
    for path in candidates:
        data = (root / path).read_bytes()
        if b"\0" in data:
            skipped += 1
            continue
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            skipped += 1
            continue
        violations.extend(retired_text_violations(path, text))
    return len(candidates), skipped, violations


def structural_inventory(root: Path) -> tuple[int, int, list[str]]:
    violations: list[str] = []
    skipped = 0
    candidate_files = 0
    for language, roots in STRUCTURAL_ROOTS.items():
        suffix = ".rs" if language == "rust" else ".py"
        candidate_files += sum(
            1
            for relative in roots
            if (root / relative).exists()
            for path in (root / relative).rglob(f"*{suffix}")
            if path.is_file()
        )
    for language, pattern in STRUCTURAL_QUERIES:
        roots = [
            relative
            for relative in STRUCTURAL_ROOTS[language]
            if (root / relative).exists()
        ]
        if not roots:
            skipped += 1
            continue
        completed = subprocess.run(
            (
                "ast-grep",
                "run",
                "--lang",
                language,
                "--pattern",
                pattern,
                "--json=compact",
                "--inspect",
                "summary",
                *roots,
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode == 0:
            violations.append(
                f"syntax: retired {language} pattern {pattern!r}: {completed.stdout.strip()}"
            )
        elif completed.returncode != 1:
            raise Wp30ZeroStateError(
                f"ast-grep failed for {pattern!r}: {completed.stderr.strip()}"
            )
        match = re.search(r"skippedFileCount=(\d+)", completed.stderr)
        if match:
            skipped += int(match.group(1))
    return candidate_files * len(STRUCTURAL_QUERIES), skipped, violations


def cargo_payload_inventory(
    payloads: Iterable[Mapping[str, Any]],
) -> tuple[int, list[str]]:
    candidate_count = 0
    violations: list[str] = []
    for payload in payloads:
        for package in payload.get("packages", []):
            package_name = str(package.get("name", "<unnamed>"))
            features = set(package.get("features", {}))
            targets = {
                str(target.get("name", "")) for target in package.get("targets", [])
            }
            dependencies = {
                str(item.get("name", "")) for item in package.get("dependencies", [])
            }
            candidate_count += 1 + len(features) + len(targets) + len(dependencies)
            for feature in sorted(features & FORBIDDEN_CARGO_FEATURES):
                violations.append(f"cargo: forbidden feature {package_name}#{feature}")
            for target in sorted(targets & FORBIDDEN_CARGO_TARGETS):
                violations.append(f"cargo: forbidden target {package_name}#{target}")
    return candidate_count, violations


def cargo_inventory(root: Path) -> tuple[int, list[str]]:
    payloads: list[Mapping[str, Any]] = []
    for manifest in CARGO_MANIFESTS:
        completed = subprocess.run(
            (
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
                manifest,
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode != 0:
            raise Wp30ZeroStateError(
                f"Cargo metadata failed for {manifest}: {completed.stderr.strip()}"
            )
        payloads.append(json.loads(completed.stdout))
    return cargo_payload_inventory(payloads)


def python_package_inventory(live: Sequence[str]) -> tuple[int, list[str]]:
    prefix = "codefabric-cpg-mcp/src/codefabric_cpg_mcp/"
    paths = [path for path in live if path.startswith(prefix)]
    violations = [
        f"python_package: retired package member {path}"
        for path in paths
        if Path(path).name in FORBIDDEN_PACKAGE_MEMBERS
    ]
    return len(paths), violations


def generated_include_inventory(
    root: Path, live: Sequence[str]
) -> tuple[int, list[str]]:
    rust_paths = [path for path in live if path.endswith(".rs")]
    pattern = re.compile(
        r"include(?:_bytes|_str)?!\s*\([^)]*(?:model|ontology|bootstrap)", re.IGNORECASE
    )
    violations = []
    for path in rust_paths:
        text = (root / path).read_text(encoding="utf-8")
        if pattern.search(text):
            violations.append(f"generated_include: retired include in {path}")
    return len(rust_paths), violations


def recipe_inventory(root: Path) -> tuple[int, list[str]]:
    completed = subprocess.run(
        ("just", "--dump", "--dump-format", "json"),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise Wp30ZeroStateError(
            f"Just recipe inventory failed: {completed.stderr.strip()}"
        )
    recipes = set(json.loads(completed.stdout).get("recipes", {}))
    violations = [
        f"recipe: retired selector {name}"
        for name in sorted(recipes & FORBIDDEN_RECIPE_NAMES)
    ]
    return len(recipes), violations


def _bounded_text_dimension(
    root: Path,
    live: Sequence[str],
    *,
    dimension: str,
    roots: Sequence[str],
) -> tuple[int, list[str]]:
    paths = [path for path in live if _under_roots(path, roots)]
    violations: list[str] = []
    for path in paths:
        data = (root / path).read_bytes()
        if b"\0" in data or len(data) > MAX_TEXT_BYTES:
            continue
        text = data.decode("utf-8")
        for token in (*FORBIDDEN_TEXT, *FORBIDDEN_RECIPE_NAMES):
            if token in text:
                violations.append(
                    f"{dimension}: {path} contains retired token {token!r}"
                )
    return len(paths), violations


def ignored_source_inventory(root: Path) -> tuple[int, list[str]]:
    paths = _git_paths(
        root,
        "--others",
        "--ignored",
        "--exclude-standard",
        "--",
        "src",
        "tests",
        "scripts",
        "tooling",
        "rules",
        "rule-tests",
        "contracts",
        "rustc-extractor/src",
        "pyrefly-sidecar/src",
        "codefabric-cpg-mcp/src",
        "codefabric-cpg-mcp/tests",
        ".github",
    )
    violations = [
        f"ignored_source: retired ignored artifact {path}"
        for path in paths
        if any(token in path for token in FORBIDDEN_IGNORED_NAMES)
    ]
    return len(paths), violations


def archive_member_violations(members: Iterable[str]) -> tuple[int, list[str]]:
    normalized = sorted({member.replace("\\", "/") for member in members})
    violations = [
        f"installed_artifact: retired distribution member {member}"
        for member in normalized
        if Path(member).name in FORBIDDEN_PACKAGE_MEMBERS
    ]
    return len(normalized), violations


def installed_artifact_inventory(root: Path) -> tuple[int, list[str]]:
    with tempfile.TemporaryDirectory(prefix="codefabric-wp30-package-") as temporary:
        output = Path(temporary) / "dist"
        completed = subprocess.run(
            (
                "uv",
                "build",
                "--project",
                str(root / "codefabric-cpg-mcp"),
                "--wheel",
                "--sdist",
                "--out-dir",
                str(output),
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if completed.returncode != 0:
            raise Wp30ZeroStateError(
                f"Python distribution build failed: {completed.stderr}"
            )
        archives = sorted(output.iterdir())
        wheels = [path for path in archives if path.suffix == ".whl"]
        sdists = [path for path in archives if path.name.endswith(".tar.gz")]
        if len(wheels) != 1 or len(sdists) != 1:
            raise Wp30ZeroStateError(
                "installed-artifact inventory requires one wheel and one sdist"
            )
        with zipfile.ZipFile(wheels[0]) as wheel:
            wheel_members = wheel.namelist()
        with tarfile.open(sdists[0], mode="r:gz") as sdist:
            sdist_members = sdist.getnames()
        wheel_count, wheel_issues = archive_member_violations(wheel_members)
        sdist_count, sdist_issues = archive_member_violations(sdist_members)
        return wheel_count + sdist_count, [*wheel_issues, *sdist_issues]


def validate_negative_fixture(root: Path) -> int:
    path = root / NEGATIVE_FIXTURE
    if not path.is_file():
        raise Wp30ZeroStateError(
            f"controlled negative fixture is missing: {NEGATIVE_FIXTURE}"
        )
    if _matches(NEGATIVE_FIXTURE, HISTORY_EXCLUSIONS):
        raise Wp30ZeroStateError(
            "controlled negative fixture must remain outside history"
        )
    if not _matches(NEGATIVE_FIXTURE, ORACLE_EXCLUSIONS):
        raise Wp30ZeroStateError(
            "controlled negative fixture requires one exact oracle exclusion"
        )
    findings = retired_text_violations(
        NEGATIVE_FIXTURE,
        path.read_text(encoding="utf-8"),
        dimension="controlled_negative",
    )
    if not findings:
        raise Wp30ZeroStateError(
            "controlled live negative fixture does not fail the detector"
        )
    return len(findings)


def validate_activation_residue(root: Path) -> int:
    """Require a complete symbol-to-consumer-to-WP32 disposition for retained residue."""
    path = root / ACTIVATION_RESIDUE_MANIFEST
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("schema") != "codefabric.wp30.activation-residue.v1":
        raise Wp30ZeroStateError("activation residue manifest has the wrong schema")
    if payload.get("owner_packet") != "WP32":
        raise Wp30ZeroStateError(
            "activation residue must remain explicitly owned by WP32"
        )
    entries = payload.get("entries")
    if not isinstance(entries, list):
        raise Wp30ZeroStateError("activation residue entries must be a list")
    symbols = {entry.get("symbol") for entry in entries if isinstance(entry, dict)}
    if symbols != EXPECTED_ACTIVATION_RESIDUE or len(entries) != len(symbols):
        raise Wp30ZeroStateError(
            "activation residue symbols differ from the exact reviewed WP32 handoff"
        )
    for entry in entries:
        symbol = entry["symbol"]
        source = entry.get("path")
        consumer = entry.get("target_consumer")
        action = entry.get("wp32_action")
        if not all(
            isinstance(value, str) and value.strip()
            for value in (source, consumer, action)
        ):
            raise Wp30ZeroStateError(
                f"activation residue mapping is incomplete for {symbol}"
            )
        source_path = root / source
        if not source_path.is_file() or not re.search(
            rf"\b{re.escape(symbol)}\b", source_path.read_text(encoding="utf-8")
        ):
            raise Wp30ZeroStateError(
                f"activation residue symbol {symbol} is absent from declared path {source}"
            )
    return len(entries)


def validate_wp30_zero_state(
    root: Path = ROOT, *, build_distributions: bool = True
) -> dict[str, object]:
    files = current_files(root)
    live, excluded = classify_files(files)
    dimensions: dict[str, int] = {}
    violations: list[str] = []

    dimensions["path"], found = path_inventory(root, live)
    violations.extend(found)
    dimensions["text"], text_skipped, found = text_inventory(root, live)
    violations.extend(found)
    dimensions["syntax"], syntax_skipped, found = structural_inventory(root)
    violations.extend(found)
    dimensions["cargo"], found = cargo_inventory(root)
    violations.extend(found)
    dimensions["python_package"], found = python_package_inventory(live)
    violations.extend(found)
    dimensions["generated_include"], found = generated_include_inventory(root, live)
    violations.extend(found)
    dimensions["recipe"], found = recipe_inventory(root)
    violations.extend(found)

    bounded = {
        "workflow": (".github/workflows",),
        "service": ("src/bin", "scripts", ".github"),
        "rule": ("rules", "rule-tests"),
        "fixture": ("contracts", "tests/fixtures", "rule-tests"),
    }
    for dimension, roots in bounded.items():
        dimensions[dimension], found = _bounded_text_dimension(
            root, live, dimension=dimension, roots=roots
        )
        violations.extend(found)

    dimensions["ignored_source"], found = ignored_source_inventory(root)
    violations.extend(found)
    if build_distributions:
        dimensions["installed_artifact"], found = installed_artifact_inventory(root)
        violations.extend(found)
    else:
        dimensions["installed_artifact"] = python_package_inventory(live)[0]

    negative_fixture_matches = validate_negative_fixture(root)
    activation_residue_count = validate_activation_residue(root)
    candidate_count = sum(dimensions.values())
    skipped_count = len(excluded) + text_skipped + syntax_skipped
    exclusion_classes = Counter(
        "history"
        if _matches(path, HISTORY_EXCLUSIONS)
        else "released_evidence"
        if _matches(path, RELEASED_EVIDENCE_EXCLUSIONS)
        else "reference"
        if _matches(path, NON_IMPORTABLE_REFERENCE_EXCLUSIONS)
        else "oracle"
        for path in excluded
    )
    report: dict[str, object] = {
        "candidate_count": candidate_count,
        "skipped_count": skipped_count,
        "exclusion_count": len(excluded),
        "exclusion_classes": dict(sorted(exclusion_classes.items())),
        "dimension_candidate_counts": dict(sorted(dimensions.items())),
        "negative_fixture_candidate_count": 1,
        "negative_fixture_violation_count": negative_fixture_matches,
        "activation_residue_count": activation_residue_count,
        "violation_count": len(violations),
    }
    if candidate_count == 0 or skipped_count == 0 or not excluded:
        violations.append(
            "coverage: candidate, skipped, and exclusion counts must all be nonzero"
        )
    empty_dimensions = sorted(name for name, count in dimensions.items() if count == 0)
    if empty_dimensions:
        violations.append(f"coverage: empty inventory dimensions {empty_dimensions}")
    if violations:
        raise Wp30ZeroStateError(
            f"{json.dumps(report, sort_keys=True)}\n" + "\n".join(violations)
        )
    return report


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--skip-distributions", action="store_true")
    arguments = parser.parse_args(argv)
    try:
        report = validate_wp30_zero_state(
            arguments.root.resolve(),
            build_distributions=not arguments.skip_distributions,
        )
    except (
        OSError,
        Wp30ZeroStateError,
        subprocess.CalledProcessError,
        json.JSONDecodeError,
    ) as error:
        print(f"WP30 authority zero-state failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
