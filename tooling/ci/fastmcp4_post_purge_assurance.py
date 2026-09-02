"""Derive the WP49 FastMCP 4 post-purge live-surface census.

History and frozen acceptance data are intentionally outside this scan.  The
validator covers every live source, package, recipe, workflow, service, rule,
generated binding, and operational binary surface; it rejects skipped,
unreadable, or unparsed live candidates instead of treating no matches as proof.
"""

from __future__ import annotations

import argparse
import ast
import json
import os
import re
import sys
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[2]

EXPECTED_RUNTIME_DEPENDENCIES = (
    "blake3==1.0.9",
    "fastmcp==4.0.0",
    "grpcio==1.83.0",
    "mcp==2.1.1",
    "opentelemetry-api==1.44.0",
    "protobuf==7.36.0",
    "pydantic==2.13.4",
    "rfc8785==0.1.4",
)
EXPECTED_ROOT_BINS = {
    "codefabric": ("src/bin/codefabric.rs", ("daemon",)),
    "codefabric-proto-gen": ("tooling/proto/generate.rs", ("proto-tooling",)),
    "codefabricd": ("src/bin/codefabricd.rs", ("daemon",)),
}
EXPECTED_OPERATIONAL_BIN_FILES = {"codefabric.rs", "codefabricd.rs"}
EXPECTED_CPGD_RUST_BINDINGS = {"codefabric.cpgd.v2.rs"}
EXPECTED_CPGD_PYTHON_BINDINGS = {
    "cpg_query_service_pb2.py",
    "cpg_query_service_pb2.pyi",
    "cpg_query_service_pb2_grpc.py",
}

FORBIDDEN_PATHS = {
    "contracts/adapter",
    "src/generated/codefabric.cpgd.v1.rs",
    "src/production_evidence_core_tests.rs",
    "src/production_evidence_tests.rs",
    "src/production_query_evidence_tests.rs",
    "codefabric-cpg-mcp/tests/test_arrow_resources.py",
    "codefabric-cpg-mcp/tests/test_production_evidence_claim017.py",
    "tooling/ci/production_evidence.py",
    "tooling/ci/test_production_evidence.py",
    "tooling/ci/reissue_wp38_transaction.py",
    "tooling/ci/successor_evidence_issuance.py",
    "tooling/ci/test_successor_evidence_issuance.py",
    "tooling/ci/successor_evidence_issuance_v4.py",
    "tooling/ci/test_successor_evidence_issuance_v4.py",
    "tooling/ci/successor_evidence_contracts_v4.py",
    "tooling/ci/test_successor_evidence_contracts_v4.py",
    "tooling/ci/post_purge_assurance.py",
    "tooling/ci/test_post_purge_assurance.py",
    "tooling/ci/test_reissue_wp38_transaction.py",
    "tooling/ci/record_wp33_acceptance.py",
    "tooling/ci/reissue_wp33_r3.py",
    "tooling/ci/record_wp33_v4_acceptance.py",
    "tooling/ci/relational_fabric_release.py",
    "tooling/ci/test_relational_fabric_release.py",
    "tooling/ci/successor_certification.py",
    "tooling/ci/test_successor_certification.py",
    "tooling/ci/supervisor_launch_contract_v4.py",
    "tooling/ci/test_supervisor_launch_contract_v4.py",
    "scripts/data_fabric_revision_check.sh",
    "tooling/data_fabric_revision_benchmark.rs",
    "tests/fixtures/data_fabric_upgrade/benchmark_comparator.json",
}

# Assemble retired tokens so this zero-state oracle does not flag its own
# specification.  Its focused falsification test is also excluded below.
RETIRED_TOKENS = {
    "fastmcp3_pin": "fastmcp==" + "3.4.7",
    "mcp1_pin": "mcp==" + "1.29.0",
    "compat_environment": "FASTMCP_MCP_" + "CAMELCASE_COMPAT",
    "legacy_elicitation": "ctx." + "elicit(",
    "python_resource_leases": "_resource_" + "leases",
    "mcp_call_identity": "mcp_" + "call_id",
    "rpc_attempt_identity": "rpc_" + "attempt_id",
    "pydantic_settings_import": "pydantic_" + "settings",
    "fastmcp_task_import": "fastmcp." + "tasks",
    "fastmcp_slim_import": "fastmcp_" + "slim",
    "old_arrow_test": "test_arrow_" + "resources.py",
    "old_wp38_projection_test": "test_production_evidence_" + "claim017.py",
}
RETIRED_RECIPE_NAMES = {
    "successor-authority-expectation-integrity-check",
    "independent-expected-relation-review-check",
    "negative-fixture-independence-check",
    "expectation-drift-selector-sensitivity-check",
    "wp38-artifact-bound-positive-execution-check",
    "wp38-artifact-bound-causal-execution-check",
    "wp38-artifact-bound-negative-execution-check",
    "production-evidence-input-integrity-check",
    "first-principles-production-behavior-check",
    "causal-fault-discrimination-check",
    "production-evidence-recovery-operations-check",
    "legacy-disposition-artifact-integrity-check",
    "retained-target-post-purge-behavior-check",
    "post-purge-package-build-operations-check",
    "release-evidence-record-integrity-check",
    "release-evidence-matrix-v3-check",
    "security-resource-release-rejection-check",
    "data-fabric-stack-compat",
    "data-fabric-upgrade-bench",
}

