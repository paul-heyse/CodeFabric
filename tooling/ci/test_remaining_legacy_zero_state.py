"""Causal tests for the permanent remaining-legacy zero-state guard."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tooling.ci.remaining_legacy_zero_state import (
    COMPOSED_ZERO_STATE_COMMANDS,
    ROOT,
    RemainingLegacyError,
    cargo_payload_issues,
    classify_paths,
    run_composed_zero_state,
    structural_probe,
    text_probe,
    validate_remaining_legacy,
)


def test_live_remaining_legacy_inventory_is_zero_without_replaying_specialists() -> (
    None
):
    report = validate_remaining_legacy(ROOT, run_composed=False)
    assert report["live_file_count"] > 0
    assert report["retained_history_or_release_evidence_file_count"] > 0
    assert report["cargo_package_count"] > 0
    assert report["python_package_file_count"] > 0
    assert report["structural_probes"] > 0
    assert report["composed_zero_state_checks"] == 0


def test_history_exclusion_is_explicit_and_does_not_hide_live_residue() -> None:
    live, retained, issues = classify_paths(
        {
            "docs/plans/retained_v1_plan.md",
            "contracts/acceptance/released-artifact-census-v1.json",
            "src/current.rs",
            "contracts/governance/relational-fabric-legacy-freeze.json",
        }
    )
    assert retained == [
        "contracts/acceptance/released-artifact-census-v1.json",
        "docs/plans/retained_v1_plan.md",
    ]
    assert live == ["src/current.rs"]
    assert issues == [
        "forbidden predecessor path: contracts/governance/relational-fabric-legacy-freeze.json"
    ]


def test_fixed_string_probe_rejects_live_transition_reference(tmp_path: Path) -> None:
    source = tmp_path / "src"
    source.mkdir()
    (source / "live.py").write_text(
        "from tooling.ci import relational_fabric_transition\n", encoding="utf-8"
    )
    issues = text_probe(tmp_path, scan_paths=("src",))
    assert issues and "relational_fabric_transition" in issues[0]


def test_fixed_string_probe_rejects_retired_ontology_error_and_relation_namespaces(
    tmp_path: Path,
) -> None:
    source = tmp_path / "src"
    source.mkdir()
    (source / "live.rs").write_text(
        'const ERROR: &str = "ONTOLOGY_PROGRAM_DECODE_INVALID";\n'
        'let id = relation_id("model.accepted_fact_family");\n',
        encoding="utf-8",
    )
    issues = text_probe(tmp_path, scan_paths=("src",))
    rendered = "\n".join(issues)
    assert "ONTOLOGY_PROGRAM_" in rendered
    assert 'relation_id("model.' in rendered


def test_structural_probe_rejects_predecessor_module_and_direct_provider_call(
    tmp_path: Path,
) -> None:
    source = tmp_path / "src"
    source.mkdir()
    (source / "lib.rs").write_text(
        "mod ontology_candidate;\nfn bad() { run_pyrefly (); }\n", encoding="utf-8"
    )
    issues, coverage = structural_probe(tmp_path)
    rendered = "\n".join(issues)
    assert "mod ontology_candidate" in rendered
    assert "run_pyrefly" in rendered
    assert coverage


def test_cargo_inventory_rejects_predecessor_feature_and_target() -> None:
    issues, count = cargo_payload_issues(
        [
            {
                "packages": [
                    {
                        "name": "codefabric",
                        "features": {"model-compiler": []},
                        "targets": [{"name": "codefabric-model"}],
                    }
                ]
            }
        ]
    )
    assert count == 1
    assert issues == [
        "forbidden Cargo feature codefabric#model-compiler",
        "forbidden Cargo target codefabric#codefabric-model",
    ]


def test_composition_invokes_only_permanent_specialist_guards() -> None:
    observed: list[tuple[str, ...]] = []

    def runner(
        command: tuple[str, ...], **_: object
    ) -> subprocess.CompletedProcess[str]:
        observed.append(command)
        return subprocess.CompletedProcess(command, 0, "", "")

    run_composed_zero_state(ROOT, runner=runner)
    assert observed == list(COMPOSED_ZERO_STATE_COMMANDS)


def test_composed_guard_failure_is_not_suppressed() -> None:
    def runner(
        command: tuple[str, ...], **_: object
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(command, 1, "", "causal failure")

    with pytest.raises(RemainingLegacyError, match="causal failure"):
        run_composed_zero_state(ROOT, runner=runner)
