"""Fault tests for Cargo feature-architecture contracts."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

import pytest

from tooling.ci.feature_architecture import (
    CONTRACTS,
    FeatureArchitectureError,
    _validate_manifest,
    _validate_metadata,
)


def _metadata() -> dict[str, Any]:
    names = {
        "root": "codefabric",
        "array": "arrow-array",
        "schema": "arrow-schema",
        "error": "thiserror",
    }
    return {
        "packages": [
            {"id": identifier, "name": name} for identifier, name in names.items()
        ],
        "resolve": {
            "nodes": [
                {
                    "id": "root",
                    "features": [
                        "canonical-json",
                        "contract-models",
                        "provider-contracts",
                    ],
                    "deps": [
                        {
                            "pkg": package,
                            "dep_kinds": [{"kind": None}],
                        }
                        for package in ("array", "schema", "error")
                    ],
                },
                {"id": "array", "features": [], "deps": []},
                {"id": "schema", "features": [], "deps": []},
                {"id": "error", "features": [], "deps": []},
            ]
        },
    }


def test_provider_contract_feature_isolation() -> None:
    contract = CONTRACTS["provider-contracts"]
    _validate_manifest(
        {"features": {"provider-contracts": sorted(contract.manifest_items)}},
        "provider-contracts",
    )
    report = _validate_metadata(_metadata(), "provider-contracts")
    assert report["scope"] == "provider-contracts"


def test_provider_contracts_forbidden_package_fault_is_rejected() -> None:
    metadata = deepcopy(_metadata())
    metadata["packages"].append({"id": "tonic", "name": "tonic"})
    metadata["resolve"]["nodes"].append({"id": "tonic", "features": [], "deps": []})
    metadata["resolve"]["nodes"][0]["deps"].append(
        {"pkg": "tonic", "dep_kinds": [{"kind": None}]}
    )
    with pytest.raises(FeatureArchitectureError, match="forbidden=.*tonic"):
        _validate_metadata(metadata, "provider-contracts")


def test_provider_contracts_reverse_feature_fault_is_rejected() -> None:
    metadata = deepcopy(_metadata())
    root = metadata["resolve"]["nodes"][0]
    root["features"].append("daemon")
    with pytest.raises(FeatureArchitectureError, match="forbidden=.*daemon"):
        _validate_metadata(metadata, "provider-contracts")


def test_provider_contracts_manifest_widening_is_rejected() -> None:
    contract = CONTRACTS["provider-contracts"]
    widened = [*contract.manifest_items, "data-fabric"]
    with pytest.raises(FeatureArchitectureError, match="manifest edge mismatch"):
        _validate_manifest(
            {"features": {"provider-contracts": widened}}, "provider-contracts"
        )


def test_release_compiler_isolated_graph_is_accepted() -> None:
    contract = CONTRACTS["release-compiler"]
    _validate_manifest(
        {"features": {"release-compiler": sorted(contract.manifest_items)}},
        "release-compiler",
    )
    metadata = _metadata()
    metadata["resolve"]["nodes"][0]["features"].append("release-compiler")
    report = _validate_metadata(metadata, "release-compiler")
    assert report["scope"] == "release-compiler"


def test_release_compiler_state_dependency_fault_is_rejected() -> None:
    metadata = _metadata()
    metadata["resolve"]["nodes"][0]["features"].append("release-compiler")
    metadata["packages"].append({"id": "state", "name": "rusqlite"})
    metadata["resolve"]["nodes"].append({"id": "state", "features": [], "deps": []})
    metadata["resolve"]["nodes"][0]["deps"].append(
        {"pkg": "state", "dep_kinds": [{"kind": None}]}
    )
    with pytest.raises(FeatureArchitectureError, match="forbidden=.*rusqlite"):
        _validate_metadata(metadata, "release-compiler")


def _fact_generation_metadata() -> dict[str, Any]:
    metadata = _metadata()
    root = metadata["resolve"]["nodes"][0]
    root["features"].append("fact-generation")
    packages = (
        "blake3",
        "petgraph",
        "rayon",
        "ruff_python_ast",
        "ruff_python_index",
        "ruff_python_parser",
        "ruff_python_semantic",
        "ruff_python_trivia",
        "ruff_source_file",
        "ruff_text_size",
        "tree-sitter",
        "tree-sitter-python",
        "tree-sitter-rust",
    )
    for name in packages:
        identifier = f"package:{name}"
        metadata["packages"].append({"id": identifier, "name": name})
        metadata["resolve"]["nodes"].append(
            {"id": identifier, "features": [], "deps": []}
        )
        root["deps"].append({"pkg": identifier, "dep_kinds": [{"kind": None}]})
    return metadata


def test_fact_generation_isolated_graph_is_accepted() -> None:
    contract = CONTRACTS["fact-generation"]
    _validate_manifest(
        {"features": {"fact-generation": sorted(contract.manifest_items)}},
        "fact-generation",
    )
    report = _validate_metadata(_fact_generation_metadata(), "fact-generation")
    assert report["scope"] == "fact-generation"


def test_fact_generation_fabric_dependency_fault_is_rejected() -> None:
    metadata = _fact_generation_metadata()
    metadata["packages"].append({"id": "query", "name": "datafusion"})
    metadata["resolve"]["nodes"].append(
        {"id": "query", "features": [], "deps": []}
    )
    metadata["resolve"]["nodes"][0]["deps"].append(
        {"pkg": "query", "dep_kinds": [{"kind": None}]}
    )
    with pytest.raises(FeatureArchitectureError, match="forbidden=.*datafusion"):
        _validate_metadata(metadata, "fact-generation")