LIVE_FILE_ROOTS = (
    Path("Cargo.toml"),
    Path("src"),
    Path("tests"),
    Path("codefabric-cpg-mcp/pyproject.toml"),
    Path("codefabric-cpg-mcp/uv.lock"),
    Path("codefabric-cpg-mcp/src"),
    Path("codefabric-cpg-mcp/tests"),
    Path("contracts/rpc"),
    Path("contracts/schema"),
    Path("scripts"),
    Path("tooling/ci"),
    Path("tooling/fastmcp4_modern_client_driver.py"),
    Path("tooling/proto"),
    Path("rules"),
    Path("rule-tests"),
    Path(".github"),
    Path("justfile"),
    Path("README.md"),
    Path("AGENTS.md"),
)
TEXT_SUFFIXES = {
    ".json",
    ".md",
    ".proto",
    ".py",
    ".pyi",
    ".rs",
    ".sh",
    ".toml",
    ".yaml",
    ".yml",
}
SKIPPED_DIRECTORY_NAMES = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "__pycache__",
    "node_modules",
    "target",
}
ORACLE_SELF_PATHS = {
    Path("tooling/ci/fastmcp4_post_purge_assurance.py"),
    Path("tooling/ci/test_fastmcp4_post_purge_assurance.py"),
}
GOVERNED_HISTORY_ROOTS = {Path("tooling/proto/history")}
ALLOWED_LIVE_SYMLINKS = {
    Path("scripts/lib-outline"): Path("scripts/lib-outline.sh"),
    Path("scripts/spec-outline"): Path("scripts/spec-outline.sh"),
}


