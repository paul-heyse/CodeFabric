"""Executable contracts for independently useful Cargo feature boundaries."""

from __future__ import annotations

import argparse
import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import tomllib

ROOT = Path(__file__).resolve().parents[2]


class FeatureArchitectureError(ValueError):
    """A declared or resolved feature graph violates the application boundary."""


@dataclass(frozen=True)
class FeatureContract:
    manifest_items: frozenset[str]
    required_root_features: frozenset[str]
    forbidden_root_features: frozenset[str]
    required_packages: frozenset[str]
    forbidden_packages: frozenset[str]


CONTRACTS = {
    "provider-contracts": FeatureContract(
        manifest_items=frozenset(
            {
                "contract-models",
                "dep:arrow-array",
                "dep:arrow-schema",
                "dep:thiserror",
            }
        ),
        required_root_features=frozenset(
            {"canonical-json", "contract-models", "provider-contracts"}
        ),
        forbidden_root_features=frozenset(
            {
                "compatibility-probes",
                "daemon",
                "data-fabric",
                "fact-generation",
                "local-workstation",
                "operational-state",
                "repository-input",
                "repository-state",
                "rpc",
                "s3-storage",
                "semantic-release",
            }
        ),
        required_packages=frozenset(
            {"arrow-array", "arrow-schema", "codefabric", "thiserror"}
        ),
        forbidden_packages=frozenset(
            {
                "arc-swap",
                "datafusion",
                "deltalake",
                "gix",
                "prost",
                "rayon",
                "ruff_python_ast",
                "rusqlite",
                "tokio",
                "tonic",
                "tree-sitter",
            }
        ),
    )
}


def _validate_manifest(manifest: dict[str, Any], scope: str) -> None:
    contract = CONTRACTS[scope]
    features = manifest.get("features")
    if not isinstance(features, dict):
        raise FeatureArchitectureError("Cargo.toml has no feature table")
    observed = features.get(scope)
    if not isinstance(observed, list) or set(observed) != contract.manifest_items:
        raise FeatureArchitectureError(
            f"{scope} manifest edge mismatch: observed={observed!r}, "
            f"expected={sorted(contract.manifest_items)!r}"
        )


def _validate_metadata(metadata: dict[str, Any], scope: str) -> dict[str, Any]:
    contract = CONTRACTS[scope]
    resolve = metadata.get("resolve")
    packages = metadata.get("packages")
    if not isinstance(resolve, dict) or not isinstance(packages, list):
        raise FeatureArchitectureError("cargo metadata omitted packages or resolution")
    package_names = {
        package["id"]: package["name"]
        for package in packages
        if isinstance(package, dict)
        and isinstance(package.get("id"), str)
        and isinstance(package.get("name"), str)
    }
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list):
        raise FeatureArchitectureError("cargo metadata omitted resolved nodes")
    nodes_by_id = {
        node["id"]: node
        for node in nodes
        if isinstance(node, dict) and isinstance(node.get("id"), str)
    }
    root_ids = [
        identifier
        for identifier in nodes_by_id
        if package_names.get(identifier) == "codefabric"
    ]
    if len(root_ids) != 1:
        raise FeatureArchitectureError(
            "cargo metadata did not identify one root package"
        )
    reachable = set(root_ids)
    pending = list(root_ids)
    while pending:
        node = nodes_by_id[pending.pop()]
        dependencies = node.get("deps", [])
        if not isinstance(dependencies, list):
            raise FeatureArchitectureError(
                "cargo metadata node has invalid dependencies"
            )
        for dependency in dependencies:
            if not isinstance(dependency, dict) or not isinstance(
                dependency.get("pkg"), str
            ):
                continue
            dependency_kinds = dependency.get("dep_kinds", [])
            if not isinstance(dependency_kinds, list) or not any(
                isinstance(kind, dict) and kind.get("kind") != "dev"
                for kind in dependency_kinds
            ):
                continue
            package_id = dependency["pkg"]
            if package_id in nodes_by_id and package_id not in reachable:
                reachable.add(package_id)
                pending.append(package_id)
    resolved_names = {package_names[identifier] for identifier in reachable}
    missing_packages = contract.required_packages - resolved_names
    forbidden_packages = contract.forbidden_packages & resolved_names
    if missing_packages or forbidden_packages:
        raise FeatureArchitectureError(
            f"{scope} package graph mismatch: missing={sorted(missing_packages)}, "
            f"forbidden={sorted(forbidden_packages)}"
        )

    root_node = nodes_by_id[root_ids[0]]
    if not isinstance(root_node.get("features"), list):
        raise FeatureArchitectureError(
            "cargo metadata did not identify one root feature set"
        )
    root_features = set(root_node["features"])
    missing_features = contract.required_root_features - root_features
    forbidden_features = contract.forbidden_root_features & root_features
    if missing_features or forbidden_features:
        raise FeatureArchitectureError(
            f"{scope} root feature mismatch: missing={sorted(missing_features)}, "
            f"forbidden={sorted(forbidden_features)}"
        )
    return {
        "scope": scope,
        "root_features": sorted(root_features),
        "resolved_package_count": len(resolved_names),
        "required_packages": sorted(contract.required_packages),
    }


def validate(scope: str, root: Path = ROOT) -> dict[str, Any]:
    """Validate one manifest and live resolved graph.

    Raises:
        FeatureArchitectureError: if the scope is unknown or its graph differs.
    """
    if scope not in CONTRACTS:
        raise FeatureArchitectureError(
            f"unsupported feature architecture scope: {scope}"
        )
    with (root / "Cargo.toml").open("rb") as source:
        manifest = tomllib.load(source)
    _validate_manifest(manifest, scope)
    completed = subprocess.run(
        (
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--no-default-features",
            "--features",
            scope,
        ),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(completed.stdout)
    if not isinstance(metadata, dict):
        raise FeatureArchitectureError("cargo metadata root is not an object")
    return _validate_metadata(metadata, scope)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("scope", choices=sorted(CONTRACTS))
    args = parser.parse_args()
    print(json.dumps(validate(args.scope), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
