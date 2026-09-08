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
                "dep:arrow-buffer",
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
            {"arrow-array", "arrow-buffer", "arrow-schema", "codefabric", "thiserror"}
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
    ),
    "release-compiler": FeatureContract(
        manifest_items=frozenset({"provider-contracts"}),
        required_root_features=frozenset(
            {
                "canonical-json",
                "contract-models",
                "provider-contracts",
                "release-compiler",
            }
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
            {"arrow-array", "arrow-buffer", "arrow-schema", "codefabric", "thiserror"}
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
    ),
    "fact-generation": FeatureContract(
        manifest_items=frozenset(
            {
                "provider-contracts",
                "dep:blake3",
                "dep:petgraph",
                "dep:rayon",
                "dep:ruff_python_ast",
                "dep:ruff_python_index",
                "dep:ruff_python_parser",
                "dep:ruff_python_semantic",
                "dep:ruff_python_trivia",
                "dep:ruff_source_file",
                "dep:ruff_text_size",
                "dep:tree-sitter",
                "dep:tree-sitter-python",
                "dep:tree-sitter-rust",
                "dep:thiserror",
            }
        ),
        required_root_features=frozenset(
            {
                "canonical-json",
                "contract-models",
                "fact-generation",
                "provider-contracts",
            }
        ),
        forbidden_root_features=frozenset(
            {
                "compatibility-probes",
                "daemon",
                "data-fabric",
                "local-workstation",
                "operational-state",
                "release-compiler",
                "repository-input",
                "repository-state",
                "rpc",
                "s3-storage",
                "semantic-release",
            }
        ),
        required_packages=frozenset(
            {
                "arrow-array",
                "arrow-schema",
                "blake3",
                "codefabric",
                "petgraph",
                "rayon",
                "ruff_python_ast",
                "ruff_python_index",
                "ruff_python_parser",
                "ruff_python_semantic",
                "ruff_python_trivia",
                "ruff_source_file",
                "ruff_text_size",
                "thiserror",
                "tree-sitter",
                "tree-sitter-python",
                "tree-sitter-rust",
            }
        ),
        forbidden_packages=frozenset(
            {
                "arc-swap",
                "datafusion",
                "deltalake",
                "gix",
                "prost",
                "rusqlite",
                "tokio",
                "tonic",
            }
        ),
    ),
    "data-fabric": FeatureContract(
        manifest_items=frozenset(
            {
                "canonical-json",
                "contract-models",
                "provider-contracts",
                "dep:async-trait",
                "dep:arrow",
                "dep:arrow-array",
                "dep:arrow-buffer",
                "dep:arrow-cast",
                "dep:arrow-ipc",
                "dep:arrow-ord",
                "dep:arrow-row",
                "dep:arrow-schema",
                "dep:arrow-select",
                "dep:arrow-string",
                "dep:bytes",
                "dep:datafusion",
                "dep:deltalake",
                "dep:futures",
                "dep:object_store",
                "dep:parquet",
                "dep:petgraph",
                "dep:tokio",
                "dep:tracing",
                "dep:url",
            }
        ),
        required_root_features=frozenset(
            {"canonical-json", "contract-models", "data-fabric", "provider-contracts"}
        ),
        forbidden_root_features=frozenset(
            {
                "compatibility-probes",
                "daemon",
                "fact-generation",
                "local-workstation",
                "operational-state",
                "release-compiler",
                "repository-input",
                "rpc",
                "s3-storage",
                "semantic-release",
            }
        ),
        required_packages=frozenset(
            {
                "arrow-array",
                "arrow-schema",
                "codefabric",
                "datafusion",
                "deltalake",
                "object_store",
                "parquet",
                "petgraph",
                "tokio",
            }
        ),
        forbidden_packages=frozenset(
            {
                "arc-swap",
                "gix",
                "rayon",
                "ruff_python_ast",
                "rusqlite",
                "tonic",
                "tree-sitter",
            }
        ),
    ),
    "repository-input": FeatureContract(
        manifest_items=frozenset(
            {"contract-models", "dep:gix", "dep:rustix", "dep:url"}
        ),
        required_root_features=frozenset(
            {"canonical-json", "contract-models", "repository-input"}
        ),
        forbidden_root_features=frozenset(
            {
                "compatibility-probes",
                "daemon",
                "data-fabric",
                "fact-generation",
                "local-workstation",
                "operational-state",
                "release-compiler",
                "rpc",
                "s3-storage",
                "semantic-release",
            }
        ),
        required_packages=frozenset({"codefabric", "gix", "rustix", "url"}),
        forbidden_packages=frozenset(
            {
                "arrow-array",
                "datafusion",
                "deltalake",
                "prost",
                "rayon",
                "ruff_python_ast",
                "rusqlite",
                "tokio",
                "tonic",
                "tree-sitter",
            }
        ),
    ),
    "operational-state": FeatureContract(
        manifest_items=frozenset(
            {
                "contract-models",
                "dep:arrow-schema",
                "dep:rusqlite",
                "dep:rustix",
                "dep:url",
            }
        ),
        required_root_features=frozenset(
            {"canonical-json", "contract-models", "operational-state"}
        ),
        forbidden_root_features=frozenset(
            {
                "compatibility-probes",
                "daemon",
                "data-fabric",
                "fact-generation",
                "local-workstation",
                "release-compiler",
                "repository-input",
                "rpc",
                "s3-storage",
                "semantic-release",
            }
        ),
        required_packages=frozenset(
            {"arrow-schema", "codefabric", "rusqlite", "rustix", "url"}
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
                "tokio",
                "tonic",
                "tree-sitter",
            }
        ),
    ),
    "semantic-release": FeatureContract(
        manifest_items=frozenset(
            {"data-fabric", "fact-generation", "release-compiler"}
        ),
        required_root_features=frozenset(
            {
                "canonical-json",
                "contract-models",
                "data-fabric",
                "fact-generation",
                "provider-contracts",
                "release-compiler",
                "semantic-release",
            }
        ),
        forbidden_root_features=frozenset(
            {
                "compatibility-probes",
                "daemon",
                "local-workstation",
                "operational-state",
                "repository-input",
                "rpc",
                "s3-storage",
            }
        ),
        required_packages=frozenset(
            {
                "arrow-array",
                "codefabric",
                "datafusion",
                "deltalake",
                "rayon",
                "ruff_python_ast",
                "tree-sitter",
            }
        ),
        forbidden_packages=frozenset({"arc-swap", "gix", "rusqlite", "tonic"}),
    ),
    "daemon": FeatureContract(
        manifest_items=frozenset(
            {
                "dep:arc-swap",
                "contract-models",
                "semantic-release",
                "repository-input",
                "operational-state",
                "rpc",
                "dep:notify-debouncer-full",
                "dep:hyper-util",
                "dep:command-fds",
                "dep:sha2",
                "dep:tokio-stream",
                "dep:tokio-util",
                "dep:tonic-health",
                "dep:toml",
                "dep:tower",
                "dep:tracing",
            }
        ),
        required_root_features=frozenset(
            {
                "canonical-json",
                "contract-models",
                "daemon",
                "data-fabric",
                "fact-generation",
                "operational-state",
                "provider-contracts",
                "release-compiler",
                "repository-input",
                "rpc",
                "semantic-release",
            }
        ),
        forbidden_root_features=frozenset(
            {"compatibility-probes", "local-workstation", "s3-storage"}
        ),
        required_packages=frozenset(
            {
                "arc-swap",
                "arrow-array",
                "arrow-schema",
                "codefabric",
                "command-fds",
                "datafusion",
                "deltalake",
                "gix",
                "hyper-util",
                "notify-debouncer-full",
                "prost",
                "ruff_python_ast",
                "rusqlite",
                "sha2",
                "tokio",
                "tokio-stream",
                "tokio-util",
                "toml",
                "tonic",
                "tonic-health",
                "tonic-prost",
                "tower",
                "tree-sitter",
            }
        ),
        forbidden_packages=frozenset({"aws-config", "aws-sdk-s3", "deltalake-aws"}),
    ),
}