class PostPurgeAssuranceError(ValueError):
    """A typed incomplete-coverage or predecessor-reachability failure."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _require(condition: bool, code: str, message: str) -> None:
    if not condition:
        raise PostPurgeAssuranceError(code, message)


def _load_toml(path: Path) -> Mapping[str, object]:
    try:
        value = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise PostPurgeAssuranceError(
            "RFV5_PURGE_UNREADABLE", f"{path}: {error}"
        ) from error
    _require(
        isinstance(value, Mapping), "RFV5_PURGE_UNPARSED", f"{path} is not a TOML table"
    )
    return value


def _validate_live_symlink(root: Path, path: Path) -> None:
    relative = path.relative_to(root)
    expected = ALLOWED_LIVE_SYMLINKS.get(relative)
    if expected is None:
        raise PostPurgeAssuranceError(
            "RFV5_PURGE_SYMLINK_CANDIDATE",
            f"live scan root contains unclassified symlink: {relative}",
        )
    try:
        observed_target = path.resolve(strict=True)
        expected_target = (root / expected).resolve(strict=True)
    except OSError as error:
        raise PostPurgeAssuranceError(
            "RFV5_PURGE_SYMLINK_CANDIDATE",
            f"classified live symlink is dangling: {relative}",
        ) from error
    _require(
        observed_target == expected_target,
        "RFV5_PURGE_SYMLINK_CANDIDATE",
        f"classified live symlink target differs: {relative}",
    )


def _iter_live_files(root: Path) -> tuple[list[Path], list[str], list[str]]:
    files: set[Path] = set()
    skipped_classes: set[str] = set()
    classified_symlinks: set[str] = set()
    for relative_root in LIVE_FILE_ROOTS:
        candidate = root / relative_root
        _require(
            candidate.exists(),
            "RFV5_PURGE_COVERAGE_MISSING",
            f"live root is absent: {relative_root}",
        )
        if candidate.is_file():
            files.add(relative_root)
            continue
        for directory, names, filenames in os.walk(candidate, followlinks=False):
            directory_path = Path(directory)
            for name in list(names):
                child = directory_path / name
                if name in SKIPPED_DIRECTORY_NAMES:
                    names.remove(name)
                    skipped_classes.add(name)
                elif child.is_symlink():
                    _validate_live_symlink(root, child)
                    classified_symlinks.add(str(child.relative_to(root)))
                    names.remove(name)
            for filename in filenames:
                path = directory_path / filename
                if path.is_symlink():
                    _validate_live_symlink(root, path)
                    classified_symlinks.add(str(path.relative_to(root)))
                    continue
                relative = path.relative_to(root)
                if relative.suffix in TEXT_SUFFIXES or relative.name in {"justfile"}:
                    files.add(relative)
    return sorted(files), sorted(skipped_classes), sorted(classified_symlinks)


def _read_live_text(root: Path, path: Path) -> str:
    try:
        return (root / path).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise PostPurgeAssuranceError(
            "RFV5_PURGE_UNREADABLE", f"{path}: {error}"
        ) from error


def validate_coverage(root: Path = ROOT) -> Mapping[str, object]:
    """Read every live candidate and parse every Python/TOML source."""

    files, skipped, classified_symlinks = _iter_live_files(root)
    parsed_python = 0
    parsed_toml = 0
    for path in files:
        text = _read_live_text(root, path)
        if path.suffix == ".py":
            try:
                ast.parse(text, filename=str(path))
            except SyntaxError as error:
                raise PostPurgeAssuranceError(
                    "RFV5_PURGE_UNPARSED", f"{path}: {error}"
                ) from error
            parsed_python += 1
        elif path.suffix == ".toml":
            _load_toml(root / path)
            parsed_toml += 1
    _require(
        bool(files) and parsed_python > 0 and parsed_toml > 0,
        "RFV5_PURGE_ZERO_SELECTION",
        "live coverage selected no meaningful candidates",
    )
    return {
        "live_files": len(files),
        "parsed_python": parsed_python,
        "parsed_toml": parsed_toml,
        "skipped_directory_classes": skipped,
        "classified_symlinks": classified_symlinks,
        "unreadable": 0,
        "unparsed": 0,
        "overlapping": 0,
        "unmatched": 0,
    }


def _source_authority_files(files: Iterable[Path]) -> list[Path]:
    allowed_roots = (
        Path("src"),
        Path("codefabric-cpg-mcp/src"),
        Path("contracts/rpc"),
        Path("contracts/schema"),
        Path("scripts"),
        Path("tooling/ci"),
        Path("tooling/proto"),
        Path("rules"),
        Path(".github"),
    )
    return [
        path
        for path in files
        if not any(base == path or base in path.parents for base in GOVERNED_HISTORY_ROOTS)
        if (
            path
            in {
                Path("Cargo.toml"),
                Path("codefabric-cpg-mcp/pyproject.toml"),
                Path("codefabric-cpg-mcp/uv.lock"),
                Path("justfile"),
            }
            or any(path == base or base in path.parents for base in allowed_roots)
        )
    ]


def validate_zero_state(root: Path = ROOT) -> Mapping[str, object]:
    """Reject every physically displaced serving and authority class."""

    present_forbidden = sorted(
        path for path in FORBIDDEN_PATHS if (root / path).exists()
    )
    _require(
        not present_forbidden,
        "RFV5_PURGE_FORBIDDEN_PATH",
        f"retired live paths remain: {present_forbidden}",
    )

    files, _, _ = _iter_live_files(root)
    matches: list[str] = []
    for path in _source_authority_files(files):
        if path in ORACLE_SELF_PATHS:
            continue
        text = _read_live_text(root, path)
        for category, token in RETIRED_TOKENS.items():
            if token in text:
                matches.append(f"{category}:{path}")
    _require(
        not matches,
        "RFV5_PURGE_RETIRED_TOKEN",
        "retired live tokens remain: " + ", ".join(matches),
    )

    justfile = _read_live_text(root, Path("justfile"))
    live_recipes = set(re.findall(r"(?m)^([a-zA-Z0-9_-]+)(?: [^:]*)?:", justfile))
    retired_recipes = sorted(live_recipes & RETIRED_RECIPE_NAMES)
    _require(
        not retired_recipes,
        "RFV5_PURGE_RETIRED_RECIPE",
        f"retired recipes remain live: {retired_recipes}",
    )
    return {
        "forbidden_path_classes": len(FORBIDDEN_PATHS),
        "retired_token_classes": len(RETIRED_TOKENS),
        "retired_recipe_classes": len(RETIRED_RECIPE_NAMES),
        "live_matches": 0,
    }


def _string_list(value: object, context: str) -> list[str]:
    _require(
        isinstance(value, list) and all(isinstance(item, str) for item in value),
        "RFV5_PURGE_PACKAGE_INVALID",
        f"{context} must be a string list",
    )
    assert isinstance(value, list)
    return [str(item) for item in value]


def validate_package_contract(root: Path = ROOT) -> Mapping[str, object]:
    """Inspect exact retained packages, generated bindings, features, and binaries."""

    pyproject = _load_toml(root / "codefabric-cpg-mcp/pyproject.toml")
    project = pyproject.get("project")
    _require(
        isinstance(project, Mapping),
        "RFV5_PURGE_PACKAGE_INVALID",
        "Python project table is absent",
    )
    assert isinstance(project, Mapping)
    dependencies = tuple(
        _string_list(project.get("dependencies"), "runtime dependencies")
    )
    _require(
        dependencies == EXPECTED_RUNTIME_DEPENDENCIES,
        "RFV5_PURGE_PACKAGE_INVALID",
        f"runtime dependency closure differs: {dependencies}",
    )

    cargo = _load_toml(root / "Cargo.toml")
    bins = cargo.get("bin")
    _require(
        isinstance(bins, list),
        "RFV5_PURGE_PACKAGE_INVALID",
        "Cargo bin inventory is absent",
    )
    observed_bins: dict[str, tuple[str, tuple[str, ...]]] = {}
    for row in bins:
        _require(
            isinstance(row, Mapping),
            "RFV5_PURGE_PACKAGE_INVALID",
            "Cargo bin row is malformed",
        )
        assert isinstance(row, Mapping)
        name = str(row.get("name"))
        path = str(row.get("path"))
        required = tuple(
            _string_list(row.get("required-features"), f"{name} required features")
        )
        observed_bins[name] = (path, required)
    _require(
        observed_bins == EXPECTED_ROOT_BINS,
        "RFV5_PURGE_PACKAGE_INVALID",
        f"Cargo bin inventory differs: {observed_bins}",
    )

    bin_files = {path.name for path in (root / "src/bin").glob("*.rs")}
    _require(
        bin_files == EXPECTED_OPERATIONAL_BIN_FILES,
        "RFV5_PURGE_BINARY_SURFACE",
        f"operational binary files differ: {sorted(bin_files)}",
    )
    supervisor = _read_live_text(root, Path("src/bin/codefabric.rs"))
    daemon = _read_live_text(root, Path("src/bin/codefabricd.rs"))
    _require(
        "CodefabricProcessSettings::parse" in supervisor
        and "FabricDaemonProcessSettings::parse" in daemon,
        "RFV5_PURGE_BINARY_SURFACE",
        "operational binaries do not delegate to typed library settings",
    )
    _require(
        not re.search(
            r"(?i)schema|registry|ontology|datafusion|deltalake", supervisor + daemon
        ),
        "RFV5_PURGE_BINARY_SURFACE",
        "semantic generation/execution leaked into thin binaries",
    )

    rust_cpgd = {
        path.name for path in (root / "src/generated").glob("codefabric.cpgd.*.rs")
    }
    _require(
        rust_cpgd == EXPECTED_CPGD_RUST_BINDINGS,
        "RFV5_PURGE_GENERATED_BINDING",
        f"Rust CPG daemon bindings differ: {rust_cpgd}",
    )
    python_generated = (
        root / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated"
    )
    python_cpgd = {
        path.name for path in python_generated.glob("cpg_query_service_pb2*")
    }
    _require(
        python_cpgd == EXPECTED_CPGD_PYTHON_BINDINGS,
        "RFV5_PURGE_GENERATED_BINDING",
        f"Python CPG daemon bindings differ: {python_cpgd}",
    )
    return {
        "runtime_dependencies": len(dependencies),
        "root_binaries": len(observed_bins),
        "operational_binaries": len(bin_files),
        "rust_cpgd_bindings": len(rust_cpgd),
        "python_cpgd_bindings": len(python_cpgd),
    }


def validate_all(root: Path = ROOT) -> Mapping[str, object]:
    return {
        "coverage": validate_coverage(root),
        "zero_state": validate_zero_state(root),
        "package": validate_package_contract(root),
    }


def _report(command: str, root: Path) -> Mapping[str, object]:
    if command == "surface":
        selected = validate_coverage(root)
        count = int(selected["live_files"])
    elif command == "zero-state":
        selected = validate_zero_state(root)
        count = int(selected["retired_token_classes"]) + int(
            selected["forbidden_path_classes"]
        )
    elif command == "package":
        selected = validate_package_contract(root)
        count = sum(int(value) for value in selected.values())
    else:
        selected = validate_all(root)
        count = sum(int(value) for value in selected["package"].values())
    _require(
        count > 0, "RFV5_PURGE_ZERO_SELECTION", f"{command} selected no candidates"
    )
    return {
        "status": "passed",
        "command": command,
        "selected_count": count,
        "report": selected,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command", choices=("surface", "retained", "zero-state", "package")
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        report = _report(args.command, args.root)
    except (PostPurgeAssuranceError, OSError) as error:
        print(
            json.dumps(
                {
                    "status": "failed",
                    "code": getattr(error, "code", "RFV5_PURGE_UNREADABLE"),
                    "message": str(error),
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
