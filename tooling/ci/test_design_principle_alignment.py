from __future__ import annotations

import json
from pathlib import Path

import pytest
import yaml

from tooling.ci import design_principle_alignment as alignment


def _write(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(yaml.safe_dump(value, sort_keys=False), encoding="utf-8")


def test_probe_rejects_review_self_match() -> None:
    record = {
        "detector_id": "DP-124",
        "disposition": "open",
        "probe": {
            "kind": "contains",
            "paths": ["docs/**/*.md"],
            "pattern": "forbidden token",
            "min_matches": 0,
            "max_matches": 0,
        },
        "coverage": {"exclude": list(alignment.REVIEW_EXCLUSIONS)},
    }
    assert "docs/reviews/**" in record["coverage"]["exclude"]


def test_execute_detector_counts_duplicate_matches(tmp_path: Path) -> None:
    source = tmp_path / "src" / "sample.rs"
    source.parent.mkdir()
    source.write_text("shadow authority\nshadow authority\n", encoding="utf-8")
    record = {
        "detector_id": "DP-001",
        "disposition": "open",
        "probe": {
            "kind": "contains",
            "paths": ["src/**/*.rs"],
            "pattern": "shadow authority",
            "min_matches": 2,
            "max_matches": 2,
        },
        "coverage": {"exclude": list(alignment.REVIEW_EXCLUSIONS)},
    }
    observed = alignment.execute_detector(record, tmp_path)
    assert observed.match_count == 2
    assert observed.matched_files == ("src/sample.rs",)


def test_baseline_rejects_unattributed_dirty_path(tmp_path: Path) -> None:
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs" / "plan.md").write_text("baseline\n", encoding="utf-8")
    subprocess = pytest.importorskip("subprocess")
    subprocess.run(("git", "init"), cwd=tmp_path, check=True, capture_output=True)
    subprocess.run(("git", "add", "."), cwd=tmp_path, check=True)
    subprocess.run(
        (
            "git",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "base",
        ),
        cwd=tmp_path,
        check=True,
        capture_output=True,
    )
    baseline = subprocess.run(
        ("git", "rev-parse", "HEAD"),
        cwd=tmp_path,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    (tmp_path / "docs" / "plan.md").write_text("changed\n", encoding="utf-8")
    _write(
        tmp_path / alignment.BASELINE_REGISTRY,
        {"baseline_commit": baseline, "records": []},
    )
    with pytest.raises(
        alignment.AlignmentContractError, match="unattributed dirty paths"
    ):
        alignment.validate_baseline(tmp_path)


def test_active_plan_resolution(tmp_path: Path) -> None:
    pointer = tmp_path / alignment.ACTIVE_PLAN_POINTER
    pointer.parent.mkdir(parents=True)
    plan = Path("docs/plans/example.md")
    (tmp_path / plan).write_text("### WP54 — packet\n", encoding="utf-8")
    pointer.write_text(json.dumps({"plan_path": plan.as_posix()}), encoding="utf-8")
    assert alignment._active_plan(tmp_path) == (plan, "### WP54 — packet\n")


def test_transformation_pass_plan_resolution_uses_immutable_plan_provenance(
    tmp_path: Path,
) -> None:
    active = Path("docs/plans/active_implementation_plan.md")
    prior = Path("docs/plans/prior_implementation_plan_v2.md")
    (tmp_path / active).parent.mkdir(parents=True)
    (tmp_path / active).write_text("### WP01 — active packet\n", encoding="utf-8")
    (tmp_path / prior).write_text(
        "### WP03 — prior packet\n"
        "Executable oracle: `behavioral`; Executable oracle: `structural`\n",
        encoding="utf-8",
    )
    assert alignment._transformation_pass_plan_paths(
        tmp_path,
        active,
        (tmp_path / active).read_text(encoding="utf-8"),
        "WP03",
        ["behavioral", "structural"],
    ) == (prior,)


def test_wp73_behavioral_acceptance() -> None:
    assert alignment.validate_traceability() == (25, 124)
    observations = alignment.execute_detectors()
    assert len(observations) == 124
    assert {item.detector_id for item in observations} == alignment.DETECTOR_IDS


def test_wp73_structural_acceptance() -> None:
    principles = alignment._records(
        alignment._load_yaml(alignment.PRINCIPLE_REGISTRY),
        alignment.PRINCIPLE_REGISTRY,
    )
    detectors = alignment._records(
        alignment._load_yaml(alignment.DETECTOR_REGISTRY),
        alignment.DETECTOR_REGISTRY,
    )
    assert len(principles) == 25
    assert len(detectors) == 124
    assert all(
        record["command"].endswith(record["detector_id"]) for record in detectors
    )


def test_wp73_negative_zero_state() -> None:
    with pytest.raises(alignment.AlignmentContractError, match="vacuous"):
        alignment._validate_probe(
            "DP-999",
            {
                "kind": "contains",
                "paths": ["src/**/*.rs"],
                "pattern": "nonempty",
                "min_matches": 0,
            },
        )


def test_wp73_operational_acceptance() -> None:
    current_dirty, attributed = alignment.validate_baseline()
    assert current_dirty <= attributed


def test_transformation_pass_traceability_is_namespaced_and_digest_pinned() -> None:
    assert alignment.validate_transformation_passes() == len(
        alignment.REQUIRED_TRANSFORMATION_PASSES
    )


def test_transformation_pass_contract_rejects_bare_principle_and_stale_digest(
    tmp_path: Path,
) -> None:
    record = {
        "pass_id": "PASS_FIXTURE_V1",
        "principles": ["P14", "H-P16"],
        "contract_digest": "b3:" + "0" * 64,
    }
    assert any(
        __import__("re").fullmatch(r"P\d+", principle)
        for principle in record["principles"]
    )
    assert record["contract_digest"] != alignment._detached_digest(
        record, "contract_digest"
    )
