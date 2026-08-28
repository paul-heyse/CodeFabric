"""Tests for authoritative-suite census and fail-closed navigation."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

from tooling.ci.artifact_contracts import ROOT
from tooling.ci.authoritative_design_conformance import (
    MASTERS,
    AuthoritativeDesignError,
    _legacy_hits,
    validate_authoritative_design,
    validate_master_directory,
)


def test_ontology_authoritative_design_conformance() -> None:
    report = validate_authoritative_design()
    assert report["master_count"] == 8
    assert report["generated_manifest_count"] == 3


def test_ontology_authoritative_design_path_authority(tmp_path: Path) -> None:
    for name, contract in MASTERS.items():
        (tmp_path / name).write_text(
            f"**Artifact ID:** `{contract.artifact_id}`\n"
            "**Version:** 1.0\n\n"
            f"{contract.amendment_anchor}\n",
            encoding="utf-8",
        )
    (tmp_path / ".DS_Store").write_bytes(b"stray")
    with pytest.raises(AuthoritativeDesignError, match="extra"):
        validate_master_directory(tmp_path)


def test_ontology_authoritative_design_legacy_reference_policy() -> None:
    live_hits, historical_hits = _legacy_hits(ROOT)
    assert live_hits == []
    assert historical_hits


def test_ontology_plan_v3_readiness_graph() -> None:
    completed = subprocess.run(
        ("just", "plan-dependency-check"),
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    assert "10 packets" in completed.stdout
    assert "0 disjoint-phase overlaps" in completed.stdout


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
