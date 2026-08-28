from __future__ import annotations

import copy
import subprocess
from pathlib import Path

import pytest

from tooling.ci import artifact_contracts
from tooling.ci import plan_assurance as assurance
from tooling.ci import released_fixture_verifier as verifier

WP70_PLAN = (
    verifier.ROOT
    / "docs/plans/codefabric_design_principles_full_alignment_implementation_plan_v3_2026-08-25.md"
)


def _dependency_names(recipe: dict[str, object]) -> set[str]:
    dependencies = recipe.get("dependencies")
    assert isinstance(dependencies, list)
    return {
        str(dependency["recipe"])
        for dependency in dependencies
        if isinstance(dependency, dict)
    }


def test_released_assurance_contracts_execute() -> None:
    report = verifier.verify_released_assurance()
    assert report["fixtures"]["negative_fixture_count"] == 5
    assert report["security"]["case_count"] == 5
    assert report["faults"]["fault_point_count"] == 35
    assert report["comparison"]["semantic_difference_detected"] is True


def test_wp70_behavioral_acceptance() -> None:
    for path in verifier.NEGATIVE_FIXTURES:
        value = copy.deepcopy(verifier._read_json(verifier.ROOT, path))
        assert verifier.verify_negative_fixture(verifier.ROOT, path, value)
        if path.name == "invalid-executable-field.json":
            del value["records"][0]["semantics"]["shell_command"]
        elif path.name == "broken-trace-edge.json":
            value["trace"]["requirement_id"] = min(
                verifier._requirements(verifier.ROOT)
            )
        elif path.name in {"drifted-digest.json", "perturbed-artifact.json"}:
            value["claimed_checksum"] = verifier._checksum_source(value["source_utf8"])
        else:
            schema = verifier._read_json(verifier.ROOT, Path(value["schema_path"]))
            value["replacement"] = schema["properties"]["workspace_id"]["pattern"]
        with pytest.raises(verifier.FixtureVerificationError):
            verifier.verify_negative_fixture(verifier.ROOT, path, value)


def test_wp70_structural_acceptance() -> None:
    block = artifact_contracts._packet_blocks(WP70_PLAN)["WP70"]
    mappings = {
        "wp70_behavioral_acceptance": "PC-WP70-BEH",
        "wp70_structural_acceptance": "PC-WP70-STR",
        "wp70_negative_zero_state": "PC-WP70-NEG",
        "wp70_operational_acceptance": "PC-WP70-OPS",
    }
    assert block.count("Executable oracle:") == len(mappings)
    for oracle, criterion in mappings.items():
        assert f"Executable oracle: `{oracle}`" in block
        assert f"Governed criterion: `{criterion}`" in block
    rule_stems = {path.stem for path in (verifier.ROOT / "rules").glob("*.yml")}
    test_stems = {
        path.stem.removesuffix("-test")
        for path in (verifier.ROOT / "rule-tests").glob("*-test.yml")
    }
    assert rule_stems == test_stems
    config = (verifier.ROOT / "sgconfig.yml").read_text(encoding="utf-8")
    justfile = (verifier.ROOT / "justfile").read_text(encoding="utf-8")
    assert "snapshotDir: __snapshots__" in config
    assert "--skip-snapshot-tests" not in justfile
    subprocess.run(("ast-grep", "test"), cwd=verifier.ROOT, check=True)


def test_wp70_negative_zero_state(tmp_path: Path) -> None:
    alias = tmp_path / "tests" / "alias.rs"
    alias.parent.mkdir(parents=True)
    alias.write_text(
        "fn wp70_negative_zero_state() { another_test(); }\n",
        encoding="utf-8",
    )
    with pytest.raises(assurance.PlanAssuranceError, match="single-call Rust alias"):
        assurance._rust_definitions(tmp_path, {"wp70_negative_zero_state"})
    with pytest.raises(assurance.PlanAssuranceError, match="lacks definitions"):
        assurance._require_exact_definitions(
            {"wp70_negative_zero_state"},
            [],
            context="expected-failure selector",
        )
    selector = assurance._rust_selector_command(
        "root", ["wp70_intentionally_absent_selector"]
    )
    assert selector[-1] == "--no-tests=fail"
    completed = subprocess.run(
        selector,
        cwd=verifier.ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode != 0
    assert "no tests" in (completed.stdout + completed.stderr).lower()


def test_wp70_operational_acceptance() -> None:
    recipes = artifact_contracts.load_just_recipes()
    assert {
        "released-fixture-check",
        "oracle-substance-check",
        "plan-dependency-check",
        "design-principle-traceability-check",
        "alignment-detector-check",
    } <= _dependency_names(recipes["governance"])
    assert "governance" in _dependency_names(recipes["ci-fast"])
    assert "wave-acceptance-check" in _dependency_names(recipes["ci-pr"])
    workflow = (verifier.ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    assert "just released-fixture-check" in workflow
    assert "just wave-acceptance-check" in workflow
    assert "rust_protobuf_matches_the_shared_wire_fixture/)" in workflow
    assert "--no-tests=fail" in workflow
    justfile = (verifier.ROOT / "justfile").read_text(encoding="utf-8")
    selectors = [
        line
        for line in justfile.splitlines()
        if "cargo nextest run" in line and " -E " in line
    ]
    assert selectors
    assert all("--no-tests=fail" in line for line in selectors)
