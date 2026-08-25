"""WP69 model-plane truth, census, cutover, and boundary acceptance oracles."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOVERNANCE = ROOT / "contracts/generated/model/governance"


def _json_lines(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def test_wp69_behavioral_acceptance() -> None:
    desired_tree_source = (ROOT / "src/bin/codefabric_model/desired_tree.rs").read_text(
        encoding="utf-8"
    )
    assert "fn model_plan_real_drift_fails_read_only_check()" in desired_tree_source
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--no-default-features",
            "--features",
            "model-compiler",
            "--bin",
            "codefabric-model",
            "desired_tree::tests::model_plan_real_drift_fails_read_only_check",
            "--",
            "--exact",
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "1 passed; 0 failed" in completed.stdout


def test_wp69_structural_acceptance() -> None:
    suite = json.loads((GOVERNANCE / "suite-manifest.json").read_text(encoding="utf-8"))
    validation = json.loads(
        (GOVERNANCE / "validation.json").read_text(encoding="utf-8")
    )
    adapter_index = json.loads(
        (
            ROOT
            / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json"
        ).read_text(encoding="utf-8")
    )
    outputs = suite["outputs"]
    paths = [output["path"] for output in outputs]
    requirements = _json_lines(GOVERNANCE / "requirements.jsonl")
    traces = _json_lines(GOVERNANCE / "traceability.jsonl")

    assert len(outputs) == len(set(paths)) == validation["output_count"] == 78
    assert adapter_index["outputs"] == outputs
    assert len(requirements) == len(traces) == 84
    assert len({record["requirement_id"] for record in requirements}) == 84
    assert len({record["normative_text"] for record in requirements}) == 84
    assert all(
        record["implements"] and record["verified_by"] for record in requirements
    )
    assert all(
        set(record) == {"requirement_id", "implements", "traces_to", "verified_by"}
        for record in traces
    )


def test_wp69_negative_zero_state() -> None:
    stale_registry = ROOT / "contracts/generated/registry"
    assert not stale_registry.exists() or not any(stale_registry.iterdir())
    assert not (ROOT / "contracts/schema/arrow-delta/table-specs.json").exists()
    assert (GOVERNANCE / "requirements.jsonl").read_bytes() != (
        GOVERNANCE / "traceability.jsonl"
    ).read_bytes()

    drivers = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (ROOT / "src/bin/codefabric_model").glob("*_driver.rs")
    )
    desired_tree = (ROOT / "src/bin/codefabric_model/desired_tree.rs").read_text(
        encoding="utf-8"
    )
    operational_store = (ROOT / "src/operational_store.rs").read_text(encoding="utf-8")
    assert "serde_json_canonicalizer" not in drivers
    assert "struct NoDuplicate" not in drivers
    assert "ActionKeyMaterial" not in desired_tree
    assert "compile_shadow_actions" not in desired_tree
    assert '.strip_prefix("CREATE TABLE ")' not in operational_store
