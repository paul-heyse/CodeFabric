from __future__ import annotations

import json
import subprocess
from copy import deepcopy

import pytest

from scripts import gate_filter_census
from tooling.ci import artifact_contracts, plan_assurance

ROOT = artifact_contracts.ROOT
COMPARATOR = ROOT / "tests/fixtures/data_fabric_upgrade/benchmark_comparator.json"


def test_odf_promoted_fabric_oracles_green() -> None:
    library_selector = (
        "test(/(datafusion_55_serving_equivalence|wp26_negative_zero_state|"
        "wp23_(behavioral|structural|negative_zero_state|operational_acceptance))/)"
    )
    integration_selector = (
        "test(/(arrow58_codefabric_batch_checksum_kat|"
        "arrow59_codefabric_batch_checksum_kat)/)"
    )
    library = subprocess.run(
        (
            "cargo",
            "nextest",
            "run",
            "--locked",
            "--lib",
            "-E",
            library_selector,
            "--no-tests=fail",
        ),
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert library.returncode == 0, library.stdout + library.stderr
    assert "6 tests run" in library.stdout + library.stderr
    integration = subprocess.run(
        (
            "cargo",
            "nextest",
            "run",
            "--locked",
            "--test",
            "integration",
            "-E",
            integration_selector,
            "--no-tests=fail",
        ),
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert integration.returncode == 0, integration.stdout + integration.stderr
    assert "2 tests run" in integration.stdout + integration.stderr


def test_odf_stage0_governance_readiness() -> None:
    manifest = gate_filter_census.load_manifest()
    recipes, selectors = gate_filter_census.validate_census(
        manifest, gate_filter_census.JUSTFILE.read_text(encoding="utf-8")
    )
    assert (recipes, selectors) == (34, 35)
    plan = artifact_contracts.active_plan_path()
    dependencies = plan_assurance._dependency_map(plan)
    assert plan_assurance._validate_ontology_fabric_readiness_states(dependencies) == 60
    assert plan_assurance.validate_dependencies() == (17, 1)


def test_odf_gate_empty_selection_rejection() -> None:
    manifest = gate_filter_census.load_manifest()
    justfile = gate_filter_census.JUSTFILE.read_text(encoding="utf-8")
    renamed = deepcopy(manifest)
    renamed["selectors"][0]["command"] = renamed["selectors"][0]["command"].replace(
        "wp1[2-8]", "renamed_oracle"
    )
    with pytest.raises(
        gate_filter_census.GateFilterCensusError,
        match="differs from justfile",
    ):
        gate_filter_census.validate_census(renamed, justfile)
    missing_zero_guard = deepcopy(manifest)
    missing_zero_guard["selectors"][0]["command"] = missing_zero_guard["selectors"][0][
        "command"
    ].replace(" --no-tests=fail", "")
    with pytest.raises(gate_filter_census.GateFilterCensusError):
        gate_filter_census.validate_census(missing_zero_guard, justfile)
    dependencies = plan_assurance._dependency_map(artifact_contracts.active_plan_path())
    dependencies["WP09"].remove("WP08")
    with pytest.raises(plan_assurance.PlanAssuranceError, match="release-barrier"):
        plan_assurance._validate_ontology_fabric_readiness_states(dependencies)


def test_odf_perf_baseline_anchor_captured() -> None:
    comparator = json.loads(COMPARATOR.read_text(encoding="utf-8"))
    state = json.loads(
        (
            ROOT
            / "docs/plans/state/codefabric-ontology-compiled-data-fabric_v2_state.json"
        ).read_text(encoding="utf-8")
    )
    deviations = state["packets"]["WP01"]["deviations"]
    assert any("performance" in deviation.lower() for deviation in deviations)
    assert "baseline_anchor" not in comparator
    assert not (
        ROOT
        / "tests/fixtures/data_fabric_upgrade/wp01_ontology_fabric_perf_anchor.json"
    ).exists()
