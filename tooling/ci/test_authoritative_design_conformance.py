"""Tests for relational authoritative-suite discovery and fail-closed routing."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

from tooling.ci.artifact_contracts import ROOT
from tooling.ci.authoritative_design_conformance import (
    REQUIRED_TAGS,
    AuthoritativeDesignError,
    _legacy_hits,
    validate_authoritative_design,
    validate_master_directory,
)


def test_relational_authoritative_design_conformance() -> None:
    report = validate_authoritative_design()
    assert report["current_master_count"] == 8
    assert report["historical_master_count"] == 16
    assert report["generated_manifest_authority_count"] == 0
    assert report["suite_id"] == "codefabric-relational-data-fabric"
    assert report["suite_version"] == "2.1.0"
    assert report["plan_selection"] in {
        "approved-v3-activation-pending",
        "active-v3",
    }


def _write_suite(root: Path) -> Path:
    directory = root / "docs/authoritative_design"
    directory.mkdir(parents=True)
    for tag in sorted(REQUIRED_TAGS):
        v1_name = f"{tag.lower()}_v1.md"
        v2_name = f"{tag.lower()}_v2.md"
        current_name = f"{tag.lower()}_v2_1.md"
        v1 = directory / v1_name
        v1.write_text(f"# historical {tag}\n", encoding="utf-8")
        artifact_id = f"fixture-{tag.lower()}"
        (directory / v2_name).write_text(
            "---\n"
            "artifact: authoritative-design\n"
            f"artifact_id: {artifact_id}\n"
            "suite_id: codefabric-relational-data-fabric\n"
            "suite_version: 2.0.0\n"
            f"artifact_tag: {tag}\n"
            "artifact_version: 2.0.0\n"
            "authority_status: historical\n"
            f"successor_path: docs/authoritative_design/{current_name}\n"
            f"predecessor_path: docs/authoritative_design/{v1_name}\n"
            "---\n\n"
            f"# historical v2 {tag}\n\n{chr(96)}{artifact_id}{chr(96)}\n",
            encoding="utf-8",
        )
        (directory / current_name).write_text(
            "---\n"
            "artifact: authoritative-design\n"
            f"artifact_id: {artifact_id}\n"
            "suite_id: codefabric-relational-data-fabric\n"
            "suite_version: 2.1.0\n"
            f"artifact_tag: {tag}\n"
            "artifact_version: 2.1.0\n"
            "authority_status: current\n"
            f"predecessor_path: docs/authoritative_design/{v2_name}\n"
            "---\n\n"
            f"# current {tag}\n\n{chr(96)}{artifact_id}{chr(96)}\n",
            encoding="utf-8",
        )
    return directory


def test_relational_authoritative_design_rejects_stray_authority_entry(
    tmp_path: Path,
) -> None:
    directory = _write_suite(tmp_path)
    (directory / ".DS_Store").write_bytes(b"stray")
    with pytest.raises(AuthoritativeDesignError, match="non-master"):
        validate_master_directory(directory, root=tmp_path)


def test_relational_authoritative_design_rejects_duplicate_current_role(
    tmp_path: Path,
) -> None:
    directory = _write_suite(tmp_path)
    original = directory / "ont_v2_1.md"
    duplicate = directory / "ont_duplicate_v2_1.md"
    duplicate.write_text(
        original.read_text(encoding="utf-8").replace(
            "fixture-ont", "fixture-ont-duplicate"
        ),
        encoding="utf-8",
    )
    with pytest.raises(
        AuthoritativeDesignError, match="duplicate current artifact tag"
    ):
        validate_master_directory(directory, root=tmp_path)


def test_relational_authoritative_design_legacy_reference_policy() -> None:
    live_hits, historical_hits = _legacy_hits(ROOT)
    assert live_hits == []
    assert historical_hits


def test_relational_plan_v3_dependency_graph_without_activation() -> None:
    completed = subprocess.run(
        (
            "just",
            "plan-dependency-check",
            (
                "docs/plans/"
                "codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md"
            ),
        ),
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    assert "15 packets" in completed.stdout


@pytest.mark.parametrize("mode", ["missing", "empty"])
def test_spec_outline_fails_for_missing_or_empty_default(
    tmp_path: Path, mode: str
) -> None:
    candidate = tmp_path / mode
    if mode == "empty":
        candidate.mkdir()
    environment = os.environ.copy()
    environment["CODEFABRIC_SPEC_OUTLINE_DEFAULT"] = str(candidate)
    completed = subprocess.run(
        ("./scripts/spec-outline.sh",),
        cwd=ROOT,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode != 0
    assert "authoritative design root" in completed.stderr
