"""Tests for command-graph proof coverage."""

from copy import deepcopy

import pytest

from tooling.ci.proof_coverage import (
    ProofCoverageError,
    compare_candidate,
    load_manifest,
    proof_closure,
)


def _dependencies(manifest: dict[str, object]) -> dict[str, list[str]]:
    recipes = manifest["recipes"]
    assert isinstance(recipes, dict)
    return {
        name: declaration.get("dependencies", [])
        for name, declaration in recipes.items()
    }


def test_stdio_optimization_preserves_declared_proofs() -> None:
    manifest = load_manifest()
    recipes = manifest["recipes"]
    candidate = manifest["optimization_candidates"][0]
    result = compare_candidate(candidate, recipes, _dependencies(manifest))
    assert result["before"] == result["after"]
    assert "adapter.stdio.protocol-silence" in result["after"]


def test_removed_proof_is_detected() -> None:
    manifest = load_manifest()
    mutated = deepcopy(manifest)
    mutated["recipes"]["adapter-test"]["proofs"].remove(
        "adapter.stdio.protocol-silence"
    )
    with pytest.raises(ProofCoverageError, match="changes proof coverage"):
        compare_candidate(
            mutated["optimization_candidates"][0],
            mutated["recipes"],
            _dependencies(mutated),
        )


def test_dependency_cycle_is_detected() -> None:
    manifest = load_manifest()
    dependencies = _dependencies(manifest)
    dependencies["root-test-rust"] = ["root-test"]
    with pytest.raises(ProofCoverageError, match="cycle"):
        proof_closure("root-test", manifest["recipes"], dependencies)
