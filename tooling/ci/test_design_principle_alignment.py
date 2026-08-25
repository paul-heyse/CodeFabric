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
