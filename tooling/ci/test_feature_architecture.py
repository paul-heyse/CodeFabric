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


def test_provider_contracts_exact_manifest_and_resolution_are_accepted() -> None:
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
