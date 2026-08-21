from __future__ import annotations

import json
from pathlib import Path

from codefabric_cpg_mcp.contracts import registries

VECTORS = Path(__file__).parents[2] / "contracts/fixtures/registries/enum-flag-v1-vectors.json"


def test_wp08_behavioral_acceptance() -> None:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    for vector in vectors["enum_triples"]:
        assert (
            tuple(vector[key] for key in ("code", "name", "slug"))
            in registries.ENUM_TRIPLES[vector["domain"]]
        )

    for vector in vectors["flag_words"]:
        assert vector["domain"] == "FACT_FLAGS"
        word = registries.FactFlags.NONE
        for name in vector["names"]:
            word |= registries.FactFlags[name]
        assert int(word) == vector["word"]


def test_wp08_structural_acceptance() -> None:
    assert registries.EvidenceCertainty is not registries.ResolutionClass
    assert registries.Completeness is not registries.CompletenessState
    assert len(registries.ENUM_TRIPLES["EFFECT_KIND"]) == 37
    assert len(registries.ENUM_TRIPLES["RESOURCE_KIND"]) == 10
    assert len(registries.REGISTRY_IDS["projections"]) == 13
    assert len(registries.REGISTRY_IDS["capabilities"]) == 22
    assert len(registries.REGISTRY_IDS["public_errors"]) == 60


def test_wp08_negative_zero_state() -> None:
    for enum_type in (registries.EvidenceCertainty, registries.ResolutionClass):
        assert 0 not in enum_type._value2member_map_
    assert int(registries.FactFlags.COMPILER_SYNTHETIC) < 1 << 56
    assert not int(registries.FactFlags.COMPILER_SYNTHETIC) & (1 << 63)


def test_wp08_operational_acceptance() -> None:
    machine_types = (
        registries.WorkspaceLifecycle,
        registries.SourceTrustState,
        registries.EventStreamHealth,
        registries.GitAccelerationStatus,
        registries.UpdateWaveState,
        registries.ProviderRunState,
        registries.OwnerCapabilityState,
        registries.DurablePublicationState,
        registries.ServingActivationState,
        registries.QueryExecutionState,
        registries.ArtifactState,
        registries.WorkspaceRegistryLifecycle,
    )
    assert len(machine_types) == 12
    assert all(list(machine_type) for machine_type in machine_types)
