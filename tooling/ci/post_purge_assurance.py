"""Validate the compact v3 disposition and post-purge package boundary.

The disposition ledger intentionally stores only stable coverage intent.  The accepted
plan remains the source of each L/DB decision, while this validator derives row closure,
target proof, deletion proof, history exclusions, recipes, and the live package inventory.
It never restores a predecessor selector or a frozen file census.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

import tomllib

from tooling.ci import artifact_contracts
from tooling.ci.remaining_legacy_zero_state import (
    NON_EXECUTABLE_REFERENCE_GLOBS,
    RELEASED_EVIDENCE_GLOBS,
    RETAINED_HISTORY_GLOBS,
)

ROOT = Path(__file__).resolve().parents[2]
LEDGER_PATH = Path("contracts/governance/relational-fabric-v3-disposition-ledger.json")
PLAN_PATH = Path(
    "docs/plans/"
    "codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md"
)

EXPECTED_ORACLES = {
    "integrity": "legacy-disposition-artifact-integrity-check",
    "retained_behavior": "retained-target-post-purge-behavior-check",
    "negative": "remaining-legacy-zero-state-check",
    "package_operations": "post-purge-package-build-operations-check",
}
EXPECTED_RETAINED_BEHAVIOR_DEPENDENCIES = frozenset(
    {
        "adapter-test",
        "analysis-producer-semantic-check",
        "exact-provider-batch-check",
        "programmatic-production-composition-check",
        "semantic-request-program-check",
    }
)
EXPECTED_PACKAGE_OPERATION_DEPENDENCIES = frozenset(
    {
        "adapter-wheel-test",
        "extractor-check",
        "features-each",
        "proto-repro-check",
        "root-check",
        "sidecar-check",
        "stable-graph-check",
    }
)
EXPECTED_L_IDS = frozenset(f"L-{number}" for number in range(20, 56))
EXPECTED_DB_IDS = frozenset(f"DB{number:02d}" for number in range(9, 14))

EXPECTED_CARGO_MANIFESTS = frozenset(
    {
        "Cargo.toml",
        "fuzz/Cargo.toml",
        "pyrefly-sidecar/Cargo.toml",
        "rustc-extractor/Cargo.toml",
        "tooling/ci/duplicate-family-fixture/Cargo.toml",
    }
)
EXPECTED_BUILD_DOMAINS = {
    "Cargo.toml": "codefabric",
    "rustc-extractor/Cargo.toml": "codefabric-rustc-extractor",
    "pyrefly-sidecar/Cargo.toml": "codefabric-pyrefly-sidecar",
}
EXPECTED_ASSURANCE_PACKAGES = {
    "fuzz/Cargo.toml": "codefabric-fuzz",
    "tooling/ci/duplicate-family-fixture/Cargo.toml": (
        "codefabric-duplicate-family-negative-fixture"
    ),
}
PYTHON_MANIFEST = "codefabric-cpg-mcp/pyproject.toml"
PYTHON_PACKAGE = "codefabric-cpg-mcp"
EXPECTED_LOCKS = frozenset(
    {
        "Cargo.lock",
        "rustc-extractor/Cargo.lock",
        "pyrefly-sidecar/Cargo.lock",
        "codefabric-cpg-mcp/uv.lock",
    }
)
EXPECTED_ROOT_FEATURES = frozenset(
    {
        "canonical-json",
        "compatibility-probes",
        "contract-models",
        "daemon",
        "data-fabric",
        "default",
        "fact-generation",
        "local-workstation",
        "proto-tooling",
        "repository-state",
        "rpc",
        "s3-storage",
    }
)
EXPECTED_ROOT_BINS = {
    "codefabric": ("src/bin/codefabric.rs", ("daemon",)),
    "codefabric-proto-gen": (
        "tooling/proto/generate.rs",
        ("proto-tooling",),
    ),
    "codefabricd": ("src/bin/codefabricd.rs", ("daemon",)),
}
EXPECTED_PYTHON_RUNTIME_DEPENDENCIES = frozenset(
    {
        "blake3==1.0.9",
        "fastmcp==3.4.7",
        "grpcio==1.83.0",
        "mcp==1.29.0",
        "protobuf==7.36.0",
        "pydantic==2.13.4",
        "pydantic-settings==2.15.0",
        "rfc8785==0.1.4",
    }
)
EXPECTED_PYTHON_DEV_DEPENDENCIES = frozenset(
    {
        "grpcio-tools==1.83.0",
        "jsonschema==4.26.0",
        "pyyaml==6.0.3",
        "pyrefly>=1.2.0",
        "pytest>=9.1.1",
        "ruff>=0.16.3",
    }
)

SKIPPED_DIRECTORY_NAMES = frozenset(
    {
        ".git",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        ".venv",
        "__pycache__",
        "node_modules",
        "target",
    }
)


class PostPurgeAssuranceError(ValueError):
    """A v3 disposition or surviving package boundary is incomplete."""


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise PostPurgeAssuranceError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def _load_json(path: Path, context: str) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicates
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise PostPurgeAssuranceError(
            f"missing or invalid {context}: {path}"
        ) from error
    if not isinstance(value, dict):
        raise PostPurgeAssuranceError(f"{context} must be an object")
    return value


def _load_toml(path: Path, context: str) -> dict[str, Any]:
    try:
        value = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise PostPurgeAssuranceError(
            f"missing or invalid {context}: {path}"
        ) from error
    if not isinstance(value, dict):
        raise PostPurgeAssuranceError(f"{context} must be a table")
    return value


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise PostPurgeAssuranceError(f"{context} must be an object")
    return value


def _strict_keys(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    if set(value) != expected:
        raise PostPurgeAssuranceError(
            f"{context} keys differ: expected={sorted(expected)}, "
            f"observed={sorted(value)}"
        )


def _string_list(value: object, context: str) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or not all(isinstance(item, str) and item for item in value)
    ):
        raise PostPurgeAssuranceError(f"{context} must be a nonempty string list")
    return value


def _plan_l_rows(plan: str) -> dict[str, tuple[str, str, str, str, str]]:
    start = plan.find("## 6. Successor L-20--L-55 disposition map")
    end = plan.find("## 7. Decommission batches", start)
    if start < 0 or end < 0:
        raise PostPurgeAssuranceError("successor L disposition section is absent")
    rows: dict[str, tuple[str, str, str, str, str]] = {}
    for line in plan[start:end].splitlines():
        if re.match(r"^\| L-\d+ \|", line) is None:
            continue
        cells = tuple(cell.strip() for cell in line.strip().strip("|").split("|"))
        if len(cells) != 6:
            raise PostPurgeAssuranceError(f"malformed disposition row: {line}")
        disposition_id, treatment, outcome, cutover, positive, negative = cells
        if disposition_id in rows:
            raise PostPurgeAssuranceError(
                f"duplicate disposition row: {disposition_id}"
            )
        if not all((treatment, outcome, cutover, positive, negative)):
            raise PostPurgeAssuranceError(
                f"{disposition_id} omits a disposition, target, or proof"
            )
        if not re.search(r"delete|remove|replace|reshape|preserve", treatment):
            raise PostPurgeAssuranceError(
                f"{disposition_id} has no recognized retained outcome"
            )
        if re.fullmatch(r"`[a-z0-9-]+`", positive) is None:
            raise PostPurgeAssuranceError(
                f"{disposition_id} target consumer oracle is malformed"
            )
        if re.fullmatch(r"`[a-z0-9-]+`", negative) is None:
            raise PostPurgeAssuranceError(
                f"{disposition_id} deletion oracle is malformed"
            )
        rows[disposition_id] = (treatment, outcome, cutover, positive, negative)
    return rows


def _plan_db_rows(plan: str) -> dict[str, tuple[str, str]]:
    rows: dict[str, tuple[str, str]] = {}
    matches = list(re.finditer(r"^### (DB\d{2}) — .+$", plan, re.MULTILINE))
    for index, match in enumerate(matches):
        disposition_id = match.group(1)
        if disposition_id not in EXPECTED_DB_IDS:
            continue
        end = matches[index + 1].start() if index + 1 < len(matches) else len(plan)
        block = plan[match.start() : end]
        disposition = re.search(
            r"\*\*Disposition\.\*\*\s*(.*?)(?=\n\n\*\*Exit\.\*\*)",
            block,
            re.DOTALL,
        )
        exit_contract = re.search(
            r"\*\*Exit\.\*\*\s*(.*?)(?=\n\n(?:###|##)|\Z)",
            block,
            re.DOTALL,
        )
        if disposition is None or exit_contract is None:
            raise PostPurgeAssuranceError(
                f"{disposition_id} omits its disposition or exit proof"
            )
        rows[disposition_id] = (
            " ".join(disposition.group(1).split()),
            " ".join(exit_contract.group(1).split()),
        )
    return rows


def _recipes(root: Path) -> dict[str, Any]:
    try:
        return artifact_contracts.load_just_recipes(root)
    except (
        OSError,
        json.JSONDecodeError,
        artifact_contracts.ArtifactContractError,
    ) as error:
        raise PostPurgeAssuranceError(
            "cannot derive the live Just recipe graph"
        ) from error


def _recipe_dependencies(recipe: Mapping[str, Any], context: str) -> set[str]:
    dependencies = recipe.get("dependencies")
    if not isinstance(dependencies, list):
        raise PostPurgeAssuranceError(f"{context} dependencies are malformed")
    result: set[str] = set()
    for value in dependencies:
        item = _mapping(value, f"{context} dependency")
        dependency = item.get("recipe")
        if not isinstance(dependency, str) or not dependency:
            raise PostPurgeAssuranceError(f"{context} dependency is unnamed")
        result.add(dependency)
    return result


def validate_retained_recipe_contract(root: Path = ROOT) -> dict[str, int]:
    """Require exact retained-behavior and four-domain operation composition."""
    recipes = _recipes(root)
    missing_recipes = set(EXPECTED_ORACLES.values()) - recipes.keys()
    if missing_recipes:
        raise PostPurgeAssuranceError(
            f"disposition oracle recipes are absent: {sorted(missing_recipes)}"
        )
    behavior = _mapping(
        recipes[EXPECTED_ORACLES["retained_behavior"]],
        "retained behavior recipe",
    )
    behavior_dependencies = _recipe_dependencies(behavior, "retained behavior recipe")
    if behavior_dependencies != EXPECTED_RETAINED_BEHAVIOR_DEPENDENCIES:
        raise PostPurgeAssuranceError(
            "retained behavior recipe dependency coverage differs"
        )
    package = _mapping(
        recipes[EXPECTED_ORACLES["package_operations"]],
        "package operations recipe",
    )
    package_dependencies = _recipe_dependencies(package, "package operations recipe")
    if package_dependencies != EXPECTED_PACKAGE_OPERATION_DEPENDENCIES:
        raise PostPurgeAssuranceError(
            "package operations recipe dependency coverage differs"
        )
    body = package.get("body")
    if not isinstance(
        body, list
    ) or "post_purge_assurance.py package-inventory" not in json.dumps(
        body, sort_keys=True
    ):
        raise PostPurgeAssuranceError(
            "package operations recipe omits the derived package inventory"
        )
    return {
        "retained_behavior_dependencies": len(behavior_dependencies),
        "package_operation_dependencies": len(package_dependencies),
    }


def validate_disposition_integrity(root: Path = ROOT) -> dict[str, int]:
    """Prove compact artifact coverage against the accepted plan and live recipes."""
    ledger = _load_json(root / LEDGER_PATH, "v3 disposition ledger")
    _strict_keys(
        ledger,
        {
            "artifact_id",
            "schema_version",
            "plan_path",
            "covered_l_range",
            "covered_db_range",
            "history_exclusions",
            "oracles",
        },
        "v3 disposition ledger",
    )
    if (
        ledger["artifact_id"]
        != "codefabric.governance.relational-fabric-v3-disposition-ledger"
        or ledger["schema_version"] != 1
        or ledger["plan_path"] != str(PLAN_PATH)
    ):
        raise PostPurgeAssuranceError("v3 disposition ledger identity differs")
    if ledger["covered_l_range"] != {"first": 20, "last": 55}:
        raise PostPurgeAssuranceError("L disposition coverage range differs")
    if ledger["covered_db_range"] != {"first": 9, "last": 13}:
        raise PostPurgeAssuranceError("DB disposition coverage range differs")

    exclusions = _mapping(ledger["history_exclusions"], "history exclusions")
    _strict_keys(
        exclusions,
        {
            "retained_history_globs",
            "released_evidence_globs",
            "non_executable_reference_globs",
        },
        "history exclusions",
    )
    expected_exclusions = {
        "retained_history_globs": list(RETAINED_HISTORY_GLOBS),
        "released_evidence_globs": list(RELEASED_EVIDENCE_GLOBS),
        "non_executable_reference_globs": list(NON_EXECUTABLE_REFERENCE_GLOBS),
    }
    if dict(exclusions) != expected_exclusions:
        raise PostPurgeAssuranceError(
            "history exclusions differ from the permanent negative oracle"
        )
    oracles = _mapping(ledger["oracles"], "disposition oracles")
    if dict(oracles) != EXPECTED_ORACLES:
        raise PostPurgeAssuranceError("v3 disposition oracle mapping differs")

    try:
        plan = (root / PLAN_PATH).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise PostPurgeAssuranceError("accepted v3 plan is unavailable") from error
    l_rows = _plan_l_rows(plan)
    db_rows = _plan_db_rows(plan)
    if set(l_rows) != EXPECTED_L_IDS:
        raise PostPurgeAssuranceError(
            "L disposition coverage differs: "
            f"missing={sorted(EXPECTED_L_IDS - l_rows.keys())}, "
            f"extra={sorted(l_rows.keys() - EXPECTED_L_IDS)}"
        )
    if set(db_rows) != EXPECTED_DB_IDS:
        raise PostPurgeAssuranceError(
            "DB disposition coverage differs: "
            f"missing={sorted(EXPECTED_DB_IDS - db_rows.keys())}, "
            f"extra={sorted(db_rows.keys() - EXPECTED_DB_IDS)}"
        )
    recipe_report = validate_retained_recipe_contract(root)
    return {
        "l_dispositions": len(l_rows),
        "db_dispositions": len(db_rows),
        "history_exclusion_classes": len(exclusions),
        "oracles": len(oracles),
        **recipe_report,
    }


def _discover_cargo_manifests(root: Path) -> tuple[set[str], set[str]]:
    manifests: set[str] = set()
    skipped: set[str] = set()
    for directory, names, files in os.walk(root):
        relative_directory = Path(directory).relative_to(root)
        retained: list[str] = []
        for name in sorted(names):
            relative = relative_directory / name
            if name in SKIPPED_DIRECTORY_NAMES:
                skipped.add(relative.as_posix())
            else:
                retained.append(name)
        names[:] = retained
        if "Cargo.toml" in files:
            manifests.add((relative_directory / "Cargo.toml").as_posix())
    return manifests, skipped


def _package_name(manifest: Mapping[str, Any], context: str) -> str:
    package = _mapping(manifest.get("package"), f"{context} package")
    value = package.get("name")
    if not isinstance(value, str) or not value:
        raise PostPurgeAssuranceError(f"{context} package name is absent")
    if package.get("publish") is not False:
        raise PostPurgeAssuranceError(f"{context} must remain unpublished")
    return value


def _feature_inventory(manifest: Mapping[str, Any]) -> dict[str, list[str]]:
    features = _mapping(manifest.get("features"), "root features")
    result: dict[str, list[str]] = {}
    for name, members in features.items():
        if (
            not isinstance(name, str)
            or not isinstance(members, list)
            or not all(isinstance(member, str) and member for member in members)
        ):
            raise PostPurgeAssuranceError(f"invalid root feature declaration: {name}")
        result[name] = list(members)
    if set(result) != EXPECTED_ROOT_FEATURES:
        raise PostPurgeAssuranceError(
            "root feature inventory differs: "
            f"missing={sorted(EXPECTED_ROOT_FEATURES - result.keys())}, "
            f"extra={sorted(result.keys() - EXPECTED_ROOT_FEATURES)}"
        )
    dependencies = _mapping(manifest.get("dependencies"), "root dependencies")
    for feature, members in result.items():
        for member in members:
            if member.startswith("dep:"):
                dependency = member.removeprefix("dep:")
                if dependency not in dependencies:
                    raise PostPurgeAssuranceError(
                        f"{feature} references absent dependency {dependency}"
                    )
            elif "/" in member:
                dependency = member.split("/", 1)[0]
                if dependency not in dependencies:
                    raise PostPurgeAssuranceError(
                        f"{feature} references absent dependency feature {member}"
                    )
            elif member not in result:
                raise PostPurgeAssuranceError(
                    f"{feature} references absent feature {member}"
                )
    if result["default"] != ["local-workstation"]:
        raise PostPurgeAssuranceError("default feature is not the local workstation")
    return result


def _bin_inventory(
    manifest: Mapping[str, Any],
) -> dict[str, tuple[str, tuple[str, ...]]]:
    values = manifest.get("bin")
    if not isinstance(values, list):
        raise PostPurgeAssuranceError("root binary inventory is absent")
    result: dict[str, tuple[str, tuple[str, ...]]] = {}
    for index, value in enumerate(values, 1):
        item = _mapping(value, f"root binary {index}")
        if set(item) != {"name", "path", "required-features"}:
            raise PostPurgeAssuranceError(f"root binary {index} keys differ")
        name = item.get("name")
        path = item.get("path")
        required = item.get("required-features")
        if (
            not isinstance(name, str)
            or not isinstance(path, str)
            or not isinstance(required, list)
            or not all(isinstance(feature, str) for feature in required)
        ):
            raise PostPurgeAssuranceError(f"root binary {index} is malformed")
        result[name] = (path, tuple(required))
    if result != EXPECTED_ROOT_BINS:
        raise PostPurgeAssuranceError("root binary inventory differs")
    return result


def validate_package_inventory(root: Path = ROOT) -> dict[str, object]:
    """Derive and validate the exact production, assurance, feature, and lock roots."""
    manifests, skipped_directories = _discover_cargo_manifests(root)
    if manifests != EXPECTED_CARGO_MANIFESTS:
        raise PostPurgeAssuranceError(
            "Cargo manifest inventory differs: "
            f"missing={sorted(EXPECTED_CARGO_MANIFESTS - manifests)}, "
            f"extra={sorted(manifests - EXPECTED_CARGO_MANIFESTS)}"
        )
    if not skipped_directories:
        raise PostPurgeAssuranceError("package discovery recorded no explicit skips")
    missing_locks = sorted(
        path for path in EXPECTED_LOCKS if not (root / path).is_file()
    )
    if missing_locks:
        raise PostPurgeAssuranceError(f"package locks are absent: {missing_locks}")

    cargo_documents = {
        path: _load_toml(root / path, path) for path in sorted(manifests)
    }
    for path, package_name in {
        **EXPECTED_BUILD_DOMAINS,
        **EXPECTED_ASSURANCE_PACKAGES,
    }.items():
        document = cargo_documents[path]
        if "workspace" in document:
            raise PostPurgeAssuranceError(f"{path} creates a Cargo workspace")
        if _package_name(document, path) != package_name:
            raise PostPurgeAssuranceError(f"{path} package identity differs")

    root_manifest = cargo_documents["Cargo.toml"]
    features = _feature_inventory(root_manifest)
    bins = _bin_inventory(root_manifest)
    library = _mapping(root_manifest.get("lib"), "root library")
    if library.get("name") != "codefabric" or library.get("crate-type") != ["rlib"]:
        raise PostPurgeAssuranceError("stable root library identity differs")

    python = _load_toml(root / PYTHON_MANIFEST, PYTHON_MANIFEST)
    project = _mapping(python.get("project"), "adapter project")
    if project.get("name") != PYTHON_PACKAGE:
        raise PostPurgeAssuranceError("adapter package identity differs")
    runtime_dependencies = project.get("dependencies")
    if not isinstance(runtime_dependencies, list) or set(runtime_dependencies) != set(
        EXPECTED_PYTHON_RUNTIME_DEPENDENCIES
    ):
        raise PostPurgeAssuranceError("adapter runtime dependency inventory differs")
    groups = _mapping(python.get("dependency-groups"), "adapter dependency groups")
    if set(groups) != {"dev"} or not isinstance(groups["dev"], list):
        raise PostPurgeAssuranceError("adapter dependency groups differ")
    if set(groups["dev"]) != set(EXPECTED_PYTHON_DEV_DEPENDENCIES):
        raise PostPurgeAssuranceError(
            "adapter development dependency inventory differs"
        )

    return {
        "production_build_domains": [
            *sorted(EXPECTED_BUILD_DOMAINS),
            PYTHON_MANIFEST,
        ],
        "assurance_cargo_roots": sorted(EXPECTED_ASSURANCE_PACKAGES),
        "root_features": sorted(features),
        "root_binaries": sorted(bins),
        "locks": sorted(EXPECTED_LOCKS),
        "explicitly_skipped_directory_count": len(skipped_directories),
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("disposition-integrity", "package-inventory"))
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.mode == "disposition-integrity":
            report: Mapping[str, object] = validate_disposition_integrity()
        else:
            report = validate_package_inventory()
    except (
        OSError,
        PostPurgeAssuranceError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        print(f"post-purge assurance failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