STATE_SCOPES = ("repository-input", "operational-state")
ALL_SCOPES = (*CONTRACTS, "cancellation")


def _resolved_root_graph(root: Path, feature: str) -> tuple[set[str], set[str]]:
    completed = subprocess.run(
        (
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--no-default-features",
            "--features",
            feature,
        ),
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(completed.stdout)
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
        for dependency in node.get("deps", []):
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
    root_features = nodes_by_id[root_ids[0]].get("features")
    if not isinstance(root_features, list):
        raise FeatureArchitectureError("cargo metadata omitted root feature selection")
    return ({package_names[identifier] for identifier in reachable}, set(root_features))


def _validate_cancellation(root: Path) -> dict[str, Any]:
    with (root / "Cargo.toml").open("rb") as source:
        manifest = tomllib.load(source)
    features = manifest.get("features")
    dependencies = manifest.get("dependencies")
    if not isinstance(features, dict) or not isinstance(dependencies, dict):
        raise FeatureArchitectureError("Cargo.toml omitted features or dependencies")
    daemon = features.get("daemon")
    tokio_util = dependencies.get("tokio-util")
    if not isinstance(daemon, list) or "dep:tokio-util" not in daemon:
        raise FeatureArchitectureError("daemon does not directly activate tokio-util")
    if not isinstance(tokio_util, dict) or tokio_util.get("optional") is not True:
        raise FeatureArchitectureError(
            "tokio-util must remain an optional direct dependency"
        )
    if tokio_util.get("version") != "=0.7.19" or tokio_util.get("features") != ["rt"]:
        raise FeatureArchitectureError("tokio-util cancellation pin/features changed")
    inward = (
        "provider-contracts",
        "release-compiler",
        "fact-generation",
        "data-fabric",
    )
    leaked = [
        feature
        for feature in inward
        if "dep:tokio-util" in set(features.get(feature, []))
    ]
    if leaked:
        raise FeatureArchitectureError(
            f"tokio-util leaked into inward capabilities: {sorted(leaked)}"
        )
    daemon_packages, daemon_features = _resolved_root_graph(root, "daemon")
    fact_packages, _ = _resolved_root_graph(root, "fact-generation")
    if "tokio-util" not in daemon_packages or "daemon" not in daemon_features:
        raise FeatureArchitectureError(
            "daemon resolution omitted structured cancellation"
        )
    if "tokio-util" in fact_packages:
        raise FeatureArchitectureError(
            "fact-generation resolved daemon cancellation support"
        )
    return {
        "scope": "cancellation",
        "daemon_feature": "daemon",
        "tokio_util": "0.7.19",
        "fact_generation_isolated": True,
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
    if scope == "state":
        return {
            "scope": scope,
            "capabilities": [validate(child, root) for child in STATE_SCOPES],
        }
    if scope == "all":
        return {
            "scope": scope,
            "capabilities": [validate(child, root) for child in ALL_SCOPES],
        }
    if scope == "cancellation":
        return _validate_cancellation(root)
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
    parser.add_argument(
        "scope", choices=[*sorted(CONTRACTS), "all", "cancellation", "state"]
    )
    args = parser.parse_args()
    print(json.dumps(validate(args.scope), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
